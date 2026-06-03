//! Memory Tier Classification.
//!
//! Defines the four maturation tiers for memory fragments:
//! working → episodic → semantic → procedural.

use serde::{Deserialize, Serialize};

/// The four maturation tiers, ordered from lowest to highest.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    #[default]
    Working = 0,
    Episodic = 1,
    Semantic = 2,
    Procedural = 3,
}

impl MemoryTier {
    /// Returns true if `target` is strictly higher than `self`.
    pub const fn can_promote_to(&self, target: Self) -> bool {
        (target as u8) > (*self as u8)
    }

    /// Returns the tier as a static string slice.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Episodic => "episodic",
            Self::Semantic => "semantic",
            Self::Procedural => "procedural",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_working() {
        assert_eq!(MemoryTier::default(), MemoryTier::Working);
    }

    #[test]
    fn can_promote_to_higher_tier() {
        assert!(MemoryTier::Working.can_promote_to(MemoryTier::Episodic));
        assert!(MemoryTier::Working.can_promote_to(MemoryTier::Semantic));
        assert!(MemoryTier::Working.can_promote_to(MemoryTier::Procedural));
        assert!(MemoryTier::Episodic.can_promote_to(MemoryTier::Semantic));
        assert!(MemoryTier::Episodic.can_promote_to(MemoryTier::Procedural));
        assert!(MemoryTier::Semantic.can_promote_to(MemoryTier::Procedural));
    }

    #[test]
    fn cannot_promote_to_same_or_lower_tier() {
        assert!(!MemoryTier::Working.can_promote_to(MemoryTier::Working));
        assert!(!MemoryTier::Episodic.can_promote_to(MemoryTier::Working));
        assert!(!MemoryTier::Episodic.can_promote_to(MemoryTier::Episodic));
        assert!(!MemoryTier::Semantic.can_promote_to(MemoryTier::Working));
        assert!(!MemoryTier::Semantic.can_promote_to(MemoryTier::Episodic));
        assert!(!MemoryTier::Semantic.can_promote_to(MemoryTier::Semantic));
        assert!(!MemoryTier::Procedural.can_promote_to(MemoryTier::Working));
        assert!(!MemoryTier::Procedural.can_promote_to(MemoryTier::Episodic));
        assert!(!MemoryTier::Procedural.can_promote_to(MemoryTier::Semantic));
        assert!(!MemoryTier::Procedural.can_promote_to(MemoryTier::Procedural));
    }

    #[test]
    fn as_str_returns_correct_values() {
        assert_eq!(MemoryTier::Working.as_str(), "working");
        assert_eq!(MemoryTier::Episodic.as_str(), "episodic");
        assert_eq!(MemoryTier::Semantic.as_str(), "semantic");
        assert_eq!(MemoryTier::Procedural.as_str(), "procedural");
    }

    #[test]
    fn ordering_matches_maturation_order() {
        assert!(MemoryTier::Working < MemoryTier::Episodic);
        assert!(MemoryTier::Episodic < MemoryTier::Semantic);
        assert!(MemoryTier::Semantic < MemoryTier::Procedural);
    }

    #[test]
    fn serde_round_trip() {
        for tier in [
            MemoryTier::Working,
            MemoryTier::Episodic,
            MemoryTier::Semantic,
            MemoryTier::Procedural,
        ] {
            let json = serde_json::to_string(&tier).unwrap();
            let deserialized: MemoryTier = serde_json::from_str(&json).unwrap();
            assert_eq!(tier, deserialized);
        }
    }

    #[test]
    fn serde_uses_snake_case() {
        assert_eq!(serde_json::to_string(&MemoryTier::Working).unwrap(), "\"working\"");
        assert_eq!(serde_json::to_string(&MemoryTier::Episodic).unwrap(), "\"episodic\"");
        assert_eq!(serde_json::to_string(&MemoryTier::Semantic).unwrap(), "\"semantic\"");
        assert_eq!(serde_json::to_string(&MemoryTier::Procedural).unwrap(), "\"procedural\"");
    }
}
