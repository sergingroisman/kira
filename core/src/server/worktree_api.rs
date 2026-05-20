//! `/v1/worktree/*` handlers.

use anyhow::Context;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use tracing::instrument;
use uuid::Uuid;

use crate::proto::worktree::{DestroyResponse, SpawnRequest, Worktree};
use crate::server::{error::AppError, AppState};

#[instrument(skip(state, req), fields(base = %req.base))]
pub async fn spawn(
    State(state): State<AppState>,
    Json(req): Json<SpawnRequest>,
) -> Result<Json<Worktree>, AppError> {
    let wt = state.worktrees.spawn(req).await?;
    Ok(Json(wt))
}

#[instrument(skip(state), fields(id = %id))]
pub async fn destroy(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DestroyResponse>, AppError> {
    let uuid = Uuid::parse_str(&id).context("invalid uuid")?;
    let resp = state.worktrees.destroy(uuid, true).await?;
    Ok(Json(resp))
}

pub async fn list(State(state): State<AppState>) -> Json<Vec<Worktree>> {
    Json(state.worktrees.list())
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Worktree>, (StatusCode, String)> {
    let uuid = Uuid::parse_str(&id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    match state.worktrees.get(uuid) {
        Some(wt) => Ok(Json(wt)),
        None => Err((StatusCode::NOT_FOUND, format!("worktree not found: {id}"))),
    }
}

// Suppress unused warning from IntoResponse import where not directly needed
#[allow(dead_code)]
fn _ensure_into_response_imported() -> impl IntoResponse {
    "ok"
}
