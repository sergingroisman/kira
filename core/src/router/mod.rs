//! Tiered router with fallback chain + circuit breaker per (provider, model).
//!
//! Flow:
//!   1. `BudgetGuard::check()` — hard-stop if over monthly cap.
//!   2. Resolve candidates: explicit `model` override > tier list.
//!   3. For each candidate: skip if breaker `Open`. Try call.
//!      On success: `record_success` + `track_call`, return.
//!      On retryable failure: `record_failure`, continue.
//!      On non-retryable failure: return error.
//!   4. All exhausted → return last error.

pub mod budget;
pub mod circuit;
pub mod failure;
pub mod tier;

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::{info, instrument, warn};

use crate::llm::types::{ChatMessageUnified, ChatOutput, ChatParams, ToolDef};
use crate::llm::LlmProvider;
use crate::proto::chat::{ChatMessage, ChatRequest};

use self::budget::BudgetGuard;
use self::circuit::CircuitBreaker;
use self::tier::{Candidate, Tier, TierMap};

/// Per-model aggregate usage tracked for the REPL stats endpoint.
#[derive(Debug, Clone, Default)]
pub struct ModelUsage {
    pub model: String,
    pub requests: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
}

/// Shared state for per-model token tracking.
#[derive(Debug, Default)]
struct ModelUsageTracker {
    entries: Mutex<HashMap<String, ModelUsage>>,
}

impl ModelUsageTracker {
    fn record(&self, model: &str, tokens_in: u32, tokens_out: u32) {
        let mut map = self.entries.lock().expect("model usage mutex poisoned");
        let entry = map.entry(model.to_string()).or_default();
        entry.model = model.to_string();
        entry.requests += 1;
        entry.tokens_in += u64::from(tokens_in);
        entry.tokens_out += u64::from(tokens_out);
    }

    fn snapshot(&self) -> Vec<crate::server::stats_api::ModelUsage> {
        let map = self.entries.lock().expect("model usage mutex poisoned");
        map.values()
            .map(|e| crate::server::stats_api::ModelUsage {
                model: e.model.clone(),
                requests: e.requests,
                tokens_in: e.tokens_in,
                tokens_out: e.tokens_out,
            })
            .collect()
    }
}

/// The selected route for a chat call (provider name, model name, provider handle).
pub struct Selection {
    pub provider: Arc<LlmProvider>,
    pub provider_name: String,
    pub model: String,
}

#[derive(Clone)]
pub struct Router {
    providers: HashMap<String, Arc<LlmProvider>>,
    breakers: Arc<Mutex<HashMap<String, Arc<CircuitBreaker>>>>,
    pub budget: Arc<BudgetGuard>,
    model_usage: Arc<ModelUsageTracker>,
    tiers: TierMap,
}

impl Router {
    pub fn new(
        providers: HashMap<String, Arc<LlmProvider>>,
        budget: Arc<BudgetGuard>,
        tiers: TierMap,
    ) -> Self {
        Self {
            providers,
            breakers: Arc::new(Mutex::new(HashMap::new())),
            budget,
            model_usage: Arc::new(ModelUsageTracker::default()),
            tiers,
        }
    }

    /// Returns a snapshot of per-model token usage for the stats endpoint.
    pub fn model_usage(&self) -> Vec<crate::server::stats_api::ModelUsage> {
        self.model_usage.snapshot()
    }

    pub fn known_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Public accessor for tier candidates.
    pub fn tier_candidates(&self, tier: Tier) -> Vec<Candidate> {
        self.tiers.candidates_for(tier).to_vec()
    }

    fn breaker_for(&self, key: &str) -> Arc<CircuitBreaker> {
        let mut map = self.breakers.lock().expect("breakers mutex poisoned");
        map.entry(key.to_string())
            .or_insert_with(|| Arc::new(CircuitBreaker::default()))
            .clone()
    }

    /// Public accessor for breakers (used by streaming handler).
    pub fn breaker_for_public(&self, key: &str) -> Arc<CircuitBreaker> {
        self.breaker_for(key)
    }

    /// Public accessor for a provider by name.
    pub fn provider_handle(&self, name: &str) -> Option<Arc<LlmProvider>> {
        self.providers.get(name).cloned()
    }

    /// Resolve candidates to try, honoring explicit model override.
    fn candidates(&self, req: &ChatRequest) -> Vec<Candidate> {
        if let Some(model) = req.model.as_deref() {
            return self
                .providers
                .keys()
                .map(|p| Candidate::new(p.clone(), model.to_string()))
                .collect();
        }
        let tier = Tier::from_str_lossy(&req.tier);
        self.tiers.candidates_for(tier).to_vec()
    }

    /// Pick a non-broken candidate. Returns selection + the breaker handle to
    /// the caller so they can record success/failure based on actual call outcome.
    #[instrument(skip(self, req), fields(tier = %req.tier))]
    pub fn select(&self, req: &ChatRequest) -> Result<(Selection, Arc<CircuitBreaker>)> {
        self.budget.check()?;

        let candidates = self.candidates(req);
        if candidates.is_empty() {
            anyhow::bail!("no candidates for tier {}", req.tier);
        }

        for cand in &candidates {
            let key = cand.key();
            let breaker = self.breaker_for(&key);
            if breaker.is_open() {
                warn!(candidate = %key, "skipping — breaker Open");
                continue;
            }
            let Some(provider) = self.providers.get(&cand.provider).cloned() else {
                warn!(provider = %cand.provider, "provider not configured, skipping");
                continue;
            };
            info!(provider = %cand.provider, model = %cand.model, "router selected");
            return Ok((
                Selection {
                    provider,
                    provider_name: cand.provider.clone(),
                    model: cand.model.clone(),
                },
                breaker,
            ));
        }

        anyhow::bail!(
            "all candidates exhausted for tier {} (no providers up or all broken)",
            req.tier
        )
    }

