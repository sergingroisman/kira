//! Axum-compatible error type wrapping `anyhow::Error`.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use tracing::error;

/// Wraps `anyhow::Error` so axum can return it as HTTP 500 with JSON body.
pub struct AppError(pub anyhow::Error);

impl AppError {
    /// Create an AppError with a 400 Bad Request status.
    pub fn bad_request(msg: &str) -> Self {
        Self(anyhow::anyhow!("{msg}"))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        error!(error = ?self.0, "request failed");
        let status = if self.0.to_string().contains("bad_request")
            || self.0.to_string().contains("not found")
        {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        (
            status,
            Json(serde_json::json!({
                "error": self.0.to_string(),
                "chain": format!("{:#}", self.0),
            })),
        )
            .into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
