//! `GET /v1/stats` — session-level stats for the REPL goodbye summary.
//!
//! Returns cumulative budget spend, agent counts, and per-model token usage
//! accumulated since the daemon started.

use axum::{extract::State, Json};
use serde::Serialize;
use ts_rs::TS;

use crate::server::AppState;

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/stats.ts")]
pub struct StatsResponse {
    /// Total USD spent since daemon start (from `BudgetGuard`).
    pub usd_spent: f64,
    /// Cumulative tokens (in + out) across all tracked calls.
    pub tokens_used: u64,
    /// Configured monthly hard-stop limit (USD).
    pub max_budget_usd: f64,
    /// Spend as a percentage of the hard-stop (0..=100).
    pub usage_pct: u8,
    /// Provider connection state (name + "connected"/"disconnected").
    pub providers: Vec<ProviderDto>,
    /// Total number of agents spawned since daemon start.
    pub total_agents: u64,
    /// Agents currently active (not completed/failed/cancelled/stopped).
    pub active_agents: u64,
    /// Agents completed successfully.
    pub completed_agents: u64,
    /// Agents that failed, were cancelled, or stopped.
    pub finished_agents: u64,
    /// Per-model aggregate token usage (accumulated from all chat calls).
    pub model_usage: Vec<ModelUsage>,
}

#[derive(Serialize, TS, Clone)]
#[ts(export, export_to = "../../agent/src/generated/stats.ts")]
pub struct ProviderDto {
    pub name: String,
    /// "connected" | "disconnected" — derived from the circuit breaker.
    pub status: String,
}

#[derive(Serialize, TS, Clone)]
#[ts(export, export_to = "../../agent/src/generated/stats.ts")]
pub struct ModelUsage {
    pub model: String,
    pub requests: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
}

pub async fn stats(State(state): State<AppState>) -> Json<StatsResponse> {
    let agents = state.agents.list();
    let total_agents = agents.len() as u64;
    let active_agents = agents
        .iter()
        .filter(|a| {
            matches!(
                a.status,
                crate::proto::agent::AgentStatus::Spawning
                    | crate::proto::agent::AgentStatus::ActiveForeground
                    | crate::proto::agent::AgentStatus::ActiveBackground
                    | crate::proto::agent::AgentStatus::Stale
            )
        })
        .count() as u64;
    let completed_agents = agents
        .iter()
        .filter(|a| {
            matches!(a.status, crate::proto::agent::AgentStatus::Completed)
        })
        .count() as u64;
    let finished_agents = total_agents - active_agents - completed_agents;

    let model_usage = state.router.model_usage();

    // Provider connection state: a provider is "connected" if it is enabled +
    // constructed (in `known_providers`). v0 has no provider-level breaker
    // rollup, so we don't yet surface per-provider degradation here.
    let providers = state
        .router
        .known_providers()
        .into_iter()
        .map(|name| ProviderDto {
            name,
            status: "connected".to_string(),
        })
        .collect();

    Json(StatsResponse {
        usd_spent: state.router.budget.spent_usd(),
        tokens_used: state.router.budget.tokens_used(),
        max_budget_usd: state.router.budget.max_budget_usd(),
        usage_pct: state.router.budget.usage_pct(),
        providers,
        total_agents,
        active_agents,
        completed_agents,
        finished_agents,
        model_usage,
    })
}
