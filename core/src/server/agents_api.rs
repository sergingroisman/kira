//! `/v1/agents/*` handlers.

use std::path::PathBuf;

use anyhow::Context;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tracing::instrument;
use uuid::Uuid;

use crate::llm::types::ChatMessageUnified;
use crate::proto::agent::{
    Agent, AgentCostSnapshot, ApprovalRequest, ApprovalResponse, CancelResponse, SpawnAgentRequest,
};
use crate::server::{error::AppError, AppState};
use crate::worktree::git::diff_head;

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

pub async fn history(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<ChatMessageUnified>>, (StatusCode, String)> {
    let uuid = Uuid::parse_str(&id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    match state.agents.history(uuid) {
        Some(messages) => Ok(Json(messages)),
        None => Err((StatusCode::NOT_FOUND, format!("history not found for agent: {id}"))),
    }
}

pub async fn cost(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AgentCostSnapshot>, (StatusCode, String)> {
    let uuid = Uuid::parse_str(&id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    match state.agents.cost(uuid) {
        Some(snapshot) => Ok(Json(snapshot)),
        None => Err((StatusCode::NOT_FOUND, format!("cost snapshot not found for agent: {id}"))),
    }
}

/// `GET /v1/agents/{id}/context` — per-LLM context-window snapshot.
/// Mirrors the Claude Code "Janela de contexto" UI. Categories beyond
/// `messages` are placeholders for now; tracker hooks land in subsequent
/// passes (see `proto::context_window` doc). Returns 404 when the agent
/// hasn't run any iteration yet (no model committed).
pub async fn context_window(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::proto::context_window::ContextWindowSnapshot>, (StatusCode, String)> {
    let uuid = Uuid::parse_str(&id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    match state.agents.context_window(uuid) {
        Some(snap) => Ok(Json(snap)),
        None => Err((StatusCode::NOT_FOUND, format!("context window unavailable for agent: {id}"))),
    }
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

#[instrument(skip(state, req), fields(id = %id))]
pub async fn approve(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ApprovalRequest>,
) -> Result<Json<ApprovalResponse>, AppError> {
    let uuid = Uuid::parse_str(&id).context("invalid uuid")?;
    let resolved = state.agents.approve(uuid, req.approved);
    Ok(Json(ApprovalResponse {
        resolved,
        id,
        approved: req.approved,
    }))
}

#[instrument(skip(state), fields(id = %id))]
pub async fn diff(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<String, (StatusCode, String)> {
    let uuid = Uuid::parse_str(&id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let agent = state
        .agents
        .get(uuid)
        .ok_or((StatusCode::NOT_FOUND, format!("agent not found: {id}")))?;
    let wt_id =
        Uuid::parse_str(&agent.worktree_id).map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, "invalid worktree id".into())
        })?;
    let wt = state
        .worktrees
        .get(wt_id)
        .ok_or((StatusCode::NOT_FOUND, "worktree not found".into()))?;
    let diff = diff_head(&PathBuf::from(&wt.path))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(diff)
}
