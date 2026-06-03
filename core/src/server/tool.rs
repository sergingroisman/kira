//! Tool endpoints. v0: `bash`, `file_read`, `file_write`.

use anyhow::{Context, Result};
use axum::Json;
use std::time::Duration;
use tracing::instrument;

use crate::proto::bash::{BashRequest, BashResponse};
use crate::proto::file::{
    FileReadRequest, FileReadResponse, FileWriteRequest, FileWriteResponse,
};
use crate::safety::cmd_allowlist::{
    validate_argv, BASH_OUTPUT_MAX_BYTES, BASH_TIMEOUT_MAX_SECS,
};
use crate::safety::{path_guard, FILE_WRITE_MAX_BYTES_HARD};
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

// ============================================================================
// /v1/tool/file_read — SC4 path guard
// ============================================================================

#[instrument(skip(req), fields(path = %req.path, base_dir = %req.base_dir))]
pub async fn tool_file_read(
    Json(req): Json<FileReadRequest>,
) -> Result<Json<FileReadResponse>, AppError> {
    let resolved = path_guard::resolve_within(&req.path, &req.base_dir)?;

    let metadata = std::fs::metadata(&resolved)
        .with_context(|| format!("stat failed: {}", resolved.display()))?;
    if !metadata.is_file() {
        return Err(anyhow::anyhow!("target is not a regular file: {}", resolved.display()).into());
    }

    let bytes = std::fs::read(&resolved)
        .with_context(|| format!("read failed: {}", resolved.display()))?;
    let total_bytes = bytes.len();

    let mut content = String::from_utf8_lossy(&bytes).into_owned();
    let mut truncated = false;
    if content.len() > req.max_bytes {
        content.truncate(req.max_bytes);
        truncated = true;
    }

    Ok(Json(FileReadResponse {
        content,
        bytes_read: total_bytes,
        truncated,
        canonical_path: resolved.display().to_string(),
    }))
}

// ============================================================================
// /v1/tool/file_write — SC4 path guard + atomic .tmp+rename
// ============================================================================

#[instrument(skip(req), fields(path = %req.path, base_dir = %req.base_dir, bytes = req.content.len()))]
pub async fn tool_file_write(
    Json(req): Json<FileWriteRequest>,
) -> Result<Json<FileWriteResponse>, AppError> {
    let bytes = req.content.as_bytes();
    if bytes.len() > FILE_WRITE_MAX_BYTES_HARD {
        return Err(anyhow::anyhow!(
            "file too large: {} bytes > hard cap {}",
            bytes.len(),
            FILE_WRITE_MAX_BYTES_HARD
        )
        .into());
    }

    // path_guard FIRST — before any FS write — so create_dirs cannot escape.
    let resolved = path_guard::resolve_within(&req.path, &req.base_dir)?;

    // Optional parent mkdir, but ONLY for paths already verified inside base_dir.
    if req.create_dirs
        && let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create_dir_all failed: {}", parent.display()))?;
        }

    let exists = resolved.exists();
    if exists && !req.overwrite {
        return Err(anyhow::anyhow!(
            "file exists and overwrite=false: {}",
            resolved.display()
        )
        .into());
    }

    // Atomic: write to `.<filename>.kira-<uuid>.tmp` then rename.
    let parent = resolved
        .parent()
        .context("resolved path has no parent")?;
    let file_name = resolved
        .file_name()
        .context("resolved path has no file_name")?
        .to_string_lossy()
        .into_owned();
    let tmp = parent.join(format!(".{file_name}.kira-{}.tmp", uuid::Uuid::now_v7()));

    std::fs::write(&tmp, bytes)
        .with_context(|| format!("tmp write failed: {}", tmp.display()))?;
    std::fs::rename(&tmp, &resolved).with_context(|| {
        // Best-effort cleanup of tmp if rename fails.
        let _ = std::fs::remove_file(&tmp);
        format!("atomic rename failed: {} → {}", tmp.display(), resolved.display())
    })?;

    Ok(Json(FileWriteResponse {
        bytes_written: bytes.len(),
        canonical_path: resolved.display().to_string(),
        created: !exists,
    }))
}
