//! Agent spawning + walking-skeleton agent_loop.
//!
//! v0 contract (single-shot):
//!   1. spawn() builds/reuses a worktree, claims default ownership for the
//!      kind, registers the agent in the store, and `tokio::spawn`s `agent_loop`.
//!   2. agent_loop sends one chat request via the Router using the kind's
//!      system prompt + the user's task.
//!   3. Result is written to the store, sent to "core"'s mailbox as kind="result".
//!   4. Agent status flips to Completed/Failed/Cancelled.
//!
//! Sem 10+ adds: tool calls dispatch, iterative loop, heartbeats, judge eval
//! writes, plan/act gating.

use anyhow::{Context, Result};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use crate::agents::mailbox::MailboxFs;
use crate::agents::ownership::OwnershipRegistry;
use crate::agents::store::Store as AgentStore;
use crate::proto::agent::{Agent, AgentKind, AgentStatus, SpawnAgentRequest};
use crate::proto::chat::{ChatMessage, ChatRequest};
use crate::proto::mailbox::SendRequest;
use crate::proto::ownership::{Claim, OwnershipMode};
use crate::proto::worktree::SpawnRequest as WorktreeSpawnRequest;
use crate::router::Router;
use crate::worktree::WorktreeManager;

const DEFAULT_BUDGET_USD: f64 = 1.00;
const DEFAULT_MAX_ITERS: u32 = 1; // walking skeleton — Sem 10 raises to 50

/// Default ownership patterns per agent kind (plan T7).
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
    cancellers: Arc<std::sync::Mutex<std::collections::HashMap<Uuid, oneshot::Sender<()>>>>,
}

impl AgentManager {
    pub fn new(
        router: Router,
        worktrees: WorktreeManager,
        mailbox: MailboxFs,
        ownership: OwnershipRegistry,
    ) -> Self {
        Self {
            store: AgentStore::new(),
            router,
            worktrees,
            mailbox,
            ownership,
            cancellers: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn list(&self) -> Vec<Agent> {
        self.store.list()
    }

    pub fn get(&self, id: Uuid) -> Option<Agent> {
        self.store.get(id)
    }

    /// Returns true if the agent existed and was signaled to cancel.
    pub fn cancel(&self, id: Uuid) -> bool {
        let mut map = self.cancellers.lock().expect("cancellers poisoned");
        if let Some(tx) = map.remove(&id) {
            let _ = tx.send(());
            true
        } else {
            false
        }
    }

    /// Spawn a fresh agent. Creates a worktree if `req.worktree_id` is absent.
    /// Returns the registered `Agent` immediately; the loop runs in the
    /// background (`tokio::spawn`).
    #[instrument(skip(self, req), fields(kind = ?req.kind))]
    pub async fn spawn(&self, req: SpawnAgentRequest) -> Result<Agent> {
        let kind = req.kind;
        let tier = kind.default_tier().to_string();
        let max_iters = req.max_iters.unwrap_or(DEFAULT_MAX_ITERS);
        let budget_cap = req.budget_usd.unwrap_or(DEFAULT_BUDGET_USD);

        // Resolve worktree: reuse if provided, else spawn fresh.
        let worktree_id = match req.worktree_id.as_deref() {
            Some(s) => {
                let id = Uuid::parse_str(s).context("invalid worktree_id")?;
                self.worktrees
                    .get(id)
                    .with_context(|| format!("worktree not found: {id}"))?;
                id.to_string()
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
                wt.id
            }
        };

        let id = Uuid::now_v7();
        let id_str = id.to_string();

        // Claim ownership scoped to this agent kind.
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
            created_at: now,
            finished_at: None,
        };
        self.store.insert(id, agent.clone());

        // Register cancel channel.
        let (cancel_tx, cancel_rx) = oneshot::channel();
        self.cancellers
            .lock()
            .expect("cancellers poisoned")
            .insert(id, cancel_tx);

        // Spawn the loop. Clone what it needs (Arc-style).
        let ctx = AgentContext {
            id,
            kind,
            tier,
            task: req.task,
            max_iters,
            budget_cap,
            store: self.store.clone(),
            router: self.router.clone(),
            mailbox: self.mailbox.clone(),
            ownership: self.ownership.clone(),
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
    // Used by Sem 10's iterative loop; v0 single-shot ignores them.
    #[allow(dead_code)]
    max_iters: u32,
    #[allow(dead_code)]
    budget_cap: f64,
    store: AgentStore,
    router: Router,
    mailbox: MailboxFs,
    ownership: OwnershipRegistry,
    cancellers: Arc<std::sync::Mutex<std::collections::HashMap<Uuid, oneshot::Sender<()>>>>,
}

#[instrument(skip(ctx, cancel_rx), fields(agent_id = %ctx.id, kind = ?ctx.kind))]
async fn agent_loop(ctx: AgentContext, mut cancel_rx: oneshot::Receiver<()>) {
    let id = ctx.id;
    ctx.store
        .update(id, |a| a.status = AgentStatus::Active);

    // Walking-skeleton: 1 LLM call (max_iters=1 default). Loop kept for Sem 10.
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: ctx.kind.system_prompt().to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: ctx.task.clone(),
        },
    ];

    let req = ChatRequest {
        tier: ctx.tier.clone(),
        model: None,
        messages: messages.clone(),
    };

    // Race the LLM call against cancellation.
    let route = ctx.router.route_chat(&req, &messages);
    tokio::select! {
        _ = &mut cancel_rx => {
            warn!(agent_id = %id, "cancelled before LLM completed");
            finish(&ctx, AgentStatus::Cancelled, None, Some("cancelled".into()), None, 0.0).await;
            return;
        }
        result = route => {
            match result {
                Ok((resp, _provider, model)) => {
                    let usd = match resp.usage.as_ref() {
                        Some(u) => approx_cost(&model, u.prompt_tokens, u.completion_tokens),
                        None => 0.0,
                    };
                    let content = resp
                        .choices
                        .into_iter()
                        .next()
                        .and_then(|c| c.message.content)
                        .unwrap_or_default();
                    finish(&ctx, AgentStatus::Completed, Some(content), None, Some(model), usd).await;
                }
                Err(e) => {
                    error!(agent_id = %id, error = ?e, "agent LLM call failed");
                    finish(&ctx, AgentStatus::Failed, None, Some(e.to_string()), None, 0.0).await;
                }
            }
        }
    }
}

async fn finish(
    ctx: &AgentContext,
    status: AgentStatus,
    result: Option<String>,
    error: Option<String>,
    model: Option<String>,
    usd: f64,
) {
    let id = ctx.id;
    let finished_at = Utc::now().to_rfc3339();
    ctx.store.update(id, |a| {
        a.status = status;
        a.result = result.clone();
        a.error = error.clone();
        a.model = model.clone();
        a.iters = 1;
        a.usd_spent = usd;
        a.finished_at = Some(finished_at.clone());
    });

    // Best-effort: send result to "core" mailbox.
    let payload = serde_json::json!({
        "status": format!("{status:?}"),
        "result": result,
        "error": error,
        "model": model,
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

    // Release ownership claim.
    ctx.ownership.release(&id.to_string());

    // Drop canceller (so cancel() is a no-op after finish).
    ctx.cancellers
        .lock()
        .expect("cancellers poisoned")
        .remove(&id);

    info!(agent_id = %id, ?status, usd, "agent finished");
}

/// Lightweight cost estimate — duplicates `router::budget::price_per_million`
/// to avoid a circular dep. Kept in sync with the budget table.
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
