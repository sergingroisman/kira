//! ACP error types and axum response conversion.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Errors that can occur during ACP message handling.
#[derive(Debug, thiserror::Error)]
pub enum AcpError {
    /// The recipient endpoint was not found.
    #[error("unknown recipient")]
    UnknownRecipient,

    /// The request timed out before a response was received.
    #[error("request timed out")]
    Timeout,

    /// Failed to serialize or deserialize a message.
    #[error("serialization error")]
    Serialization(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// An internal server error occurred.
    #[error("internal error")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl IntoResponse for AcpError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::UnknownRecipient => (StatusCode::NOT_FOUND, self.to_string()),
            Self::Timeout => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Self::Serialization(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
