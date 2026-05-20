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
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs,
};
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
use crate::hooks::HookRunner;
use crate::proto::agent::{Agent, AgentKind, AgentStatus, SpawnAgentRequest};
use crate::proto::mailbox::SendRequest;
use crate::proto::ownership::{Claim, OwnershipMode};
use crate::proto::worktree::SpawnRequest as WorktreeSpawnRequest;
use crate::router::Router;
use crate::worktree::WorktreeManager;

const DEFAULT_BUDGET_USD: f64 = 1.00;
const DEFAULT_MAX_ITERS: u32 = 10;

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
    cancellers: Arc<std::sync::Mutex<HashMap<Uuid, oneshot::Sender<()>>>>,
}

impl AgentManager {
    pub fn new(
        router: Router,
        worktrees: WorktreeManager,
        mailbox: MailboxFs,
        ownership: OwnershipRegistry,
        hooks: HookRunner,
    ) -> Self {
        Self {
            store: AgentStore::new(),
            router,
            worktrees,
            mailbox,
            ownership,
            hooks,
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
                .claim(Claim::now(id_str.clone(), patterns, mode))
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

        let ctx = AgentContext {
            id,
            kind,
            tier,
            task: req.task,
            max_iters,
            budget_cap,
            worktree_path,
            worktree_id: worktree_id.clone(),
            store: self.store.clone(),
            router: self.router.clone(),
            mailbox: self.mailbox.clone(),
            ownership: self.ownership.clone(),
            hooks: self.hooks.clone(),
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
    store: AgentStore,
    router: Router,
    mailbox: MailboxFs,
    ownership: OwnershipRegistry,
    hooks: HookRunner,
    cancellers: Arc<std::sync::Mutex<HashMap<Uuid, oneshot::Sender<()>>>>,
}

#[instrument(skip(ctx, cancel_rx), fields(agent_id = %ctx.id, kind = ?ctx.kind))]
async fn agent_loop(ctx: AgentContext, mut cancel_rx: oneshot::Receiver<()>) {
    ctx.store
        .update(ctx.id, |a| a.status = AgentStatus::Active);

    let tools = agent_tools(&ctx.worktree_path.display().to_string());

    let mut messages: Vec<ChatCompletionRequestMessage> = Vec::new();
    // System prompt
    if let Ok(m) = ChatCompletionRequestSystemMessageArgs::default()
        .content(ctx.kind.system_prompt())
        .build()
    {
        messages.push(m.into());
    }
    // User task
    if let Ok(m) = ChatCompletionRequestUserMessageArgs::default()
        .content(ctx.task.clone())
        .build()
    {
        messages.push(m.into());
    }

    let mut iters: u32 = 0;
    let mut usd_spent: f64 = 0.0;
    let mut model_used: Option<String> = None;
    let mut final_result: Option<String> = None;
    let mut error: Option<String> = None;

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
        let (response, _provider, model) = tokio::select! {
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
        if let Some(usage) = response.usage.as_ref() {
            usd_spent += approx_cost(&model, usage.prompt_tokens, usage.completion_tokens);
        }
        let now_iter = Utc::now().to_rfc3339();
        ctx.store.update(ctx.id, |a| {
            a.iters = iters;
            a.usd_spent = usd_spent;
            a.model = Some(model.clone());
            a.last_progress_at = Some(now_iter.clone());
        });

        let choice = match response.choices.into_iter().next() {
            Some(c) => c,
            None => {
                error = Some("LLM returned no choices".into());
                break;
            }
        };
        let assistant_msg = choice.message;

        // Push the assistant message into history (with potential tool_calls).
        let assistant_for_history = {
            let mut builder = ChatCompletionRequestAssistantMessageArgs::default();
            if let Some(c) = assistant_msg.content.as_ref() {
                builder.content(c.clone());
            }
            if let Some(tcs) = assistant_msg.tool_calls.as_ref() {
                builder.tool_calls(tcs.clone());
            }
            match builder.build() {
                Ok(m) => m.into(),
                Err(e) => {
                    error = Some(format!("build assistant message: {e}"));
                    break;
                }
            }
        };
        messages.push(assistant_for_history);

        // Decide next step.
        let tool_calls = assistant_msg.tool_calls.unwrap_or_default();
        if tool_calls.is_empty() {
            // Content-only response. Treat as final.
            final_result = assistant_msg.content;
            break;
        }

        // Execute each tool call sequentially. Append tool result messages.
        let mut break_outer_with_done: Option<String> = None;
        for tc in tool_calls {
            let ChatCompletionMessageToolCall { id, function, .. } = tc;
            let name = function.name.clone();
            let arguments = function.arguments.clone();
            debug!(tool = %name, args_len = arguments.len(), "agent invoked tool");

            let result = dispatch_with_hooks(
                &name,
                &arguments,
                &ctx.worktree_path,
                &ctx.hooks,
                &ctx.id.to_string(),
                &ctx.worktree_id,
            )
            .await;

            // Tool result message
            match ChatCompletionRequestToolMessageArgs::default()
                .content(result.payload.clone())
                .tool_call_id(id.clone())
                .build()
            {
                Ok(m) => messages.push(m.into()),
                Err(e) => {
                    error = Some(format!("build tool message: {e}"));
                    break_outer_with_done = None;
                    break;
                }
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
        if error.is_some() {
            break;
        }
    }

    let status = if error.is_some() {
        AgentStatus::Failed
    } else if final_result.is_some() {
        AgentStatus::Completed
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
