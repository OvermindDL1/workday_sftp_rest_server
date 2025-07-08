// use crate::banner::queries as BannerQueries;
use crate::banner::BannerConnPool;
use crate::canvas::csv_defs::*;
use crate::configuration::BACKUP_KEEP;
use crate::csv_row_definitions::{INT069ARow, INT069BRow};
use crate::utils::csv::{get_last_matching_file_delete_rest_as_csv, validate_csv};
use crate::utils::{AnyResult, Token};
use crate::SETTINGS;
use anyhow::Context;
use async_stream::stream;
use axum::body::Body;
use axum::body::Bytes;
use axum::extract::Path;
use axum::extract::Query;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use axum_auth::AuthBearer;
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use hyper::HeaderMap;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use tracing::*;

pub mod csv_defs;

const TOKEN_GENERAL: Token = Token::new([
	34, 23, 34, 20, 40, 160, 68, 51, 102, 241, 157, 171, 244, 48, 196, 243, 179, 79, 225, 172, 37, 168, 194, 219, 65,
	11, 102, 80, 162, 45, 7, 44, 8, 9, 147, 142, 193, 126, 233, 184, 17, 65, 120, 49, 136, 90, 217, 196, 19, 111, 162,
	103, 240, 185, 60, 1, 82, 6, 142, 150, 143, 149, 203, 255,
]);

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
	pub base_url: String,
	pub token: String,
	pub account: String,
	pub username: String,
	pub password: String,
	#[serde(skip, default = "Settings::default_url")]
	sis_account_url: Url,
}

impl Default for Settings {
	fn default() -> Self {
		Settings {
			base_url: "https://yoururl.instructure.com".to_string(),
			token: "accesstokenhereabcdefghijklmnopqrstuvwxyz".to_string(),
			account: "12345".to_string(),
			username: "usernamehere".to_string(),
			password: "passwordhere".to_string(),
			sis_account_url: Settings::default_url(),
		}
	}
}

impl Settings {
	fn default_url() -> Url {
		Url::parse("https://unset/").expect("invalid unset default Url?")
	}

	pub fn rebuild_cache(self) -> anyhow::Result<Self> {
		Ok(Self {
			sis_account_url: Url::parse(&format!(
				"{base_url}/api/v1/accounts/{account}/",
				base_url = self.base_url,
				account = self.account,
			))?,
			..self
		})
	}

	pub fn get_sis_import_url(&self) -> Result<Url, url::ParseError> {
		self.sis_account_url.join("sis_imports")
	}

	pub fn get_sis_check_url(&self, workflow_id: u64) -> Result<Url, url::ParseError> {
		self.sis_account_url.join(&format!("sis_imports/{workflow_id}"))
	}
}

pub async fn post_csv_to_canvas<T: serde::Serialize>(
	client: &reqwest::Client,
	data: &[T],
) -> anyhow::Result<serde_json::Value> {
	let settings = SETTINGS.get().context("settings not loaded")?;
	validate_csv(data)?;
	let mut writer = csv::Writer::from_writer(vec![]);
	for row in data {
		writer.serialize(row)?;
	}
	let data = writer.into_inner()?;
	let url = settings.canvas.get_sis_import_url()?;
	trace!(
		data = ?String::from_utf8_lossy(&data),
		sis_import_url = ?url,
	);
	// return Ok(serde_json::json!({"import": String::from_utf8_lossy(&data).to_string(), "workflow": "done"}));
	let resp = client
		.post(url)
		.header("Content-Type", "text/csv")
		.bearer_auth(&settings.canvas.token)
		.body(data)
		.send()
		.await?;
	trace!(response = ?resp);
	if !resp.status().is_success() {
		let err = resp.text().await?;
		trace!(error = err);
		anyhow::bail!("canvas response was unsuccessful: {err}")
	}
	let json = resp.json::<serde_json::Value>().await?;
	trace!(json = ?json);
	let workflow_id = json
		.as_object()
		.context("SIS import did not return a json object")?
		.get("id")
		.context("SIS import did not return an id")?
		.as_u64()
		.context("SIS import id was not a u64")?;
	let workflow_result = verify_canvas_workflow(client, workflow_id).await?;
	let results = serde_json::json!({ "import": json, "workflow": workflow_result });
	Ok(results)
}

