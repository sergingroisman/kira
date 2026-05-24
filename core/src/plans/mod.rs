//! Plan/Act split: planner emits Plan{steps}; human approves; executor agent
//! runs the plan as its task (free-form for v0; structured step-by-step in v0.5).
//!
//! v0 flow:
//!   1. `draft(goal, kind)` → spawn planner Heavy-tier LLM call → parse JSON
//!      → store as `Draft`, return Plan.
//!   2. `approve(plan_id)` → spawn an AgentManager.spawn() with task =
//!      "GOAL: <goal>\nPLAN:\n<steps serialized>\nExecute the plan."
//!      Plan flips to `Approved` then `Executing`. Bound to the agent.
//!   3. `cancel(plan_id)` → if executor exists, agents.cancel(agent_id).
//!   4. TTL expiry handled lazily (computed in `is_expired()`).

pub mod store;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};

use crate::llm::types::ChatMessageUnified;
use serde_json::Value;
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::agents::AgentManager;
use crate::proto::agent::{AgentKind, SpawnAgentRequest};
use crate::proto::plan::{Plan, PlanScope, PlanStatus, PlanStep};
use crate::router::Router;

use self::store::Store;

const DEFAULT_TTL_SECS: u64 = 3600;
const PLANNER_SYSTEM_PROMPT: &str = include_str!("prompts/planner.md");

#[derive(Clone)]
pub struct PlanManager {
    store: Store,
    router: Router,
    agents: AgentManager,
}

impl PlanManager {
    pub fn new(
        router: Router,
        agents: AgentManager,
        db: Option<crate::state::KiraDb>,
    ) -> Self {
        let store = match db {
            Some(d) => Store::with_db(d),
            None => Store::new(),
        };
        Self {
            store,
            router,
            agents,
        }
    }

    pub fn list(&self) -> Vec<Plan> {
        self.store.list()
    }

    pub fn get(&self, id: Uuid) -> Option<Plan> {
        let mut plan = self.store.get(id)?;
        if plan.status == PlanStatus::Draft && is_expired(&plan) {
            plan.status = PlanStatus::Expired;
            self.store
                .update(id, |p| p.status = PlanStatus::Expired);
            return Some(plan);
        }
        // Project executor state onto plan status when approved.
        if matches!(plan.status, PlanStatus::Approved | PlanStatus::Executing) {
            if let Some(exec_id) = plan.executor_agent_id.as_deref() {
                if let Ok(uuid) = Uuid::parse_str(exec_id) {
                    if let Some(agent) = self.agents.get(uuid) {
                        use crate::proto::agent::AgentStatus;
                        let new_status = match agent.status {
                            AgentStatus::Active | AgentStatus::Spawning => {
                                PlanStatus::Executing
                            }
                            AgentStatus::Completed => PlanStatus::Done,
                            AgentStatus::Failed => PlanStatus::Failed,
                            AgentStatus::Cancelled => PlanStatus::Cancelled,
                            AgentStatus::Stopped => PlanStatus::Failed,
                            AgentStatus::Stale => PlanStatus::Failed,
                        };
                        if new_status != plan.status {
                            plan.status = new_status;
                            let new_status_copy = new_status;
                            let finished = matches!(
                                new_status_copy,
                                PlanStatus::Done
                                    | PlanStatus::Failed
                                    | PlanStatus::Cancelled
                            );
                            let now = if finished {
                                Some(chrono::Utc::now().to_rfc3339())
                            } else {
                                None
                            };
                            self.store.update(id, |p| {
                                p.status = new_status_copy;
                                if let Some(n) = &now {
                                    p.finished_at = Some(n.clone());
                                }
                                if matches!(new_status_copy, PlanStatus::Failed)
                                    && p.error.is_none()
                                {
                                    p.error = agent.error.clone();
                                }
                            });
                            if let Some(n) = now {
                                plan.finished_at = Some(n);
                            }
                        }
                    }
                }
            }
        }
        Some(plan)
    }

