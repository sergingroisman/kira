//! HTTP+SSE server: app state, router, handlers.

pub mod agents_api;
pub mod chat;
pub mod embed_api;
pub mod error;
pub mod eval_api;
pub mod events_api;
pub mod gates_api;
pub mod health;
pub mod lessons_api;
pub mod mailbox_api;
pub mod maturation_api;
pub mod mcp_api;
pub mod memory_api;
pub mod ownership_api;
pub mod queue_api;
pub mod detective_api;
pub mod review_api;
pub mod sdd_api;
pub mod stats_api;
pub mod workflow_api;
pub mod text_api;
pub mod web_api;
pub mod tool;
pub mod worktree_api;

use anyhow::{Context, Result};
use axum::{
    Router as AxumRouter,
    routing::{delete, get, post},
};
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::agents::{AgentManager, mailbox::MailboxFs, ownership::OwnershipRegistry};
use crate::config::Tokens;
use crate::mcp::McpRegistry;
use crate::hooks::HookRunner;
use crate::lessons::LessonRegistry;
use crate::llm::embed_cache::EmbedCache;
use crate::llm::LlmProvider;
use crate::memory::consolidator::{self, ConsolidatorState};
use crate::observability::pillar::{self, PillarTx};
use crate::router::Router;
use crate::router::budget::{BudgetConfig, BudgetGuard};
use crate::router::tier::TierMap;
use crate::sdd::SddManager;
use crate::state::{KiraDb, resolve_state_path};
use crate::workflow::manager::WorkflowManager;
use crate::workflow::registry::{LocHeuristicSizer, ServiceRegistry};
use crate::worktree::WorktreeManager;

#[derive(Clone)]
pub struct AppState {
    pub router: Router,
    pub worktrees: WorktreeManager,
    pub mailbox: MailboxFs,
    pub ownership: OwnershipRegistry,
    pub agents: AgentManager,
    pub hooks: HookRunner,
    pub lessons: LessonRegistry,
    pub embed_cache: EmbedCache,
    pub tokens: Option<Tokens>,
    /// Broadcast channel for pillar transparency events (REPL breadcrumbs).
    pub pillar_tx: PillarTx,
    pub sdd: SddManager,
    pub workflow: WorkflowManager,
    pub mcp: McpRegistry,
    /// Cross-thread semantic memory store (None if backend unavailable).
    pub memory: Option<Arc<crate::memory::sqlite_backend::SqliteVecStore>>,
    /// Message queue (None if queue disabled or backend unavailable).
    pub queue: Option<Arc<crate::queue::InMemoryQueue>>,
    /// Consolidator shared state for maturation API (None if not initialized).
    pub consolidator_state: Option<Arc<ConsolidatorState>>,
    /// Durable state database (None if redb open failed).
    pub db: Option<KiraDb>,
}

