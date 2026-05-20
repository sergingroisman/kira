//! HTTP+SSE server: app state, router, handlers.

pub mod chat;
pub mod error;
pub mod health;
pub mod tool;
pub mod worktree_api;

use anyhow::{Context, Result};
use axum::{
    routing::{delete, get, post},
    Router as AxumRouter,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::llm::{dashscope, deepseek, LlmProvider};
use crate::router::budget::{BudgetConfig, BudgetGuard};
use crate::router::Router;
use crate::worktree::WorktreeManager;

/// Shared application state passed to handlers via `axum::extract::State`.
#[derive(Clone)]
pub struct AppState {
    pub router: Router,
    pub worktrees: WorktreeManager,
}

impl AppState {
    pub async fn new() -> Result<Self> {
        let mut providers: Vec<LlmProvider> = Vec::new();

        providers.push(dashscope::build().context("dashscope build (v0 mandatory)")?);

        match deepseek::build() {
            Ok(Some(p)) => providers.push(p),
            Ok(None) => warn!("DEEPSEEK_API_KEY not set — router will fallback to dashscope only"),
            Err(e) => warn!(error = ?e, "deepseek build failed; continuing without it"),
        }

        let names: Vec<String> = providers.iter().map(|p| p.name.clone()).collect();
        info!(providers = ?names, "router providers configured");

        let budget = Arc::new(BudgetGuard::new(BudgetConfig::default()));
        let router = Router::new(providers, budget);

        // Anchor worktree manager at the current working directory's git toplevel.
        let cwd = std::env::current_dir().context("cwd unavailable")?;
        let worktrees = WorktreeManager::new(cwd).await?;

        Ok(Self { router, worktrees })
    }
}

/// Build the axum router with all `/v1/*` routes attached.
pub fn router(state: AppState) -> AxumRouter {
    AxumRouter::new()
        .route("/v1/health", get(health::health))
        .route("/v1/chat", post(chat::chat))
        .route("/v1/chat/stream", post(chat::chat_stream))
        .route("/v1/tool/bash", post(tool::tool_bash))
        .route("/v1/tool/file_read", post(tool::tool_file_read))
        .route("/v1/tool/file_write", post(tool::tool_file_write))
        .route("/v1/worktree", get(worktree_api::list))
        .route("/v1/worktree/spawn", post(worktree_api::spawn))
        .route("/v1/worktree/:id", get(worktree_api::get_one))
        .route("/v1/worktree/:id", delete(worktree_api::destroy))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
