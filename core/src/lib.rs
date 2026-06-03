//! Kira core daemon — library entrypoint.
//!
//! `main.rs` does CLI parsing + tracing init and delegates to [`run`].
//! Everything else lives in submodules.

// unsafe_code = "deny" is set in Cargo.toml [lints.rust]
// sqlite_backend.rs has a scoped #![allow(unsafe_code)] for FFI extension loading.

use anyhow::{Context, Result};
use std::net::SocketAddr;
use tracing::info;

pub mod agents;
pub mod broker;
pub mod config;
pub mod eval;
pub mod gates;
pub mod hooks;
pub mod lessons;
pub mod llm;
pub mod mcp;
pub mod memory;
pub mod observability;
pub mod orchestrator;
pub mod pilot;
pub mod proto;
pub mod queue;
pub mod router;
pub mod safety;
pub mod sandbox;
pub mod sdd;
pub mod server;
pub mod state;
pub mod text;
pub mod watchdogs;
pub mod web;
pub mod workflow;
pub mod workflows;
pub mod worktree;

use config::Tokens;
use server::AppState;

/// Load tokens.toml — tries project root, then CWD, then falls back to defaults.
fn load_tokens_config() -> Option<Tokens> {
    let candidates = [
        // Project root (dev: running from repo root)
        std::path::PathBuf::from("tokens.toml"),
        // Parent of core/ (dev: running from core/)
        std::path::PathBuf::from("../tokens.toml"),
        // Standard location
        {
            let mut p = std::env::current_dir().ok()?;
            p.push("tokens.toml");
            p
        },
    ];
    for path in &candidates {
        if path.exists() {
            match config::load_tokens(path) {
                Ok(t) => {
                    info!(path = %path.display(), "tokens.toml loaded");
                    return Some(t);
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "tokens.toml load failed");
                }
            }
        }
    }
    tracing::warn!("tokens.toml not found — using all hardcoded defaults");
    None
}

/// Boot and serve the HTTP+SSE daemon. Returns when the listener exits.
pub async fn run(bind: SocketAddr) -> Result<()> {
    let tokens = load_tokens_config();
    let state = AppState::new(tokens.clone()).await?;

    // Spawn SIGHUP handler for maturation config hot-reload (Unix only).
    let maturation_cfg = tokens
        .as_ref()
        .map(|t| t.memory.maturation.clone())
        .unwrap_or_default();
    let shared_maturation = config::reload::shared_config(maturation_cfg);
    config::reload::spawn_sighup_handler(shared_maturation);

    // Background watchdogs.
    watchdogs::spawn_all(state.agents.clone(), state.worktrees.clone());

    let app = server::router(state);

    info!(addr = %bind, "kira-cored starting");

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("failed to bind {bind}"))?;

    axum::serve(listener, app)
        .await
        .context("axum::serve exited unexpectedly")?;

    Ok(())
}
