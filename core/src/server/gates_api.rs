//! `POST /v1/gates/run` — execute the 5-level completion gate cascade.
//!
//! When the cascade fails, fires a fire-and-forget lesson generator (Light tier
//! LLM) so the failure feeds the flywheel automatically. The caller's response
//! is not delayed.

use axum::{extract::State, Json};
use tracing::instrument;

use crate::gates::run_cascade;
use crate::lessons::generator;
use crate::proto::gate::{GateOutcome, RunGatesRequest, RunGatesResponse};
use crate::server::AppState;

#[instrument(skip(state, req))]
pub async fn run(
    State(state): State<AppState>,
    Json(req): Json<RunGatesRequest>,
) -> Json<RunGatesResponse> {
    let resp = run_cascade(&state.router, Some(&state.embed_cache), &req).await;

    // Flywheel: any failure produces a lesson, fire-and-forget.
    if resp.overall == GateOutcome::Fail {
        if let Some(failed) = resp.results.iter().find(|r| r.outcome == GateOutcome::Fail) {
            let reason = failed.reason.clone().unwrap_or_default();
            let context = format!(
                "Gate {:?} failed.\nReason: {}\nSpec: {}\nOutput (first 400 chars): {}",
                failed.gate,
                reason,
                req.spec.chars().take(400).collect::<String>(),
                req.output.chars().take(400).collect::<String>(),
            );
            generator::fire_and_forget(state.router.clone(), state.lessons.clone(), context, None);
        }
    }

    Json(resp)
}
