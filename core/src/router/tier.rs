//! Tier enum + config‑driven candidate lists.
//!
//! The actual mapping lives in `tokens.toml [tiers.*]`.  `TierMap` is
//! populated at startup and passed to the `Router`.

use serde::{Deserialize, Serialize};

use crate::config::TiersConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Heavy,
    Standard,
    Light,
    LongContext,
    /// Dedicated tier for the L5 Judge LLM gate (P3 Verification).
    /// MUST NOT overlap with writer tiers — `judge_model ≠ writer_model`.
    Judge,
    /// Local-only tier (Ollama M1, no remote fallback). For conceptual chat,
    /// dedup, and tasks where cost > latency. No escalation — controlled.
    Local,
}

impl Tier {
    /// Parse from wire string ("`heavy"/"standard"/"light"/"long_context"/"judge`").
    /// Unknown → Standard (default).
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "heavy" => Self::Heavy,
            "light" => Self::Light,
            "long_context" | "longctx" | "long-context" => Self::LongContext,
            "judge" => Self::Judge,
            "local" => Self::Local,
            _ => Self::Standard,
        }
    }

    /// Escalation chain (C3): when all candidates in `self` exhaust, the
    /// router may promote the call to the next tier in this slice.
    /// Judge has no escalation — it must never coincide with a writer tier.
    /// Local has no escalation — caller chose local explicitly to cap cost.
    pub const fn escalation_chain(self) -> &'static [Self] {
        match self {
            Self::Light => &[Self::Standard, Self::Heavy],
            Self::Standard => &[Self::Heavy],
            Self::LongContext => &[Self::Heavy],
            Self::Heavy => &[],
            Self::Judge => &[],
            Self::Local => &[],
        }
    }
}

/// A routing candidate — try this (provider, model) combination.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub provider: String,
    pub model: String,
}

impl Candidate {
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
        }
    }

    /// Stable key for circuit breaker / metrics lookup.
    pub fn key(&self) -> String {
        format!("{}:{}", self.provider, self.model)
    }
}

/// Ordered candidate lists loaded from `tokens.toml` at startup.
#[derive(Debug, Clone)]
pub struct TierMap {
    heavy: Vec<Candidate>,
    standard: Vec<Candidate>,
    light: Vec<Candidate>,
    long_context: Vec<Candidate>,
    judge: Vec<Candidate>,
    local: Vec<Candidate>,
}

impl TierMap {
    /// Build from config.  Each `"provider:model"` string is split on the
    /// first `:`.  Malformed entries are skipped with a warning.
    pub fn from_config(cfg: &TiersConfig) -> Self {
        fn parse_list(list: &[String]) -> Vec<Candidate> {
            list.iter()
                .filter_map(|s| {
                    let (provider, model) = s.split_once(':')?;
                    Some(Candidate::new(provider.to_string(), model.to_string()))
                })
                .collect()
        }

        // Judge defaults to a list disjoint from Heavy when not configured:
        // gpt-4o-mini first (OpenAI family ≠ DeepSeek writer), then
        // deepseek-v4-flash as fallback (Spektor invariant: judge ≠ writer).
        let judge_candidates = cfg
            .judge
            .as_ref().map_or_else(|| {
                vec![
                    Candidate::new("openai", "gpt-4o-mini"),
                    Candidate::new("deepseek", "deepseek-v4-flash"),
                ]
            }, |j| parse_list(&j.candidates));

        Self {
            heavy: parse_list(&cfg.heavy.candidates),
            standard: parse_list(&cfg.standard.candidates),
            light: parse_list(&cfg.light.candidates),
            long_context: parse_list(&cfg.long_context.candidates),
            judge: judge_candidates,
            local: cfg
                .local
                .as_ref()
                .map(|l| parse_list(&l.candidates))
                .unwrap_or_default(),
        }
    }

