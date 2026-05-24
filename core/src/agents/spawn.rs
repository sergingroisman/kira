//! Agent spawning + iterative agent_loop (Sem 10).
//!
//! Loop contract:
//!   1. Initial messages = [system_prompt, user_task].
//!   2. Each iteration:
//!      - Router.route_chat_with_tools(messages, tools)
//!      - If response has `tool_calls`: append assistant message (with
//!        tool_calls) + execute each tool + append `tool` result messages.
//!      - If response is content-only (no tool_calls): treat as final.
//!      - If "done" tool called: extract `final_result`, break.
//!   3. Stop on: done | content-only response | max_iters | budget cap | cancel.

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use crate::agents::mailbox::MailboxFs;
use crate::agents::ownership::OwnershipRegistry;
use crate::agents::store::Store as AgentStore;
use crate::agents::tools::{agent_tools, dispatch_with_hooks};
use crate::gates::run_cascade;
use crate::hooks::HookRunner;
use crate::lessons::{generator, render_inject_snippet, LessonRegistry};
use crate::llm::embed_cache::EmbedCache;
use crate::llm::types::{ChatMessageUnified, ToolCall};
use crate::proto::gate::{GateOutcome, RunGatesRequest};
use crate::proto::agent::{Agent, AgentKind, AgentStatus, SpawnAgentRequest};
use crate::proto::mailbox::SendRequest;
use crate::proto::ownership::{Claim, OwnershipMode};
use crate::proto::worktree::SpawnRequest as WorktreeSpawnRequest;
use crate::router::Router;
use crate::worktree::WorktreeManager;

const DEFAULT_BUDGET_USD: f64 = 1.00;
const DEFAULT_MAX_ITERS: u32 = 10;