pub async fn verify_canvas_workflow(client: &reqwest::Client, workflow_id: u64) -> anyhow::Result<serde_json::Value> {
	let settings = SETTINGS.get().context("settings not loaded")?;
	let mut sleep_time = std::time::Duration::from_secs(1);
	let url = settings.canvas.get_sis_check_url(workflow_id)?;
	debug!(url = ?url);
	let mut json = serde_json::json!({});
	for _check_time in 0..180 {
		let resp = client
			.get(url.clone())
			.bearer_auth(&settings.canvas.token)
			.send()
			.await?;
		trace!(workflow = "check", response = ?resp, workflow_id = ?workflow_id);
		if !resp.status().is_success() {
			let err = resp.text().await?;
			trace!(error = err);
			anyhow::bail!("canvas workflow {workflow_id} response was unsuccessful: {err}")
		}
		json = resp.json::<serde_json::Value>().await?;
		trace!(workflow = "result", json = ?json, workflow_id = ?workflow_id);
		let status = json
			.as_object()
			.context("SIS workflow did not return a json object")?
			.get("workflow_state")
			.context("SIS workflow did not have a workflow_state key")?
			.as_str()
			.context("SIS workflow workflow_state was not a string")?;
		match status {
			"imported" => return Ok(json),
			"imported_with_messages" => return Ok(json),
			"importing" => (),
			"created" => (),
			_issue => anyhow::bail!("SIS workflow issue: {json:?}"),
		}
		debug!("Workflow {workflow_id} is still in state `{status}`, sleeping {sleep_time:?}");
		tokio::time::sleep(sleep_time).await;
		if sleep_time.as_secs() < 15 {
			sleep_time += Duration::from_secs(1);
		} else {
			sleep_time = Duration::from_secs(15);
		}
	}
	warn!(workflow = "did not complete in time", workflow_id = ?workflow_id, json = ?json);
	anyhow::bail!("SIS workflow did not complete in time, is Canvas having issues?  Workflow ID: {workflow_id}")
}

pub fn routes() -> Router {
	Router::new()
		.route("/hr_training", get(route_hr_training))
		.route(
			"/inject_user",
			post(router_inject_ssotest_user).delete(router_delete_ssotest_user),
		)
		.route(
			"/depaginate/{*canvas_url}",
			get(route_depaginate_get).post(route_depaginate_post),
		)
	//.nest("/depaginate", Router::new().fallback(get(route_depaginate_get).post(route_depaginate_post)))
}

