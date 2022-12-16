pub mod csv;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tokio::signal;
use tracing::info;

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
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("error: {}\n", self.0),
        )
            .into_response()
    }
}

pub async fn shutdown_signal() {
    info!("awaiting shutdown signal...");

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
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
