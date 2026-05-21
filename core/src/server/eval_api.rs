//! `POST /v1/eval/run` — run a comparison suite of N tasks × M models.

use axum::{extract::State, Json};
use tracing::instrument;

use crate::eval::run_eval;
use crate::proto::eval::{RunEvalRequest, RunEvalResponse};
use crate::server::AppState;

#[instrument(skip(state, req), fields(n_tasks = req.tasks.len(), n_models = req.models.len()))]
pub async fn run(
    State(state): State<AppState>,
    Json(req): Json<RunEvalRequest>,
) -> Json<RunEvalResponse> {
    let resp = run_eval(&state.router, &state.embed_cache, &req).await;
    Json(resp)
}
