use crate::SETTINGS;
use axum::body;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use hyper::body::Buf;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::Tls;
use lettre::transport::smtp::extension::ClientId;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
	pub smtp_from: Mailbox,
	pub smtp_reply_to: Option<Mailbox>,
	pub smtp_server: String,
	pub smtp_login: Option<String>,
	pub smtp_password: Option<String>,
	pub replies: Vec<SettingsReplies>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SettingsReplies {
	#[serde(with = "crate::utils::serde_regex")]
	pub path: Regex,
	pub to: Vec<Mailbox>,
	pub cc: Vec<Mailbox>,
	pub bcc: Vec<Mailbox>,
}

impl Default for Settings {
	fn default() -> Self {
		Settings {
			smtp_from: "Someone <from@address.tld>".parse().unwrap(),
			smtp_reply_to: Some(
				"Reply To Me or delete this whole setting to not use it <elsewhere@address.tld>"
					.parse()
					.unwrap(),
			),
			smtp_server: "127.0.0.1".to_string(),
			smtp_login: Some("theloginmightsameasemail".to_string()),
			smtp_password: Some("emailpassword".to_string()),
			replies: vec![
				SettingsReplies {
					path: Regex::new("/blah/blorp.*").unwrap(),
					to: vec!["The To <to@address.tld>".parse().unwrap()],
					cc: vec!["A CC <a@address.tld>".parse().unwrap()],
					bcc: vec![],
				},
				SettingsReplies {
					path: Regex::new("/blah/blorp/bloop").unwrap(),
					to: vec!["The To <to@address.tld>".parse().unwrap()],
					cc: vec![
						"A CC <a@address.tld>".parse().unwrap(),
						"Another CC <another@address.tld>".parse().unwrap(),
					],
					bcc: vec!["A BCC <b@address.tld>".parse().unwrap()],
				},
			],
		}
	}
}

impl Settings {
	pub fn rebuild_cache(self) -> anyhow::Result<Self> {
		Ok(self)
	}
}

pub async fn mail_layer_middleware(request: Request<body::Body>, next: Next) -> Response {
	let uri = request.uri().path().to_string();
	let response = next.run(request).await;
	let (parts, body) = response.into_parts();
	let data = match axum::body::to_bytes(body, usize::MAX).await {
		Ok(data) => data,
		Err(err) => {
			error!("processing mail_layer_middleware, failure to process body: {err:?}");
			return (
				StatusCode::INTERNAL_SERVER_ERROR,
				"unknown server error in processing mail output body",
			)
				.into_response();
		}
	};
	if data.len() < 30000 {
		let mail_data = data.clone();
		tokio::spawn(async move {
			if let Err(err) = send_email(&uri, Ok(mail_data.chunk())).await {
				error!("sending email: {err:?}")
			}
		});
	}
	Response::from_parts(parts, body::Body::from(data))
}

#[instrument(level = "debug", skip(body))]
pub async fn send_email(uri: &str, body: Result<&[u8], String>) -> anyhow::Result<()> {
	dbg!(uri);
	let settings = SETTINGS.get().expect("settings not loaded");
	let msg = Message::builder().from(settings.email.smtp_from.clone());
	let msg = if let Some(reply_to) = &settings.email.smtp_reply_to {
		msg.reply_to(reply_to.clone())
	} else {
		msg
	};
	let mut msg = msg;
	for rep in settings.email.replies.iter().filter(|rep| rep.path.is_match(uri)) {
		msg = rep.to.iter().fold(msg, |m, to| m.to(to.clone()));
		msg = rep.cc.iter().fold(msg, |m, cc| m.cc(cc.clone()));
		msg = rep.bcc.iter().fold(msg, |m, bcc| m.bcc(bcc.clone()));
	}
	let msg = match body {
		Ok(data) => {
			let body = if let Ok(json) = serde_json::from_slice::<serde_json::Value>(data) {
				serde_json::to_string_pretty(&json).unwrap_or_else(|_| String::from_utf8_lossy(data).to_string())
			} else {
				String::from_utf8_lossy(data).to_string()
			};
			msg.subject(format!("Success Process: {uri}")).body(body)?
		}
		Err(err) => msg.subject(format!("Failed Process: {uri}")).body(err)?,
	};
	let creds = match (&settings.email.smtp_login, &settings.email.smtp_password) {
		(Some(login), Some(password)) => Some(Credentials::new(login.to_string(), password.to_string())),
		_ => None,
	};
	let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.email.smtp_server)?
		.port(25)
		.hello_name(ClientId::Domain("sftp_workday.clovis.edu".to_string()));
	let mailer = if let Some(creds) = creds {
		mailer.credentials(creds)
	} else {
		mailer
	};
	let mailer = mailer.tls(Tls::None).build();
	mailer.send(msg).await?;
	Ok(())
}
