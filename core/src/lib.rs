//! Kira core daemon — library entrypoint.
//!
//! `main.rs` does CLI parsing + tracing init and delegates to [`run`].
//! Everything else lives in submodules.

#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use std::net::SocketAddr;
use tracing::info;

pub mod llm;
pub mod proto;
pub mod safety;
pub mod server;

use server::AppState;

/// Boot and serve the HTTP+SSE daemon. Returns when the listener exits.
pub async fn run(bind: SocketAddr) -> Result<()> {
    let state = AppState::new()?;
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
