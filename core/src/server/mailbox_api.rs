//! `/v1/mailbox/*` handlers.

use axum::{
    extract::{Path, State},
    Json,
};
use tracing::instrument;

use crate::proto::mailbox::{AckResponse, Message, SendRequest, SendResponse};
use crate::server::{error::AppError, AppState};

#[instrument(skip(state, req), fields(to = %to, from = %req.from))]
pub async fn send(
    State(state): State<AppState>,
    Path(to): Path<String>,
    Json(req): Json<SendRequest>,
) -> Result<Json<SendResponse>, AppError> {
    let (id, path) = state.mailbox.send(&to, req).await?;
    Ok(Json(SendResponse {
        id,
        path: path.display().to_string(),
    }))
}

#[instrument(skip(state), fields(agent_id = %agent_id))]
pub async fn poll(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<Vec<Message>>, AppError> {
    let messages = state.mailbox.poll(&agent_id).await?;
    Ok(Json(messages))
}

#[instrument(skip(state), fields(agent_id = %agent_id, id = %id))]
pub async fn ack(
    State(state): State<AppState>,
    Path((agent_id, id)): Path<(String, String)>,
) -> Result<Json<AckResponse>, AppError> {
    let acked = state.mailbox.ack(&agent_id, &id).await?;
    Ok(Json(AckResponse { acked, id }))
}