impl AppState {
    pub async fn new(tokens: Option<Tokens>) -> Result<Self> {
        let mut providers = HashMap::new();

        if let Some(ref t) = tokens {
            for (name, entry) in &t.providers.entries {
                match LlmProvider::from_config(name, entry) {
                    Ok(Some(p)) => {
                        info!(%name, protocol = %p.protocol.as_str(), "provider configured");
                        providers.insert(name.clone(), Arc::new(p));
                    }
                    Ok(None) => warn!(%name, "provider not configured, skipping"),
                    Err(e) => warn!(%name, error = ?e, "provider build failed"),
                }
            }
        }

        let names: Vec<String> = providers.keys().cloned().collect();
        info!(?names, "router providers configured");

        let budget_cfg = tokens
            .as_ref()
            .map_or_else(BudgetConfig::default, |t| BudgetConfig {
                monthly_hard_stop_usd: t.budget.monthly_hard_stop_usd,
                warn_pct: t.budget.warn_pct,
            });
        let budget = Arc::new(BudgetGuard::new(budget_cfg));

        let tier_map = tokens.as_ref().map_or_else(
            || TierMap::from_config(&crate::config::TiersConfig {
                heavy: crate::config::TierEntry { candidates: vec!["deepseek:deepseek-v4-pro".into(), "dashscope:qwen-max".into()] },
                standard: crate::config::TierEntry { candidates: vec!["deepseek:deepseek-v4-pro".into(), "dashscope:qwen-plus".into()] },
                light: crate::config::TierEntry { candidates: vec!["deepseek:deepseek-v4-flash".into(), "dashscope:qwen-flash".into()] },
                long_context: crate::config::TierEntry { candidates: vec!["deepseek:deepseek-v4-pro".into(), "dashscope:qwen-max".into()] },
                judge: Some(crate::config::TierEntry { candidates: vec!["dashscope:qwen-max".into(), "dashscope:qwen-plus".into()] }),
                local: None,
            }),
            |t| TierMap::from_config(&t.tiers),
        );
        let router_base = Router::new(providers, budget, tier_map);

        // Durable state — opens or creates the redb file. Path comes from
        // [paths] config when present (B3) — falls back to ~/.local/share/kira.
        let db_path = resolve_state_path(tokens.as_ref().map(|t| &t.paths))?;
        let db = match KiraDb::open(&db_path) {
            Ok(d) => Some(d),
            Err(e) => {
                warn!(error = ?e, path = %db_path.display(), "redb open failed — running in-memory only");
                None
            }
        };
        // C2 — Attach DB to router so circuit breaker snapshots persist
        // across daemon restarts.
        let router = match db.clone() {
            Some(d) => router_base.with_db(d),
            None => router_base,
        };

        let cwd = std::env::current_dir().context("cwd unavailable")?;
        let max_wt = tokens.as_ref().map_or(2, |t| t.concurrency.max_worktrees);
        let max_agents = tokens.as_ref().map_or(4, |t| t.concurrency.max_agents);
        let worktrees = WorktreeManager::new(cwd.clone(), db.clone(), max_wt).await?;
        let repo_root = crate::worktree::git::toplevel(&cwd).await?;
        let mailbox = MailboxFs::new(repo_root.clone());
        let ownership = OwnershipRegistry::new();
        let hooks = HookRunner::load(&repo_root).unwrap_or_else(|e| {
            warn!(error = ?e, "hooks load failed — running with no hooks");
            HookRunner::empty()
        });
        let lessons = LessonRegistry::new(repo_root.clone(), db.clone());
        let embed_cache = EmbedCache::new(db.clone());
        let gate_cfg = tokens
            .as_ref()
            .map_or_else(crate::gates::GateConfig::default, |t| {
                crate::gates::GateConfig {
                    faithfulness_pass_threshold: t.thresholds.faithfulness_pass_below,
                    faithfulness_escalate_threshold: t.thresholds.faithfulness_escalate,
                    judge_pass_score: t.thresholds.judge_pass_score,
                    change_suff_min: t.thresholds.change_sufficiency_min,
                    change_suff_max: t.thresholds.change_sufficiency_max,
                    meta_review_coverage_min: t.thresholds.meta_review_coverage,
                }
            });

        // MCP client registry — connects to servers listed in .kira/mcp.toml.
        let mcp = McpRegistry::load_from_paths(&repo_root).await;
        let mcp_servers = mcp.connected_servers().await;
        if !mcp_servers.is_empty() {
            info!(?mcp_servers, "MCP servers connected");
        }

        let runs_dir = repo_root.join(".kira/runs");
        let pricing = tokens
            .as_ref().map_or_else(|| Arc::new(crate::config::PricingConfig::default()), |t| Arc::new(t.pricing));
        let models = tokens
            .as_ref().map_or_else(|| Arc::new(crate::config::ModelsConfig::default()), |t| Arc::new(t.models.clone()));
        let approval_timeout = tokens.as_ref().map_or(300, |t| t.time.approval_timeout_secs);
        let default_max_iters = tokens.as_ref().map_or(20, |t| t.concurrency.max_agent_iters);
        let default_agent_budget_usd = tokens.as_ref().map_or(1.00, |t| t.budget.agent_default_usd);
        // Pillar channel must exist before AgentManager so the agent loop
        // can emit per-iter P7 cost events to the SSE stream. Previously
        // created later (just before SddManager) which left agents unable
        // to publish — fixed 2026-05-26 as part of Sprint 2.1 instrumentation.
        let pillar_tx = pillar::channel();
        let agents = AgentManager::new(
            router.clone(),
            worktrees.clone(),
            mailbox.clone(),
            ownership.clone(),
            hooks.clone(),
            db.clone(),
            lessons.clone(),
            embed_cache.clone(),
            gate_cfg,
            pricing,
            models,
            mcp.clone(),
            runs_dir,
            max_agents,
            approval_timeout,
            default_max_iters,
            default_agent_budget_usd,
            pillar_tx.clone(),
        );

        // Spawn watchdog tasks (stale detection + worktree cleanup).
        crate::watchdogs::spawn_all(agents.clone(), worktrees.clone());

        let sdd = SddManager::new(
            agents.clone(),
            worktrees.clone(),
            router.clone(),
            embed_cache.clone(),
            pillar_tx.clone(),
            db.clone(),
        ).await;

        // WorkflowManager — methodology-agnostic front-end. Register built-in services.
        let mut wf_registry = ServiceRegistry::new();
        let loc_sizer = std::sync::Arc::new(LocHeuristicSizer::default());
        wf_registry.register_sizer("loc_heuristic", loc_sizer);
        let bundled_wf_dir = crate::workflow::loader::bundled_workflow_dir();
        let workflow = WorkflowManager::new(
            agents.clone(),
            worktrees.clone(),
            router.clone(),
            embed_cache.clone(),
            pillar_tx.clone(),
            db.clone(),
            wf_registry,
            bundled_wf_dir.as_deref(),
        ).await;

        // Initialize memory store.
        let memory = Self::init_memory(tokens.as_ref());

        // Spawn consolidation timer if memory store is available and maturation is enabled.
        let maturation_cfg = tokens
            .as_ref()
            .map(|t| t.memory.maturation.clone())
            .unwrap_or_default();
        let consolidator_state = if maturation_cfg.enabled {
            if let Some(ref store) = memory {
                let cfg = Arc::new(maturation_cfg);
                let cs = consolidator::spawn_consolidation_timer(
                    cfg,
                    Arc::clone(store),
                    pillar_tx.clone(),
                    Arc::new(router.clone()),
                )
                .await;
                info!("maturation consolidation timer active");
                Some(cs)
            } else {
                info!("maturation enabled but memory store unavailable — consolidation timer not spawned");
                None
            }
        } else {
            info!("maturation disabled — consolidation timer not spawned");
            None
        };

        Ok(Self {
            router,
            worktrees,
            mailbox,
            ownership,
            agents,
            hooks,
            lessons,
            embed_cache,
            tokens: tokens.clone(),
            pillar_tx: pillar_tx.clone(),
            sdd,
            workflow,
            mcp,
            memory,
            queue: Some(Arc::new(crate::queue::InMemoryQueue::new())),
            consolidator_state,
            db,
        })
    }

