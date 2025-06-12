pub mod queries;

use crate::SETTINGS;
use r2d2_oracle::r2d2::{Pool, PooledConnection};
use r2d2_oracle::OracleConnectionManager;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
	pub conn: String,
	pub username: String,
	pub password: String,
	pub concurrent_connections: u32,
	pub update_user_id: Option<String>,
}

impl Default for Settings {
	fn default() -> Self {
		Settings {
			conn: "//localhost:1521/orcl".to_string(),
			username: "banner".to_string(),
			password: "no-password-set".to_string(),
			concurrent_connections: 2,
			update_user_id: None,
		}
	}
}

impl Settings {
	pub fn rebuild_cache(self) -> anyhow::Result<Self> {
		Ok(self)
	}
}

#[derive(Clone, Debug)]
pub struct BannerConnPool {
	pub pool: Pool<OracleConnectionManager>,
}

impl BannerConnPool {
	#[tracing::instrument(level = "debug")]
	pub fn new() -> anyhow::Result<BannerConnPool> {
		let conn_args = &SETTINGS.get().expect("settings must already be initialized").banner;
		if let Some(banner_update_user_id) = &conn_args.update_user_id {
			if banner_update_user_id.is_empty() {
				anyhow::bail!("Banner update user ID must be filled in if specified");
			}
			for b in banner_update_user_id.bytes() {
				if !b.is_ascii_alphanumeric() {
					anyhow::bail!("Banner update user ID must be alphanumeric only");
				}
			}
		}
		let oracle_manager = OracleConnectionManager::new(&conn_args.username, &conn_args.password, &conn_args.conn);
		let pool = Pool::builder().max_size(2).build(oracle_manager)?;
		tracing::info!(
			"Initialized Banner connection pool with {} connection(s)",
			conn_args.concurrent_connections
		);
		Ok(BannerConnPool { pool })
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub fn get(&self) -> anyhow::Result<PooledConnection<OracleConnectionManager>> {
		let conn = self.pool.get()?;
		Ok(conn)
	}
}
