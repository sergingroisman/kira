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
}

impl Tier {
    /// Parse from wire string ("heavy"/"standard"/"light"/"long_context").
    /// Unknown → Standard (default).
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "heavy" => Self::Heavy,
            "light" => Self::Light,
            "long_context" | "longctx" | "long-context" => Self::LongContext,
            _ => Self::Standard,
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

        Self {
            heavy: parse_list(&cfg.heavy.candidates),
            standard: parse_list(&cfg.standard.candidates),
            light: parse_list(&cfg.light.candidates),
            long_context: parse_list(&cfg.long_context.candidates),
        }
    }

    /// Return the ordered candidates for a tier.
    pub fn candidates_for(&self, tier: Tier) -> &[Candidate] {
        match tier {
            Tier::Heavy => &self.heavy,
            Tier::Standard => &self.standard,
            Tier::Light => &self.light,
            Tier::LongContext => &self.long_context,
        }
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
        }
    }

    #[test]
    fn tier_parses_common_inputs() {
        assert_eq!(Tier::from_str_lossy("heavy"), Tier::Heavy);
        assert_eq!(Tier::from_str_lossy("Light"), Tier::Light);
        assert_eq!(Tier::from_str_lossy("STANDARD"), Tier::Standard);
        assert_eq!(Tier::from_str_lossy("longctx"), Tier::LongContext);
        assert_eq!(Tier::from_str_lossy("???"), Tier::Standard);
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
