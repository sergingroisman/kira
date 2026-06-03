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
pub mod fallback;
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
    /// C2 — Optional DB handle for persisting breaker snapshots so daemon
    /// restarts preserve Open state.
    db: Option<crate::state::KiraDb>,
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
            db: None,
        }
    }

    /// C2 — Attach a DB handle and restore any persisted breaker snapshots.
    /// Returns the count of restored breakers for logging.
    pub fn with_db(mut self, db: crate::state::KiraDb) -> Self {
        let restored = self.restore_breakers_from_db(&db);
        self.db = Some(db);
        if restored > 0 {
            tracing::info!(count = restored, "circuit breakers restored from redb");
        }
        self
    }

    fn restore_breakers_from_db(&self, db: &crate::state::KiraDb) -> usize {
        use crate::router::circuit::{BreakerConfig, BreakerSnapshot};
        use crate::state::TBL_BREAKERS;
        let pairs: Vec<(String, String)> = db.list_kv(TBL_BREAKERS).unwrap_or_default();
        let cfg = BreakerConfig::default();
        let mut map = self.breakers.lock().expect("breakers mutex poisoned");
        let mut count = 0;
        for (key, json) in pairs {
            if let Ok(snap) = serde_json::from_str::<BreakerSnapshot>(&json) {
                map.insert(key, Arc::new(CircuitBreaker::restore(cfg, &snap)));
                count += 1;
            }
        }
        count
    }

    /// C2 — Fire-and-forget persist of one breaker's current state.
    /// Called by `record_call_outcome` on state-changing transitions.
    fn persist_breaker_async(&self, key: String, breaker: Arc<CircuitBreaker>) {
        let Some(db) = self.db.clone() else {
            return;
        };
        tokio::spawn(async move {
            use crate::state::TBL_BREAKERS;
            let snap = breaker.snapshot();
            if let Ok(json) = serde_json::to_string(&snap)
                && let Err(e) = db.put_kv(TBL_BREAKERS, &key, &json) {
                    tracing::warn!(key, error = ?e, "breaker persist failed");
                }
        });
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

    /// Whether `tier` has any configured candidates. Used by fast-path
    /// routing decisions (e.g. chat YOLO conceptual probe before deciding
    /// whether to use `tier=local` or fall through to agent spawn).
    pub fn tier_has_candidates(&self, tier: Tier) -> bool {
        !self.tiers.candidates_for(tier).is_empty()
    }

    /// Check if a tier has at least one candidate with a non-open circuit breaker.
    ///
    /// Used by maturation subsystems (consolidator, handoff) to short-circuit
    /// before loading prompts or building messages when the LLM is known to be
    /// unavailable. This avoids unnecessary I/O when all providers are broken.
    pub fn is_tier_available(&self, tier: Tier) -> bool {
        let candidates = self.tiers.candidates_for(tier);
        if candidates.is_empty() {
            return false;
        }
        candidates.iter().any(|cand| {
            let breaker = self.breaker_for(&cand.key());
            !breaker.is_open()
        })
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
    /// All OpenAI-compatible providers (including DeepSeek) go through
    /// `chat_with_tools_raw`, which builds JSON manually and correctly echoes
    /// `reasoning_content` on subsequent turns. No provider demotion needed.
    pub async fn route_chat_with_tools(
        &self,
        tier: &str,
        model_override: Option<&str>,
        messages: Vec<ChatMessageUnified>,
        tools: Vec<ToolDef>,
        params: Option<ChatParams>,
    ) -> Result<(ChatOutput, String, String)> {
        self.budget.check()?;

        // C3 — Build escalation tier chain. Try requested tier first, then
        // upper tiers if all candidates exhaust. Judge tier never escalates.
        let primary_tier = Tier::from_str_lossy(tier);
        let mut tiers_to_try: Vec<Tier> = vec![primary_tier];
        tiers_to_try.extend_from_slice(primary_tier.escalation_chain());

        let mut last_err: Option<anyhow::Error> = None;

        for (idx, current_tier) in tiers_to_try.iter().enumerate() {
            if idx > 0 {
                info!(
                    from = ?primary_tier,
                    to = ?current_tier,
                    "cross-tier escalation — primary exhausted"
                );
            }
            let tier_str = format!("{current_tier:?}").to_lowercase();
            let pseudo_req = ChatRequest {
                tier: tier_str,
                model: if idx == 0 { model_override.map(str::to_string) } else { None },
                messages: Vec::new(),
                mode: None,
            };
            let candidates = self.candidates(&pseudo_req);

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
                        self.persist_breaker_async(key.clone(), breaker.clone());
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
                        self.persist_breaker_async(key.clone(), breaker.clone());
                        last_err = Some(e);
                        continue;
                    }
                }
            }
        }

        Err(last_err
            .unwrap_or_else(|| anyhow::anyhow!("all tool-call candidates exhausted (tier {tier} + escalation chain)")))
    }

    /// Convenience: full route + chat (non-streaming). Iterates fallback on failure.
    /// Returns (`ChatOutput`, `provider_name`, model).
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
                    self.persist_breaker_async(key.clone(), breaker.clone());
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
                    self.persist_breaker_async(key.clone(), breaker.clone());
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
    use crate::router::budget::BudgetConfig;

    fn mk_router() -> Router {
        let tiers = TierMap::from_config(&crate::config::TiersConfig {
            heavy: crate::config::TierEntry { candidates: vec![] },
            standard: crate::config::TierEntry { candidates: vec![] },
            light: crate::config::TierEntry { candidates: vec![] },
            long_context: crate::config::TierEntry { candidates: vec![] },
            judge: None,
            local: None,
        });
        Router::new(HashMap::new(), Arc::new(BudgetGuard::new(BudgetConfig::default())), tiers)
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
            judge: None,
            local: None,
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
