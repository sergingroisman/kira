//! Agent spawning + iterative `agent_loop` (Sem 10).
//!
//! Loop contract:
//!   1. Initial messages = [`system_prompt`, `user_task`].
//!   2. Each iteration:
//!      - `Router.route_chat_with_tools(messages`, tools)
//!      - If response has `tool_calls`: append assistant message (with
//!        `tool_calls`) + execute each tool + append `tool` result messages.
//!      - If response is content-only (no `tool_calls)`: treat as final.
//!      - If "done" tool called: extract `final_result`, break.
//!   3. Stop on: done | content-only response | `max_iters` | budget cap | cancel.

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{oneshot, OwnedSemaphorePermit, Semaphore};
use tokio::time::{timeout, Duration as TokioDuration};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use crate::agents::inject::{default_registry, InjectContext, InjectionRegistry};
use crate::agents::mailbox::MailboxFs;
use crate::agents::ownership::OwnershipRegistry;
use crate::agents::store::Store as AgentStore;
use crate::agents::tools::{agent_tools, dispatch_with_hooks};
use crate::agents::transcript::{restore_context, TranscriptWriter};
use crate::gates::run_cascade;
use crate::hooks::HookRunner;
use crate::lessons::{generator, render_inject_snippet, LessonRegistry};
use crate::llm::embed_cache::EmbedCache;
use crate::llm::types::{ChatMessageUnified, ToolCall};
use crate::text::tokenizer::{ApproxTokenizer, Tokenizer};
use crate::broker::EventBus;
use crate::proto::gate::{GateOutcome, RunGatesRequest};
use crate::proto::agent::{
    Agent, AgentCostIteration, AgentCostSnapshot, AgentEvent, AgentKind, AgentStatus,
    PendingApproval, SpawnAgentRequest,
};
use crate::proto::mailbox::SendRequest;
use crate::proto::ownership::{Claim, OwnershipMode};
use crate::proto::worktree::SpawnRequest as WorktreeSpawnRequest;
use crate::mcp::McpRegistry;
use crate::router::Router;
use crate::safety::approval::{ApprovalGate, ApprovalVerdict};
use crate::safety::delimiters::{wrap_system, wrap_tool_output, wrap_user_data, UNTRUSTED_DATA_NOTICE};
use crate::safety::input::{InputCheckResult, InputGuardrailConfig};
use crate::safety::pipeline;
use crate::safety::tool::{ToolCheckResult, ToolGuardrailConfig};
use crate::memory::MemoryStore;
use crate::memory::tier::MemoryTier;
use crate::config::{ModelsConfig, PricingConfig};
use crate::safety::permission::{PermissionRuleset, RuleAction};
use crate::safety::risk::{bash_argv_is_critical, bash_argv_risk, builtin_tool_risk, ToolRisk};
use crate::worktree::WorktreeManager;

const DEFAULT_BUDGET_USD: f64 = 1.00;

/// Stall detection (G6) — 3 signals:
///
/// 1. **`SameToolLoop`**: same `(tool, args)` repeats this many times in a row.
/// 2. **`OutputUnchanged`**: hash of `(assistant_content + sorted tool sigs)`
///    unchanged across this many consecutive iterations. Catches "agent rewrites
///    same plan over and over without producing new evidence" pattern.
/// 3. **`NoProgress`**: wall-clock seconds without any useful output advance.
///    Acts as a wallclock backstop when iter-based checks miss.
const STALL_SAME_TOOL_THRESHOLD: usize = 3;
const TOOL_HISTORY_CAP: usize = 5;
const STALL_OUTPUT_UNCHANGED_ITERS: usize = 3;
const STALL_NO_PROGRESS_SECS: u64 = 90;

fn tool_call_signature(name: &str, arguments: &str) -> String {
    // FNV-1a 64-bit over name + args.
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in name.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash ^= u64::from(b':');
    hash = hash.wrapping_mul(0x100000001b3);
    for b in arguments.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{name}::{hash:016x}")
}

/// Human-readable activity label for the Layers status strip — shows WHAT the
/// agent is doing, not just which tool. Extracts the most relevant argument
/// (query / path / url / symbol) so the UI reads "code_search: fn spawn"
/// instead of a bare "executing code_search".
fn tool_activity_label(name: &str, arguments: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
    let arg = [
        "query", "pattern", "symbol", "path", "url", "file_path", "file", "name",
    ]
    .iter()
    .find_map(|k| v.get(*k).and_then(serde_json::Value::as_str))
    .unwrap_or("");
    if arg.is_empty() {
        format!("executing {name}")
    } else {
        let preview: String = arg.chars().take(48).collect();
        format!("{name}: {preview}")
    }
}

