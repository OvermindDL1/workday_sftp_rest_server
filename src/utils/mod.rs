pub mod csv;
pub mod email;
pub mod serde_regex;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tokio::signal;
use tracing::info;

pub struct Token([u8; 64]);
// impl Default for Token {
//     fn default() -> Self {
//         Token(&[
//             34, 23, 34, 20, 40, 160, 68, 51, 102, 241, 157, 171, 244, 48, 196, 243, 179, 79, 225,
//             172, 37, 168, 194, 219, 65, 11, 102, 80, 162, 45, 7, 44, 8, 9, 147, 142, 193, 126, 233,
//             184, 17, 65, 120, 49, 136, 90, 217, 196, 19, 111, 162, 103, 240, 185, 60, 1, 82, 6,
//             142, 150, 143, 149, 203, 255,
//         ])
//     }
// }
impl Token {
	pub const fn new(hash: [u8; 64]) -> Token {
		Token(hash)
	}

	pub fn assert_valid(&self, token: &str) -> anyhow::Result<()> {
		use sha2::Digest;
		let mut hasher = sha2::Sha512::new();
		hasher.update(token.as_bytes());
		let hash = hasher.finalize();
		if self.0 != hash.as_slice() {
			anyhow::bail!("invalid token");
		}
		Ok(())
	}
}

// Make our own error that wraps `anyhow::Error`.
pub struct AnyError(anyhow::Error);
pub type AnyResult<T> = Result<T, AnyError>;

impl<E> From<E> for AnyError
where
	E: Into<anyhow::Error>,
{
	fn from(err: E) -> Self {
		Self(err.into())
	}
}

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AnyError {
	fn into_response(self) -> Response {
		(StatusCode::INTERNAL_SERVER_ERROR, format!("error: {}\n", self.0)).into_response()
	}
}

pub async fn shutdown_signal() {
	info!("awaiting shutdown signal...");

	let ctrl_c = async {
		signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
	};

	#[cfg(unix)]
	let terminate = async {
		signal::unix::signal(signal::unix::SignalKind::terminate())
			.expect("failed to install signal handler")
			.recv()
			.await;
	};

	#[cfg(not(unix))]
	let terminate = std::future::pending::<()>();

	tokio::select! {
		_ = ctrl_c => {},
		_ = terminate => {},
	}

	tracing::info!("shutdown signal received");
}
