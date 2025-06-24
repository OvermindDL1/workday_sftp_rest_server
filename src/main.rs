#![allow(non_snake_case)]
#![recursion_limit = "256"]

use anyhow::Context as _;
use axum::extract::Query;
use axum::handler::HandlerWithoutStateExt;
use axum::http::{StatusCode, Uri};
use axum::response::Redirect;
use axum::{middleware, routing::get, Extension, Json, Router};
use axum_auth::AuthBearer;
use axum_extra::extract::Host;
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use chrono::NaiveDate;
use clap::Parser;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tracing::*;
use workday_sftp_rest_server::args::Args;
use workday_sftp_rest_server::banner::BannerConnPool;
use workday_sftp_rest_server::canvas;
use workday_sftp_rest_server::configuration::BACKUP_KEEP;
use workday_sftp_rest_server::csv_row_definitions::*;
use workday_sftp_rest_server::logging::setup_logger;
use workday_sftp_rest_server::utils::csv::{get_last_matching_file_delete_rest_as_csv, write_csv_with_date_to};
use workday_sftp_rest_server::utils::email::mail_layer_middleware;
use workday_sftp_rest_server::utils::{shutdown_signal, AnyError, AnyResult, Token};
use workday_sftp_rest_server::{banner, papercut};

const TOKEN_GENERAL: Token = Token::new([
	34, 23, 34, 20, 40, 160, 68, 51, 102, 241, 157, 171, 244, 48, 196, 243, 179, 79, 225, 172, 37, 168, 194, 219, 65,
	11, 102, 80, 162, 45, 7, 44, 8, 9, 147, 142, 193, 126, 233, 184, 17, 65, 120, 49, 136, 90, 217, 196, 19, 111, 162,
	103, 240, 185, 60, 1, 82, 6, 142, 150, 143, 149, 203, 255,
]);

#[derive(Clone, Copy)]
struct Ports {
	http: u16,
	https: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	// TODO: Automatic starting via systemd service
	// TODO: (Tokio-based?) internal cron-job functionality
	let args = Args::parse();
	args.init_settings().context("initializing settings")?;
	setup_logger(&args)?;

	let banner_pool = banner::BannerConnPool::new()?;

	#[cfg(not(debug_assertions))]
	let ports = Ports {
		http: 4080,
		https: 4443,
	};
	#[cfg(debug_assertions)]
	let ports = Ports {
		http: 4081,
		https: 4444,
	};

	let app = Router::new()
		.route("/INT004.json", get(route_INT004))
		.route("/INT005A.json", get(route_INT005A))
		.route("/INT069A.json", get(route_INT069A))
		.route("/INT069B.json", get(route_INT069B))
		.route("/is_employee_check.json", get(route_is_employee_check))
		.route("/employees", get(route_employees))
		.nest("/canvas", canvas::routes())
		.nest("/papercut", papercut::routes())
		.route("/", get(|| async { "pong\n" }))
		.layer(middleware::from_fn(mail_layer_middleware))
		.layer(Extension(banner_pool));

	let config = RustlsConfig::from_pem_file(
		PathBuf::from(".").join("self_signed_certs").join("cert.pem"),
		PathBuf::from(".").join("self_signed_certs").join("key.pem"),
	)
	.await?;

