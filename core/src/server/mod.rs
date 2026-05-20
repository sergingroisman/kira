//! HTTP+SSE server: app state, router, handlers.

pub mod chat;
pub mod error;
pub mod health;
pub mod tool;

use anyhow::{Context, Result};
use axum::{
    routing::{get, post},
    Router as AxumRouter,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::llm::{dashscope, deepseek, LlmProvider};
use crate::router::budget::{BudgetConfig, BudgetGuard};
use crate::router::Router;

/// Shared application state passed to handlers via `axum::extract::State`.
#[derive(Clone)]
pub struct AppState {
    pub router: Router,
}

impl AppState {
    pub fn new() -> Result<Self> {
        let mut providers: Vec<LlmProvider> = Vec::new();

        // DashScope is mandatory in v0 (spike provider). Fail if missing.
        providers.push(dashscope::build().context("dashscope build (v0 mandatory)")?);

        // DeepSeek is optional in v0 — warn and continue if key missing.
        match deepseek::build() {
            Ok(Some(p)) => providers.push(p),
            Ok(None) => warn!("DEEPSEEK_API_KEY not set — router will fallback to dashscope only"),
            Err(e) => warn!(error = ?e, "deepseek build failed; continuing without it"),
        }

        let names: Vec<String> = providers.iter().map(|p| p.name.clone()).collect();
        info!(providers = ?names, "router providers configured");

        let budget = Arc::new(BudgetGuard::new(BudgetConfig::default()));
        let router = Router::new(providers, budget);

        Ok(Self { router })
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
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
