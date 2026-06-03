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
    let cfg = state.tokens.as_ref().map_or_else(
        crate::gates::GateConfig::default,
        |t| crate::gates::GateConfig {
            faithfulness_pass_threshold: t.thresholds.faithfulness_pass_below,
            faithfulness_escalate_threshold: t.thresholds.faithfulness_escalate,
            judge_pass_score: t.thresholds.judge_pass_score,
            change_suff_min: t.thresholds.change_sufficiency_min,
            change_suff_max: t.thresholds.change_sufficiency_max,
            meta_review_coverage_min: t.thresholds.meta_review_coverage,
        },
    );
    let resp = run_cascade(&state.router, Some(&state.embed_cache), &req, cfg).await;

    // Flywheel: any failure produces a lesson, fire-and-forget.
    if resp.overall == GateOutcome::Fail
        && let Some(failed) = resp.results.iter().find(|r| r.outcome == GateOutcome::Fail) {
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

    Json(resp)
}