	let addr_http = SocketAddr::from(([0, 0, 0, 0], ports.http));
	let addr_https = SocketAddr::from(([0, 0, 0, 0], ports.https));
	let handle_http = Handle::new();
	let handle_shutdown_http = handle_http.clone();
	let handle_https = Handle::new();
	let handle_shutdown_https = handle_https.clone();
	// Serve both, because argos can't talk self-signed certs and can't get a real cert without exposing to internet...
	// tokio::spawn(redirect_http_to_https(ports, handle.clone()));
	tokio::spawn(async move {
		shutdown_signal().await;
		info!("shutdown signal triggerred, performing graceful shutdown");
		handle_shutdown_http.graceful_shutdown(Some(Duration::from_secs(30)));
		handle_shutdown_https.graceful_shutdown(Some(Duration::from_secs(30)));
	});
	let app_http = app.clone();
	tokio::spawn(async move {
		info!("HTTP listening on {}", addr_http);
		axum_server::bind(addr_http)
			.handle(handle_http)
			.serve(app_http.into_make_service())
			.await
			.expect("http server error");
	});
	info!("HTTPS listening on {}", addr_https);
	axum_server::bind_rustls(addr_https, config)
		.handle(handle_https)
		.serve(app.into_make_service())
		.await?;

	Ok(())
}

#[allow(dead_code)]
async fn redirect_http_to_https(ports: Ports, handle: Handle) {
	fn make_https(host: String, uri: Uri, ports: Ports) -> anyhow::Result<Uri> {
		let mut parts = uri.into_parts();

		parts.scheme = Some(axum::http::uri::Scheme::HTTPS);

		if parts.path_and_query.is_none() {
			parts.path_and_query = Some("/".parse()?);
		}

		let https_host = host.replace(&ports.http.to_string(), &ports.https.to_string());
		parts.authority = Some(https_host.parse()?);

		Ok(Uri::from_parts(parts)?)
	}

	let redirect = move |Host(host): Host, uri: Uri| async move {
		match make_https(host, uri, ports) {
			Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
			Err(error) => {
				tracing::warn!(%error, "failed to convert URI to HTTPS");
				Err(StatusCode::BAD_REQUEST)
			}
		}
	};

	let addr = SocketAddr::from(([0, 0, 0, 0], ports.http));
	info!("HTTP redirect listening on {}", addr);

	axum_server::bind(addr)
		.handle(handle)
		.serve(redirect.into_make_service())
		.await
		.expect("HTTP-to-HTTPS server errored");
}

#[instrument(skip(auth))]
async fn route_is_employee_check(
	AuthBearer(auth): AuthBearer,
	Query(params): Query<HashMap<String, String>>,
) -> AnyResult<Json<Result<String, String>>> {
	TOKEN_GENERAL.assert_valid(&auth)?;
	let cnum = params.get("cnum").context("missing required query 'cnum'")?;
	let int069a = get_last_matching_file_delete_rest_as_csv::<INT069ARow>("INT069A*.csv", BACKUP_KEEP)?;
	for user in int069a {
		if &user.Legacy_Banner_ID == cnum {
			let int069b = get_last_matching_file_delete_rest_as_csv::<INT069BRow>("INT069B*.csv", BACKUP_KEEP)?;
			for pos in int069b {
				if pos.Employee_ID == user.Employee_ID {
					//return Ok(format!(r#"{{"ok": {}}}"#, ));
					return Ok(Json(Ok(pos.Position_ID)));
				}
			}
			//return Ok(r#"{"error": "user has no associated position"}"#);
			return Ok(Json(Err("user has no associated position".to_string())));
		}
	}
	//Ok(r#"{"error": "user not found"}"#)
	Ok(Json(Err("user not found".to_string())))
}

