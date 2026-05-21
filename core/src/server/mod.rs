//! HTTP+SSE server: app state, router, handlers.

pub mod agents_api;
pub mod chat;
pub mod embed_api;
pub mod error;
pub mod eval_api;
pub mod gates_api;
pub mod health;
pub mod lessons_api;
pub mod mailbox_api;
pub mod ownership_api;
pub mod plans_api;
pub mod review_api;
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
use crate::lessons::LessonRegistry;
use crate::llm::embed_cache::EmbedCache;
use crate::llm::{dashscope, deepseek, LlmProvider};
use crate::plans::PlanManager;
use crate::router::budget::{BudgetConfig, BudgetGuard};
use crate::router::Router;
use crate::state::{default_state_path, KiraDb};
use crate::worktree::WorktreeManager;

#[derive(Clone)]
pub struct AppState {
    pub router: Router,
    pub worktrees: WorktreeManager,
    pub mailbox: MailboxFs,
    pub ownership: OwnershipRegistry,
    pub agents: AgentManager,
    pub hooks: HookRunner,
    pub plans: PlanManager,
    pub lessons: LessonRegistry,
    pub embed_cache: EmbedCache,
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

        // Durable state — opens or creates the redb file.
        let db_path = default_state_path()?;
        let db = match KiraDb::open(&db_path) {
            Ok(d) => Some(d),
            Err(e) => {
                warn!(error = ?e, path = %db_path.display(), "redb open failed — running in-memory only");
                None
            }
        };

        let cwd = std::env::current_dir().context("cwd unavailable")?;
        let worktrees = WorktreeManager::new(cwd.clone(), db.clone()).await?;
        let repo_root = crate::worktree::git::toplevel(&cwd).await?;
        let mailbox = MailboxFs::new(repo_root.clone());
        let ownership = OwnershipRegistry::new();
        let hooks = HookRunner::load(&repo_root).unwrap_or_else(|e| {
            warn!(error = ?e, "hooks load failed — running with no hooks");
            HookRunner::empty()
        });
        let lessons = LessonRegistry::new(repo_root.clone(), db.clone());
        let embed_cache = EmbedCache::new(db.clone());
        let agents = AgentManager::new(
            router.clone(),
            worktrees.clone(),
            mailbox.clone(),
            ownership.clone(),
            hooks.clone(),
            db.clone(),
            lessons.clone(),
            embed_cache.clone(),
        );
        let plans = PlanManager::new(router.clone(), agents.clone(), db);

        Ok(Self {
            router,
            worktrees,
            mailbox,
            ownership,
            agents,
            hooks,
            plans,
            lessons,
            embed_cache,
        })
    }
}

pub fn router(state: AppState) -> AxumRouter {
    AxumRouter::new()
        // chat
        .route("/v1/health", get(health::health))
        .route("/v1/chat", post(chat::chat))
        .route("/v1/chat/stream", post(chat::chat_stream))
        .route("/v1/embed", post(embed_api::embed))
        .route("/v1/gates/run", post(gates_api::run))
        .route("/v1/eval/run", post(eval_api::run))
        .route("/v1/review", post(review_api::run))
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
        // plans
        .route("/v1/plan", get(plans_api::list))
        .route("/v1/plan/draft", post(plans_api::draft))
        .route("/v1/plan/:id", get(plans_api::get_one))
        .route("/v1/plan/:id/approve", post(plans_api::approve))
        .route("/v1/plan/:id/cancel", post(plans_api::cancel))
        // lessons
        .route("/v1/lessons", get(lessons_api::list).post(lessons_api::create))
        .route("/v1/lessons/:id", get(lessons_api::get_one).delete(lessons_api::delete))
        .route("/v1/lessons/:id/promote", post(lessons_api::promote))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
