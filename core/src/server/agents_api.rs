//! `/v1/agents/*` handlers.

use anyhow::Context;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tracing::instrument;
use uuid::Uuid;

use crate::proto::agent::{Agent, CancelResponse, SpawnAgentRequest};
use crate::server::{error::AppError, AppState};

#[instrument(skip(state, req), fields(kind = ?req.kind))]
pub async fn spawn(
    State(state): State<AppState>,
    Json(req): Json<SpawnAgentRequest>,
) -> Result<Json<Agent>, AppError> {
    let agent = state.agents.spawn(req).await?;
    Ok(Json(agent))
}

pub async fn list(State(state): State<AppState>) -> Json<Vec<Agent>> {
    Json(state.agents.list())
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Agent>, (StatusCode, String)> {
    let uuid = Uuid::parse_str(&id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    match state.agents.get(uuid) {
        Some(a) => Ok(Json(a)),
        None => Err((StatusCode::NOT_FOUND, format!("agent not found: {id}"))),
    }
}

#[instrument(skip(state), fields(id = %id))]
pub async fn cancel(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CancelResponse>, AppError> {
    let uuid = Uuid::parse_str(&id).context("invalid uuid")?;
    let cancelled = state.agents.cancel(uuid);
    Ok(Json(CancelResponse { cancelled, id }))
}
