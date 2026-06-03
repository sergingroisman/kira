//! Service registry — named services resolved by Workflow phases.
//!
//! Workflows reference services by name in their TOML (`gate = "judge"`).
//! The registry maps those names to concrete implementations. Adding a
//! service requires Rust; using it is config-only.
//!
//! Services in v0 (implementations elsewhere, registered at startup):
//! - `judge`           — `Gate` backed by LLM-as-Judge
//! - `detective`       — `Gate` backed by the Detective multi-dim reviewer
//! - `loc_heuristic`   — `Sizer` based on estimated LOC + file count

use std::collections::HashMap;
use std::sync::Arc;

// ── Trait definitions ──────────────────────────────────────────────────────

/// Gate service: evaluates an artifact and returns pass/fail + optional score.
pub trait Gate: Send + Sync {
    fn evaluate<'a>(
        &'a self,
        artifact: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = GateResult> + Send + 'a>>;
}

#[derive(Debug, Clone)]
pub struct GateResult {
    pub passed: bool,
    pub score: Option<f64>,
    pub message: String,
}

/// Sizer service: decides which phases can be skipped given a goal hint.
pub trait Sizer: Send + Sync {
    /// Returns the set of phase ids that should be skipped.
    fn skippable_phases(&self, hint: &SizingHint) -> Vec<String>;
}

#[derive(Debug, Clone, Default)]
pub struct SizingHint {
    pub estimated_loc: Option<u32>,
    pub estimated_files: Option<u32>,
    pub force_full: bool,
    pub kind: Option<String>,
}

// ── Registry ───────────────────────────────────────────────────────────────

/// Registry of named services. Cheaply cloneable (Arc interior).
#[derive(Clone, Default)]
pub struct ServiceRegistry {
    gates: HashMap<String, Arc<dyn Gate>>,
    sizers: HashMap<String, Arc<dyn Sizer>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_gate(&mut self, name: impl Into<String>, gate: Arc<dyn Gate>) {
        self.gates.insert(name.into(), gate);
    }

    pub fn register_sizer(&mut self, name: impl Into<String>, sizer: Arc<dyn Sizer>) {
        self.sizers.insert(name.into(), sizer);
    }

    /// Look up a gate by name. Returns `None` when no gate is registered or
    /// the workflow phase has `gate = None` (pass-through).
    pub fn gate(&self, name: &str) -> Option<&Arc<dyn Gate>> {
        self.gates.get(name)
    }

    /// Look up a sizer by name.
    pub fn sizer(&self, name: &str) -> Option<&Arc<dyn Sizer>> {
        self.sizers.get(name)
    }

    pub fn has_gate(&self, name: &str) -> bool {
        self.gates.contains_key(name)
    }

    pub fn has_sizer(&self, name: &str) -> bool {
        self.sizers.contains_key(name)
    }
}

impl std::fmt::Debug for ServiceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceRegistry")
            .field("gates", &self.gates.keys().collect::<Vec<_>>())
            .field("sizers", &self.sizers.keys().collect::<Vec<_>>())
            .finish()
    }
}

// ── LocHeuristicSizer ──────────────────────────────────────────────────────

/// Default sizer — mirrors the v0 `auto_size()` heuristic in `sdd/mod.rs`.
/// Skips `specify` and `design` for trivial bug fixes (≤5 LOC, ≤1 file).
pub struct LocHeuristicSizer {
    trivial_loc: u32,
    trivial_files: u32,
    skip_phases: Vec<String>,
}

impl Default for LocHeuristicSizer {
    fn default() -> Self {
        Self {
            trivial_loc: 5,
            trivial_files: 1,
            skip_phases: vec!["specify".into(), "design".into()],
        }
    }
}

impl LocHeuristicSizer {
    pub fn new(trivial_loc: u32, trivial_files: u32, skip_phases: Vec<String>) -> Self {
        Self { trivial_loc, trivial_files, skip_phases }
    }
}

impl Sizer for LocHeuristicSizer {
    fn skippable_phases(&self, hint: &SizingHint) -> Vec<String> {
        if hint.force_full {
            return vec![];
        }
        let is_bugfix = hint.kind.as_deref() == Some("bug_fix");
        let loc_ok = hint.estimated_loc.map_or(false, |l| l <= self.trivial_loc);
        let files_ok = hint.estimated_files.map_or(false, |f| f <= self.trivial_files);
        if is_bugfix && loc_ok && files_ok {
            self.skip_phases.clone()
        } else {
            vec![]
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loc_heuristic_skips_trivial_bugfix() {
        let sizer = LocHeuristicSizer::default();
        let hint = SizingHint {
            estimated_loc: Some(3),
            estimated_files: Some(1),
            force_full: false,
            kind: Some("bug_fix".into()),
        };
        let skipped = sizer.skippable_phases(&hint);
        assert!(skipped.contains(&"specify".to_string()));
        assert!(skipped.contains(&"design".to_string()));
    }

    #[test]
    fn loc_heuristic_keeps_feature_full() {
        let sizer = LocHeuristicSizer::default();
        let hint = SizingHint {
            estimated_loc: Some(100),
            estimated_files: Some(5),
            force_full: false,
            kind: Some("feature".into()),
        };
        assert!(sizer.skippable_phases(&hint).is_empty());
    }

    #[test]
    fn force_full_overrides_trivial() {
        let sizer = LocHeuristicSizer::default();
        let hint = SizingHint {
            estimated_loc: Some(1),
            estimated_files: Some(1),
            force_full: true,
            kind: Some("bug_fix".into()),
        };
        assert!(sizer.skippable_phases(&hint).is_empty());
    }

    #[test]
    fn registry_gate_lookup() {
        let mut reg = ServiceRegistry::new();
        assert!(!reg.has_gate("judge"));
        // register a stub (not testing Gate logic, just registry plumbing)
        struct PassGate;
        impl Gate for PassGate {
            fn evaluate<'a>(&'a self, _: &'a str)
                -> std::pin::Pin<Box<dyn std::future::Future<Output = GateResult> + Send + 'a>>
            {
                Box::pin(async { GateResult { passed: true, score: Some(1.0), message: "ok".into() } })
            }
        }
        reg.register_gate("judge", Arc::new(PassGate));
        assert!(reg.has_gate("judge"));
        assert!(!reg.has_gate("detective"));
    }
}
