//! OwnershipRegistry — claim/check/release of FS path patterns per agent.
//!
//! v0 is in-memory only (`Arc<Mutex<HashMap>>`). Plan T7/T8 moves this into a
//! `redb::Tree::transaction` to defeat claim races. For Mac-local solo v0 the
//! Mutex is enough.
//!
//! Integration with `PreToolUse` hook (auto-block writes outside owned scope)
//! lands Sem 9 when the agent loop has a session context to identify caller.

use anyhow::{Context, Result};
use chrono::Utc;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, instrument};

use crate::proto::ownership::{Claim, OwnershipMode, WriteDecision};

#[derive(Clone, Default)]
pub struct OwnershipRegistry {
    inner: Arc<Mutex<HashMap<String, CompiledClaim>>>,
}

/// Stored claim with a precompiled glob set for fast `is_match`.
struct CompiledClaim {
    claim: Claim,
    globs: GlobSet,
}

impl OwnershipRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a claim. Returns error if patterns are invalid globs.
    /// Overwrites any existing claim for the same `agent_id`.
    #[instrument(skip(self), fields(agent_id = %claim.agent_id, mode = ?claim.mode))]
    pub fn claim(&self, claim: Claim) -> Result<()> {
        let mut builder = GlobSetBuilder::new();
        for p in &claim.patterns {
            let g = Glob::new(p).with_context(|| format!("invalid glob: {p}"))?;
            builder.add(g);
        }
        let globs = builder.build().context("globset build failed")?;
        let mut map = self.inner.lock().expect("ownership mutex poisoned");
        map.insert(
            claim.agent_id.clone(),
            CompiledClaim { claim, globs },
        );
        Ok(())
    }

    /// Release this agent's claim entirely. Returns true if a claim existed.
    pub fn release(&self, agent_id: &str) -> bool {
        let mut map = self.inner.lock().expect("ownership mutex poisoned");
        map.remove(agent_id).is_some()
    }

    /// Check whether `agent_id` may write `path`.
    ///
    /// Logic:
    ///   - If some OTHER agent has an Exclusive claim covering `path` → Denied.
    ///   - Otherwise → Allowed.
    ///
    /// Note: An agent without any claim is allowed to write anywhere — v0 lets
    /// caller decide scope. v0.5+ may flip to "no claim = no write".
    #[instrument(skip(self), fields(agent_id = %agent_id, path = %path))]
    pub fn check_write(&self, agent_id: &str, path: &str) -> WriteDecision {
        let map = self.inner.lock().expect("ownership mutex poisoned");
        for (other_id, entry) in map.iter() {
            if other_id == agent_id {
                continue;
            }
            if matches!(entry.claim.mode, OwnershipMode::Exclusive)
                && entry.globs.is_match(path)
            {
                debug!(
                    %other_id,
                    matched_patterns = ?entry.claim.patterns,
                    "ownership conflict"
                );
                return WriteDecision::Denied {
                    reason: format!(
                        "Exclusive claim by agent {other_id} matches path '{path}'"
                    ),
                    conflicting_agent: Some(other_id.clone()),
                };
            }
        }
        WriteDecision::Allowed
    }

    /// List all active claims.
    pub fn list(&self) -> Vec<Claim> {
        let map = self.inner.lock().expect("ownership mutex poisoned");
        map.values().map(|c| c.claim.clone()).collect()
    }
}

impl Claim {
    pub fn now(agent_id: String, patterns: Vec<String>, mode: OwnershipMode) -> Self {
        Self {
            agent_id,
            patterns,
            mode,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> OwnershipRegistry {
        OwnershipRegistry::new()
    }

    #[test]
    fn no_claims_allows_write() {
        let r = registry();
        assert!(matches!(
            r.check_write("agent-A", "src/foo.rs"),
            WriteDecision::Allowed
        ));
    }

    #[test]
    fn exclusive_blocks_other_agent() {
        let r = registry();
        r.claim(Claim::now(
            "writer".into(),
            vec!["src/content/**".into()],
            OwnershipMode::Exclusive,
        ))
        .unwrap();
        match r.check_write("deployer", "src/content/index.md") {
            WriteDecision::Denied { conflicting_agent, .. } => {
                assert_eq!(conflicting_agent.as_deref(), Some("writer"));
            }
            _ => panic!("expected Denied"),
        }
    }

    #[test]
    fn exclusive_allows_owner() {
        let r = registry();
        r.claim(Claim::now(
            "writer".into(),
            vec!["src/content/**".into()],
            OwnershipMode::Exclusive,
        ))
        .unwrap();
        assert!(matches!(
            r.check_write("writer", "src/content/index.md"),
            WriteDecision::Allowed
        ));
    }

    #[test]
    fn shared_does_not_block() {
        let r = registry();
        r.claim(Claim::now(
            "writer".into(),
            vec!["public/**".into()],
            OwnershipMode::Shared,
        ))
        .unwrap();
        assert!(matches!(
            r.check_write("deployer", "public/index.html"),
            WriteDecision::Allowed
        ));
    }

    #[test]
    fn release_returns_true_when_exists() {
        let r = registry();
        r.claim(Claim::now(
            "writer".into(),
            vec!["x/**".into()],
            OwnershipMode::Exclusive,
        ))
        .unwrap();
        assert!(r.release("writer"));
        assert!(!r.release("writer"));
    }

    #[test]
    fn invalid_glob_errors() {
        let r = registry();
        let bad = Claim::now(
            "agent".into(),
            vec!["[invalid".into()],
            OwnershipMode::Exclusive,
        );
        assert!(r.claim(bad).is_err());
    }
}
