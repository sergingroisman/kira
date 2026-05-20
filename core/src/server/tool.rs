//! Tool endpoints. v0: `/v1/tool/bash`.
//! Day 7+: `/v1/tool/file_read`, `/v1/tool/file_write` (SC4 path guard).

use anyhow::Context;
use axum::Json;
use std::time::Duration;
use tracing::instrument;

use crate::proto::bash::{BashRequest, BashResponse};
use crate::safety::cmd_allowlist::{
    validate_argv, BASH_OUTPUT_MAX_BYTES, BASH_TIMEOUT_MAX_SECS,
};
use crate::server::error::AppError;

#[instrument(skip(req), fields(exe, args_count = req.argv.len().saturating_sub(1)))]
pub async fn tool_bash(Json(req): Json<BashRequest>) -> Result<Json<BashResponse>, AppError> {
    let timeout = req.timeout_secs.min(BASH_TIMEOUT_MAX_SECS);

    let (exe, args) = validate_argv(&req.argv)?;
    tracing::Span::current().record("exe", exe);

    let mut cmd = tokio::process::Command::new(exe);
    cmd.args(&args);

    // SC5 — minimal env whitelist (no inheritance of secrets).
    cmd.env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", std::env::var("HOME").unwrap_or_default())
        .env("USER", std::env::var("USER").unwrap_or_default())
        .env("LANG", std::env::var("LANG").unwrap_or_else(|_| "C.UTF-8".into()));

    if let Some(cwd) = req.cwd.as_ref() {
        let canonical = std::fs::canonicalize(cwd)
            .with_context(|| format!("cwd canonicalize failed: {cwd}"))?;
        cmd.current_dir(canonical);
    }
    cmd.kill_on_drop(true);

    let start = std::time::Instant::now();
    let output = tokio::time::timeout(Duration::from_secs(timeout), cmd.output())
        .await
        .with_context(|| format!("bash tool timed out after {timeout}s"))?
        .context("spawn failed")?;
    let elapsed = start.elapsed();

    let mut stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let mut truncated = false;
    if stdout.len() > BASH_OUTPUT_MAX_BYTES {
        stdout.truncate(BASH_OUTPUT_MAX_BYTES);
        truncated = true;
    }
    if stderr.len() > BASH_OUTPUT_MAX_BYTES {
        stderr.truncate(BASH_OUTPUT_MAX_BYTES);
        truncated = true;
    }

    Ok(Json(BashResponse {
        stdout,
        stderr,
        exit_code: output.status.code(),
        duration_ms: elapsed.as_millis(),
        truncated,
    }))
}