#[derive(Serialize)]
struct HRTraining {
	status: String,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	warnings: Vec<serde_json::Value>,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	terms: Vec<Term>,
	#[serde(skip_serializing_if = "serde_json::Value::is_null")]
	terms_response: serde_json::Value,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	courses: Vec<Course>,
	#[serde(skip_serializing_if = "serde_json::Value::is_null")]
	courses_response: serde_json::Value,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	sections: Vec<Section>,
	#[serde(skip_serializing_if = "serde_json::Value::is_null")]
	sections_response: serde_json::Value,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	enrollments: Vec<Enrollment>,
	#[serde(skip_serializing_if = "serde_json::Value::is_null")]
	enrollments_response: serde_json::Value,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	users: Vec<User>,
	#[serde(skip_serializing_if = "serde_json::Value::is_null")]
	users_response: serde_json::Value,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	user_enrollments: Vec<Enrollment>,
	#[serde(skip_serializing_if = "serde_json::Value::is_null")]
	user_enrollments_response: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct HRTrainingParams {
	#[serde(default)]
	r#return: bool,
	year: Option<i32>,
}

#[instrument(target = "debug", skip(banner_pool))]
async fn route_hr_training(
	Query(params): Query<HRTrainingParams>,
	AuthBearer(auth): AuthBearer,
	Extension(banner_pool): Extension<BannerConnPool>,
) -> AnyResult<Json<HRTraining>> {
	TOKEN_GENERAL.assert_valid(&auth)?;
	let mut warnings = Vec::new();
	let client = reqwest::Client::new();
	let year = params.year.unwrap_or_else(|| chrono::Utc::today().year());
	// These just end up being at 5pm for local time, so offset 7 hours ahead
	let hours_offset = 7;
	let start_date = NaiveDateTime::new(NaiveDate::from_ymd(year, 1, 1), NaiveTime::from_hms(0, 0, 0))
		+ chrono::Duration::hours(hours_offset);
	let end_date = NaiveDateTime::new(NaiveDate::from_ymd(year, 12, 31), NaiveTime::from_hms(23, 59, 59))
		+ chrono::Duration::hours(hours_offset);
	let term_id = TermId(format!("Calendar Year {year}"));
	let course_id = CourseId(format!("C_HR_ANNUAL_TRAINING_YEAR_{year}"));
	let section_id = SectionId(format!("S_HR_ANNUAL_TRAINING_YEAR_{year}"));
	let account_id = AccountId("Workshops".to_string());
	let teacher_user_ids = [
		UserId("C00045541".to_string()),
		UserId("C00082030".to_string()),
		UserId("C00069811".to_string()),
	];
	let teacher_role = Role("teacher".to_string());
	let student_role = Role("student".to_string());
	let name = format!("HR Annual Training {year}");

	let mut terms = vec![Term {
		term_id: term_id.clone(),
		name: term_id.0.clone(),
		status: ADStatus::Active,
		integration_id: None,
		date_override_enrollment_type: None,
		start_date: Some(start_date),
		end_date: Some(end_date),
	}];

	let mut courses = vec![Course {
		course_id: course_id.clone(),
		short_name: name.clone(),
		long_name: name.clone(),
		account_id: Some(account_id),
		term_id: Some(term_id),
		status: ADCPStatus::Active,
		integration_id: None,
		start_date: Some(SetDelete::Set(start_date)),
		end_date: Some(SetDelete::Set(end_date)),
		course_format: None,
		blueprint_course_id: None,
		grade_passback_setting: None,
		homeroom_course: None,
		friendly_name: None,
	}];

	let mut sections = vec![Section {
		section_id: section_id.clone(),
		course_id: course_id.clone(),
		name,
		status: ADStatus::Active,
		integration_id: None,
		start_date: Some(start_date),
		end_date: Some(end_date),
	}];

	let mut enrollments: Vec<Enrollment> = teacher_user_ids
		.into_iter()
		.map(|user_id| Enrollment {
			course_id: Some(course_id.clone()),
			root_account: None,
			start_date: None,
			end_date: None,
			user_id: Some(user_id),
			user_integration_id: None,
			role: Some(teacher_role.clone()),
			role_id: None,
			section_id: Some(section_id.clone()),
			status: ADCIDStatus::Active,
			associated_user_id: Some(UserId(String::new())),
			limit_section_privileges: None,
			notify: None,
		})
		.collect();

	/* Banner no longer holds employee information, workday does now, see after comment block
		let mut users: Vec<User> = {
			let conn = banner_pool.get()?;
			let rows = conn.query_as_named::<(String, String, String, String)>(
				r#"
	SELECT spriden_id, gobtpac_external_user, spriden_first_name, spriden_last_name
	  FROM  spriden, spbpers, gobtpac
	  WHERE spriden_change_ind IS NULL
	  AND   spriden_pidm = spbpers_pidm
	  AND   spriden_pidm = gobtpac_pidm
	  AND   spriden_pidm IN (SELECT DISTINCT pebempl_pidm
							  FROM payroll.pebempl
							 WHERE pebempl_empl_status = 'A'
							   AND pebempl_internal_ft_pt_ind = 'F')
			"#,
				&[],
			)?;
			rows.map(|row| {
				let (user_id, login_id, first_name, last_name) = row?;
				Ok(User {
					user_id: UserId(user_id),
					integration_id: None,
					login_id: LoginId(login_id),
					password: None,
					ssha_password: None,
					authentication_provider_id: None,
					first_name: Some(first_name),
					last_name: Some(last_name),
					full_name: None,
					sortable_name: None,
					short_name: None,
					email: None,
					pronouns: None,
					declared_user_type: None,
					canvas_password_notification: None,
					home_account: None,
					status: ASDStatus::Active,
				})
			})
			.collect::<anyhow::Result<_>>()?
		};
		*/
	let mut user_enrollments: Vec<Enrollment> = Vec::with_capacity(400);
	let mut users: Vec<User> = {
		let int069b: HashMap<String, INT069BRow> =
			get_last_matching_file_delete_rest_as_csv::<INT069BRow>("INT069B*.csv", BACKUP_KEEP)?
				.into_iter()
				.filter(|row| !row.Employee_ID.is_empty())
				// Moved to in the main bulk below so can send deletes for part times for enrollments
				// .filter(|row| row.Time_Type == "Full time") // TODO:  Only full time are automatically put into canvas, should this continue to be?
				.map(|row| (row.Employee_ID.clone(), row))
				.collect();
		let int069a = get_last_matching_file_delete_rest_as_csv::<INT069ARow>("INT069A*.csv", BACKUP_KEEP)?
			.into_iter()
			.filter(|row| int069b.contains_key(&row.Employee_ID))
			.filter(|row| {
				if row.Legacy_Banner_ID.is_empty() {
					warnings.push(json!({
						"type": "active user found with no banner login ID",
						"employee_id": row.Employee_ID,
						"preferred_last_name": row.Preferred_Name_Last_Name,
						"preferred_first_name": row.Preferred_Name_First_Name,
						"preferred_middle_name": row.Preferred_Name_Middle_Name,
					}));
					false
				} else {
					true
				}
			})
			.collect::<Vec<_>>();
		let cnums_to_logins: HashMap<String, String> = {
			let mut cnums_to_logins: HashMap<_, _> = int069a
				.iter()
				.map(|row| (row.Legacy_Banner_ID.clone(), String::new()))
				.collect();
			let conn = banner_pool.get()?;
			// Yes, it is actually substantially faster to query all using oracles horrible API than chunking by like 10
			// or so without just outright writing a complex splitter in SQL itself and just... no... probably still
			// wouldn't be faster, so just do this, takes less than a second anyway compared to many seconds otherwise.
			info_span!("cnum_login_lookup").in_scope(|| -> anyhow::Result<()> {
				for row in conn.query_as::<(String, String)>(
					r#"
				SELECT DISTINCT spriden_id, gobtpac_external_user
				  FROM spriden
				  JOIN gobtpac ON spriden_pidm = gobtpac_pidm
				 WHERE spriden_change_ind IS NULL
				"#,
					&[],
				)? {
					let (id, login) = row?;
					cnums_to_logins.entry(id).and_modify(|l| *l = login);
				}
				Ok(())
			})?;
			cnums_to_logins.retain(|_cnum, login| !login.is_empty()); // Clear ones that don't have a banner mapping
			cnums_to_logins
		};
		user_enrollments.reserve(int069a.len());
		int069a
			.into_iter()
			.filter_map(|row| {
				let user_id = row.Legacy_Banner_ID;
				let Some(job_position) = int069b.get(&row.Employee_ID) else {
					warnings.push(json!({
						"type": "employee does not have a matching job position",
						"employee_id": row.Employee_ID,
						"banner_id": user_id,
						"preferred_last_name": row.Preferred_Name_Last_Name,
						"preferred_first_name": row.Preferred_Name_First_Name,
						"preferred_middle_name": row.Preferred_Name_Middle_Name,
					}));
					return None;
				};
				let Some(login_id) = cnums_to_logins.get(&user_id) else {
					warnings.push(json!({
						"type": "active user found with no banner login ID mapping",
						"employee_id": row.Employee_ID,
						"banner_id": user_id,
						"preferred_last_name": row.Preferred_Name_Last_Name,
						"preferred_first_name": row.Preferred_Name_First_Name,
						"preferred_middle_name": row.Preferred_Name_Middle_Name,
					}));
					return None;
				};

				let enrollment_status = match job_position.Time_Type.as_str() {
					"Full time" => ADCIDStatus::Active,
					"Part time" => ADCIDStatus::Deleted,
					unexpected => {
						warnings.push(json!({
							"type": "unexpected Time_Type in INT069A",
							"extra": "not adding employee to canvas training enrollments",
							"expected": ["Full time", "Part Time"],
							"actual": unexpected,
							"employee_id": row.Employee_ID,
							"banner_id": user_id,
							"preferred_last_name": row.Preferred_Name_Last_Name,
							"preferred_first_name": row.Preferred_Name_First_Name,
							"preferred_middle_name": row.Preferred_Name_Middle_Name,
						}));
						ADCIDStatus::Deleted
					}
				};
				user_enrollments.push(Enrollment {
					course_id: Some(course_id.clone()),
					root_account: None,
					start_date: None,
					end_date: None,
					user_id: Some(UserId(user_id.clone())),
					user_integration_id: None,
					role: Some(student_role.clone()),
					role_id: None,
					section_id: Some(section_id.clone()),
					status: enrollment_status,
					associated_user_id: Some(UserId(String::new())),
					limit_section_privileges: None,
					notify: None,
				});

				Some(User {
					user_id: UserId(user_id),
					integration_id: None,
					login_id: LoginId(login_id.clone()),
					password: None,
					ssha_password: None,
					authentication_provider_id: None,
					first_name: Some(row.Preferred_Name_First_Name),
					last_name: Some(row.Preferred_Name_Last_Name),
					full_name: None,
					sortable_name: None,
					short_name: None,
					email: Some(row.User_Name), // Yes, User_Name, which is the login email of the user and thus definitely works if they can log in at all
					pronouns: None,
					declared_user_type: None,
					canvas_password_notification: None,
					home_account: None,
					status: ASDStatus::Active,
				})
			})
			.collect()
	};

	// Now to process in parallel what can be done and after necessary ones are complete for the rest
	let (terms_response, users_response, courses_response) = tokio::join!(
		post_csv_to_canvas(&client, &terms),
		post_csv_to_canvas(&client, &users),
		post_csv_to_canvas(&client, &courses),
	);
	let sections_response = post_csv_to_canvas(&client, &sections).await;
	let (enrollments_response, user_enrollments_response) = tokio::join!(
		post_csv_to_canvas(&client, &enrollments),
		post_csv_to_canvas(&client, &user_enrollments),
	);

	let no_return_check = |v: serde_json::Value| {
		if params.r#return {
			v
		} else {
			serde_json::Value::Null
		}
	};
	let jsonize_error = |e: anyhow::Error| serde_json::json!({ "error": e.to_string() });
	let terms_response = terms_response.map(no_return_check).unwrap_or_else(jsonize_error);
	let courses_response = courses_response.map(no_return_check).unwrap_or_else(jsonize_error);
	let sections_response = sections_response.map(no_return_check).unwrap_or_else(jsonize_error);
	let enrollments_response = enrollments_response.map(no_return_check).unwrap_or_else(jsonize_error);
	let users_response = users_response.map(no_return_check).unwrap_or_else(jsonize_error);
	let user_enrollments_response = user_enrollments_response
		.map(no_return_check)
		.unwrap_or_else(jsonize_error);

