//! `/v1/workflow*` — methodology-agnostic Workflow endpoints.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tracing::instrument;

use crate::proto::run::{StartWorkflowRequest, WorkflowListResponse, WorkflowRun};
use crate::proto::sdd::{AnswerQuestionsRequest, ApprovePlanRequest, PlanQuestion};
use crate::sdd::plan_view::PlanView;
use crate::server::{error::AppError, AppState};

/// POST /v1/workflow — start a new workflow run.
#[instrument(skip(state, req), fields(goal_len = req.goal.len(), workflow = %req.workflow))]
pub async fn start(
    State(state): State<AppState>,
    Json(req): Json<StartWorkflowRequest>,
) -> Result<Json<WorkflowRun>, AppError> {
    let run = state.workflow.start(req).await?;
    Ok(Json(run))
}

/// GET /v1/workflow — list all workflow runs.
pub async fn list(State(state): State<AppState>) -> Json<WorkflowListResponse> {
    Json(WorkflowListResponse { runs: state.workflow.list().await })
}

/// GET /v1/workflow/:id — get a single run.
pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<WorkflowRun>, (StatusCode, String)> {
    match state.workflow.get(&id).await {
        Some(r) => Ok(Json(r)),
        None => Err((StatusCode::NOT_FOUND, format!("workflow run not found: {id}"))),
    }
}

/// GET /v1/workflow/:id/questions — pending clarification questions.
pub async fn get_questions(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Vec<PlanQuestion>> {
    Json(state.workflow.get_questions(&id).await)
}

/// POST /v1/workflow/:id/questions — answer clarification questions.
#[instrument(skip(state, req), fields(id = %id))]
pub async fn answer_questions(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AnswerQuestionsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if state.workflow.answer_questions(&id, req).await {
        Ok(Json(serde_json::json!({ "ok": true })))
    } else {
        Err(AppError::from(anyhow::anyhow!("workflow run not found: {id}")))
    }
}

/// GET /v1/workflow/:id/plan — the plan-approval matrix (tasks + waves + files).
/// The CLI fetches this when the run reports `awaiting_plan_approval`.
pub async fn get_plan(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PlanView>, (StatusCode, String)> {
    match state.workflow.get_plan(&id).await {
        Some(view) => Ok(Json(view)),
        None => Err((StatusCode::NOT_FOUND, format!("workflow run not found: {id}"))),
    }
}

/// POST /v1/workflow/:id/approve_plan — resolve the plan-approval gate.
/// Body: `{ "approved": bool }`. `false` halts the run.
#[instrument(skip(state, req), fields(id = %id))]
pub async fn approve_plan(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ApprovePlanRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if state.workflow.approve_plan(&id, req.approved).await {
        Ok(Json(serde_json::json!({ "ok": true, "approved": req.approved })))
    } else {
        Err(AppError::from(anyhow::anyhow!(
            "workflow run not found or not awaiting approval: {id}"
        )))
    }
}

/// GET /v1/workflow/list — available workflow ids (sdd, gsd2, …).
pub async fn list_workflows(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "workflows": state.workflow.list_workflow_ids() }))
}
