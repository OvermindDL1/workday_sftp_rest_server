use crate::configuration::BACKUP_KEEP;
use crate::csv_row_definitions::{INT069ARow, INT069BRow};
use crate::utils::csv::get_last_matching_file_delete_rest_as_csv;
use crate::utils::{AnyResult, Token};
use crate::SETTINGS;
use axum::routing::post;
use axum::{Json, Router};
use axum_auth::AuthBearer;
use csv::Terminator;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use suppaftp::FtpStream;
use tracing::*;

const TOKEN_GENERAL: Token = Token::new([
	34, 23, 34, 20, 40, 160, 68, 51, 102, 241, 157, 171, 244, 48, 196, 243, 179, 79, 225, 172, 37, 168, 194, 219, 65,
	11, 102, 80, 162, 45, 7, 44, 8, 9, 147, 142, 193, 126, 233, 184, 17, 65, 120, 49, 136, 90, 217, 196, 19, 111, 162,
	103, 240, 185, 60, 1, 82, 6, 142, 150, 143, 149, 203, 255,
]);

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
	pub address: String,
	pub port: u16,
	pub username: String,
	pub password: String,
	pub create_accounts_for: CreateAccountsFor,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum CreateAccountsFor {
	None,
	PositionIDs { position_ids: Vec<String> },
	ActiveEmployees,
}

impl CreateAccountsFor {
	fn job_matches(&self, row: &INT069BRow) -> bool {
		match self {
			CreateAccountsFor::None => false,
			CreateAccountsFor::PositionIDs { position_ids } => position_ids.contains(&row.Position_ID),
			CreateAccountsFor::ActiveEmployees => !row.Employee_ID.is_empty(),
		}
	}
}

impl Default for Settings {
	fn default() -> Self {
		Settings {
			address: "10.0.1.3".to_string(),
			port: 21,
			username: "banjobs".to_string(),
			password: "".to_string(),
			create_accounts_for: CreateAccountsFor::PositionIDs {
				position_ids: vec!["P000188".to_string(), "P000189".to_string()],
			},
		}
	}
}

impl Settings {
	pub fn rebuild_cache(self) -> anyhow::Result<Self> {
		Ok(self)
	}
}

pub fn routes() -> Router {
	Router::new().route("/feed_workday_users", post(route_feed_workday_users))
}

// Papercut is apparently Oz managed, and they've set it up very... wrong... so conequently not using the bulk import
// API, it's instead a Windows Scheduled Task for whatever reason, so we need to inject a seemingly undocumented format
// at the end of the existing processing file in between when banner generates its side and the windows server task runs
//
// #[derive(Serialize)]
// #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
// enum YN {
// 	Y,
// 	N,
// }
//
// #[derive(Serialize)]
// #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
// enum InvoiceOption {
// 	AlwaysInvoice,
// 	NeverInvoice,
// 	UserChoiceOn,
// 	UserChoiceOff,
// }
//
// #[derive(Serialize)]
// #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
// enum CommentOption {
// 	NoComment,
// 	CommentRequired,
// 	CommentOptional,
// }
//
// // http://10.0.1.3:9191/content/help/applicationserver/topics/account-import-update.html?language=en
// #[derive(Serialize)]
// struct PaperCutRow {
// 	account_name: String,
// 	sub_account_name: Option<String>,
// 	enabled: Option<YN>,
// 	pin: String,
// 	credit_balance: f32,
// 	restricted_status: Option<YN>,
// 	users: Option<String>,
// 	groups: Option<String>,
// 	invoice_option: Option<InvoiceOption>,
// 	comment_option: Option<CommentOption>,
// 	notes: Option<String>,
// }