	if !params.r#return {
		terms.clear();
		courses.clear();
		sections.clear();
		enrollments.clear();
		users.clear();
		user_enrollments.clear();
	}

	let errors = [
		&terms_response,
		&courses_response,
		&sections_response,
		&enrollments_response,
		&users_response,
		&user_enrollments_response,
	]
	.into_iter()
	.filter(|v| v.get("error").is_some())
	.map(|v| {
		if let Some(workflow) = v.get("workflow") {
			if let Some(processing_warnings) = workflow.get("processing_warnings") {
				if let Some(processing_warnings) = processing_warnings.as_array() {
					warnings.extend(processing_warnings.iter().cloned())
				} else {
					warnings.push(processing_warnings.clone());
				}
			}
		}
		v
	})
	.count();

	Ok(Json(HRTraining {
		status: if errors == 0 {
			if warnings.is_empty() {
				"success".to_string()
			} else {
				"success-with-warnings".to_string()
			}
		} else {
			format!("errors: {errors}")
		},
		warnings,
		terms,
		terms_response,
		courses,
		courses_response,
		sections,
		sections_response,
		enrollments,
		enrollments_response,
		users,
		users_response,
		user_enrollments,
		user_enrollments_response,
	}))
}

#[derive(Debug, Deserialize)]
struct InjectUserParam {
	login_id: String,
	auth_provider: String,
	email: Option<String>,
}