/// Stall detection (G6) — 3 signals:
///
/// 1. **SameToolLoop**: same `(tool, args)` repeats this many times in a row.
/// 2. **OutputUnchanged**: hash of `(assistant_content + sorted tool sigs)`
///    unchanged across this many consecutive iterations. Catches "agent rewrites
///    same plan over and over without producing new evidence" pattern.
/// 3. **NoProgress**: wall-clock seconds without any useful output advance.
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
    cancellers: Arc<std::sync::Mutex<HashMap<Uuid, oneshot::Sender<()>>>>,
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
            cancellers: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn list(&self) -> Vec<Agent> {
        self.store.list()
    }

    pub fn get(&self, id: Uuid) -> Option<Agent> {
        self.store.get(id)
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

    /// Watchdog hook: flip a non-terminal agent to `Stale`.
    /// No-op if the agent is already in a terminal state or missing.
    pub fn mark_stale(&self, id: Uuid) {
        self.store.update(id, |a| {
            if matches!(a.status, AgentStatus::Active | AgentStatus::Spawning) {
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
        let (worktree_id, worktree_path) = match req.worktree_id.as_deref() {
            Some(s) => {
                let id = Uuid::parse_str(s).context("invalid worktree_id")?;
                let wt = self
                    .worktrees
                    .get(id)
                    .with_context(|| format!("worktree not found: {id}"))?;
                (id.to_string(), PathBuf::from(wt.path))
            }
            None => {
                let wt = self
                    .worktrees
                    .spawn(WorktreeSpawnRequest {
                        base: req.base.unwrap_or_else(|| "main".to_string()),
                        agent_kind: Some(format!("{kind:?}").to_lowercase()),
                    })
                    .await
                    .context("auto-spawn worktree failed")?;
                (wt.id.clone(), PathBuf::from(wt.path))
            }
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
            id: id_str.clone(),
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

        let ctx = AgentContext {
            id,
            kind,
            tier,
            task: req.task,
            max_iters,
            budget_cap,
            worktree_path,
            worktree_id: worktree_id.clone(),
            lesson_snippet,
            owned_globs: patterns.clone(),
            touched: Arc::new(std::sync::Mutex::new(Vec::new())),
            yolo_mode: yolo,
            gate_config: self.gate_config,
            store: self.store.clone(),
            router: self.router.clone(),
            mailbox: self.mailbox.clone(),
            ownership: self.ownership.clone(),
            hooks: self.hooks.clone(),
            lessons: self.lessons.clone(),
            embed_cache: self.embed_cache.clone(),
            cancellers: Arc::clone(&self.cancellers),
        };
        tokio::spawn(agent_loop(ctx, cancel_rx));

        info!(id = %id, kind = ?kind, worktree_id = %worktree_id, "agent spawned");
        Ok(agent)
    }
}

struct AgentContext {
    id: Uuid,
    kind: AgentKind,
    tier: String,
    task: String,
    max_iters: u32,
    budget_cap: f64,
    worktree_path: PathBuf,
    worktree_id: String,
    /// Lesson snippet pre-built at spawn time, appended to the kind's system prompt.
    lesson_snippet: String,
    /// Owned globs the agent claimed (used by post-finish gates).
    owned_globs: Vec<String>,
    /// Canonical paths the agent actually wrote to (populated by file_write
    /// dispatches). Read by post-finish gates for L2 file_integrity.
    touched: Arc<std::sync::Mutex<Vec<String>>>,
    /// YOLO mode: when true, skip expensive gates (embedding, judge, meta-review).
    /// Used for quick tasks where speed > thoroughness.
    yolo_mode: bool,
    /// Gate thresholds (loaded from tokens.toml).
    gate_config: crate::gates::GateConfig,
    store: AgentStore,
    router: Router,
    mailbox: MailboxFs,
    ownership: OwnershipRegistry,
    hooks: HookRunner,
    lessons: LessonRegistry,
    embed_cache: EmbedCache,
    cancellers: Arc<std::sync::Mutex<HashMap<Uuid, oneshot::Sender<()>>>>,
}

#[instrument(skip(ctx, cancel_rx), fields(agent_id = %ctx.id, kind = ?ctx.kind))]
async fn agent_loop(ctx: AgentContext, mut cancel_rx: oneshot::Receiver<()>) {
    ctx.store
        .update(ctx.id, |a| a.status = AgentStatus::Active);

    let tools = agent_tools(&ctx.worktree_path.display().to_string());

    let mut messages: Vec<ChatMessageUnified> = Vec::new();
    let mut system = ctx.kind.system_prompt().to_string();
    if !ctx.lesson_snippet.is_empty() {
        system.push_str(&ctx.lesson_snippet);
    }
    messages.push(ChatMessageUnified::System { content: system });

    messages.push(ChatMessageUnified::User {
        content: ctx.task.clone(),
    });

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

        // Race LLM call vs cancellation.
        let route = ctx
            .router
            .route_chat_with_tools(&ctx.tier, None, messages.clone(), tools.clone());
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
        usd_spent += approx_cost(&model, output.tokens_in, output.tokens_out);
        let now_iter = Utc::now().to_rfc3339();
        ctx.store.update(ctx.id, |a| {
            a.iters = iters;
            a.usd_spent = usd_spent;
            a.model = Some(model.clone());
            a.last_progress_at = Some(now_iter.clone());
        });

        // Push the assistant message into history.
        messages.push(ChatMessageUnified::Assistant {
            content: Some(output.content.clone()),
            tool_calls: output.tool_calls.clone(),
            reasoning_content: output.reasoning_content.clone(),
        });

        // Decide next step.
        let tool_calls = output.tool_calls.clone();
        if tool_calls.is_empty() {
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

            let result = dispatch_with_hooks(
                name,
                arguments,
                &ctx.worktree_path,
                &ctx.hooks,
                &ctx.id.to_string(),
                &ctx.worktree_id,
                Some(&ctx.touched),
            )
            .await;

            // Tool result message
            messages.push(ChatMessageUnified::Tool {
                call_id: id.clone(),
                content: result.payload.clone(),
            });
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

        // ── OutputUnchanged stall check ────────────────────────────────────
        // Hash this iter's "useful output": assistant content + all tool sigs
        // pushed this iter. If unchanged across N consecutive iters → stall.
        let content_snippet = messages
            .last()
            .and_then(|_| Some(""))
            .unwrap_or("");
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
                "stall: agent output (content + tool sigs) unchanged for {} iterations",
                STALL_OUTPUT_UNCHANGED_ITERS
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
        .map(|e| e.starts_with("stall:"))
        .unwrap_or(false);
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

    info!(agent_id = %id, ?status, iters, usd, "agent finished");

    // Stall path: defense triggered (status=Stopped + error starts with "stall:").
    // Fire a lesson directly so future agents learn to avoid the pattern.
    if matches!(status, AgentStatus::Stopped)
        && error
            .as_deref()
            .map(|e| e.starts_with("stall:"))
            .unwrap_or(false)
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
    if matches!(status, AgentStatus::Completed) {
        if let Some(output) = result.clone() {
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
                };
                let resp = run_cascade(&router, Some(&embed_cache), &req, gate_cfg).await;
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

fn approx_cost(model: &str, tokens_in: u32, tokens_out: u32) -> f64 {
    let (i, o) = match model {
        "qwen-max" => (1.60, 6.40),
        "qwen-plus" => (0.40, 1.20),
        "qwen-flash" => (0.15, 0.60),
        "deepseek-v4-pro" => (0.43, 0.87),
        "deepseek-v4-flash" => (0.14, 0.28),
        _ => return 0.0,
    };
    (f64::from(tokens_in) * i + f64::from(tokens_out) * o) / 1_000_000.0
}