    /// Initialize the memory store from config. Returns None on failure (graceful degradation).
    fn init_memory(tokens: Option<&Tokens>) -> Option<Arc<crate::memory::sqlite_backend::SqliteVecStore>> {
        let memory_cfg = tokens.map(|t| &t.memory);
        let sqlite_path = memory_cfg
            .map(|m| m.sqlite_path.clone())
            .unwrap_or_else(|| "~/.local/share/kira/memory.db".to_string());
        let max_per_thread = memory_cfg
            .map(|m| m.max_fragments_per_thread)
            .unwrap_or(1000);

        let resolved = crate::config::resolve_home(&sqlite_path);
        match crate::memory::sqlite_backend::SqliteVecStore::open(&resolved, max_per_thread) {
            Ok(store) => {
                info!(path = %resolved.display(), "memory store initialized (sqlite-vec)");
                Some(Arc::new(store))
            }
            Err(e) => {
                warn!(error = ?e, path = %resolved.display(), "memory store init failed — running without semantic memory");
                None
            }
        }
    }
}

pub fn router(state: AppState) -> AxumRouter {
    AxumRouter::new()
        // chat
        .route("/v1/health", get(health::health))
        .route("/v1/chat", post(chat::chat))
        .route("/v1/chat/stream", post(chat::chat_stream))
        .route("/v1/chat/yolo", post(chat::chat_yolo))
        .route("/v1/embed", post(embed_api::embed))
        .route("/v1/gates/run", post(gates_api::run))
        .route("/v1/eval/run", post(eval_api::run))
        .route("/v1/review", post(review_api::run))
        .route("/v1/events/stream", get(events_api::stream))
        // SDD (legacy P1 Spec-Driven Execution — kept for backward compat)
        .route("/v1/sdd", get(sdd_api::list).post(sdd_api::start))
        .route("/v1/sdd/:id", get(sdd_api::get_one))
        .route("/v1/sdd/:id/questions", get(sdd_api::get_questions).post(sdd_api::answer_questions))
        .route("/v1/sdd/:id/waves", get(sdd_api::get_waves))
        // Workflow (methodology-agnostic, replaces SDD)
        .route("/v1/workflow", get(workflow_api::list).post(workflow_api::start))
        .route("/v1/workflow/list", get(workflow_api::list_workflows))
        .route("/v1/workflow/:id", get(workflow_api::get_one))
        .route("/v1/workflow/:id/questions", get(workflow_api::get_questions).post(workflow_api::answer_questions))
        .route("/v1/workflow/:id/plan", get(workflow_api::get_plan))
        .route("/v1/workflow/:id/approve_plan", post(workflow_api::approve_plan))
        // Detective (universal code review)
        .route("/v1/detective/run", post(detective_api::run))
        // Web tools (P0 + soberania)
        .route("/v1/web/search", post(web_api::search))
        .route("/v1/web/fetch", post(web_api::fetch))
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
        .route("/v1/agents/:id/history", get(agents_api::history))
        .route("/v1/agents/:id/cost", get(agents_api::cost))
        .route("/v1/agents/:id/context", get(agents_api::context_window))
        .route("/v1/agents/:id/diff", get(agents_api::diff))
        .route("/v1/agents/:id/cancel", post(agents_api::cancel))
        .route("/v1/agents/:id/approval", post(agents_api::approve))
        // plans
        // lessons
        .route(
            "/v1/lessons",
            get(lessons_api::list).post(lessons_api::create),
        )
        .route(
            "/v1/lessons/:id",
            get(lessons_api::get_one).delete(lessons_api::delete),
        )
        .route("/v1/lessons/:id/promote", post(lessons_api::promote))
        // MCP
        .route("/v1/mcp/servers", get(mcp_api::servers))
        // Text pipeline (C1 / M17-M20)
        .route("/v1/text/tokenize", post(text_api::tokenize))
        .route("/v1/text/chunk", post(text_api::chunk_endpoint))
        .route("/v1/text/compress", post(text_api::compress_endpoint))
        .route("/v1/text/rerank", post(text_api::rerank_endpoint))
        // stats
        .route("/v1/stats", get(stats_api::stats))
        // memory (cross-thread semantic)
        .route("/v1/memory/search", get(memory_api::search))
        .route("/v1/memory/:run_id/context", get(memory_api::get_context))
        .route("/v1/memory/:fragment_id", delete(memory_api::delete_fragment))
        // memory maturation
        .route("/v1/memory/maturation/status", get(maturation_api::status))
        .route("/v1/memory/maturation/consolidate", post(maturation_api::consolidate))
        .route("/v1/memory/handoff", get(maturation_api::get_handoff_handler).delete(maturation_api::delete_handoff_handler))
        .route("/v1/memory/:fragment_id/metadata", get(maturation_api::fragment_metadata))
        // queue (resilience buffer)
        .route("/v1/queue", get(queue_api::list))
        .route("/v1/queue/:queue_id", delete(queue_api::cancel))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