#[instrument(target = "debug")]
async fn router_delete_ssotest_user(AuthBearer(auth): AuthBearer) -> AnyResult<Json<serde_json::Value>> {
	TOKEN_GENERAL.assert_valid(&auth)?;
	let user_id = UserId("SSO_TEST".to_string());
	let login_id = LoginId("<deleted>".to_string());
	let user = User {
		user_id,
		integration_id: None,
		login_id,
		password: None,
		ssha_password: None,
		authentication_provider_id: None,
		first_name: None,
		last_name: None,
		full_name: None,
		sortable_name: None,
		short_name: None,
		email: None,
		pronouns: None,
		declared_user_type: None,
		canvas_password_notification: None,
		home_account: None,
		status: ASDStatus::Deleted,
	};
	let details = post_csv_to_canvas(&reqwest::Client::new(), &[&user]).await?;
	Ok(Json(json!({"result": "success", "user": user, "details": details})))
}

#[instrument(target = "debug")]
async fn router_inject_ssotest_user(
	Query(params): Query<InjectUserParam>,
	AuthBearer(auth): AuthBearer,
) -> AnyResult<Json<serde_json::Value>> {
	TOKEN_GENERAL.assert_valid(&auth)?;
	let user_id = UserId("SSO_TEST".to_string());
	let login_id = LoginId(params.login_id);
	let authentication_provider_id = Some(AuthenticationProviderId(params.auth_provider));
	let first_name = Some("SSO".to_string());
	let last_name = Some("Test".to_string());
	let email = params.email;
	let _unused_json_response = router_delete_ssotest_user(AuthBearer(auth)).await?;
	let user = User {
		user_id,
		integration_id: None,
		login_id,
		password: None,
		ssha_password: None,
		authentication_provider_id,
		first_name,
		last_name,
		full_name: None,
		sortable_name: None,
		short_name: None,
		email,
		pronouns: None,
		declared_user_type: None,
		canvas_password_notification: None,
		home_account: None,
		status: ASDStatus::Active,
	};
	let details = post_csv_to_canvas(&reqwest::Client::new(), &[&user]).await?;
	Ok(Json(json!({"result": "success", "user": user, "details": details})))
}