#[instrument(skip(auth, banner_pool))]
async fn route_INT004(
	AuthBearer(auth): AuthBearer,
	Extension(banner_pool): Extension<BannerConnPool>,
) -> AnyResult<Json<Vec<INT004Row>>> {
	TOKEN_GENERAL.assert_valid(&auth)?;
	// let from = NaiveDate::from_ymd(2022, 8, 13);
	// let to = NaiveDate::from_ymd(2022, 8, 26);
	// let now = NaiveDate::from_ymd(2022, 09, 22);
	// let from = NaiveDate::from_ymd(2000, 01, 01);
	// let to = chrono::Utc::today();
	let conn = banner_pool.get()?;
	let rows = conn.query_as_named::<(String, String, String, NaiveDate, f64, NaiveDate, NaiveDate, String)>(
		r#"
select SPRIDEN.SPRIDEN_ID "CNum",
       SPRIDEN.SPRIDEN_LAST_NAME "Last_Name",
       SPRIDEN.SPRIDEN_FIRST_NAME "First_Name",
       TO_CHAR(RJRSEAR_ACTIVITY_DATE,'YYYY-MM-DD') "Activity_Date",
       RJRSEAR.RJRSEAR_AUTH_EARNINGS "Balance_Amount",
       TO_CHAR(RJRSEAR_AUTH_END_DATE,'YYYY-MM-DD') "Auth_End_Date",
       TO_CHAR(RJRSEAR_AUTH_START_DATE,'YYYY-MM-DD') "Auth_Start_Date",
       CASE
            WHEN RJRSEAR_FUND_CODE LIKE 'NM%' THEN 'STATE_AWD_AMT_MEMO'
            ELSE 'FWS_AWD_AMT_MEMO'
       END "Award_Type"
  from FAISMGR.RJRSEAR RJRSEAR,
       SATURN.SPRIDEN SPRIDEN
where ( RJRSEAR.RJRSEAR_PIDM = SPRIDEN.SPRIDEN_PIDM )
   and ( SPRIDEN.SPRIDEN_CHANGE_IND is null
         and RJRSEAR.RJRSEAR_AUTH_EARNINGS > 1
         and RJRSEAR.RJRSEAR_AUTH_END_DATE >=sysdate )
"#,
		&[
            //("main_DT_From", &from),
            //("main_DT_To", &to), // Oracle's `BETWEEN` is inclusive on both sides
        ],
	)?;
	let int069a = get_last_matching_file_delete_rest_as_csv::<INT069ARow>("INT069A*.csv", BACKUP_KEEP)?;
	let mappingCnumToEID: HashMap<String, String> = int069a
		.into_iter()
		.map(|row| (row.Legacy_Banner_ID, row.Employee_ID))
		.collect();
	let data: Vec<INT004Row> = rows
		.filter_map(move |row| -> Option<AnyResult<_>> {
			let row = match row {
				Ok(row) => row,
				Err(e) => return Some(Err(AnyError(e.into()))),
			};
			let (
				cnum,
				LAST_NAME,
				FIRST_NAME,
				ACTIVITY_DATE,
				BALANCE_AMOUNT,
				AUTH_END_DATE,
				AUTH_START_DATE,
				AWARD_TYPE,
			) = row;
			let Some(STUDENT_ID) = mappingCnumToEID.get(&cnum).cloned() else {
				return None;
			};
			//	.unwrap_or_else(|| format!("EID-NOT-FOUND-FOR-{cnum}"));
			Some(Ok(INT004Row {
				STUDENT_ID,
				LAST_NAME,
				FIRST_NAME,
				ACTIVITY_DATE,
				BALANCE_AMOUNT,
				AUTH_END_DATE,
				AUTH_START_DATE,
				AWARD_TYPE,
			}))
		})
		.collect::<Result<_, _>>()?;
	write_csv_with_date_to("INT004/INT004", &data, 0)?;
	Ok(Json(data))
}

#[instrument(skip(auth))]
async fn route_INT005A(AuthBearer(auth): AuthBearer) -> AnyResult<Json<Vec<INT005ARow>>> {
	TOKEN_GENERAL.assert_valid(&auth)?;
	let data = get_last_matching_file_delete_rest_as_csv("CCCWorkstudy*.csv", BACKUP_KEEP)?;
	Ok(Json(data))
}

#[instrument(skip(auth))]
async fn route_INT069A(AuthBearer(auth): AuthBearer) -> AnyResult<Json<Vec<INT069ARow>>> {
	TOKEN_GENERAL.assert_valid(&auth)?;
	let data = get_last_matching_file_delete_rest_as_csv("INT069A*.csv", BACKUP_KEEP)?;
	Ok(Json(data))
}

