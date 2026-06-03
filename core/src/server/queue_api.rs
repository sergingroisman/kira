//! Queue API — REST endpoints for message queue management.
//!
//! - `GET /v1/queue` — list pending messages (queued + processing)
//! - `DELETE /v1/queue/:queue_id` — cancel a queued message

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;
use uuid::Uuid;

use super::AppState;
use crate::queue::MessageQueue;

/// Response for a queue message in list.
#[derive(Debug, Serialize)]
pub struct QueueMessageResponse {
    pub queue_id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub attempts: u32,
}

/// GET /v1/queue
///
/// Returns pending messages (queued + processing), ordered by created_at asc.
pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    let Some(ref queue) = state.queue else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "queue_unavailable"})),
        );
    };

    match queue.list_pending().await {
        Ok(messages) => {
            let items: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "queue_id": m.queue_id.to_string(),
                        "status": m.status.to_string(),
                        "created_at": m.created_at.to_rfc3339(),
                        "updated_at": m.updated_at.to_rfc3339(),
                        "attempts": m.attempts,
                    })
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!({"messages": items})))
        }
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "queue_unavailable"})),
        ),
    }
}

/// DELETE /v1/queue/:queue_id
///
/// Cancels a queued message. Returns 409 if already processing.
pub async fn cancel(
    State(state): State<AppState>,
    Path(queue_id): Path<String>,
) -> impl IntoResponse {
    let uuid = match Uuid::parse_str(&queue_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid queue_id format"})),
            );
        }
    };

    let Some(ref queue) = state.queue else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "queue_unavailable"})),
        );
    };

    match queue.cancel(uuid).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "cancelled"})),
        ),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already_processing") {
                (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({"error": "message_already_processing"})),
                )
            } else {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "message_not_found"})),
                )
            }
        }
    }
}
