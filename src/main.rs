#![allow(non_snake_case)]

use anyhow::Context as _;
use axum::extract::{Host, Query};
use axum::handler::Handler;
use axum::http::{StatusCode, Uri};
use axum::response::Redirect;
use axum::{routing::get, Extension, Json, Router};
use axum_auth::AuthBearer;
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use chrono::NaiveDate;
use clap::Parser;
use r2d2_oracle::r2d2::Pool;
use r2d2_oracle::{r2d2, OracleConnectionManager};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tracing::*;
use workday_sftp_rest_server::args::Args;
use workday_sftp_rest_server::canvas;
use workday_sftp_rest_server::configuration::BACKUP_KEEP;
use workday_sftp_rest_server::csv_row_definitions::*;
use workday_sftp_rest_server::logging::setup_logger;
use workday_sftp_rest_server::utils::csv::{get_last_matching_file_delete_rest_as_csv, write_csv_with_date_to};
use workday_sftp_rest_server::utils::{shutdown_signal, AnyResult, Token};

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

type OraclePool = Pool<OracleConnectionManager>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	// TODO: Automatic starting via systemd service
	// TODO: (Tokio-based?) internal cron-job functionality
	let args = Args::parse();
	args.init_settings().context("initializing settings")?;
	setup_logger(&args)?;

	let oracleManager = OracleConnectionManager::new(
		"argos_all",
		"2014_arg0z_iz_k3wl",
		// "ditusr",
		// "fgz940ty459",
		"//prodbandb.clovis.edu/PROD.clovis.edu",
	);
	let oraclePool: OraclePool = r2d2::Pool::builder().max_size(2).build(oracleManager)?;

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
		.nest("/canvas", canvas::routes())
		.route("/", get(|| async { "pong\n" }))
		.layer(Extension(oraclePool));

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

#[instrument(skip(auth, oraclePool))]
async fn route_INT004(
	AuthBearer(auth): AuthBearer,
	Extension(oraclePool): Extension<OraclePool>,
) -> AnyResult<Json<Vec<INT004Row>>> {
	TOKEN_GENERAL.assert_valid(&auth)?;
	// let from = NaiveDate::from_ymd(2022, 8, 13);
	// let to = NaiveDate::from_ymd(2022, 8, 26);
	// let now = NaiveDate::from_ymd(2022, 09, 22);
	// let from = NaiveDate::from_ymd(2000, 01, 01);
	// let to = chrono::Utc::today();
	let conn = oraclePool.get()?;
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
		.map(move |row| -> AnyResult<_> {
			let (
				cnum,
				LAST_NAME,
				FIRST_NAME,
				ACTIVITY_DATE,
				BALANCE_AMOUNT,
				AUTH_END_DATE,
				AUTH_START_DATE,
				AWARD_TYPE,
			) = row?;
			let STUDENT_ID = mappingCnumToEID
				.get(&cnum)
				.cloned()
				.unwrap_or_else(|| format!("EID-NOT-FOUND-FOR-{cnum}"));
			Ok(INT004Row {
				STUDENT_ID,
				LAST_NAME,
				FIRST_NAME,
				ACTIVITY_DATE,
				BALANCE_AMOUNT,
				AUTH_END_DATE,
				AUTH_START_DATE,
				AWARD_TYPE,
			})
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