#[instrument(level = "debug", skip(auth))]
async fn route_feed_workday_users(AuthBearer(auth): AuthBearer) -> AnyResult<Json<serde_json::Value>> {
	TOKEN_GENERAL.assert_valid(&auth)?;
	let settings = &SETTINGS.get().expect("settings don't exist").papercut;
	let mut warnings = Vec::new();
	// Yes papercut uses Tab Delimeted Files with no headers...
	let mut csv_writer = csv::WriterBuilder::new()
		.delimiter(b'\t')
		.has_headers(false)
		.terminator(Terminator::CRLF)
		.from_writer(Vec::new());
	let int069a: HashMap<String, INT069ARow> =
		get_last_matching_file_delete_rest_as_csv::<INT069ARow>("INT069A*.csv", BACKUP_KEEP)?
			.into_iter()
			.map(|row| (row.Employee_ID.clone(), row))
			.collect();
	get_last_matching_file_delete_rest_as_csv::<INT069BRow>("INT069B*.csv", BACKUP_KEEP)?
		.into_iter()
		.filter(|row| !row.Employee_ID.is_empty())
		.filter(|row| settings.create_accounts_for.job_matches(row))
		.try_for_each(|row| -> anyhow::Result<()> {
			let Some(emp) = int069a.get(&row.Employee_ID) else {
				warnings.push(json!({"type": "position has employee but employee is not found", "position_id": row.Position_ID, "employee_id": row.Employee_ID}));
				return Ok(());
			};
			let account_name = emp.Legacy_Banner_ID.clone();
			if account_name.is_empty() || account_name.len() != 9 || !account_name.starts_with(['C', 'c']) || !account_name.as_bytes().iter().skip(1).all(u8::is_ascii_digit) {
				warnings.push(json!({"type": "legacy_banner_id for employee_id is blank or not in the right format", "employee_id": row.Employee_ID, "legacy_banner_id": emp.Legacy_Banner_ID}));
				return Ok(());
			}
			let last_name = &emp.Legal_Name_Last_Name;
			let first_name = &emp.Legal_Name_First_Name;
			let full_name = format!("{first_name} {last_name}");
			let email = &emp.User_Name; // User_Name in workday is the azure email, always valid
			// No clue what the empty fields are: "C00012345^ILastName^I^IY^IFull Name^Iemail@address^I^I^I^I^I$"
			csv_writer.write_record([account_name.as_bytes(), last_name.as_bytes(), b"", b"Y", full_name.as_bytes(), email.as_bytes(), b"", b"", b"", b"", b""])?;
			// csv_writer.serialize(PaperCutRow{
			// 	account_name,
			// 	sub_account_name: Some(emp.Preferred_Name_Last_Name.clone()),
			// 	enabled: None,
			// 	pin: "".to_string(),
			// 	credit_balance: 0.0,
			// 	restricted_status: None,
			// 	users: None,
			// 	groups: None,
			// 	invoice_option: None,
			// 	comment_option: None,
			// 	notes: None,
			// })?;
			Ok(())
		})?;
	let data = csv_writer.into_inner()?;

	// And now, because of how Oz has the papercut system setup...
	let destination = "uploads-emp.txt";
	let mut ftp = FtpStream::connect(format!("{}:{}", settings.address, settings.port))?;
	ftp.login(&settings.username, &settings.password)?;
	let written = ftp.put_file(destination, &mut data.as_slice())?;
	if written != data.len() as u64 {
		ftp.quit()?;
		return Err(anyhow::anyhow!(
			"failed to write all data to papercut, only wrote {written}, should have written {}",
			data.len()
		)
		.into());
	}
	// dbg!((written, data.len()));
	// dbg!(ftp.list(Some("/"))?);
	// dbg!(String::from_utf8_lossy(ftp.retr_as_buffer(destination)?.get_ref()));

	if warnings.is_empty() {
		Ok(Json(
			json!({ "status": "success", "papercut-destination": destination, "new_data": String::from_utf8_lossy(&data) }),
		))
	} else {
		Ok(Json(
			json!({ "status": "success-with-warnings", "papercut-destination": destination, "new_data": String::from_utf8_lossy(&data), "warnings": warnings }),
		))
	}
}
