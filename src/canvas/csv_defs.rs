// #![allow(non_snake_case)]

//! https://cloviscc.instructure.com/doc/api/file.sis_csv.html

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
	Active,
	Suspended,
	Deleted,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Terms {
	pub term_id: String,
	pub name: String,
	pub status: Status,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub integration_id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub date_override_enrollment_type: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub start_date: Option<NaiveDateTime>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub end_date: Option<NaiveDateTime>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Users {
	pub user_id: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub integration_id: Option<String>,
	pub login_id: String,
	pub password: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub ssha_password: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub authentication_provider_id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub first_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub last_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub full_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub sortable_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub short_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub email: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub pronouns: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub declared_user_type: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub canvas_password_notification: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub home_account: Option<String>,
	pub status: Status,
}