fn parse_link_next(links: &reqwest::header::HeaderValue) -> Option<String> {
	let Ok(links) = links.to_str() else {
		panic!("invalid header string: {links:?}")
	};
	for link in links.split(',') {
		let Some((url, rel)) = link.split_once("; ") else {
			eprintln!("Invalid link format semicolon: {link}");
			continue;
		};
		if rel == "rel=\"next\"" {
			let url = url.trim_start_matches('<').trim_end_matches('>');
			return Some(url.to_string());
		}
	}
	None
}

#[instrument(level = "info", skip(headers))]
async fn route_depaginate_get(
	headers: HeaderMap,
	Path(canvas_path): Path<String>,
	Query(mut params): Query<HashMap<String, String>>,
) -> AnyResult<Response> {
	use serde_json::Value;
	//let authorization = headers
	//    .get("authorization")
	//    .context("missing authorization")?
	//    .to_str()
	//    .context("headers should be strings")?
	//    .to_string();
	if let Some(unroll) = params.remove("UNROLL") {
		let Value::Object(unroll) = serde_json::from_str::<Value>(&unroll).expect("UNROLL param was not object json")
		else {
			panic!("UNROLL param was no object: {unroll}")
		};
		assert_eq!(unroll.len(), 1, "only 1 unroll value is currently supported");
		dbg!(&unroll);
		let (path_key, values) = unroll.into_iter().next().expect("1 unroll values must exist");
		match values {
			Value::Array(values) => {
				let body = Body::from_stream(stream! {
					yield Ok(Bytes::from_static(b"["));
					let mut is_first = true;
					for obj in values {
						let Value::Object(obj) = obj else {
							panic!("did not pass in an object in the array in unroll");
						};
						assert_eq!(obj.len(), 1, "object in array in unroll must have only one kv");
						let (key, value) = obj.into_iter().next().expect("should have one value");
						let value = match value {
							Value::String(value) => value,
							_ => panic!("unhandled value type in unroll request: {value:#?}"),
						};
						let canvas_path = canvas_path.replace(&format!("UNROLL:{path_key}:keyed"), &format!("{key}:{value}")).replace(&format!("UNROLL:{path_key}"), &value).to_string();
						let headers = headers.clone();
						let params = params.clone();
						let response = Box::pin(depaginate_get(headers, Path(canvas_path), Query(params))).await.expect("processing route failed");
						let bytes = axum::body::to_bytes(response.into_body(), 100_000_000).await.expect("processing body stream failed");
						//match serde_json::from_slice(bytes.as_ref()) {
						//    Serde::Array(values) => {
						//        for
						//    }
						//    unhandled => panic!("unhandled resending type: {unhandled:#?}"),
						//}
						if !is_first {
							yield Ok(Bytes::from_static(b","));
						}
						is_first = false;
						if bytes.starts_with(b"[") && bytes.ends_with(b"]") {
							let bytes = Bytes::copy_from_slice(&bytes[1..(bytes.len()-1)]);
							yield Ok::<_, Box<(dyn std::error::Error + Send + Sync + 'static)>>(bytes);
						} else {
                            panic!("Unhandled resending type: {}", (*bytes.first().unwrap()) as char);
                        }
					}
					yield Ok(Bytes::from_static(b"]"));
				});
				Ok(Response::builder()
					.header("Content-Type", "application/json")
					.body(body)
					.context("constructing response")?)
			}
			_ => panic!("unsupported UNROLL type for: {values:#?}"),
		}
	} else {
		depaginate_get(headers, Path(canvas_path), Query(params)).await
	}
}

#[instrument(level = "info", skip(headers))]
async fn depaginate_get(
	headers: HeaderMap,
	Path(canvas_path): Path<String>,
	Query(mut params): Query<HashMap<String, String>>,
) -> AnyResult<Response> {
	let authorization = headers
		.get("authorization")
		.context("missing authorization")?
		.to_str()
		.context("headers should be strings")?
		.to_string();
	let body = Body::from_stream(stream! {
		let only_one_page = params.remove("only_one_page").is_some();
		let client = reqwest::Client::new();
		let mut canvas_url = Url::parse(&format!("https://cloviscc.instructure.com/{canvas_path}")).context("invalid canvas url")?;
		canvas_url.query_pairs_mut().extend_pairs(&params);
		let mut response_handle = Some(tokio::spawn(client.get(canvas_url.clone()).header("authorization", &authorization).send()));
		yield Ok::<_, Box<dyn std::error::Error + Send + Sync>>(Bytes::from_static(b"["));
		loop {
			let url = canvas_url.to_string();
			//let response = client.get(canvas_url.clone()).header("authorization", &authorization).send().await?;
			let Some(response) = response_handle.take() else {
				panic!("response should never be empty at start of loop...");
			};
			let response = response.await.context("canvas task fetch")?.context("canvas fetch")?;
			if !response.status().is_success() {
				tracing::warn!(url, status=?response.status(), "canvas bad status response");
				yield Err(format!("canvas error code {}", response.status()).into());
				return;
			}
			let next_link = if !only_one_page {
				response.headers().get("link").and_then(parse_link_next)
			} else {
				None
			};
			if let Some(next_link) = &next_link {
				canvas_url = Url::parse(&next_link).context("next_link should always be a URL")?;
				response_handle = Some(tokio::spawn(client.get(canvas_url.clone()).header("authorization", &authorization).send()));
			}
			//let links = response.headers().get("link").map(|s| s.to_str().expect("headers should be strings").to_string());
			let body = match response.text().await {
				Ok(body) => body,
				Err(error) => {
					tracing::warn!(url, ?error, "returned canvas data is not textual");
					yield Err(format!("returned canvas data is not textual: {error:?}").into());
					return;
				}
			};
			if let Ok(body) = serde_json::from_str::<Vec<serde_json::Value>>(&body) {
				tracing::info!(url, record_count=body.len(), "processing data stream...");
				let mut first = true;
				for value in body {
					if !first {
						yield Ok(Bytes::from_static(b","));
					}
					first = false;
					yield Ok(Bytes::from_owner(serde_json::to_string(&value).expect("should always pass since it came from json").into_bytes()));
				}
			} else if let Ok(body) = serde_json::from_str::<HashMap<String, Vec<serde_json::Value>>>(&body) {
				if body.len() != 1 {
					tracing::warn!(url, ?body, "map body with more than one field");
					yield Err(format!("map body with more than one field: {body:?}").into());
				}
				let body = body.into_iter().next().expect("already confirmed 1 value").1;
				tracing::info!(url, record_count=body.len(), "processing data stream...");
				let mut first = true;
				for value in body {
					if !first {
						yield Ok(Bytes::from_static(b","));
					}
					first = false;
					yield Ok(Bytes::from_owner(serde_json::to_string(&value).expect("should always pass since it came from json").into_bytes()));
				}
			} else {
				tracing::warn!(url, body, "unhandled canvas response");
				yield Err(format!("unhandled canvas response: {body}").into());
				return;
			}
			if next_link.is_none() {
				yield Ok(Bytes::from_static(b"]"));
				return;
			} else {
				yield Ok(Bytes::from_static(b","));
			}
			//if let Some(next_link) = next_link {
			//    //canvas_url = Url::parse(&next_link).context("next_link should always be a URL")?;
			//} else {
			//    // No links, only result, end
			//    yield Ok(Bytes::from_static(b"]"));
			//    return;
			//}
		}
	});
	Ok(Response::builder()
		.header("Content-Type", "application/json")
		.body(body)
		.context("constructing response")?)
}

async fn route_depaginate_post(
	headers: HeaderMap,
	Path(canvas_path): Path<String>,
	Query(params): Query<HashMap<String, String>>,
	body: String,
) -> AnyResult<Json<serde_json::Value>> {
	dbg!(&headers, &canvas_path, &params, &body);
	todo!()
}
