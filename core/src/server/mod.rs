//! HTTP+SSE server: app state, router, handlers.

pub mod agents_api;
pub mod chat;
pub mod error;
pub mod health;
pub mod mailbox_api;
pub mod ownership_api;
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

use crate::agents::{mailbox::MailboxFs, ownership::OwnershipRegistry, AgentManager};
use crate::hooks::HookRunner;
use crate::llm::{dashscope, deepseek, LlmProvider};
use crate::router::budget::{BudgetConfig, BudgetGuard};
use crate::router::Router;
use crate::worktree::WorktreeManager;

#[derive(Clone)]
pub struct AppState {
    pub router: Router,
    pub worktrees: WorktreeManager,
    pub mailbox: MailboxFs,
    pub ownership: OwnershipRegistry,
    pub agents: AgentManager,
    pub hooks: HookRunner,
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

        let cwd = std::env::current_dir().context("cwd unavailable")?;
        let worktrees = WorktreeManager::new(cwd.clone()).await?;
        let repo_root = crate::worktree::git::toplevel(&cwd).await?;
        let mailbox = MailboxFs::new(repo_root.clone());
        let ownership = OwnershipRegistry::new();
        let hooks = HookRunner::load(&repo_root).unwrap_or_else(|e| {
            warn!(error = ?e, "hooks load failed — running with no hooks");
            HookRunner::empty()
        });
        let agents = AgentManager::new(
            router.clone(),
            worktrees.clone(),
            mailbox.clone(),
            ownership.clone(),
            hooks.clone(),
        );

        Ok(Self {
            router,
            worktrees,
            mailbox,
            ownership,
            agents,
            hooks,
        })
    }
}

pub fn router(state: AppState) -> AxumRouter {
    AxumRouter::new()
        // chat
        .route("/v1/health", get(health::health))
        .route("/v1/chat", post(chat::chat))
        .route("/v1/chat/stream", post(chat::chat_stream))
        // tools
        .route("/v1/tool/bash", post(tool::tool_bash))
        .route("/v1/tool/file_read", post(tool::tool_file_read))
        .route("/v1/tool/file_write", post(tool::tool_file_write))
        // worktrees
        .route("/v1/worktree", get(worktree_api::list))
        .route("/v1/worktree/spawn", post(worktree_api::spawn))
        .route("/v1/worktree/:id", get(worktree_api::get_one))
        .route("/v1/worktree/:id", delete(worktree_api::destroy))
        // mailbox
        .route("/v1/mailbox/:agent_id/send", post(mailbox_api::send))
        .route("/v1/mailbox/:agent_id/poll", get(mailbox_api::poll))
        .route("/v1/mailbox/:agent_id/ack/:id", post(mailbox_api::ack))
        // ownership
        .route("/v1/ownership", get(ownership_api::list))
        .route("/v1/ownership/claim", post(ownership_api::claim))
        .route("/v1/ownership/check", get(ownership_api::check))
        .route("/v1/ownership/:agent_id", delete(ownership_api::release))
        // agents
        .route("/v1/agents", get(agents_api::list))
        .route("/v1/agents/spawn", post(agents_api::spawn))
        .route("/v1/agents/:id", get(agents_api::get_one))
        .route("/v1/agents/:id/cancel", post(agents_api::cancel))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
