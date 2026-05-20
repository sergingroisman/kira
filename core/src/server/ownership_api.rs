//! `/v1/ownership/*` handlers.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use tracing::instrument;

use crate::proto::ownership::{
    Claim, ClaimRequest, ReleaseResponse, WriteDecision,
};
use crate::server::{error::AppError, AppState};

#[instrument(skip(state, req), fields(agent_id = %req.agent_id))]
pub async fn claim(
    State(state): State<AppState>,
    Json(req): Json<ClaimRequest>,
) -> Result<Json<Claim>, AppError> {
    let c = Claim::now(req.agent_id, req.patterns, req.mode);
    state.ownership.claim(c.clone())?;
    Ok(Json(c))
}

#[instrument(skip(state), fields(agent_id = %agent_id))]
pub async fn release(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Json<ReleaseResponse> {
    let released = state.ownership.release(&agent_id);
    Json(ReleaseResponse {
        released,
        agent_id,
    })
}

pub async fn list(State(state): State<AppState>) -> Json<Vec<Claim>> {
    Json(state.ownership.list())
}

#[derive(Deserialize, Debug)]
pub struct CheckParams {
    pub agent_id: String,
    pub path: String,
}

#[instrument(skip(state), fields(agent_id = %params.agent_id, path = %params.path))]
pub async fn check(
    State(state): State<AppState>,
    Query(params): Query<CheckParams>,
) -> Json<WriteDecision> {
    Json(state.ownership.check_write(&params.agent_id, &params.path))
}