    /// Return the ordered candidates for a tier.
    pub fn candidates_for(&self, tier: Tier) -> &[Candidate] {
        match tier {
            Tier::Heavy => &self.heavy,
            Tier::Standard => &self.standard,
            Tier::Light => &self.light,
            Tier::LongContext => &self.long_context,
            Tier::Judge => &self.judge,
            Tier::Local => &self.local,
        }
    }

    /// Whether the Local tier has any candidates. Used by router/health-check
    /// to know if local routing is currently viable.
    pub fn has_local(&self) -> bool {
        !self.local.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> TiersConfig {
        TiersConfig {
            heavy: crate::config::TierEntry {
                candidates: vec![
                    "deepseek:deepseek-v4-pro".into(),
                    "dashscope:qwen-max".into(),
                ],
            },
            standard: crate::config::TierEntry {
                candidates: vec![
                    "deepseek:deepseek-v4-pro".into(),
                    "dashscope:qwen-plus".into(),
                ],
            },
            light: crate::config::TierEntry {
                candidates: vec![
                    "deepseek:deepseek-v4-flash".into(),
                    "dashscope:qwen-flash".into(),
                ],
            },
            long_context: crate::config::TierEntry {
                candidates: vec![
                    "deepseek:deepseek-v4-pro".into(),
                    "dashscope:qwen-max".into(),
                ],
            },
            judge: None, // Use default disjoint list.
            local: None,
        }
    }

    #[test]
    fn tier_parses_common_inputs() {
        assert_eq!(Tier::from_str_lossy("heavy"), Tier::Heavy);
        assert_eq!(Tier::from_str_lossy("Light"), Tier::Light);
        assert_eq!(Tier::from_str_lossy("STANDARD"), Tier::Standard);
        assert_eq!(Tier::from_str_lossy("longctx"), Tier::LongContext);
        assert_eq!(Tier::from_str_lossy("judge"), Tier::Judge);
        assert_eq!(Tier::from_str_lossy("Judge"), Tier::Judge);
        assert_eq!(Tier::from_str_lossy("???"), Tier::Standard);
    }

    #[test]
    fn escalation_chain_promotes_upward_only() {
        assert_eq!(Tier::Light.escalation_chain(), &[Tier::Standard, Tier::Heavy]);
        assert_eq!(Tier::Standard.escalation_chain(), &[Tier::Heavy]);
        assert_eq!(Tier::LongContext.escalation_chain(), &[Tier::Heavy]);
        assert!(Tier::Heavy.escalation_chain().is_empty());
        // Judge MUST NOT escalate — security invariant.
        assert!(Tier::Judge.escalation_chain().is_empty());
    }

    #[test]
    fn judge_tier_default_disjoint_from_heavy() {
        // Default judge list (when config.judge is None) starts with gpt-4o-mini,
        // while heavy starts with deepseek-v4-pro — different providers, disjoint.
        let map = TierMap::from_config(&sample_config());
        let judge = map.candidates_for(Tier::Judge);
        assert!(!judge.is_empty());
        assert_eq!(&judge[0].provider, "openai");
        assert_eq!(&judge[0].model, "gpt-4o-mini");
        // Verify it does NOT match Heavy primary.
        assert_ne!(judge[0].key(), map.candidates_for(Tier::Heavy)[0].key());
    }

    #[test]
    fn each_tier_has_candidates() {
        let map = TierMap::from_config(&sample_config());
        for t in [Tier::Heavy, Tier::Standard, Tier::Light, Tier::LongContext] {
            assert!(!map.candidates_for(t).is_empty(), "tier {t:?} has no candidates");
        }
    }

    #[test]
    fn deepseek_is_primary_for_heavy_and_standard() {
        let map = TierMap::from_config(&sample_config());
        assert_eq!(&map.candidates_for(Tier::Heavy)[0].provider, "deepseek");
        assert_eq!(&map.candidates_for(Tier::Standard)[0].provider, "deepseek");
    }
}
