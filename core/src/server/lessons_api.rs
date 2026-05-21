//! `/v1/lessons/*` handlers.

use anyhow::Context;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tracing::instrument;
use uuid::Uuid;

use crate::proto::lesson::{
    CreateLessonRequest, DeleteLessonResponse, Lesson, PromoteResponse,
};
use crate::server::{error::AppError, AppState};

pub async fn list(State(state): State<AppState>) -> Json<Vec<Lesson>> {
    Json(state.lessons.list())
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Lesson>, (StatusCode, String)> {
    let uuid = Uuid::parse_str(&id).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    match state.lessons.get(uuid) {
        Some(l) => Ok(Json(l)),
        None => Err((StatusCode::NOT_FOUND, format!("lesson not found: {id}"))),
    }
}

#[instrument(skip(state, req), fields(priority = ?req.priority))]
pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateLessonRequest>,
) -> Json<Lesson> {
    Json(state.lessons.record(
        req.trigger,
        req.anti_pattern,
        req.preferred_pattern,
        req.priority,
        req.scope,
    ))
}

#[instrument(skip(state), fields(id = %id))]
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteLessonResponse>, AppError> {
    let uuid = Uuid::parse_str(&id).context("invalid uuid")?;
    let deleted = state.lessons.delete(uuid);
    Ok(Json(DeleteLessonResponse { deleted, id }))
}

#[instrument(skip(state), fields(id = %id))]
pub async fn promote(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PromoteResponse>, AppError> {
    let uuid = Uuid::parse_str(&id).context("invalid uuid")?;
    let written = state.lessons.promote(uuid)?;
    Ok(Json(PromoteResponse {
        promoted: written.is_some(),
        id,
        written_to: written.map(|p| p.display().to_string()),
    }))
}
