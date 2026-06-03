//! `/v1/sdd*` — Spec-Driven Execution endpoints (P1).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tracing::instrument;

use crate::proto::sdd::{AnswerQuestionsRequest, PlanQuestion, SddListResponse, SddRun, StartSddRequest};
use crate::server::{error::AppError, AppState};

#[instrument(skip(state, req), fields(goal_len = req.goal.len()))]
pub async fn start(
    State(state): State<AppState>,
    Json(req): Json<StartSddRequest>,
) -> Result<Json<SddRun>, AppError> {
    let run = state.sdd.start(req).await?;
    Ok(Json(run))
}

pub async fn list(State(state): State<AppState>) -> Json<SddListResponse> {
    Json(SddListResponse {
        runs: state.sdd.list().await,
    })
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SddRun>, (StatusCode, String)> {
    match state.sdd.get(&id).await {
        Some(r) => Ok(Json(r)),
        None => Err((StatusCode::NOT_FOUND, format!("sdd run not found: {id}"))),
    }
}

pub async fn get_questions(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Vec<PlanQuestion>> {
    Json(state.sdd.get_questions(&id).await)
}

#[instrument(skip(state, req), fields(id = %id))]
pub async fn answer_questions(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AnswerQuestionsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let found = state.sdd.answer_questions(&id, req).await;
    if found {
        Ok(Json(serde_json::json!({ "ok": true })))
    } else {
        Err(AppError::from(anyhow::anyhow!("sdd run not found: {id}")))
    }
}

/// GET /v1/sdd/:id/waves — returns the wave plan for an SDD run.
///
/// If the run hasn't reached the EXECUTE phase yet, returns an empty plan.
/// If the tasks.json doesn't contain dependency info, returns a single wave
/// with all tasks.
pub async fn get_waves(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let run = match state.sdd.get(&id).await {
        Some(r) => r,
        None => return Err((StatusCode::NOT_FOUND, format!("sdd run not found: {id}"))),
    };

    // If no tasks_path yet, return empty plan
    let Some(tasks_path) = run.tasks_path else {
        return Ok(Json(serde_json::json!({"waves": []})));
    };

    // Read and parse tasks.json
    let raw = match std::fs::read_to_string(&tasks_path) {
        Ok(s) => s,
        Err(_) => return Ok(Json(serde_json::json!({"waves": []}))),
    };
    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return Ok(Json(serde_json::json!({"waves": []}))),
    };
    let Some(tasks) = parsed.get("tasks").and_then(|v| v.as_array()) else {
        return Ok(Json(serde_json::json!({"waves": []})));
    };

    // Build TaskDefs
    use crate::sdd::waves::{self, TaskDef};
    use uuid::Uuid;

    let task_defs: Vec<TaskDef> = tasks
        .iter()
        .map(|t| {
            let id_str = t.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let task_id = Uuid::parse_str(id_str).unwrap_or_else(|_| Uuid::now_v7());
            let description = t.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let dependencies: Vec<Uuid> = t
                .get("dependencies")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|d| d.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                        .collect()
                })
                .unwrap_or_default();

            TaskDef { task_id, description, dependencies }
        })
        .collect();

    // Plan waves
    match waves::plan(&task_defs) {
        Ok(plan) => Ok(Json(serde_json::json!(plan))),
        Err(e) => Err((StatusCode::UNPROCESSABLE_ENTITY, format!("wave planning failed: {e}"))),
    }
}