    /// Spawn the planner (Heavy tier), parse the JSON, register as Draft.
    #[instrument(skip(self), fields(kind = ?kind))]
    pub async fn draft(
        &self,
        goal: String,
        kind: AgentKind,
        ttl_secs: Option<u64>,
    ) -> Result<Plan> {
        let messages: Vec<ChatMessageUnified> = vec![
            ChatMessageUnified::System {
                content: PLANNER_SYSTEM_PROMPT.to_string(),
            },
            ChatMessageUnified::User {
                content: format!(
                    "Goal: {goal}\nTarget agent kind: {kind_str}\nEmit the plan as JSON now.",
                    kind_str = format!("{kind:?}").to_lowercase(),
                ),
            },
        ];

        let (response, _provider, model) = self
            .router
            .route_chat_with_tools("heavy", None, messages, Vec::new(), None)
            .await
            .context("planner LLM call failed")?;

        let raw = response.content;

        let (scope, steps) =
            parse_planner_output(&raw).context("could not parse planner output")?;

        let id = Uuid::now_v7();
        let now = Utc::now();
        let ttl = ChronoDuration::seconds(ttl_secs.unwrap_or(DEFAULT_TTL_SECS) as i64);
        let plan = Plan {
            id: id.to_string(),
            goal,
            agent_kind: kind,
            scope,
            steps,
            status: PlanStatus::Draft,
            created_at: now.to_rfc3339(),
            expires_at: (now + ttl).to_rfc3339(),
            approved_at: None,
            finished_at: None,
            executor_agent_id: None,
            error: None,
        };
        self.store.insert(id, plan.clone());
        info!(plan_id = %id, model = %model, steps = plan.steps.len(), "plan drafted");
        Ok(plan)
    }

    /// Approve a Draft plan: spawn executor agent + flip to Approved/Executing.
    #[instrument(skip(self), fields(plan_id = %id))]
    pub async fn approve(&self, id: Uuid) -> Result<(Plan, String)> {
        let plan = self.store.get(id).context("plan not found")?;
        match plan.status {
            PlanStatus::Draft => {}
            other => anyhow::bail!("plan is {:?}, only Draft can be approved", other),
        }
        if is_expired(&plan) {
            self.store
                .update(id, |p| p.status = PlanStatus::Expired);
            anyhow::bail!("plan expired");
        }

        // Serialize the plan as the agent's task.
        let mut task = String::new();
        task.push_str("GOAL: ");
        task.push_str(&plan.goal);
        task.push_str("\n\nPLAN (numbered steps):\n");
        for s in &plan.steps {
            task.push_str(&format!(
                "{idx}. {desc}",
                idx = s.idx,
                desc = s.description
            ));
            if let Some(t) = s.tool.as_deref() {
                task.push_str(&format!(" [tool: {t}]"));
            }
            if !s.owned_files.is_empty() {
                task.push_str(&format!(" [writes: {}]", s.owned_files.join(", ")));
            }
            task.push('\n');
        }
        task.push_str(
            "\nExecute the plan. Call done(final_result) when every step has completed.",
        );

        // Derive executor budget from plan scope. Quick = no-frills, Complex = generous.
        let scope_iters = match plan.scope {
            PlanScope::Quick => 3,
            PlanScope::Small => 5,
            PlanScope::Medium => 10,
            PlanScope::Large => 15,
            PlanScope::Complex => 20,
        };
        let derived_max_iters =
            ((plan.steps.len() as u32 + 3).max(scope_iters)).min(scope_iters * 2);

        let agent = self
            .agents
            .spawn(SpawnAgentRequest {
                kind: plan.agent_kind,
                task,
                worktree_id: None,
                base: None,
                budget_usd: None,
                max_iters: Some(derived_max_iters),
                yolo_mode: None,
            })
            .await
            .context("executor agent spawn failed")?;

        let now = Utc::now().to_rfc3339();
        self.store.update(id, |p| {
            p.status = PlanStatus::Approved;
            p.approved_at = Some(now.clone());
            p.executor_agent_id = Some(agent.id.clone());
        });
        info!(plan_id = %id, executor = %agent.id, "plan approved + executor spawned");
        let updated = self.store.get(id).unwrap();
        Ok((updated, agent.id))
    }

