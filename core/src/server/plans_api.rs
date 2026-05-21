//! `/v1/plan/*` handlers.

use anyhow::Context;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tracing::instrument;
use uuid::Uuid;

use crate::proto::plan::{ApproveResponse, CancelPlanResponse, DraftRequest, Plan};
use crate::server::{error::AppError, AppState};

#[instrument(skip(state, req), fields(kind = ?req.agent_kind))]
pub async fn draft(
    State(state): State<AppState>,
    Json(req): Json<DraftRequest>,
) -> Result<Json<Plan>, AppError> {
    let plan = state
        .plans
        .draft(req.goal, req.agent_kind, req.ttl_secs)
        .await?;
    Ok(Json(plan))
}

#[instrument(skip(state), fields(id = %id))]
pub async fn approve(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApproveResponse>, AppError> {
    let uuid = Uuid::parse_str(&id).context("invalid uuid")?;
    let (_, executor_agent_id) = state.plans.approve(uuid).await?;
    Ok(Json(ApproveResponse {
        plan_id: id,
        executor_agent_id,
    }))
}

#[instrument(skip(state), fields(id = %id))]
pub async fn cancel(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CancelPlanResponse>, AppError> {
    let uuid = Uuid::parse_str(&id).context("invalid uuid")?;
    let cancelled = state.plans.cancel(uuid);
    Ok(Json(CancelPlanResponse { cancelled, id }))
}

pub async fn list(State(state): State<AppState>) -> Json<Vec<Plan>> {
    Json(state.plans.list())
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Plan>, (StatusCode, String)> {
    let uuid = Uuid::parse_str(&id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    match state.plans.get(uuid) {
        Some(p) => Ok(Json(p)),
        None => Err((StatusCode::NOT_FOUND, format!("plan not found: {id}"))),
    }
}
