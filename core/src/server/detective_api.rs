//! `/v1/detective` — universal code review endpoint.

use axum::{extract::State, Json};
use tracing::instrument;

use crate::agents::DetectiveRunner;
use crate::proto::detective::{RunDetectiveRequest, RunDetectiveResponse};
use crate::server::{error::AppError, AppState};

/// POST /v1/detective/run — run a full multi-lens code review.
#[instrument(skip(state, req), fields(lenses = req.lenses.len()))]
pub async fn run(
    State(state): State<AppState>,
    Json(req): Json<RunDetectiveRequest>,
) -> Result<Json<RunDetectiveResponse>, AppError> {
    let runner = DetectiveRunner::new(state.agents.clone());
    let resp = runner.run(req).await?;
    Ok(Json(resp))
}