#[instrument(skip(auth))]
async fn route_INT069B(AuthBearer(auth): AuthBearer) -> AnyResult<Json<Vec<INT069BRow>>> {
	TOKEN_GENERAL.assert_valid(&auth)?;
	let data = get_last_matching_file_delete_rest_as_csv("INT069B*.csv", BACKUP_KEEP)?;
	Ok(Json(data))
}

#[derive(Debug, serde::Deserialize)]
struct Format {
	format: String,
}

#[instrument(skip(auth))]
async fn route_employees(AuthBearer(auth): AuthBearer, Query(format): Query<Format>) -> AnyResult<String> {
	let format = match format.format.as_str() {
		"csv" => "csv",
		unhandled => Err(anyhow::anyhow!("unsupported format: {unhandled}"))?,
	};
	TOKEN_GENERAL.assert_valid(&auth)?;
	let employees = get_last_matching_file_delete_rest_as_csv("INT069A*.csv", BACKUP_KEEP)?;
	let employees = employees
		.iter()
		.map(|e: &INT069ARow| (&e.Employee_ID, e))
		.collect::<HashMap<_, _>>();
	let roles = get_last_matching_file_delete_rest_as_csv::<INT069BRow>("INT069B*.csv", BACKUP_KEEP)?;
	let result = roles
		.iter()
		.map(|role| {
			let employee = employees.get(&role.Employee_ID);
			(role, employee)
		})
		.collect::<Vec<_>>();
	match format {
		"csv" => {
			let mut out = vec![];
			let mut w = csv::Writer::from_writer(&mut out);
			#[derive(Default, serde::Serialize)]
			struct EmployeeRec<'a> {
				Employee_ID: &'a str,
				User_Name: &'a str,
				dateOfBirth: &'a str,
				Legal_Name_First_Name: &'a str,
				Legal_Name_Middle_Name: &'a str,
				Legal_Name_Last_Name: &'a str,
				Preferred_Name_First_Name: &'a str,
				Preferred_Name_Middle_Name: &'a str,
				Preferred_Name_Last_Name: &'a str,
				Email_Work: &'a str,
				Email_Home: &'a str,
				primaryWorkPhone: &'a str,
				Phone_Primary_Home: &'a str,
				Primary_Mobile_Phone: &'a str,
				Home_Address_Formatted_Line_1: &'a str,
				Home_Address_Formatted_Line_2: &'a str,
				Home_Address_Formatted_Line_3: &'a str,
				Home_Address_city: &'a str,
				Home_Address_State: &'a str,
				Home_Address_Postal_Code: &'a str,
				Legacy_Banner_ID: &'a str,
				Position_ID: &'a str,
				External_Position_ID: &'a str,
				Time_Type: &'a str,
				Employee_Type: &'a str,
				Position_Title: &'a str,
				Job_Profile_ID: &'a str,
				Job_Profile_Name: &'a str,
				Job_Family_Group: &'a str,
				Position_Start: &'a str,
				Position_End: &'a str,
				Division: &'a str,
				Department: &'a str,
				Position_Location_Address_Line_1: &'a str,
				Position_Location_Address_Line_2: &'a str,
				Position_Location_Address_Line_3: &'a str,
				Position_Location_Address_City: &'a str,
				Position_Location_Address_State: &'a str,
				Position_Location_Address_Postal_Code: &'a str,
				Cost_Center_Code: &'a str,
				Cost_Center_Description: &'a str,
				Manager_Employee_ID: &'a str,
			}
			for (r, me) in result {
				if let Some(e) = me {
					w.serialize(EmployeeRec {
						User_Name: &e.User_Name,
						dateOfBirth: &e.dateOfBirth,
						Legal_Name_First_Name: &e.Legal_Name_First_Name,
						Legal_Name_Middle_Name: &e.Legal_Name_Middle_Name,
						Legal_Name_Last_Name: &e.Legal_Name_Last_Name,
						Preferred_Name_First_Name: &e.Preferred_Name_First_Name,
						Preferred_Name_Middle_Name: &e.Preferred_Name_Middle_Name,
						Preferred_Name_Last_Name: &e.Preferred_Name_Last_Name,
						Email_Work: &e.Email_Work,
						Email_Home: &e.Email_Home,
						primaryWorkPhone: &e.primaryWorkPhone,
						Phone_Primary_Home: &e.Phone_Primary_Home,
						Primary_Mobile_Phone: &e.Primary_Mobile_Phone,
						Home_Address_Formatted_Line_1: &e.Home_Address_Formatted_Line_1,
						Home_Address_Formatted_Line_2: &e.Home_Address_Formatted_Line_2,
						Home_Address_Formatted_Line_3: &e.Home_Address_Formatted_Line_3,
						Home_Address_city: &e.Home_Address_city,
						Home_Address_State: &e.Home_Address_State,
						Home_Address_Postal_Code: &e.Home_Address_Postal_Code,
						Legacy_Banner_ID: &e.Legacy_Banner_ID,
						Position_ID: &r.Position_ID,
						External_Position_ID: &r.External_Position_ID,
						Employee_ID: &r.Employee_ID,
						Time_Type: &r.Time_Type,
						Employee_Type: &r.Employee_Type,
						Position_Title: &r.Position_Title,
						Job_Profile_ID: &r.Job_Profile_ID,
						Job_Profile_Name: &r.Job_Profile_Name,
						Job_Family_Group: &r.Job_Family_Group,
						Position_Start: &r.Position_Start,
						Position_End: &r.Position_End,
						Division: &r.Division,
						Department: &r.Department,
						Position_Location_Address_Line_1: &r.Position_Location_Address_Line_1,
						Position_Location_Address_Line_2: &r.Position_Location_Address_Line_2,
						Position_Location_Address_Line_3: &r.Position_Location_Address_Line_3,
						Position_Location_Address_City: &r.Position_Location_Address_City,
						Position_Location_Address_State: &r.Position_Location_Address_State,
						Position_Location_Address_Postal_Code: &r.Position_Location_Address_Postal_Code,
						Cost_Center_Code: &r.Cost_Center_Code,
						Cost_Center_Description: &r.Cost_Center_Description,
						Manager_Employee_ID: &r.Manager_Employee_ID,
					})?;
				} else {
					w.serialize(EmployeeRec {
						Position_ID: &r.Position_ID,
						External_Position_ID: &r.External_Position_ID,
						Employee_ID: &r.Employee_ID,
						Time_Type: &r.Time_Type,
						Employee_Type: &r.Employee_Type,
						Position_Title: &r.Position_Title,
						Job_Profile_ID: &r.Job_Profile_ID,
						Job_Profile_Name: &r.Job_Profile_Name,
						Job_Family_Group: &r.Job_Family_Group,
						Position_Start: &r.Position_Start,
						Position_End: &r.Position_End,
						Division: &r.Division,
						Department: &r.Department,
						Position_Location_Address_Line_1: &r.Position_Location_Address_Line_1,
						Position_Location_Address_Line_2: &r.Position_Location_Address_Line_2,
						Position_Location_Address_Line_3: &r.Position_Location_Address_Line_3,
						Position_Location_Address_City: &r.Position_Location_Address_City,
						Position_Location_Address_State: &r.Position_Location_Address_State,
						Position_Location_Address_Postal_Code: &r.Position_Location_Address_Postal_Code,
						Cost_Center_Code: &r.Cost_Center_Code,
						Cost_Center_Description: &r.Cost_Center_Description,
						Manager_Employee_ID: &r.Manager_Employee_ID,
						..Default::default()
					})?;
				}
			}
			drop(w);
			Ok(String::from_utf8(out)?)
		}
		_ => panic!("impossible"),
	}
}
