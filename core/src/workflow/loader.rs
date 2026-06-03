//! Workflow TOML loader.
//!
//! Reads `<dir>/<id>.toml` and deserialises into [`WorkflowDef`].
//! `<dir>` defaults to `core/workflows/` relative to the repo root.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::def::WorkflowDef;

/// Load a workflow by id from `dir`.
///
/// `dir` is typically `core/workflows/` (dev) or the configured path.
pub fn load(id: &str, dir: &Path) -> Result<WorkflowDef> {
    let path = dir.join(format!("{id}.toml"));
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("workflow not found: {}", path.display()))?;
    let def: WorkflowDef = toml::from_str(&raw)
        .with_context(|| format!("workflow parse error: {}", path.display()))?;
    anyhow::ensure!(def.id == id, "workflow id mismatch: file={id} toml={}", def.id);
    Ok(def)
}

/// Load all `.toml` files from `dir`. Returns (id, def) pairs.
pub fn load_all(dir: &Path) -> Result<Vec<WorkflowDef>> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("workflow dir not found: {}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let def: WorkflowDef = toml::from_str(&raw)
            .with_context(|| format!("parse {}", path.display()))?;
        out.push(def);
    }
    Ok(out)
}

/// Canonical location of the bundled workflow TOML files.
/// Walks up from `core/` looking for the `core/workflows/` dir.
pub fn bundled_workflow_dir() -> Option<PathBuf> {
    // Try CWD first (running from repo root), then parent (running from core/).
    for base in [
        std::env::current_dir().ok()?,
        std::env::current_dir().ok()?.join(".."),
    ] {
        let candidate = base.join("core").join("workflows");
        if candidate.is_dir() {
            return Some(candidate);
        }
        // Already inside core/
        let candidate2 = base.join("workflows");
        if candidate2.is_dir() {
            return Some(candidate2);
        }
    }
    None
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kira_wf_test_{suffix}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_minimal_workflow() {
        let dir = make_dir("minimal");
        std::fs::write(dir.join("minimal.toml"), r#"
id = "minimal"
description = "test"

[[phase]]
id = "only"
prompt = "test.md"
"#).unwrap();
        let def = load("minimal", &dir).unwrap();
        assert_eq!(def.id, "minimal");
        assert_eq!(def.phases.len(), 1);
    }

    #[test]
    fn load_missing_returns_error() {
        let dir = make_dir("missing");
        assert!(load("nope", &dir).is_err());
    }

    #[test]
    fn load_id_mismatch_returns_error() {
        let dir = make_dir("mismatch");
        std::fs::write(dir.join("file_name.toml"), r#"id = "different"
[[phase]]
id = "p"
prompt = "p.md"
"#).unwrap();
        assert!(load("file_name", &dir).is_err());
    }

    #[test]
    fn load_all_finds_toml_files() {
        let dir = make_dir("all");
        std::fs::write(dir.join("w1.toml"), r#"id = "w1"
[[phase]]
id = "p1"
prompt = "x.md"
"#).unwrap();
        std::fs::write(dir.join("w2.toml"), r#"id = "w2"
[[phase]]
id = "p1"
prompt = "y.md"
"#).unwrap();
        std::fs::write(dir.join("ignore.txt"), "nope").unwrap();
        let defs = load_all(&dir).unwrap();
        assert_eq!(defs.len(), 2);
    }
}
