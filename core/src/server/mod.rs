//! HTTP+SSE server: app state, router, handlers.

pub mod chat;
pub mod error;
pub mod health;
pub mod tool;

use anyhow::Result;
use async_openai::{config::OpenAIConfig, Client};
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

/// Shared application state passed to handlers via `axum::extract::State`.
#[derive(Clone)]
pub struct AppState {
    pub dashscope: Arc<Client<OpenAIConfig>>,
}

impl AppState {
    pub fn new() -> Result<Self> {
        Ok(Self {
            dashscope: Arc::new(crate::llm::dashscope::build_client()?),
        })
    }
}

/// Build the axum router with all `/v1/*` routes attached.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health::health))
        .route("/v1/chat", post(chat::chat))
        .route("/v1/chat/stream", post(chat::chat_stream))
        .route("/v1/tool/bash", post(tool::tool_bash))
        .route("/v1/tool/file_read", post(tool::tool_file_read))
        .route("/v1/tool/file_write", post(tool::tool_file_write))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
