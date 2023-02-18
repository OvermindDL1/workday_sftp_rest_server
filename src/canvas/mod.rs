use crate::canvas::csv_defs::{Status, Terms};
use crate::utils::csv::validate_csv;
use crate::utils::email::mail_layer_middleware;
use crate::utils::AnyResult;
use crate::SETTINGS;
use anyhow::Context;
use axum::extract::Query;
use axum::routing::get;
use axum::{middleware, Json, Router};
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::*;

pub mod csv_defs;

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
	pub base_url: String,
	pub token: String,
	pub account: String,
	pub username: String,
	pub password: String,
	#[serde(skip, default = "Settings::default_url")]
	pub sis_import_url: Url,
}

impl Default for Settings {
	fn default() -> Self {
		Settings {
			base_url: "https://yoururl.instructure.com".to_string(),
			token: "accesstokenhereabcdefghijklmnopqrstuvwxyz".to_string(),
			account: "12345".to_string(),
			username: "usernamehere".to_string(),
			password: "passwordhere".to_string(),
			sis_import_url: Settings::default_url(),
		}
	}
}

impl Settings {
	fn default_url() -> Url {
		Url::parse("https://unset/").expect("invalid unset default Url?")
	}

	pub fn rebuild_cache(self) -> anyhow::Result<Self> {
		Ok(Self {
			sis_import_url: Url::parse(&format!(
				"{base_url}/api/v1/accounts/{account}/sis_imports.json?access_token={token}",
				base_url = self.base_url,
				account = self.account,
				token = self.token
			))?,
			..self
		})
	}

	pub fn get_sis_check_url(&self, workflow_id: u64) -> Result<Url, url::ParseError> {
		Url::parse(&format!(
			"{base_url}/api/v1/accounts/{account}/sis_imports/{workflow_id}",
			base_url = self.base_url,
			account = self.account,
		))
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
	trace!(
		data = ?String::from_utf8_lossy(&data),
		sis_import_url = ?&settings.canvas.sis_import_url
	);
	let resp = client
		.post(settings.canvas.sis_import_url.clone())
		.header("Content-Type", "text/csv")
		.basic_auth(&settings.canvas.username, Some(&settings.canvas.password))
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
		.layer(middleware::from_fn(mail_layer_middleware))
}

#[derive(Default, Serialize)]
struct HRTraining {
	#[serde(skip_serializing_if = "Vec::is_empty")]
	terms: Vec<Terms>,
	#[serde(skip_serializing_if = "serde_json::Value::is_null")]
	terms_response: serde_json::Value,
}

#[derive(Deserialize)]
struct HRTrainingParams {
	#[serde(default)]
	r#return: bool,
	year: Option<i32>,
}

async fn route_hr_training(Query(params): Query<HRTrainingParams>) -> AnyResult<Json<HRTraining>> {
	let client = reqwest::Client::new();
	let year = params.year.unwrap_or_else(|| chrono::Utc::today().year());
	let terms = vec![Terms {
		term_id: format!("Calendar Year {year}"),
		name: format!("Calendar Year {year}"),
		status: Status::Active,
		integration_id: None,
		date_override_enrollment_type: None,
		start_date: Some(NaiveDateTime::new(
			NaiveDate::from_ymd(year, 1, 1),
			NaiveTime::from_hms(0, 0, 0),
		)),
		end_date: Some(NaiveDateTime::new(
			NaiveDate::from_ymd(year, 12, 31),
			NaiveTime::from_hms(23, 59, 59),
		)),
	}];
	let terms_response = post_csv_to_canvas(&client, &terms).await.unwrap_or_else(|e| {
		serde_json::json!({
			"error": e.to_string()
		})
	});
	if params.r#return {
		Ok(Json(HRTraining { terms, terms_response }))
	} else {
		Ok(Json(HRTraining::default()))
	}
}
