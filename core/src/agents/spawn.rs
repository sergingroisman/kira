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
use tokio::sync::oneshot;
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
use crate::proto::agent::{Agent, AgentEvent, AgentKind, AgentStatus, SpawnAgentRequest};
use crate::proto::mailbox::SendRequest;
use crate::proto::ownership::{Claim, OwnershipMode};
use crate::proto::worktree::SpawnRequest as WorktreeSpawnRequest;
use crate::mcp::McpRegistry;
use crate::router::Router;
use crate::safety::approval::{ApprovalGate, ApprovalVerdict};
use crate::safety::delimiters::{wrap_system, wrap_tool_output, wrap_user_data, UNTRUSTED_DATA_NOTICE};
use crate::config::{ModelsConfig, PricingConfig};
use crate::safety::permission::{PermissionRuleset, RuleAction};
use crate::safety::risk::{bash_argv_is_critical, builtin_tool_risk, ToolRisk};
use crate::worktree::WorktreeManager;

const DEFAULT_BUDGET_USD: f64 = 1.00;
const DEFAULT_MAX_ITERS: u32 = 10;

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
    db: Option<crate::state::KiraDb>,
    /// Agent lifecycle event bus — subscribe to receive async agent state changes.
    pub events: EventBus<AgentEvent>,
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
            db,
            events: EventBus::new(),
        }
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
        let tier = kind.default_tier().to_string();
        let max_iters = req.max_iters.unwrap_or(DEFAULT_MAX_ITERS);
        let budget_cap = req.budget_usd.unwrap_or(DEFAULT_BUDGET_USD);

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
        };
        self.store.insert(id, agent.clone());

        let (cancel_tx, cancel_rx) = oneshot::channel();
        self.cancellers
            .lock()
            .expect("cancellers poisoned")
            .insert(id, cancel_tx);

        // Inject active lessons matching this kind into the agent's system prompt.
        let lesson_snippet =
            render_inject_snippet(&self.lessons.inject_for(kind));

        let yolo = req.yolo_mode.unwrap_or(false);
        let background = req.background.unwrap_or(false);

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
            approval_timeout_secs: 60,
            db: self.db.clone(),
            event_bus: self.events.clone(),
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
    // SC1 — append explicit boundary notice so the LLM knows which regions
    // are untrusted data vs authoritative instructions.
    system.push_str(UNTRUSTED_DATA_NOTICE);
    // SC1 — wrap the entire system prompt in <SYSTEM_INSTRUCTIONS> so it
    // is visually / structurally distinct from <USER_DATA> and <TOOL_OUTPUT>
    // blocks injected later in the loop.
    let system_text = wrap_system(&system);
    ctx.transcript.write_prompt(&system_text);
    messages.push(ChatMessageUnified::System {
        content: system_text,
    });
    ctx.transcript.append(messages.last().unwrap());

    // SC1 — wrap user task in <USER_DATA> delimiters.
    messages.push(ChatMessageUnified::User {
        content: wrap_user_data(&ctx.task),
    });
    ctx.transcript.append(messages.last().unwrap());

    let mut iters: u32 = 0;
    let mut usd_spent: f64 = 0.0;
    let mut model_used: Option<String> = None;
    let mut final_result: Option<String> = None;
    let mut error: Option<String> = None;
    // Stall detector — tracks recent tool call signatures.
    let mut tool_history: std::collections::VecDeque<String> =
        std::collections::VecDeque::with_capacity(TOOL_HISTORY_CAP);
    // Output-unchanged stall: hash of (content + sorted tool sigs) per iter.
    let mut output_history: std::collections::VecDeque<String> =
        std::collections::VecDeque::with_capacity(STALL_OUTPUT_UNCHANGED_ITERS);
    // No-progress stall: wall-clock of last useful output change.
    let mut last_progress_instant: std::time::Instant = std::time::Instant::now();

    loop {
        if iters >= ctx.max_iters {
            warn!(agent_id = %ctx.id, iters, "max_iters reached");
            error = Some(format!("max_iters reached ({} iterations)", ctx.max_iters));
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

        // Race LLM call vs cancellation.
        let route = ctx
            .router
            .route_chat_with_tools(&ctx.tier, None, messages.clone(), tools.clone(), None);
        let (output, _provider, model) = tokio::select! {
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
        usd_spent += approx_cost(&ctx.pricing, &model, output.tokens_in, output.tokens_out);
        let now_iter = Utc::now().to_rfc3339();
        ctx.store.update(ctx.id, |a| {
            a.iters = iters;
            a.usd_spent = usd_spent;
            a.model = Some(model.clone());
            a.last_progress_at = Some(now_iter.clone());
        });
        ctx.event_bus.publish(AgentEvent::Heartbeat {
            agent_id: ctx.id.to_string(),
            iters,
            usd_spent,
        });

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
            const SUMMARY_MIN_CHARS: usize = 200;
            const CONTINUATION_PROMPT: &str =
                "Please continue and provide a more detailed and comprehensive response. \
                 Include specific findings, reasoning, and concrete deliverables.";

            if output.content.len() < SUMMARY_MIN_CHARS
                && iters < ctx.max_iters
                && usd_spent < ctx.budget_cap
            {
                debug!(
                    agent_id = %ctx.id,
                    content_len = output.content.len(),
                    "short response — sending continuation prompt"
                );
                messages.push(ChatMessageUnified::User {
                    content: CONTINUATION_PROMPT.to_string(),
                });
                ctx.transcript.append(messages.last().unwrap());
                continue;
            }
            // Content-only response. Treat as final.
            final_result = Some(output.content);
            break;
        }

        // Execute each tool call sequentially. Append tool result messages.
        let mut break_outer_with_done: Option<String> = None;
        let mut break_outer_with_stall = false;
        for tc in &tool_calls {
            let ToolCall { id, name, arguments } = tc;
            debug!(tool = %name, args_len = arguments.len(), "agent invoked tool");

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

            let result = dispatch_with_hooks(
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

            // Tool result message — SC1 wrap payload as untrusted data.
            messages.push(ChatMessageUnified::Tool {
                call_id: id.clone(),
                content: wrap_tool_output(name, &result.payload),
            });
            ctx.transcript.append(messages.last().unwrap());
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
        // Special-case bash: parse argv and check for critical patterns.
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
fn relativize(absolute: &str, worktree_root: &PathBuf) -> String {
    let root_str = worktree_root.display().to_string();
    if let Some(rest) = absolute.strip_prefix(&root_str) {
        rest.trim_start_matches('/').to_string()
    } else {
        absolute.to_string()
    }
}

#[cfg(test)]
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