/// Parse text-format ("DSML") tool calls that some models emit inside the
/// content field instead of the native `tool_calls` API field. Returns the
/// recovered calls (empty if the content isn't a tool-call block).
///
/// Shape (delimiter junk like `｜｜DSML｜｜` is ignored):
/// ```text
/// <…tool_calls>
///   <…invoke name="web_fetch">
///     <…parameter name="url" …>https://…</…parameter>
///   </…invoke>
/// </…tool_calls>
/// ```
fn parse_dsml_tool_calls(content: &str) -> Vec<ToolCall> {
    if !(content.contains("invoke name=") && content.contains("tool_calls")) {
        return Vec::new();
    }
    let invoke_re = match regex::Regex::new(r#"(?s)invoke name="([^"]+)"(.*?)</[^>]*invoke>"#) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let param_re =
        match regex::Regex::new(r#"(?s)parameter name="([^"]+)"[^>]*>(.*?)</[^>]*parameter>"#) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

    let mut calls = Vec::new();
    for (i, cap) in invoke_re.captures_iter(content).enumerate() {
        let name = cap[1].to_string();
        let body = &cap[2];
        let mut args = serde_json::Map::new();
        for p in param_re.captures_iter(body) {
            args.insert(p[1].to_string(), serde_json::Value::String(p[2].trim().to_string()));
        }
        calls.push(ToolCall {
            id: format!("dsml_{i}"),
            name,
            arguments: serde_json::Value::Object(args).to_string(),
        });
    }
    calls
}

/// Strip text-format ("DSML") tool-call markup from a model answer, keeping
/// only prose. Used as a last-resort cleaner when the model emits a tool call
/// as text where a plain answer was expected (e.g. the force-final-answer turn).
fn strip_dsml_markup(content: &str) -> String {
    if !content.contains("invoke name=") && !content.contains("tool_calls") {
        return content.to_string();
    }
    content
        .lines()
        .filter(|l| {
            let t = l.trim();
            !(t.contains("DSML")
                || t.contains("tool_calls")
                || t.contains("invoke name=")
                || t.contains("parameter name=")
                || t.starts_with("</")
                || t.starts_with("<|")
                || t.starts_with("<｜"))
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Extract the most relevant single argument (query / path / url / symbol)
/// from a tool call's JSON args, for the activity-log chip target. Empty when
/// none apply (e.g. `done`).
/// Truncate `s` to at most `max` bytes, snapping DOWN to a UTF-8 char boundary
/// so it never slices through a multi-byte char (which panics). Appends `…`
/// when truncated.
fn truncate_on_char_boundary(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

fn tool_arg_preview(arguments: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
    [
        "query", "pattern", "symbol", "path", "url", "file_path", "file", "name", "command",
    ]
    .iter()
    .find_map(|k| v.get(*k).and_then(serde_json::Value::as_str))
    .unwrap_or("")
    .chars()
    .take(64)
    .collect()
}

fn default_ownership(kind: AgentKind) -> (Vec<String>, OwnershipMode) {
    match kind {
        AgentKind::Writer => (
            vec!["src/content/**".into(), "public/**".into()],
            OwnershipMode::Exclusive,
        ),
        AgentKind::Deployer => (
            vec!["scripts/deploy.sh".into(), "dist/**".into()],
            OwnershipMode::Exclusive,
        ),
        AgentKind::Judge => (vec![], OwnershipMode::ReadOnly),
        AgentKind::Reviewer => (vec![], OwnershipMode::ReadOnly),
        AgentKind::Explorer => (vec![], OwnershipMode::ReadOnly),
        AgentKind::Detective => (vec![], OwnershipMode::ReadOnly),
    }
}

#[derive(Clone)]
pub struct AgentManager {
    store: AgentStore,
    router: Router,
    worktrees: WorktreeManager,
    mailbox: MailboxFs,
    ownership: OwnershipRegistry,
    hooks: HookRunner,
    lessons: LessonRegistry,
    embed_cache: EmbedCache,
    gate_config: crate::gates::GateConfig,
    pricing: Arc<PricingConfig>,
    models: Arc<ModelsConfig>,
    mcp_registry: McpRegistry,
    runs_dir: PathBuf,
    cancellers: Arc<std::sync::Mutex<HashMap<Uuid, oneshot::Sender<()>>>>,
    approval_gate: ApprovalGate,
    /// Seconds to wait for human approval before auto-rejecting (from tokens.toml).
    approval_timeout_secs: u64,
    /// Default max iterations for agent loops (from tokens.toml [concurrency].max_agent_iters).
    default_max_iters: u32,
    /// Default per-agent USD cap when `SpawnAgentRequest.budget_usd` is omitted
    /// (AP11). Pulled from `tokens.toml [budget].agent_default_usd`.
    default_agent_budget_usd: f64,
    db: Option<crate::state::KiraDb>,
    /// Agent lifecycle event bus — subscribe to receive async agent state changes.
    pub events: EventBus<AgentEvent>,
    /// Concurrency cap: at most `max_agents` agent loops run simultaneously.
    concurrency: Arc<Semaphore>,
    cost_snapshots: Arc<std::sync::Mutex<HashMap<Uuid, AgentCostSnapshot>>>,
    /// Pillar event channel — emits `P7Cognition` per iteration so the REPL
    /// SSE stream surfaces live token + cost breakdowns. Workshop SPEKTOR
    /// P7: "events tipados em event store auditável."
    pillar_tx: crate::observability::pillar::PillarTx,
    /// Cross-thread semantic memory store (None if unavailable).
    memory_store: Option<Arc<crate::memory::sqlite_backend::SqliteVecStore>>,
}

impl AgentManager {
    pub fn new(
        router: Router,
        worktrees: WorktreeManager,
        mailbox: MailboxFs,
        ownership: OwnershipRegistry,
        hooks: HookRunner,
        db: Option<crate::state::KiraDb>,
        lessons: LessonRegistry,
        embed_cache: EmbedCache,
        gate_config: crate::gates::GateConfig,
        pricing: Arc<PricingConfig>,
        models: Arc<ModelsConfig>,
        mcp_registry: McpRegistry,
        runs_dir: PathBuf,
        max_agents: usize,
        approval_timeout_secs: u64,
        default_max_iters: u32,
        default_agent_budget_usd: f64,
        pillar_tx: crate::observability::pillar::PillarTx,
    ) -> Self {
        let store = match db.clone() {
            Some(d) => AgentStore::with_db(d),
            None => AgentStore::new(),
        };
        Self {
            store,
            router,
            worktrees,
            mailbox,
            ownership,
            hooks,
            lessons,
            embed_cache,
            gate_config,
            pricing,
            models,
            mcp_registry,
            runs_dir,
            cancellers: Arc::new(std::sync::Mutex::new(HashMap::new())),
            approval_gate: ApprovalGate::new(),
            approval_timeout_secs,
            default_max_iters,
            default_agent_budget_usd,
            db,
            events: EventBus::new(),
            concurrency: Arc::new(Semaphore::new(max_agents.max(1))),
            cost_snapshots: Arc::new(std::sync::Mutex::new(HashMap::new())),
            pillar_tx,
            memory_store: None, // Set via set_memory_store() after construction
        }
    }

    /// Set the memory store after construction (avoids circular dependency with AppState).
    pub fn set_memory_store(&mut self, store: Option<Arc<crate::memory::sqlite_backend::SqliteVecStore>>) {
        self.memory_store = store;
    }

    pub fn list(&self) -> Vec<Agent> {
        self.store.list()
    }

    pub fn get(&self, id: Uuid) -> Option<Agent> {
        self.store.get(id)
    }

    /// Read the agent's conversation history from its transcript.
    /// Returns `None` if the agent hasn't written a context.jsonl yet.
    pub fn history(&self, id: Uuid) -> Option<Vec<ChatMessageUnified>> {
        restore_context(&self.runs_dir, id)
    }

    pub fn cost(&self, id: Uuid) -> Option<AgentCostSnapshot> {
        self.cost_snapshots
            .lock()
            .expect("cost snapshots poisoned")
            .get(&id)
            .cloned()
    }

    /// `GET /v1/agents/{id}/context` backend — render a `ContextWindowSnapshot`
    /// keyed on the model the agent last used. Categories beyond `Messages`
    /// are placeholders until the per-category counters land; in the
    /// meantime the snapshot is still useful for tracking `used_pct` per
    /// model (each LLM has its own `context_limit`).
    pub fn context_window(
        &self,
        id: Uuid,
    ) -> Option<crate::proto::context_window::ContextWindowSnapshot> {
        use crate::proto::context_window::{ContextCategory, ContextWindowSnapshot};

        let snap = self.cost(id)?;
        let agent = self.store.get(id)?;
        let model = agent
            .model
            .clone()
            .or_else(|| snap.iterations.last().map(|it| it.model.clone()))?;

        // Resolve context limit from `tokens.toml [models.<name>]`. Unknown
        // models fall back to the configured default (131_072) — visible
        // in the snapshot as a generous ceiling rather than zero.
        let context_limit = self
            .models
            .entries
            .get(&model)
            .map_or(131_072_u64, |e| e.context_limit.try_into().unwrap_or(131_072_u64));

        // Best-effort messages category: sum of all iteration prompt-token
        // estimates is an upper bound on what's living in the rolling chat
        // history. Cheap to compute, accurate enough for v0.
        let messages_tokens: u64 = snap
            .iterations
            .iter()
            .map(|it| it.prompt_tokens_estimate as u64)
            .max()
            .unwrap_or(0);

        let categories = vec![
            ContextCategory {
                name: "Messages".into(),
                tokens: messages_tokens,
                pct: 0.0, // recomputed by build()
                item_count: u32::try_from(snap.iterations.len()).ok(),
                deferred: false,
            },
            ContextCategory {
                name: "System prompt".into(),
                tokens: 0,
                pct: 0.0,
                item_count: None,
                deferred: false,
            },
            ContextCategory {
                name: "System tools".into(),
                tokens: 0,
                pct: 0.0,
                item_count: None,
                deferred: false,
            },
            ContextCategory {
                name: "Skills".into(),
                tokens: 0,
                pct: 0.0,
                item_count: None,
                deferred: false,
            },
            ContextCategory {
                name: "MCP tools".into(),
                tokens: 0,
                pct: 0.0,
                item_count: None,
                deferred: false,
            },
            ContextCategory {
                name: "Custom agents".into(),
                tokens: 0,
                pct: 0.0,
                item_count: None,
                deferred: false,
            },
            ContextCategory {
                name: "Memory files".into(),
                tokens: 0,
                pct: 0.0,
                item_count: None,
                deferred: false,
            },
            ContextCategory {
                name: "MCP tools (deferred)".into(),
                tokens: 0,
                pct: 0.0,
                item_count: None,
                deferred: true,
            },
            ContextCategory {
                name: "System tools (deferred)".into(),
                tokens: 0,
                pct: 0.0,
                item_count: None,
                deferred: true,
            },
            ContextCategory {
                name: "Autocompact buffer".into(),
                tokens: 0,
                pct: 0.0,
                item_count: None,
                deferred: false,
            },
        ];

        Some(ContextWindowSnapshot::build(
            id.to_string(),
            model,
            context_limit,
            categories,
            Utc::now().to_rfc3339(),
        ))
    }

    pub fn cancel(&self, id: Uuid) -> bool {
        let mut map = self.cancellers.lock().expect("cancellers poisoned");
        if let Some(tx) = map.remove(&id) {
            let _ = tx.send(());
            true
        } else {
            false
        }
    }

    /// Approve or reject a pending HIGH-risk tool call for the given agent.
    /// Returns `true` if the agent had a pending approval request.
    pub fn approve(&self, id: Uuid, approved: bool) -> bool {
        self.approval_gate.respond(id, approved)
    }

    /// Watchdog hook: flip a non-terminal agent to `Stale`.
    /// No-op if the agent is already in a terminal state or missing.
    pub fn mark_stale(&self, id: Uuid) {
        self.store.update(id, |a| {
            if matches!(
                a.status,
                AgentStatus::ActiveForeground
                    | AgentStatus::ActiveBackground
                    | AgentStatus::Spawning
            ) {
                a.status = AgentStatus::Stale;
                a.finished_at = Some(Utc::now().to_rfc3339());
            }
        });
        // Also drop the canceller so /cancel doesn't try to signal a stale agent.
        self.cancellers
            .lock()
            .expect("cancellers poisoned")
            .remove(&id);
        // Release ownership so other agents can write to the same scope.
        self.ownership.release(&id.to_string());
    }

    #[instrument(skip(self, req), fields(kind = ?req.kind))]
    pub async fn spawn(&self, req: SpawnAgentRequest) -> Result<Agent> {
        let kind = req.kind;
        let tier = req
            .tier
            .clone()
            .unwrap_or_else(|| kind.default_tier().to_string());
        let max_iters = req.max_iters.unwrap_or(self.default_max_iters);
        let budget_cap = req.budget_usd.unwrap_or(self.default_agent_budget_usd);

        // Resolve worktree (reuse or auto-spawn).
        let (worktree_id, worktree_path) = if let Some(s) = req.worktree_id.as_deref() {
            let id = Uuid::parse_str(s).context("invalid worktree_id")?;
            let wt = self
                .worktrees
                .get(id)
                .with_context(|| format!("worktree not found: {id}"))?;
            (id.to_string(), PathBuf::from(wt.path))
        } else {
            let wt = self
                .worktrees
                .spawn(WorktreeSpawnRequest {
                    base: req.base.unwrap_or_else(|| "main".to_string()),
                    agent_kind: Some(format!("{kind:?}").to_lowercase()),
                })
                .await
                .context("auto-spawn worktree failed")?;
            (wt.id.clone(), PathBuf::from(wt.path))
        };

        let id = Uuid::now_v7();
        let id_str = id.to_string();

        let (patterns, mode) = default_ownership(kind);
        if !patterns.is_empty() {
            self.ownership
                .claim(Claim::now(id_str.clone(), patterns.clone(), mode))
                .context("ownership claim failed")?;
        }

        let now = Utc::now().to_rfc3339();
        let agent = Agent {
            id: id_str,
            kind,
            status: AgentStatus::Spawning,
            worktree_id: worktree_id.clone(),
            task: req.task.clone(),
            tier: tier.clone(),
            model: None,
            iters: 0,
            max_iters,
            budget_usd_cap: budget_cap,
            usd_spent: 0.0,
            result: None,
            error: None,
            created_at: now.clone(),
            finished_at: None,
            last_progress_at: Some(now),
            awaiting_approval: None,
        };
        self.store.insert(id, agent.clone());

        let (cancel_tx, cancel_rx) = oneshot::channel();
        self.cancellers
            .lock()
            .expect("cancellers poisoned")
            .insert(id, cancel_tx);

        // Inject active lessons matching this kind into the agent's system prompt.
        // M20 (Sprint 3): pass task text so BM25 reranker picks lessons most
        // relevant to this specific task, not just by recency/priority.
        let lesson_snippet =
            render_inject_snippet(&self.lessons.inject_for(kind, &req.task));

        let yolo = req.yolo_mode.unwrap_or(false);
        let background = req.background.unwrap_or(false);
        self.cost_snapshots
            .lock()
            .expect("cost snapshots poisoned")
            .insert(
                id,
                AgentCostSnapshot {
                    agent_id: id.to_string(),
                    yolo_mode: yolo,
                    max_iters,
                    budget_usd_cap: budget_cap,
                    lesson_chars: lesson_snippet.len(),
                    system_prompt_chars: 0,
                    total_tokens_in: 0,
                    total_tokens_out: 0,
                    total_tool_calls: 0,
                    total_usd: 0.0,
                    iterations: Vec::new(),
                },
            );

        // Acquire concurrency permit — blocks (async) until a slot is free.
        let permit = Arc::clone(&self.concurrency)
            .acquire_owned()
            .await
            .context("concurrency semaphore closed")?;

        let runs_dir = self.runs_dir.clone();
        let transcript = TranscriptWriter::new(&runs_dir, id);
        let injections = default_registry(kind);
        let ctx = AgentContext {
            id,
            kind,
            tier,
            task: req.task,
            background,
            max_iters,
            budget_cap,
            worktree_path,
            worktree_id: worktree_id.clone(),
            runs_dir,
            lesson_snippet,
            owned_globs: patterns,
            touched: Arc::new(std::sync::Mutex::new(Vec::new())),
            yolo_mode: yolo,
            gate_config: self.gate_config,
            pricing: Arc::clone(&self.pricing),
            models: Arc::clone(&self.models),
            store: self.store.clone(),
            router: self.router.clone(),
            mailbox: self.mailbox.clone(),
            ownership: self.ownership.clone(),
            hooks: self.hooks.clone(),
            lessons: self.lessons.clone(),
            embed_cache: self.embed_cache.clone(),
            mcp_registry: self.mcp_registry.clone(),
            cancellers: Arc::clone(&self.cancellers),
            transcript,
            injections,
            plan_mode: false,
            ruleset: PermissionRuleset::new(),
            approval_gate: self.approval_gate.clone(),
            approval_timeout_secs: self.approval_timeout_secs,
            db: self.db.clone(),
            event_bus: self.events.clone(),
            cost_snapshots: Arc::clone(&self.cost_snapshots),
            pillar_tx: self.pillar_tx.clone(),
            memory_store: self.memory_store.clone(),
            _concurrency_permit: permit,
        };

        self.events.publish(AgentEvent::Spawned {
            agent_id: id.to_string(),
            kind,
            worktree_id: worktree_id.clone(),
        });

        tokio::spawn(agent_loop(ctx, cancel_rx));

        // Spawn timeout task: cancels the agent if it exceeds the timeout.
        if let Some(timeout_secs) = req.timeout_secs {
            let timeout_agents = self.clone();
            let timeout_id = id;
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(timeout_secs)).await;
                timeout_agents.cancel(timeout_id);
            });
        }

        info!(id = %id, kind = ?kind, worktree_id = %worktree_id, "agent spawned");
        Ok(agent)
    }
}

struct AgentContext {
    id: Uuid,
    kind: AgentKind,
    tier: String,
    task: String,
    /// Run in background (non-blocking).
    background: bool,
    max_iters: u32,
    budget_cap: f64,
    worktree_path: PathBuf,
    worktree_id: String,
    runs_dir: PathBuf,
    /// Lesson snippet pre-built at spawn time, appended to the kind's system prompt.
    lesson_snippet: String,
    /// Owned globs the agent claimed (used by post-finish gates).
    owned_globs: Vec<String>,
    /// Canonical paths the agent actually wrote to (populated by `file_write`
    /// dispatches). Read by post-finish gates for L2 `file_integrity`.
    touched: Arc<std::sync::Mutex<Vec<String>>>,
    /// YOLO mode: when true, skip expensive gates (embedding, judge, meta-review).
    /// Used for quick tasks where speed > thoroughness.
    yolo_mode: bool,
    /// Gate thresholds (loaded from tokens.toml).
    gate_config: crate::gates::GateConfig,
    /// Model pricing (loaded from tokens.toml).
    pricing: Arc<PricingConfig>,
    /// Model metadata (loaded from tokens.toml).
    models: Arc<ModelsConfig>,
    store: AgentStore,
    router: Router,
    mailbox: MailboxFs,
    ownership: OwnershipRegistry,
    hooks: HookRunner,
    lessons: LessonRegistry,
    embed_cache: EmbedCache,
    mcp_registry: McpRegistry,
    cancellers: Arc<std::sync::Mutex<HashMap<Uuid, oneshot::Sender<()>>>>,
    /// Durable transcript writer — appends every message to context.jsonl.
    transcript: TranscriptWriter,
    /// Dynamic injection providers — inject system reminders between steps.
    injections: InjectionRegistry,
    /// Plan mode: when true, only read-only tools are available
    /// (`file_read`, `web_search`, `web_fetch`, done). `file_write` and bash are blocked.
    plan_mode: bool,
    /// SC5 — permission ruleset for fine-grained tool control.
    ruleset: PermissionRuleset,
    /// SC2 — approval gate for HIGH-risk tools.
    approval_gate: ApprovalGate,
    /// Seconds to wait for human approval before auto-rejecting.
    approval_timeout_secs: u64,
    /// Durable state db for writing judge decisions to `TBL_EVAL`.
    db: Option<crate::state::KiraDb>,
    /// Agent lifecycle event bus — publish state changes for subscribers.
    event_bus: EventBus<AgentEvent>,
    cost_snapshots: Arc<std::sync::Mutex<HashMap<Uuid, AgentCostSnapshot>>>,
    /// Pillar SSE channel — used to emit P7 cost breakdown per iter.
    pillar_tx: crate::observability::pillar::PillarTx,
    /// Cross-thread semantic memory store (None if unavailable).
    memory_store: Option<Arc<crate::memory::sqlite_backend::SqliteVecStore>>,
    /// Held for the lifetime of agent_loop; dropped in finish() → releases concurrency slot.
    _concurrency_permit: OwnedSemaphorePermit,
}

#[instrument(skip(ctx, cancel_rx), fields(agent_id = %ctx.id, kind = ?ctx.kind))]
async fn agent_loop(ctx: AgentContext, mut cancel_rx: oneshot::Receiver<()>) {
    ctx.store
        .update(ctx.id, |a| {
            a.status = if ctx.background {
                AgentStatus::ActiveBackground
            } else {
                AgentStatus::ActiveForeground
            };
        });

    let mut tools = agent_tools(
        &ctx.worktree_path.display().to_string(),
        ctx.kind,
        ctx.plan_mode,
    );
    // Append MCP tools from all connected servers.
    // In plan mode, MCP tools are skipped (they may have side effects).
    if !ctx.plan_mode {
        let mcp_tools = ctx.mcp_registry.all_tool_defs().await;
        if !mcp_tools.is_empty() {
            debug!(count = mcp_tools.len(), "MCP tools injected into agent catalog");
            tools.extend(mcp_tools);
        }
    }

    let mut messages: Vec<ChatMessageUnified> = Vec::new();
    let mut system = ctx.kind.system_prompt().to_string();
    if !ctx.lesson_snippet.is_empty() {
        system.push_str(&ctx.lesson_snippet);
    }
    // YOLO / chat mode: task came from the interactive REPL, not a coding plan.
    // Inject a directive that overrides the file-writing default: the agent must
    // return its answer via the `done` tool, not write files.  File operations
    // are still allowed when the user *explicitly* asks to create/modify files.
    if ctx.yolo_mode {
        system.push_str(
            "\n\n## Chat Mode (YOLO)\n\
             This task is a DIRECT CHAT request from the user.\n\
             - Return your complete answer in `done(final_result: \"...\")` — this is the response shown to the user.\n\
             - DO NOT write files unless the task explicitly asks you to create or modify a file.\n\
             - ANSWER FROM YOUR OWN KNOWLEDGE FIRST. For conceptual or general questions (definitions, \
             explanations, \"what is X\", \"how does Y work\") you almost certainly already know the answer — \
             just call `done` with it. Do NOT search the web for things you know.\n\
             - Only use web_search / web_fetch when the question needs current/external facts (recent events, \
             specific docs, live data) OR you are genuinely unsure. If a web tool fails, fall back to your \
             own knowledge instead of retrying endlessly.\n\
             - HONESTY ABOUT SOURCES (hard rule): only cite a URL as a source if a web_fetch of it ACTUALLY \
             SUCCEEDED in this session. NEVER invent or list sources/links you did not fetch. If web search \
             was unavailable or every fetch failed and you are answering from your own training, START the \
             answer with a one-line note like \"⚠️ Sem busca web — resposta do meu conhecimento, pode estar \
             desatualizada\" and do NOT add a fake \"Sources\" section.\n\
             - Answer in the same language the user used.",
        );
    }
    // SC1 — append explicit boundary notice so the LLM knows which regions
    // are untrusted data vs authoritative instructions.
    system.push_str(UNTRUSTED_DATA_NOTICE);
    // SC1 — wrap the entire system prompt in <SYSTEM_INSTRUCTIONS> so it
    // is visually / structurally distinct from <USER_DATA> and <TOOL_OUTPUT>
    // blocks injected later in the loop.
    let system_text = wrap_system(&system);
    ctx.transcript.write_prompt(&system_text);
    ctx.cost_snapshots
        .lock()
        .expect("cost snapshots poisoned")
        .entry(ctx.id)
        .and_modify(|snapshot| {
            snapshot.system_prompt_chars = system_text.len();
        });
    messages.push(ChatMessageUnified::System {
        content: system_text,
    });
    ctx.transcript.append(messages.last().unwrap());

    // SC1 — wrap user task in <USER_DATA> delimiters.
    // Level 1 Input Guardrail: check user task before injecting into prompt.
    let task_for_prompt = {
        let input_cfg = InputGuardrailConfig::default(); // TODO: load from ctx.config
        match pipeline::check_input(&ctx.task, &input_cfg) {
            InputCheckResult::Passed { message, pii_count } => {
                if pii_count > 0 {
                    debug!(agent_id = %ctx.id, pii_count, "input guardrail: PII redacted");
                }
                message
            }
            InputCheckResult::Blocked { reason } => {
                warn!(agent_id = %ctx.id, reason = %reason, "input guardrail BLOCKED user task");
                let block_err = format!("input guardrail blocked: {reason}");
                finish(&ctx, AgentStatus::Failed, None, Some(block_err), None, 0.0, 0).await;
                return;
            }
        }
    };
    messages.push(ChatMessageUnified::User {
        content: wrap_user_data(&task_for_prompt),
    });
    ctx.transcript.append(messages.last().unwrap());

    // Level 2 — Memory Retrieval: inject relevant cross-thread fragments before the loop.
    // Uses EmbedClient to compute query embedding, then searches the memory store.
    // Graceful: if memory is unavailable or embedding fails, proceeds without fragments.
    if let Some(ref memory_store) = ctx.memory_store {
        let embed_client = crate::memory::embed::EmbedClient::new(
            crate::memory::embed::EmbedConfig::default(), // TODO: load from ctx.config
        );
        match embed_client.embed(&task_for_prompt).await {
            Ok(query_embedding) => {
                let retrieval_cfg = crate::memory::retrieval::RetrievalConfig::default();
                let decay_cfg = crate::memory::decay::DecayConfig::default(); // TODO: load from ctx.config.memory.maturation
                let retrieval_result = crate::memory::retrieval::retrieve(
                    &query_embedding,
                    memory_store.as_ref(),
                    &retrieval_cfg,
                    &decay_cfg,
                ).await;
                if !retrieval_result.fragments.is_empty() {
                    debug!(
                        agent_id = %ctx.id,
                        fragment_count = retrieval_result.fragments.len(),
                        "memory retrieval: injecting fragments into prompt"
                    );
                    messages.push(ChatMessageUnified::User {
                        content: retrieval_result.rag_block,
                    });
                    ctx.transcript.append(messages.last().unwrap());
                }
            }
            Err(e) => {
                debug!(agent_id = %ctx.id, error = %e, "memory retrieval: embedding failed, proceeding without");
            }
        }
    }

    let mut iters: u32 = 0;
    let mut usd_spent: f64 = 0.0;
    let mut model_used: Option<String> = None;
    let mut final_result: Option<String> = None;
    let mut error: Option<String> = None;
    // AP1 hardening (2026-05-26): cap the kimi-cli-style continuation
    // prompt to fire at most once per agent run. Previously a model that
    // returned an empty string would loop until `max_iters` paying token
    // burn for every retry. Workshop SPEKTOR: "tarefa trivial não vira
    // burocracia" — accept the short answer after one nudge.
    let mut continuation_fired: bool = false;
    // Stall detector — tracks recent tool call signatures.
    let mut tool_history: std::collections::VecDeque<String> =
        std::collections::VecDeque::with_capacity(TOOL_HISTORY_CAP);
    // Output-unchanged stall: hash of (content + sorted tool sigs) per iter.
    let mut output_history: std::collections::VecDeque<String> =
        std::collections::VecDeque::with_capacity(STALL_OUTPUT_UNCHANGED_ITERS);
    // No-progress stall: wall-clock of last useful output change.
    let mut last_progress_instant: std::time::Instant = std::time::Instant::now();
    // Per-run tool-result cache: avoids re-dispatching (and re-injecting the
    // same payload tokens for) identical read-only tool calls within one agent
    // run. Keyed by hash(name + args). Conservatively cleared on any successful
    // file_write so stale file_read/code_search results are never served.
    let mut tool_cache: std::collections::HashMap<u64, String> = std::collections::HashMap::new();
    // Web circuit-breaker: after repeated web tool failures (no Brave key /
    // SearXNG down / hallucinated URLs), stop the model from flailing on the
    // web and force it to answer from its own knowledge.
    let mut web_fail_streak: u32 = 0;
    let mut web_nudge_fired = false;

    loop {
        if iters >= ctx.max_iters {
            warn!(agent_id = %ctx.id, iters, "max_iters reached — forcing final answer");
            // Don't fail with an empty result: spend one last no-tools turn so
            // the user always gets an answer synthesised from what the agent
            // gathered (and the model's own knowledge). Tool flailing (e.g. a
            // dead web_search) must not swallow the response.
            messages.push(ChatMessageUnified::User {
                content: "You have used all available research steps. Answer the \
                          user's original question NOW, directly and completely, \
                          using what you already gathered plus your own knowledge. \
                          Do not call any tools."
                    .to_string(),
            });
            let synth = ctx.router.route_chat_with_tools(
                &ctx.tier,
                None,
                messages.clone(),
                Vec::new(), // no tools — force a text answer
                None,
            );
            match tokio::select! {
                _ = &mut cancel_rx => None,
                r = synth => r.ok(),
            } {
                Some((out, _p, m)) => {
                    // The model may STILL emit DSML tool-call markup here even
                    // though no tools were offered — strip it so the user never
                    // sees raw `｜｜DSML｜｜tool_calls` as the answer.
                    let cleaned = strip_dsml_markup(&out.content);
                    if cleaned.trim().is_empty() {
                        error = Some(
                            "reached the step limit without producing a final answer \
                             (web tools were unavailable — set BRAVE_API_KEY or start \
                             SearXNG, or ask a more specific question)"
                                .to_string(),
                        );
                    } else {
                        model_used = Some(m);
                        final_result = Some(cleaned);
                    }
                }
                None => {
                    error = Some(format!("max_iters reached ({} iterations)", ctx.max_iters));
                }
            }
            break;
        }
        if usd_spent >= ctx.budget_cap {
            warn!(agent_id = %ctx.id, usd_spent, cap = ctx.budget_cap, "budget cap reached");
            error = Some(format!(
                "budget cap reached (${usd_spent:.4} >= ${cap:.4})",
                cap = ctx.budget_cap
            ));
            break;
        }

        // Dynamic injection: collect system reminders from providers.
        let inject_ctx = InjectContext {
            step: iters,
            max_iters: ctx.max_iters,
            usd_spent,
            budget_cap: ctx.budget_cap,
            kind: ctx.kind,
        };
        for reminder in ctx.injections.collect(&inject_ctx) {
            messages.push(ChatMessageUnified::User {
                content: reminder,
            });
            ctx.transcript.append(messages.last().unwrap());
        }

        // Compaction (P6 resilience): if message history is approaching the
        // model's context limit, prune oldest non-system messages.
        if let Some(ref m) = model_used {
            compact_if_needed(&mut messages, &ctx.models, m);
        }

        // Layers transparency (P7→Perception, P2→Reasoning): mark the
        // context-assembly + reasoning milestones so the TUI Layers View
        // pulses during a turn. `emit_wire` is SSE-only (fast UI breadcrumb,
        // no JSONL noise). classify() maps phase "context"→Perception and
        // P2Coordination→Reasoning.
        {
            use crate::observability::pillar;
            use crate::proto::pillar::Pillar;
            pillar::emit_wire(
                &ctx.pillar_tx,
                Pillar::P7Cognition,
                "context",
                format!("assembling context ({} msgs)", messages.len()),
            );
            pillar::emit_wire(
                &ctx.pillar_tx,
                Pillar::P2Coordination,
                "start",
                if iters == 0 {
                    "reasoning: thinking through the request".to_string()
                } else {
                    format!("reasoning: analyzing results (step {})", iters + 1)
                },
            );
        }

        // Race LLM call vs cancellation.
        let route = ctx
            .router
            .route_chat_with_tools(&ctx.tier, None, messages.clone(), tools.clone(), None);
        let (mut output, _provider, model) = tokio::select! {
            _ = &mut cancel_rx => {
                warn!(agent_id = %ctx.id, "cancelled mid-loop");
                ctx.store.update(ctx.id, |a| a.status = AgentStatus::Cancelled);
                finish(&ctx, AgentStatus::Cancelled, None, Some("cancelled".into()), model_used.clone(), usd_spent, iters).await;
                return;
            }
            r = route => match r {
                Ok(t) => t,
                Err(e) => {
                    error!(agent_id = %ctx.id, error = ?e, "router call failed");
                    error = Some(e.to_string());
                    break;
                }
            }
        };

        iters += 1;
        model_used = Some(model.clone());
        let iter_usd = approx_cost(&ctx.pricing, &model, output.tokens_in, output.tokens_out);
        let prompt_tokens_estimate = count_messages_tokens(&messages);
        let messages_count = messages.len();
        let tool_calls_count = output.tool_calls.len();
        let assistant_chars = output.content.len();
        usd_spent += iter_usd;
        ctx.cost_snapshots
            .lock()
            .expect("cost snapshots poisoned")
            .entry(ctx.id)
            .and_modify(|snapshot| {
                snapshot.total_tokens_in += u64::from(output.tokens_in);
                snapshot.total_tokens_out += u64::from(output.tokens_out);
                snapshot.total_tool_calls += tool_calls_count as u64;
                snapshot.total_usd = usd_spent;
                snapshot.iterations.push(AgentCostIteration {
                    iter: iters,
                    model: model.clone(),
                    prompt_tokens_estimate,
                    messages_count,
                    tokens_in: output.tokens_in,
                    tokens_out: output.tokens_out,
                    tool_calls: tool_calls_count,
                    usd: iter_usd,
                    cumulative_usd: usd_spent,
                    assistant_chars,
                });
            });
        let now_iter = Utc::now().to_rfc3339();
        ctx.store.update(ctx.id, |a| {
            a.iters = iters;
            a.usd_spent = usd_spent;
            a.model = Some(model.clone());
            a.last_progress_at = Some(now_iter.clone());
        });
        // P7 cost transparency (2026-05-26): publish per-iter token + tool
        // counts as part of the Heartbeat so SSE consumers (REPL StatusBar)
        // can render live cost without polling `/v1/agents/{id}/cost`.
        // Cumulative values come straight from `cost_snapshots` to avoid
        // double-accounting against `output.tokens_in`/`out` for the iter.
        let (cum_tokens_in, cum_tokens_out, cum_tool_calls) = {
            let snaps = ctx.cost_snapshots.lock().expect("cost snapshots poisoned");
            snaps.get(&ctx.id).map_or((0, 0, 0), |s| {
                (s.total_tokens_in, s.total_tokens_out, s.total_tool_calls)
            })
        };
        ctx.event_bus.publish(AgentEvent::Heartbeat {
            agent_id: ctx.id.to_string(),
            iters,
            usd_spent,
            tokens_in: cum_tokens_in,
            tokens_out: cum_tokens_out,
            tool_calls: cum_tool_calls,
            model: Some(model.clone()),
        });

        // P7 pillar emission: REPL listens to /v1/events/stream (Pillars)
        // not the AgentEvent bus, so we mirror the heartbeat here as a
        // structured cost step. Workshop SPEKTOR P7: every cost decision
        // becomes an auditable event with model, tokens, and dollars.
        {
            use crate::observability::pillar;
            use crate::proto::pillar::Pillar;
            let mut fields = serde_json::Map::new();
            fields.insert("iter".into(), serde_json::json!(iters));
            fields.insert("model".into(), serde_json::json!(model));
            fields.insert("tokens_in".into(), serde_json::json!(output.tokens_in));
            fields.insert("tokens_out".into(), serde_json::json!(output.tokens_out));
            fields.insert("cum_tokens_in".into(), serde_json::json!(cum_tokens_in));
            fields.insert("cum_tokens_out".into(), serde_json::json!(cum_tokens_out));
            fields.insert("usd_iter".into(), serde_json::json!(iter_usd));
            fields.insert("usd_cum".into(), serde_json::json!(usd_spent));
            fields.insert("tool_calls".into(), serde_json::json!(tool_calls_count));
            fields.insert("cum_tool_calls".into(), serde_json::json!(cum_tool_calls));
            fields.insert("prompt_tokens_estimate".into(), serde_json::json!(prompt_tokens_estimate));
            fields.insert("assistant_chars".into(), serde_json::json!(assistant_chars));
            pillar::emit_full(
                &ctx.pillar_tx,
                Pillar::P7Cognition,
                "cost",
                format!(
                    "iter {iters} · {} · in={} out={} · ${:.4} (cum ${:.4})",
                    model, output.tokens_in, output.tokens_out, iter_usd, usd_spent
                ),
                None,
                Some(ctx.id.to_string()),
                fields,
            );
        }

        // Recover text-format tool calls. Some models (e.g. deepseek-v4-flash)
        // emit tool calls as markup inside `content` (a `｜｜DSML｜｜tool_calls`
        // block) instead of the API's native `tool_calls` field. Unparsed, that
        // markup leaks to the user as the "answer" and the task never runs.
        // Parse it back into real ToolCalls and clear the leaked content.
        if output.tool_calls.is_empty() {
            let recovered = parse_dsml_tool_calls(&output.content);
            if !recovered.is_empty() {
                warn!(
                    agent_id = %ctx.id,
                    n = recovered.len(),
                    "recovered text-format (DSML) tool calls from content"
                );
                output.tool_calls = recovered;
                output.content = String::new();
            }
        }

        // Push the assistant message into history.
        messages.push(ChatMessageUnified::Assistant {
            content: Some(output.content.clone()),
            tool_calls: output.tool_calls.clone(),
            reasoning_content: output.reasoning_content.clone(),
        });
        ctx.transcript.append(messages.last().unwrap());

        // Decide next step.
        let tool_calls = output.tool_calls.clone();
        if tool_calls.is_empty() {
            // Summary continuation: if content is too short and we have
            // budget, ask for more detail (kimi-cli pattern: < 200 chars
            // triggers a continuation prompt, max 1 extra turn).
            //
            // AP1 (SPEKTOR "barato primeiro"): force-continuation disabled in
            // YOLO/chat mode. Short answers to conceptual questions are valid
            // ("3 layers" is a complete answer); paying tokens to elaborate
            // burns budget. Kept for SDD/agent tasks where short output often
            // signals incomplete work.
            const SUMMARY_MIN_CHARS: usize = 200;
            const CONTINUATION_PROMPT: &str =
                "Please continue and provide a more detailed and comprehensive response. \
                 Include specific findings, reasoning, and concrete deliverables.";

            if !ctx.yolo_mode
                && !continuation_fired
                && output.content.len() < SUMMARY_MIN_CHARS
                && iters < ctx.max_iters
                && usd_spent < ctx.budget_cap
            {
                debug!(
                    agent_id = %ctx.id,
                    content_len = output.content.len(),
                    "short response — sending continuation prompt (one-shot)"
                );
                messages.push(ChatMessageUnified::User {
                    content: CONTINUATION_PROMPT.to_string(),
                });
                ctx.transcript.append(messages.last().unwrap());
                continuation_fired = true;
                continue;
            }
            // Content-only response. Treat as final. A reasoning model
            // (e.g. deepseek-reasoner) can return an empty `content` with the
            // real answer in `reasoning_content`; fall back to it so the user
            // never gets a blank reply.
            let final_text = if output.content.trim().is_empty() {
                output
                    .reasoning_content
                    .clone()
                    .filter(|r| !r.trim().is_empty())
                    .unwrap_or_else(|| output.content.clone())
            } else {
                output.content.clone()
            };
            final_result = Some(final_text);
            break;
        }

        // Execute each tool call sequentially. Append tool result messages.
        let mut break_outer_with_done: Option<String> = None;
        let mut break_outer_with_stall = false;
        for tc in &tool_calls {
            let ToolCall { id, name, arguments } = tc;
            debug!(tool = %name, args_len = arguments.len(), "agent invoked tool");

            // Layers transparency: tool execution → Execution layer
            // (P6Resilience + phase "tool" maps there via classify()).
            if name != "done" {
                use crate::observability::pillar;
                use crate::proto::pillar::Pillar;
                // Layers pulse (Execution).
                pillar::emit_wire(
                    &ctx.pillar_tx,
                    Pillar::P6Resilience,
                    "tool",
                    tool_activity_label(name, arguments),
                );
                // Activity-log chip: Running (spinner). Paired with the
                // `tool_result` event below which flips it to ✓/✗.
                let mut f = serde_json::Map::new();
                f.insert("tool".into(), serde_json::json!(name));
                f.insert("target".into(), serde_json::json!(tool_arg_preview(arguments)));
                pillar::emit_wire_full(
                    &ctx.pillar_tx,
                    Pillar::P6Resilience,
                    "tool_start",
                    name.clone(),
                    Some(ctx.id.to_string()),
                    f,
                );
            }

            // Stall detection (G6 SameToolLoop).
            // `done` is excluded from stall detection — it's the legitimate exit.
            if name != "done" {
                let sig = tool_call_signature(name, arguments);
                tool_history.push_back(sig.clone());
                if tool_history.len() > TOOL_HISTORY_CAP {
                    tool_history.pop_front();
                }
                let recent_same = tool_history
                    .iter()
                    .rev()
                    .take(STALL_SAME_TOOL_THRESHOLD)
                    .filter(|s| s.as_str() == sig.as_str())
                    .count();
                if recent_same >= STALL_SAME_TOOL_THRESHOLD {
                    warn!(
                        agent_id = %ctx.id,
                        tool = %name,
                        repeats = recent_same,
                        "stall detected: same tool+args repeated"
                    );
                    error = Some(format!(
                        "stall: tool '{name}' called {recent_same} times in a row with identical args"
                    ));
                    break_outer_with_stall = true;
                    break;
                }
            }

            // SC2 — classify risk + audit before dispatch.
            // CRITICAL: BLOCKED (tool call skipped, agent notified).
            // HIGH: warn-only in v0; approval gate in v0.5.
            let risk = classify_tool_risk(name, arguments, &ctx.mcp_registry).await;
            if matches!(risk, Some(ToolRisk::Critical)) {
                let msg = format!("BLOCKED: CRITICAL-risk tool '{name}' is not permitted");
                warn!(
                    agent_id = %ctx.id,
                    tool = %name,
                    risk = "critical",
                    msg
                );
                messages.push(ChatMessageUnified::Tool {
                    call_id: id.clone(),
                    content: wrap_tool_output(name, &serde_json::to_string(&serde_json::json!({"error": msg})).unwrap()),
                });
                ctx.transcript.append(messages.last().unwrap());
                continue;
            } else if matches!(risk, Some(ToolRisk::High)) {
                // SC5 — evaluate permission ruleset before the approval gate.
                let action = ctx.ruleset.evaluate(name, arguments);
                if action == RuleAction::Block {
                    let msg = format!(
                        "BLOCKED: HIGH-risk tool '{name}' matched a Block rule in the permission ruleset"
                    );
                    warn!(agent_id = %ctx.id, tool = %name, "HIGH-risk tool blocked by ruleset");
                    messages.push(ChatMessageUnified::Tool {
                        call_id: id.clone(),
                        content: wrap_tool_output(
                            name,
                            &serde_json::to_string(&serde_json::json!({"error": msg}))
                                .unwrap(),
                        ),
                    });
                    ctx.transcript.append(messages.last().unwrap());
                    continue;
                } else if action == RuleAction::Allow {
                    debug!(
                        agent_id = %ctx.id,
                        tool = %name,
                        "HIGH-risk tool allowed by ruleset — skipping approval"
                    );
                    // Fall through to dispatch below (same path as LOW-risk).
                } else {
                    // Ask → proceed to approval gate.
                    warn!(
                        agent_id = %ctx.id,
                        tool = %name,
                        risk = "high",
                        "HIGH-risk tool call — awaiting approval"
                    );

                    // Truncate args to ~2 KB for display, on a UTF-8 char
                    // boundary (slicing mid-char — e.g. inside '─'/'é' from a
                    // markdown file_write — panics the agent thread).
                    let args_display =
                        truncate_on_char_boundary(&serde_json::to_string(arguments).unwrap_or_default(), 2048);

                    // Signal REPL — set awaiting_approval on the Agent snapshot.
                    ctx.store.update(ctx.id, |a| {
                        a.awaiting_approval = Some(PendingApproval {
                            tool_name: name.clone(),
                            arguments: args_display.clone(),
                        });
                    });
                    // Bridge to the SSE pillar stream so the TUI pops the
                    // approval modal. AgentEvent (event_bus) is NOT wired to
                    // SSE — without this the gate is invisible and the agent
                    // silently times out (file never created, no prompt shown).
                    {
                        use crate::observability::pillar;
                        use crate::proto::pillar::Pillar;
                        let mut fields = serde_json::Map::new();
                        fields.insert("tool_name".into(), serde_json::json!(name));
                        fields.insert("risk_level".into(), serde_json::json!("HIGH"));
                        fields.insert("arguments".into(), serde_json::json!(args_display.clone()));
                        fields.insert(
                            "timeout_secs".into(),
                            serde_json::json!(ctx.approval_timeout_secs),
                        );
                        pillar::emit_wire_full(
                            &ctx.pillar_tx,
                            Pillar::P5Guardrails,
                            "awaiting_approval",
                            format!("approval required: {name}"),
                            Some(ctx.id.to_string()),
                            fields,
                        );
                    }
                    ctx.event_bus.publish(AgentEvent::AwaitingApproval {
                        agent_id: ctx.id.to_string(),
                        tool_name: name.clone(),
                        arguments: args_display,
                    });

                    let mut rx = ctx.approval_gate.request(ctx.id);
                let approved = match timeout(
                    TokioDuration::from_secs(ctx.approval_timeout_secs),
                    rx.changed(),
                )
                .await
                {
                    Ok(Ok(())) => *rx.borrow() == ApprovalVerdict::Approved,
                    Ok(Err(_)) => {
                        warn!(agent_id = %ctx.id, "approval channel closed");
                        false
                    }
                    Err(_) => {
                        warn!(
                            agent_id = %ctx.id,
                            secs = ctx.approval_timeout_secs,
                            "approval timeout — auto-rejecting"
                        );
                        false
                    }
                };
                ctx.approval_gate.remove(ctx.id);

                // Clear approval state and notify subscribers.
                ctx.store.update(ctx.id, |a| { a.awaiting_approval = None; });
                ctx.event_bus.publish(AgentEvent::ApprovalResolved {
                    agent_id: ctx.id.to_string(),
                    approved,
                });

                if !approved {
                    let msg = format!(
                        "REJECTED: HIGH-risk tool '{name}' was not approved (timeout or explicit rejection)"
                    );
                    warn!(agent_id = %ctx.id, tool = %name, "HIGH-risk tool rejected");
                    messages.push(ChatMessageUnified::Tool {
                        call_id: id.clone(),
                        content: wrap_tool_output(
                            name,
                            &serde_json::to_string(&serde_json::json!({"error": msg}))
                                .unwrap(),
                        ),
                    });
                    ctx.transcript.append(messages.last().unwrap());
                    continue;
                }
                }
            }

            // Result cache: serve identical cacheable read-only calls from the
            // per-run cache instead of re-dispatching + re-paying the payload
            // tokens. `tool_meta` declares which tools are safe to cache.
            let meta = crate::agents::tools::tool_meta(name);
            let cache_key = meta.cacheable.then(|| tool_cache_key(name, arguments));

            let result = if let Some(key) = cache_key {
                if let Some(payload) = tool_cache.get(&key) {
                    debug!(agent_id = %ctx.id, tool = %name, "tool cache hit");
                    crate::agents::tools::ToolResult {
                        payload: payload.clone(),
                        done: None,
                    }
                } else {
                    let r = dispatch_with_hooks(
                        name,
                        arguments,
                        &ctx.worktree_path,
                        &ctx.hooks,
                        &ctx.id.to_string(),
                        &ctx.worktree_id,
                        Some(&ctx.touched),
                        Some(&ctx.mcp_registry),
                    )
                    .await;
                    // Only cache successful payloads (no "error" field).
                    if r.done.is_none() && !payload_is_error(&r.payload) {
                        tool_cache.insert(key, r.payload.clone());
                    }
                    r
                }
            } else {
                dispatch_with_hooks(
                    name,
                    arguments,
                    &ctx.worktree_path,
                    &ctx.hooks,
                    &ctx.id.to_string(),
                    &ctx.worktree_id,
                    Some(&ctx.touched),
                    Some(&ctx.mcp_registry),
                )
                .await
            };

            // A successful write/edit may invalidate previously cached reads/searches.
            if (name == "file_write" || name == "edit")
                && result.done.is_none()
                && !payload_is_error(&result.payload)
            {
                tool_cache.clear();
            }

            // Level 2 — Tool Guardrail: validate output before injecting into context.
            let tool_cfg = ToolGuardrailConfig::default(); // TODO: load from ctx.config
            let tool_check = crate::safety::tool::validate(name, &result.payload, None, &tool_cfg);
            let validated_payload = match tool_check {
                ToolCheckResult::Passed { output, truncated, .. } => {
                    if truncated {
                        debug!(agent_id = %ctx.id, tool = %name, "tool output truncated by guardrail");
                    }
                    output
                }
                ToolCheckResult::Blocked { reason, .. } => {
                    warn!(agent_id = %ctx.id, tool = %name, reason = %reason, "tool output BLOCKED by guardrail");
                    serde_json::to_string(&serde_json::json!({"error": reason})).unwrap_or_default()
                }
            };

            // Tool result message — SC1 wrap payload as untrusted data.
            messages.push(ChatMessageUnified::Tool {
                call_id: id.clone(),
                content: wrap_tool_output(name, &validated_payload),
            });
            ctx.transcript.append(messages.last().unwrap());

            // Activity-log chip (Kiro-style): emit a terminal tool event the TUI
            // renders as "✓/✗ <verb> <target>". Deterministic — driven by the
            // tool call, not by LLM text.
            {
                use crate::observability::pillar;
                use crate::proto::pillar::Pillar;
                let mut fields = serde_json::Map::new();
                fields.insert("tool".into(), serde_json::json!(name));
                fields.insert("target".into(), serde_json::json!(tool_arg_preview(arguments)));
                fields.insert("ok".into(), serde_json::json!(!payload_is_error(&validated_payload)));
                // Detail body for the expandable chip — first few lines of the
                // tool output (capped).
                let detail: Vec<String> = validated_payload
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .take(8)
                    .map(|l| l.chars().take(160).collect::<String>())
                    .collect();
                fields.insert("detail".into(), serde_json::json!(detail));
                pillar::emit_wire_full(
                    &ctx.pillar_tx,
                    Pillar::P6Resilience,
                    "tool_result",
                    name.clone(),
                    Some(ctx.id.to_string()),
                    fields,
                );
            }

            // Web circuit-breaker: count TOTAL web tool failures (not just
            // consecutive — partial web_fetch successes were resetting the
            // streak and letting the model flail for the whole iter budget).
            // Once they pile up, force it off the web onto its own knowledge.
            if matches!(name.as_str(), "web_search" | "web_fetch")
                && payload_is_error(&validated_payload)
            {
                web_fail_streak += 1;
            }
            const WEB_FAIL_LIMIT: u32 = 4;
            if web_fail_streak >= WEB_FAIL_LIMIT && !web_nudge_fired {
                web_nudge_fired = true;
                warn!(agent_id = %ctx.id, web_fail_streak, "web tools failing — nudging to knowledge");
                messages.push(ChatMessageUnified::User {
                    content: "Web tools are unavailable this session (repeated failures). \
                              STOP calling web_search and web_fetch. Complete the user's \
                              request NOW using your own knowledge — write the file(s) or \
                              give the answer directly. Do not search the web again."
                        .to_string(),
                });
                ctx.transcript.append(messages.last().unwrap());
            }

            if let Some(done_payload) = result.done {
                break_outer_with_done = Some(done_payload);
                break;
            }
        }

        if let Some(done_payload) = break_outer_with_done {
            final_result = Some(done_payload);
            break;
        }
        if break_outer_with_stall || error.is_some() {
            break;
        }

        // P6 — checkpoint message history after each successful iteration.
        if let Some(ref db) = ctx.db {
            checkpoint_save(db, ctx.id, &messages);
        }

        // ── OutputUnchanged stall check ────────────────────────────────────
        // Hash this iter's "useful output": assistant content + all tool sigs
        // pushed this iter. If unchanged across N consecutive iters → stall.
        let content_snippet = messages
            .last().map_or("", |_| "");
        let recent_sigs: Vec<&str> = tool_history
            .iter()
            .rev()
            .take(4)
            .map(String::as_str)
            .collect();
        let mut combo = format!("c:{content_snippet}|");
        for s in &recent_sigs {
            combo.push_str(s);
            combo.push(';');
        }
        let iter_hash = tool_call_signature("iter", &combo);
        output_history.push_back(iter_hash.clone());
        if output_history.len() > STALL_OUTPUT_UNCHANGED_ITERS {
            output_history.pop_front();
        }
        if output_history.len() == STALL_OUTPUT_UNCHANGED_ITERS
            && output_history.iter().all(|h| h == &iter_hash)
        {
            warn!(
                agent_id = %ctx.id,
                iters = output_history.len(),
                "stall detected: output unchanged across iterations"
            );
            error = Some(format!(
                "stall: agent output (content + tool sigs) unchanged for {STALL_OUTPUT_UNCHANGED_ITERS} iterations"
            ));
            break;
        }

        // ── NoProgress wallclock stall ─────────────────────────────────────
        // Tracks whether *some* signature appeared this iter. If output_hash
        // changes vs previous, mark progress; else check elapsed.
        let advanced = output_history
            .iter()
            .rev()
            .nth(1)
            .is_none_or(|prev| prev != &iter_hash);
        if advanced {
            last_progress_instant = std::time::Instant::now();
        } else {
            let elapsed = last_progress_instant.elapsed().as_secs();
            if elapsed >= STALL_NO_PROGRESS_SECS {
                warn!(
                    agent_id = %ctx.id,
                    elapsed_secs = elapsed,
                    "stall detected: no-progress wallclock exceeded"
                );
                error = Some(format!(
                    "stall: no useful output advance for {elapsed}s (threshold {STALL_NO_PROGRESS_SECS}s)"
                ));
                break;
            }
        }
    }

    // Stall is a defense, not a failure — mark Stopped so watchdog/cleanup
    // policies treat it gently. Real bugs still flow into Failed via the
    // error.is_some() path below (which discriminates).
    let stall_only = error
        .as_deref()
        .is_some_and(|e| e.starts_with("stall:"));

    // Level 3 — Output Guardrail (LLM-as-Judge): evaluate final response before delivery.
    // Only runs when: (1) there's a final_result, (2) guardrail is enabled, (3) we have a model.
    // The actual Judge LLM call is a TODO — for now we log the intent and pass through.
    if let (Some(result_text), Some(writer)) = (&final_result, &model_used) {
        let output_cfg = crate::safety::output::OutputGuardrailConfig::default();
        if output_cfg.enabled {
            // TODO: wire real JudgeCaller implementation using ctx.router with tier "judge"
            // For now, log that the output guardrail checkpoint was reached.
            debug!(
                agent_id = %ctx.id,
                writer_model = %writer,
                result_len = result_text.len(),
                "output guardrail checkpoint: judge evaluation pending (wire JudgeCaller)"
            );
        }
    }

    let status = if final_result.is_some() {
        AgentStatus::Completed
    } else if stall_only {
        AgentStatus::Stopped
    } else if error.is_some() {
        AgentStatus::Failed
    } else {
        AgentStatus::Stopped
    };
    finish(&ctx, status, final_result, error, model_used, usd_spent, iters).await;
}

/// Stable cache key for a tool call: hash of tool name + raw argument string.
/// Identical calls (same name + same args JSON) collide, which is exactly the
/// cache-hit we want; different args never collide in practice.
fn tool_cache_key(name: &str, args: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut h);
    args.hash(&mut h);
    h.finish()
}

/// Whether a tool payload is an error envelope (`{"error": ...}`). Error
/// payloads are never cached so a transient failure isn't replayed.
fn payload_is_error(payload: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .is_some_and(|v| v.get("error").is_some())
}

async fn finish(
    ctx: &AgentContext,
    status: AgentStatus,
    result: Option<String>,
    error: Option<String>,
    model: Option<String>,
    usd: f64,
    iters: u32,
) {
    let id = ctx.id;
    let finished_at = Utc::now().to_rfc3339();

    // Promote files written in the isolated worktree back to the repo root so
    // they survive worktree pruning (otherwise the operator's created/edited
    // files vanish). Only on success; only the approved file_write/edit paths
    // recorded in `touched`.
    let mut result = result;
    if matches!(status, AgentStatus::Completed) {
        let promoted = promote_worktree_files(ctx);
        if !promoted.is_empty() {
            info!(agent_id = %id, files = ?promoted, "promoted worktree files to repo");
            let note = format!("\n\n📄 Salvo no repo: {}", promoted.join(", "));
            result = Some(result.unwrap_or_default() + &note);
        }
    }

    ctx.store.update(id, |a| {
        a.status = status;
        a.result = result.clone();
        a.error = error.clone();
        a.model = model.clone();
        a.iters = iters;
        a.usd_spent = usd;
        a.finished_at = Some(finished_at.clone());
    });

    // Write durable transcript snapshot.
    if let Some(agent) = ctx.store.get(ctx.id) {
        ctx.transcript.write_meta(&agent);
    }

    let payload = serde_json::json!({
        "status": format!("{status:?}"),
        "result": result,
        "error": error,
        "model": model,
        "iters": iters,
        "usd_spent": usd,
    });
    let _ = ctx
        .mailbox
        .send(
            "core",
            SendRequest {
                from: id.to_string(),
                kind: "result".to_string(),
                payload,
            },
        )
        .await;

    ctx.ownership.release(&id.to_string());
    ctx.cancellers
        .lock()
        .expect("cancellers poisoned")
        .remove(&id);

    // P6 — clean up checkpoint now that the agent is terminal.
    if let Some(ref db) = ctx.db {
        checkpoint_delete(db, id);
    }

    info!(agent_id = %id, ?status, iters, usd, "agent finished");

    // Push the terminal state over SSE (wire-only) so the TUI learns of
    // completion + gets the result instantly, instead of waiting up to 2s for
    // the next status poll. The poller stays as a reconnect fallback.
    {
        use crate::observability::pillar;
        use crate::proto::pillar::Pillar;
        let status_str = serde_json::to_value(status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let mut fields = serde_json::Map::new();
        fields.insert("status".into(), serde_json::json!(status_str));
        fields.insert("result".into(), serde_json::json!(result));
        fields.insert("error".into(), serde_json::json!(error));
        pillar::emit_wire_full(
            &ctx.pillar_tx,
            Pillar::P2Coordination,
            "agent_terminal",
            "agent finished",
            Some(id.to_string()),
            fields,
        );
    }

    // Publish lifecycle event to the async event bus.
    let ev = match status {
        AgentStatus::Completed => AgentEvent::Completed {
            agent_id: id.to_string(),
            result: result.clone(),
            iters,
            usd_spent: usd,
        },
        AgentStatus::Stopped if error.as_deref().is_some_and(|e| e.starts_with("stall:")) => {
            AgentEvent::Stalled {
                agent_id: id.to_string(),
                reason: error.clone().unwrap_or_default(),
            }
        }
        AgentStatus::Cancelled => AgentEvent::Cancelled { agent_id: id.to_string() },
        _ => AgentEvent::Failed {
            agent_id: id.to_string(),
            error: error.clone().unwrap_or_else(|| format!("{status:?}")),
            iters,
            usd_spent: usd,
        },
    };
    ctx.event_bus.publish(ev);

    // Stall path: defense triggered (status=Stopped + error starts with "stall:").
    // Fire a lesson directly so future agents learn to avoid the pattern.
    if matches!(status, AgentStatus::Stopped)
        && error
            .as_deref()
            .is_some_and(|e| e.starts_with("stall:"))
    {
        let router = ctx.router.clone();
        let lessons = ctx.lessons.clone();
        let task = ctx.task.clone();
        let err_text = error.clone().unwrap_or_default();
        let kind = ctx.kind;
        let aid = id;
        tokio::spawn(async move {
            warn!(agent_id = %aid, "stall: generating lesson");
            let context = format!(
                "Agent {kind:?} stalled.\nError: {err_text}\nTask (first 400 chars): {t}",
                t = task.chars().take(400).collect::<String>(),
            );
            crate::lessons::generator::fire_and_forget(router, lessons, context, Some(kind));
        });
    }

    // Flywheel: if Completed with a result, fire gates async. Any failure feeds
    // a lesson via fire-and-forget. Bounded cost: at most 1 embed + 1 judge call.
    if matches!(status, AgentStatus::Completed)
        && let Some(output) = result.clone() {
            let router = ctx.router.clone();
            let lessons = ctx.lessons.clone();
            let embed_cache = ctx.embed_cache.clone();
            let spec = ctx.task.clone();
            let owned = ctx.owned_globs.clone();
            let yolo = ctx.yolo_mode;
            let gate_cfg = ctx.gate_config;
            // Snapshot the canonical paths the agent wrote during its run.
            // Map each absolute path to its repo-relative form (after the
            // worktree root) — ownership globs are usually repo-relative.
            let touched: Vec<String> = ctx
                .touched
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default()
                .into_iter()
                .map(|p| relativize(&p, &ctx.worktree_path))
                // `.kira/` is framework-managed runtime state (SDD run artifacts:
                // spec.md / design.md / tasks.json / research-notes.md). It is
                // never part of an agent's `owned_files`, so exclude it from the
                // FileIntegrity check — otherwise every SDD phase agent fails the
                // gate for writing its own legitimate artifact (noise + wasted
                // lesson-generation calls every phase).
                .filter(|p| !p.starts_with(".kira/"))
                .collect();
            let kind = ctx.kind;
            let aid = id;
            // B1 — Pass the writer model used for this agent so L5 can
            // enforce `judge_model ≠ writer_model`.
            let writer_model_for_gate = model.clone();
            let db = ctx.db.clone();
            tokio::spawn(async move {
                // YOLO mode: skip expensive gates (embedding + judge).
                // Only run L1 structural + L2 file integrity for speed.
                let req = RunGatesRequest {
                    spec: spec.clone(),
                    output: output.clone(),
                    owned_paths: owned,
                    touched_paths: touched,
                    estimated_loc: None,
                    actual_loc: None,
                    skip_embedding: yolo,
                    skip_judge: yolo,
                    haystack: String::new(),
                    skip_meta_review: yolo || true, // always skip meta-review
                    worktree_path: String::new(),
                    skip_structural_lint: true,
                    writer_model: writer_model_for_gate.clone(),
                };
                let resp = run_cascade(&router, Some(&embed_cache), &req, gate_cfg).await;

                // P7 — persist judge decisions to redb TBL_EVAL (queryable).
                if let Some(ref db) = db
                    && let Some(judge_result) = resp.results.iter().find(|r| {
                        matches!(r.gate, crate::proto::gate::GateKind::JudgeLlm)
                            && r.outcome != crate::proto::gate::GateOutcome::Skipped
                    }) {
                        let eval_id = uuid::Uuid::now_v7().to_string();
                        let record = serde_json::json!({
                            "eval_id": eval_id,
                            "agent_id": aid.to_string(),
                            "spec": spec.chars().take(2000).collect::<String>(),
                            "output": output.chars().take(2000).collect::<String>(),
                            "score": judge_result.score,
                            "verdict": format!("{:?}", judge_result.outcome).to_lowercase(),
                            "rationale": judge_result.reason.clone().unwrap_or_default(),
                            "writer_model": writer_model_for_gate.clone(),
                            "ts": chrono::Utc::now().to_rfc3339(),
                        });
                        if let Err(e) = db.put(crate::state::TBL_EVAL, &eval_id, &record) {
                            warn!(error = ?e, "failed to write judge decision to TBL_EVAL");
                        }
                    }

                if resp.overall == GateOutcome::Fail {
                    if let Some(failed) =
                        resp.results.iter().find(|r| r.outcome == GateOutcome::Fail)
                    {
                        let reason = failed.reason.clone().unwrap_or_default();
                        warn!(
                            agent_id = %aid,
                            gate = ?failed.gate,
                            reason = %reason,
                            "post-finish gate failed — generating lesson"
                        );
                        let context = format!(
                            "Agent {kind:?} task gate fail.\n\
                             Gate: {gate:?}\nReason: {reason}\n\
                             Spec: {s}\nOutput (first 400 chars): {o}",
                            gate = failed.gate,
                            s = spec.chars().take(400).collect::<String>(),
                            o = output.chars().take(400).collect::<String>(),
                        );
                        generator::fire_and_forget(router, lessons, context, Some(kind));
                    }
                } else {
                    info!(agent_id = %aid, "post-finish gates passed");
                }
            });
        }

    // Memory Extractor: async extraction of persistible fragments from completed runs.
    // Spawns a background task that analyzes the transcript and persists new memories.
    if matches!(status, AgentStatus::Completed) {
        if let Some(ref memory_store) = ctx.memory_store {
            let store = Arc::clone(memory_store);
            let task_text = ctx.task.clone();
            let agent_id = id;
            let run_id = id; // Use agent_id as run_id for now
            let thread_id = id; // TODO: proper thread_id from session
            tokio::spawn(async move {
                debug!(agent_id = %agent_id, "memory extractor: starting async extraction");
                // Simple extraction: persist the task itself as a memory fragment
                // (full LLM-based extraction via ExtractionClassifier is a future enhancement)
                let embed_client = crate::memory::embed::EmbedClient::new(
                    crate::memory::embed::EmbedConfig::default(),
                );
                match embed_client.embed(&task_text).await {
                    Ok(embedding) => {
                        let fragment = crate::memory::store::MemoryFragment {
                            fragment_id: uuid::Uuid::now_v7(),
                            content: task_text.chars().take(500).collect(),
                            embedding,
                            thread_id,
                            run_id,
                            created_at: chrono::Utc::now(),
                            score: 0.7, // default extraction score
                            tier: MemoryTier::default(),
                            access_count: 0,
                            last_accessed: None,
                            consolidated_into: None,
                            promoted_at: None,
                            session_id: None,
                        };
                        if let Err(e) = (*store).persist(&fragment).await {
                            warn!(agent_id = %agent_id, error = %e, "memory extractor: persist failed");
                        } else {
                            debug!(agent_id = %agent_id, "memory extractor: fragment persisted");
                        }
                    }
                    Err(e) => {
                        debug!(agent_id = %agent_id, error = %e, "memory extractor: embedding failed");
                    }
                }
            });
        }
    }
}

/// SC2 — Classify a tool call's risk.
///
/// Returns:
/// - `Some(risk)` for built-in tools or MCP tools registered in the registry
/// - `None` for unknown tool names (treated as Low at call sites)
///
/// `bash` calls whose argv matches a destructive pattern escalate to
/// `Critical` regardless of the static `High` baseline.
async fn classify_tool_risk(
    name: &str,
    arguments: &str,
    mcp: &McpRegistry,
) -> Option<ToolRisk> {
    if let Some(base) = builtin_tool_risk(name) {
        // Special-case bash: parse argv and refine risk by leading command.
        // Critical patterns (rm -rf etc) escalate; readonly commands (grep, ls,
        // find) downgrade to Low so they skip the approval prompt (H8/AP7).
        if name == "bash"
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(arguments)
                && let Some(argv_arr) = parsed.get("argv").and_then(|v| v.as_array()) {
                    let argv: Vec<String> = argv_arr
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect();
                    if bash_argv_is_critical(&argv) {
                        return Some(ToolRisk::Critical);
                    }
                    if let Some(refined) = bash_argv_risk(&argv) {
                        return Some(refined);
                    }
                }
        return Some(base);
    }
    // MCP tool: look up risk in registry.
    if name.starts_with("mcp_") {
        return mcp.tool_risk(name).await;
    }
    None
}

/// Strip the worktree root prefix so the path matches owned globs
/// (which are usually written as `src/content/**`, not absolute).
/// Copy files the agent wrote in its worktree back to the repo root, preserving
/// relative paths. Returns the relative paths actually promoted. Skips anything
/// that escapes the worktree (`..`, absolute) or no longer exists.
fn promote_worktree_files(ctx: &AgentContext) -> Vec<String> {
    // worktree_path = <repo>/.worktrees/wt-X  →  repo_root = two levels up.
    let Some(repo_root) = ctx.worktree_path.parent().and_then(std::path::Path::parent) else {
        return Vec::new();
    };
    let touched: Vec<String> = ctx
        .touched
        .lock()
        .map(|t| t.clone())
        .unwrap_or_default();

    let mut promoted = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for abs in touched {
        let rel = relativize(&abs, &ctx.worktree_path);
        if rel.is_empty() || rel.starts_with('/') || rel.contains("..") {
            continue;
        }
        if !seen.insert(rel.clone()) {
            continue;
        }
        let src = std::path::Path::new(&abs);
        if !src.is_file() {
            continue;
        }
        let dst = repo_root.join(&rel);
        if let Some(parent) = dst.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::copy(src, &dst) {
            Ok(_) => promoted.push(rel),
            Err(e) => warn!(file = %rel, error = %e, "promote: copy failed"),
        }
    }
    promoted
}

fn relativize(absolute: &str, worktree_root: &PathBuf) -> String {
    let root_str = worktree_root.display().to_string();
    if let Some(rest) = absolute.strip_prefix(&root_str) {
        rest.trim_start_matches('/').to_string()
    } else {
        absolute.to_string()
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module, clippy::assertions_on_constants)]
mod stall_tests {
    use super::tool_call_signature;

    #[test]
    fn signature_stable() {
        let a = tool_call_signature("file_write", r#"{"path":"a.md","content":"x"}"#);
        let b = tool_call_signature("file_write", r#"{"path":"a.md","content":"x"}"#);
        assert_eq!(a, b);
    }

    #[test]
    fn signature_differs_on_args() {
        let a = tool_call_signature("file_write", r#"{"path":"a.md"}"#);
        let b = tool_call_signature("file_write", r#"{"path":"b.md"}"#);
        assert_ne!(a, b);
    }

    #[test]
    fn signature_differs_on_name() {
        let a = tool_call_signature("bash", "{}");
        let b = tool_call_signature("file_read", "{}");
        assert_ne!(a, b);
    }

    #[test]
    fn signature_includes_name_prefix() {
        let s = tool_call_signature("bash", "{}");
        assert!(s.starts_with("bash::"));
    }

    #[test]
    fn output_unchanged_threshold_constant_is_safe() {
        // Catch accidental zero/one defaults — too aggressive would trip on
        // first iter, too lax would never fire.
        assert!(super::STALL_OUTPUT_UNCHANGED_ITERS >= 2);
        assert!(super::STALL_OUTPUT_UNCHANGED_ITERS <= 10);
    }

    #[test]
    fn no_progress_secs_constant_is_safe() {
        assert!(super::STALL_NO_PROGRESS_SECS >= 30);
        assert!(super::STALL_NO_PROGRESS_SECS <= 600);
    }

    #[test]
    fn parse_dsml_recovers_tool_calls() {
        // Exact shape captured from deepseek-v4-flash leaking into content.
        let content = "<｜｜DSML｜｜tool_calls>\n\
            <｜｜DSML｜｜invoke name=\"web_fetch\">\n\
            <｜｜DSML｜｜parameter name=\"url\" string=\"true\">https://example.com/a</｜｜DSML｜｜parameter>\n\
            </｜｜DSML｜｜invoke>\n\
            <｜｜DSML｜｜invoke name=\"file_write\">\n\
            <｜｜DSML｜｜parameter name=\"path\" string=\"true\">estudo.md</｜｜DSML｜｜parameter>\n\
            <｜｜DSML｜｜parameter name=\"content\" string=\"true\">oi</｜｜DSML｜｜parameter>\n\
            </｜｜DSML｜｜invoke>\n\
            </｜｜DSML｜｜tool_calls>";
        let calls = super::parse_dsml_tool_calls(content);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "web_fetch");
        assert!(calls[0].arguments.contains("https://example.com/a"));
        assert_eq!(calls[1].name, "file_write");
        let v: serde_json::Value = serde_json::from_str(&calls[1].arguments).unwrap();
        assert_eq!(v["path"], "estudo.md");
        assert_eq!(v["content"], "oi");
    }

    #[test]
    fn truncate_never_splits_multibyte_char() {
        // '─' is 3 bytes. Build a string whose byte 2048 lands inside one.
        let s = format!("{}─────────", "a".repeat(2046));
        // Must not panic and must produce valid UTF-8 ending in the ellipsis.
        let out = super::truncate_on_char_boundary(&s, 2048);
        assert!(out.ends_with('…'));
        assert!(out.len() <= 2048 + "…".len());
        // Short strings pass through unchanged.
        assert_eq!(super::truncate_on_char_boundary("café ─ ok", 2048), "café ─ ok");
    }

    #[test]
    fn parse_dsml_ignores_plain_prose() {
        assert!(super::parse_dsml_tool_calls("This is a normal answer about invoke patterns.").is_empty());
        assert!(super::parse_dsml_tool_calls("").is_empty());
    }

    #[test]
    fn strip_dsml_removes_markup_keeps_prose() {
        let s = "Aqui está a resposta:\n\
            <｜｜DSML｜｜tool_calls>\n\
            <｜｜DSML｜｜invoke name=\"bash\">\n\
            <｜｜DSML｜｜parameter name=\"argv\" string=\"false\">[\"mkdir\"]</｜｜DSML｜｜parameter>\n\
            </｜｜DSML｜｜invoke>\n\
            </｜｜DSML｜｜tool_calls>";
        let cleaned = super::strip_dsml_markup(s);
        assert_eq!(cleaned, "Aqui está a resposta:");
    }

    #[test]
    fn strip_dsml_passthrough_plain() {
        assert_eq!(super::strip_dsml_markup("normal answer"), "normal answer");
    }

    #[test]
    fn strip_dsml_pure_markup_becomes_empty() {
        let s = "<｜｜DSML｜｜tool_calls>\n<｜｜DSML｜｜invoke name=\"bash\">\n</｜｜DSML｜｜tool_calls>";
        assert!(super::strip_dsml_markup(s).is_empty());
    }
}

/// Return the context-token limit for a model name.
fn model_context_limit(models: &ModelsConfig, model: &str) -> usize {
    models
        .entries
        .get(model)
        .map_or(131_072, |e| e.context_limit)
}

/// Token-count the message history using the approximate tokenizer.
fn count_messages_tokens(messages: &[ChatMessageUnified]) -> usize {
    let tokenizer = ApproxTokenizer::default();
    let text: String = messages.iter().map(super::super::llm::types::ChatMessageUnified::content).collect();
    tokenizer.count(&text)
}

/// If message history exceeds ~80 % of the model's context limit, prune
/// oldest non-system messages while preserving the system prompt and the
/// most recent assistant/tool exchanges.
fn compact_if_needed(
    messages: &mut Vec<ChatMessageUnified>,
    models: &ModelsConfig,
    model: &str,
) {
    const COMPACTION_RATIO: f64 = 0.80;
    const MIN_PRESERVE_TAIL: usize = 4; // keep last ~2 assistant/tool turns

    let limit = model_context_limit(models, model);
    let threshold = (limit as f64 * COMPACTION_RATIO) as usize;
    let current = count_messages_tokens(messages);

    if current <= threshold {
        return; // no compaction needed
    }

    // Need at least system (idx 0) + user task (idx 1) + tail to preserve.
    if messages.len() <= MIN_PRESERVE_TAIL + 2 {
        warn!(
            model = %model,
            tokens = current,
            limit,
            "context near limit but history too short to compact safely"
        );
        return;
    }

    // Prune middle messages: keep system (0), user task (1), last N.
    let tail_start = messages.len() - MIN_PRESERVE_TAIL;
    let mut pruned = Vec::with_capacity(2 + MIN_PRESERVE_TAIL);
    pruned.push(messages[0].clone()); // system
    pruned.push(messages[1].clone()); // original user task
    pruned.extend_from_slice(&messages[tail_start..]);

    let new_count = count_messages_tokens(&pruned);
    info!(
        model = %model,
        before_tokens = current,
        after_tokens = new_count,
        removed = messages.len() - pruned.len(),
        "context compacted: pruned oldest messages"
    );
    *messages = pruned;
}

/// P6 — persist the current message history to redb so the agent can
/// resume after a daemon crash.
fn checkpoint_save(db: &crate::state::KiraDb, agent_id: Uuid, messages: &[ChatMessageUnified]) {
    let key = agent_id.to_string();
    if let Ok(json) = serde_json::to_string(messages)
        && let Err(e) = db.put_raw(crate::state::TBL_AGENT_CHECKPOINTS, &key, &json) {
            warn!(error = ?e, agent_id = %agent_id, "checkpoint save failed");
        }
}

/// P6 — load a previously checkpointed message history.
fn checkpoint_load(
    db: &crate::state::KiraDb,
    agent_id: Uuid,
) -> Option<Vec<ChatMessageUnified>> {
    let key = agent_id.to_string();
    let raw = db.list_one(crate::state::TBL_AGENT_CHECKPOINTS, &key).ok()??;
    serde_json::from_str(&raw).ok()
}

/// P6 — remove checkpoint once the agent finishes (success or failure).
fn checkpoint_delete(db: &crate::state::KiraDb, agent_id: Uuid) {
    let key = agent_id.to_string();
    if let Err(e) = db.delete(crate::state::TBL_AGENT_CHECKPOINTS, &key) {
        warn!(error = ?e, agent_id = %agent_id, "checkpoint delete failed");
    }
}

fn approx_cost(pricing: &PricingConfig, model: &str, tokens_in: u32, tokens_out: u32) -> f64 {
    let (i, o) = pricing.price_for(model);
    if i == 0.0 && o == 0.0 {
        return 0.0;
    }
    f64::from(tokens_in).mul_add(i, f64::from(tokens_out) * o) / 1_000_000.0
}

#[cfg(test)]
mod cache_tests {
    use super::{payload_is_error, tool_cache_key};

    #[test]
    fn cache_key_stable_and_distinct() {
        let a = tool_cache_key("file_read", r#"{"path":"a.rs"}"#);
        let a2 = tool_cache_key("file_read", r#"{"path":"a.rs"}"#);
        let b = tool_cache_key("file_read", r#"{"path":"b.rs"}"#);
        let c = tool_cache_key("code_search", r#"{"path":"a.rs"}"#);
        assert_eq!(a, a2, "same name+args → same key");
        assert_ne!(a, b, "different args → different key");
        assert_ne!(a, c, "different tool → different key");
    }

    #[test]
    fn detects_error_payloads() {
        assert!(payload_is_error(r#"{"error":"boom"}"#));
        assert!(!payload_is_error(r#"{"content":"ok","truncated":false}"#));
        assert!(!payload_is_error("not json at all"));
    }
}