    /// Route a tool‑calling chat.  Accepts `ChatMessageUnified` messages
    /// (so agent loops can include `Tool` result messages between turns)
    /// and `ToolDef` schemas.
    ///
    /// **Tool‑call ordering note**: when `tools` is non-empty, DeepSeek
    /// candidates are demoted to the end of the fallback chain. Reason: the
    /// `deepseek-v4-*` family runs in thinking mode by default and returns
    /// `reasoning_content` in the assistant message that the API requires us
    /// to echo back on the next turn. async‑openai 0.27 does not surface
    /// `reasoning_content` in request types, so iter≥2 of any agent loop
    /// would fail and fall back. Demoting upfront skips the wasted call.
    pub async fn route_chat_with_tools(
        &self,
        tier: &str,
        model_override: Option<&str>,
        messages: Vec<ChatMessageUnified>,
        tools: Vec<ToolDef>,
        params: Option<ChatParams>,
    ) -> Result<(ChatOutput, String, String)> {
        self.budget.check()?;

        let pseudo_req = ChatRequest {
            tier: tier.to_string(),
            model: model_override.map(str::to_string),
            messages: Vec::new(),
            mode: None,
        };
        let all = self.candidates(&pseudo_req);
        let candidates = if tools.is_empty() {
            all
        } else {
            let (deepseek, others): (Vec<_>, Vec<_>) =
                all.into_iter().partition(|c| c.provider == "deepseek");
            let mut out = others;
            out.extend(deepseek);
            out
        };
        let mut last_err: Option<anyhow::Error> = None;

        for cand in candidates {
            let key = cand.key();
            let breaker = self.breaker_for(&key);
            if breaker.is_open() {
                warn!(candidate = %key, "skipping — breaker Open");
                continue;
            }
            let Some(provider) = self.providers.get(&cand.provider).cloned() else {
                continue;
            };

            match provider
                .chat_with_tools(&cand.model, messages.clone(), tools.clone(), params.clone())
                .await
            {
                Ok(output) => {
                    breaker.record_success();
                    self.budget
                        .track_call(&cand.model, output.tokens_in, output.tokens_out);
                    self.model_usage
                        .record(&cand.model, output.tokens_in, output.tokens_out);
                    return Ok((output, cand.provider.clone(), cand.model.clone()));
                }
                Err(e) => {
                    let err_msg = format!("{e:#}");
                    warn!(candidate = %key, error = %err_msg, "tool-call candidate failed, trying next");
                    breaker.record_failure();
                    last_err = Some(e);
                    continue;
                }
            }
        }

        Err(last_err
            .unwrap_or_else(|| anyhow::anyhow!("all tool-call candidates exhausted (tier {})", tier)))
    }

    /// Convenience: full route + chat (non-streaming). Iterates fallback on failure.
    /// Returns (ChatOutput, provider_name, model).
    pub async fn route_chat(
        &self,
        req: &ChatRequest,
        messages: &[ChatMessage],
    ) -> Result<(ChatOutput, String, String)> {
        self.budget.check()?;

        let candidates = self.candidates(req);
        let mut last_err: Option<anyhow::Error> = None;

        for cand in candidates {
            let key = cand.key();
            let breaker = self.breaker_for(&key);
            if breaker.is_open() {
                warn!(candidate = %key, "skipping — breaker Open");
                continue;
            }
            let Some(provider) = self.providers.get(&cand.provider).cloned() else {
                continue;
            };

            match provider.chat(&cand.model, messages).await {
                Ok(output) => {
                    breaker.record_success();
                    self.budget
                        .track_call(&cand.model, output.tokens_in, output.tokens_out);
                    self.model_usage
                        .record(&cand.model, output.tokens_in, output.tokens_out);
                    return Ok((output, cand.provider.clone(), cand.model.clone()));
                }
                Err(e) => {
                    let err_msg = format!("{e:#}");
                    warn!(candidate = %key, error = %err_msg, "candidate failed, trying next");
                    breaker.record_failure();
                    last_err = Some(e);
                    continue;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            anyhow::anyhow!("all candidates exhausted for tier {}", req.tier)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_router() -> Router {
        let tiers = TierMap::from_config(&crate::config::TiersConfig {
            heavy: crate::config::TierEntry { candidates: vec![] },
            standard: crate::config::TierEntry { candidates: vec![] },
            light: crate::config::TierEntry { candidates: vec![] },
            long_context: crate::config::TierEntry { candidates: vec![] },
        });
        Router::new(HashMap::new(), Arc::new(BudgetGuard::new(Default::default())), tiers)
    }

    #[test]
    fn empty_router_known_providers_empty() {
        let r = mk_router();
        assert!(r.known_providers().is_empty());
    }

    #[test]
    fn budget_blocks_select() {
        let cfg = budget::BudgetConfig {
            monthly_hard_stop_usd: 0.001,
            warn_pct: 50.0,
        };
        let budget = Arc::new(BudgetGuard::new(cfg));
        budget.track_call("deepseek-v4-pro", 100, 100);
        let tiers = TierMap::from_config(&crate::config::TiersConfig {
            heavy: crate::config::TierEntry { candidates: vec!["fake:model".into()] },
            standard: crate::config::TierEntry { candidates: vec!["fake:model".into()] },
            light: crate::config::TierEntry { candidates: vec![] },
            long_context: crate::config::TierEntry { candidates: vec![] },
        });
        let r = Router::new(HashMap::new(), budget, tiers);
        let req = ChatRequest {
            tier: "standard".to_string(),
            model: None,
            messages: vec![],
            mode: None,
        };
        assert!(r.select(&req).is_err());
    }
}
