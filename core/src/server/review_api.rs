//! `/v1/review` — TRUST-inspired multi-dim PR review orchestration.
//!
//! The actual workflow runs on a detached `tokio::spawn` so REPL/HTTP
//! disconnects do NOT kill in-progress reviewers. The handler awaits a
//! `oneshot` channel; if the handler future is dropped, the background
//! task still completes and writes `REVIEW.md` + `.kira/runs/<id>/result.json`.

use axum::{extract::State, Json};
use std::path::PathBuf;
use tokio::sync::oneshot;
use tracing::{error, instrument, warn};

use crate::proto::review::{RunReviewRequest, RunReviewResponse};
use crate::server::{error::AppError, AppState};
use crate::workflows::{self, ReviewExecutor};

#[instrument(skip(state, req), fields(head = %req.head))]
pub async fn run(
    State(state): State<AppState>,
    Json(req): Json<RunReviewRequest>,
) -> Result<Json<RunReviewResponse>, AppError> {
    // Resolve repo root via the existing worktree manager.
    let cwd = std::env::current_dir().map_err(|e| anyhow::anyhow!("cwd: {e}"))?;
    let repo_root = crate::worktree::git::toplevel(&cwd)
        .await
        .map_err(|e| anyhow::anyhow!("toplevel: {e}"))?;

    // Resolve workflow path.
    let wf_path: PathBuf = match req.workflow.as_deref() {
        Some(p) if p.starts_with('/') => PathBuf::from(p),
        Some(p) => repo_root.join(p),
        None => workflows::default_workflow_path(&repo_root),
    };
    let workflow = workflows::load(&wf_path)?;

    let base = req.base.clone().unwrap_or_else(|| "main".to_string());
    let timeout = req.timeout_secs.unwrap_or(workflows::DEFAULT_TIMEOUT);
    let head = req.head.clone();

    let executor = ReviewExecutor {
        repo_root: repo_root.clone(),
        agents: state.agents.clone(),
        router: state.router.clone(),
        embed_cache: state.embed_cache.clone(),
    };

    // Detach the workflow from the HTTP handler. The spawned task survives
    // client disconnect; it always writes `result.json` to disk so the user
    // can recover the response even if `fetch` aborted.
    let (tx, rx) = oneshot::channel::<anyhow::Result<RunReviewResponse>>();
    tokio::spawn(async move {
        let result = workflows::execute_pr_review(&executor, &workflow, &head, &base, timeout)
            .await;
        // Persist result to disk regardless of whether the handler is still listening.
        if let Ok(resp) = &result {
            let run_dir = std::path::Path::new(&resp.review_md_path)
                .parent().map_or_else(|| repo_root.join(".kira/runs").join(&resp.run_id), std::path::Path::to_path_buf);
            let path = run_dir.join("result.json");
            match serde_json::to_string_pretty(resp) {
                Ok(json) => {
                    if let Err(e) = std::fs::write(&path, json) {
                        warn!(error = ?e, path = %path.display(), "result.json write failed");
                    }
                }
                Err(e) => warn!(error = ?e, "result.json serialize failed"),
            }
        } else if let Err(e) = &result {
            error!(error = ?e, "execute_pr_review failed");
        }
        // Best-effort notify the handler. If receiver is gone (client disconnected),
        // we silently drop — work is already persisted to disk.
        let _ = tx.send(result);
    });

    let resp = rx
        .await
        .map_err(|e| anyhow::anyhow!("workflow task dropped before completion: {e}"))?
        .map_err(AppError::from)?;
    Ok(Json(resp))
}