    /// Cancel a plan. If the executor is running, the agent is also cancelled.
    pub fn cancel(&self, id: Uuid) -> bool {
        let Some(plan) = self.store.get(id) else {
            return false;
        };
        if matches!(
            plan.status,
            PlanStatus::Done | PlanStatus::Cancelled | PlanStatus::Failed | PlanStatus::Expired
        ) {
            return false;
        }
        if let Some(agent_id) = plan.executor_agent_id.as_deref() {
            if let Ok(aid) = Uuid::parse_str(agent_id) {
                self.agents.cancel(aid);
            }
        }
        let now = Utc::now().to_rfc3339();
        self.store.update(id, |p| {
            p.status = PlanStatus::Cancelled;
            p.finished_at = Some(now);
        });
        true
    }
}



fn is_expired(plan: &Plan) -> bool {
    let Some(expires) = DateTime::parse_from_rfc3339(&plan.expires_at).ok() else {
        return false;
    };
    Utc::now() > expires.with_timezone(&Utc)
}

/// Parse the planner LLM output into (scope, steps). Tolerates ```json
/// fences and surrounding whitespace. Defaults scope to Medium if absent.
fn parse_planner_output(raw: &str) -> Result<(PlanScope, Vec<PlanStep>)> {
    let cleaned = strip_code_fences(raw);
    let value: Value = serde_json::from_str(cleaned.trim())
        .with_context(|| format!("planner output not JSON: {}", short_preview(&cleaned)))?;

    let scope = match value.get("scope").and_then(|s| s.as_str()) {
        Some("quick") => PlanScope::Quick,
        Some("small") => PlanScope::Small,
        Some("medium") => PlanScope::Medium,
        Some("large") => PlanScope::Large,
        Some("complex") => PlanScope::Complex,
        _ => PlanScope::Medium,
    };

    let steps_val = value
        .get("steps")
        .context("planner JSON missing 'steps' field")?;
    let raw_steps: Vec<Value> = serde_json::from_value(steps_val.clone())
        .context("'steps' is not an array")?;
    if raw_steps.is_empty() {
        anyhow::bail!("planner emitted zero steps");
    }
    let mut out = Vec::with_capacity(raw_steps.len());
    for (i, v) in raw_steps.into_iter().enumerate() {
        let step: PlanStep = match serde_json::from_value::<PlanStep>(v.clone()) {
            Ok(s) => s,
            Err(_) => PlanStep {
                idx: (i + 1) as u32,
                description: v
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("(no description)")
                    .to_string(),
                tool: v.get("tool").and_then(|t| t.as_str()).map(str::to_string),
                rationale: v
                    .get("rationale")
                    .and_then(|r| r.as_str())
                    .map(str::to_string),
                owned_files: v
                    .get("owned_files")
                    .and_then(|f| f.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                estimated_loc: v
                    .get("estimated_loc")
                    .and_then(|e| e.as_u64())
                    .map(|n| n as u32),
            },
        };
        out.push(step);
    }
    Ok((scope, out))
}

fn strip_code_fences(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.starts_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() >= 2 {
            let body_start = if lines[0].starts_with("```json") || lines[0] == "```" {
                1
            } else {
                0
            };
            let body_end = if *lines.last().unwrap_or(&"") == "```" {
                lines.len() - 1
            } else {
                lines.len()
            };
            return lines[body_start..body_end].join("\n");
        }
    }
    trimmed.to_string()
}

fn short_preview(s: &str) -> String {
    if s.len() > 200 {
        format!("{}…", &s[..200])
    } else {
        s.to_string()
    }
}

// Hourly-ish cleanup for expired plans is left as a watchdog future-work.
// For v0 lazy expiry in `get()` is enough since callers always look up
// before acting.
#[allow(dead_code)]
fn _expire_marker(_: Arc<PlanManager>) {}
