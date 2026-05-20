//! Tier enum + canonical mapping to (provider, model) candidates.
//!
//! v0: hardcoded mapping per the plan. v0.5 reads from `tokens.toml`.

use serde::{Deserialize, Serialize};

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

/// Return the ordered list of candidates for a tier.
/// First = primary, rest = fallback chain.
pub fn candidates_for(tier: Tier) -> Vec<Candidate> {
    match tier {
        Tier::Heavy => vec![
            Candidate::new("deepseek", "deepseek-v4-pro"),
            Candidate::new("dashscope", "qwen-max"),
            Candidate::new("dashscope", "qwen-plus"),
        ],
        Tier::Standard => vec![
            Candidate::new("deepseek", "deepseek-v4-pro"),
            Candidate::new("dashscope", "qwen-plus"),
            Candidate::new("deepseek", "deepseek-v4-flash"),
        ],
        Tier::Light => vec![
            Candidate::new("deepseek", "deepseek-v4-flash"),
            Candidate::new("dashscope", "qwen-flash"),
        ],
        Tier::LongContext => vec![
            Candidate::new("deepseek", "deepseek-v4-pro"),
            Candidate::new("dashscope", "qwen-max"),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for t in [Tier::Heavy, Tier::Standard, Tier::Light, Tier::LongContext] {
            let c = candidates_for(t);
            assert!(!c.is_empty(), "tier {t:?} has no candidates");
        }
    }

    #[test]
    fn deepseek_is_primary_for_heavy_and_standard() {
        assert_eq!(&candidates_for(Tier::Heavy)[0].provider, "deepseek");
        assert_eq!(&candidates_for(Tier::Standard)[0].provider, "deepseek");
    }
}
