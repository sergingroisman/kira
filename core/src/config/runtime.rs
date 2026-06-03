//! Unified runtime config — `kira-core.toml` at repo root.
//!
//! Consolidates hooks + MCP server lists into a single file. Backward
//! compatible: when `kira-core.toml` does not exist, falls back to the
//! legacy split files (`.kira/hooks.toml` + `.kira/mcp.toml`).
//!
//! Resolution order:
//! 1. `<repo_root>/kira-core.toml` (primary)
//! 2. Merge of `<repo_root>/.kira/hooks.toml` + `<repo_root>/.kira/mcp.toml`
//! 3. Empty default
//!
//! The new file lets users keep everything in one place while keeping the
//! legacy paths working for existing setups.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use tracing::{info, warn};

use crate::hooks::HookConfig;
use crate::mcp::types::McpConfig;

/// Top-level runtime config — what `kira-core.toml` contains.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct KiraCoreConfig {
    /// Hook list (`PreToolUse` / `PostToolUse` / etc).
    #[serde(default)]
    pub hooks: Vec<HookConfig>,
    /// MCP server registry.
    #[serde(default)]
    pub mcp: McpConfig,
}

impl KiraCoreConfig {
    /// Resolve runtime config from a repo root.
    pub fn load_from_repo(repo_root: &Path) -> Self {
        let unified_path = repo_root.join("kira-core.toml");
        if unified_path.exists() {
            match load_unified(&unified_path) {
                Ok(cfg) => {
                    info!(
                        path = %unified_path.display(),
                        hooks = cfg.hooks.len(),
                        mcp_servers = cfg.mcp.servers.len(),
                        "kira-core.toml loaded (unified config)"
                    );
                    return cfg;
                }
                Err(e) => {
                    warn!(
                        path = %unified_path.display(),
                        error = ?e,
                        "kira-core.toml parse failed; falling back to legacy split files"
                    );
                }
            }
        }
        // Legacy fallback: read .kira/hooks.toml + .kira/mcp.toml separately.
        Self::load_legacy(repo_root)
    }

    fn load_legacy(repo_root: &Path) -> Self {
        let hooks = load_legacy_hooks(repo_root);
        let mcp = load_legacy_mcp(repo_root);
        info!(
            hooks_count = hooks.len(),
            mcp_servers_count = mcp.servers.len(),
            "runtime config loaded from legacy split files (.kira/hooks.toml, .kira/mcp.toml)"
        );
        Self { hooks, mcp }
    }
}

fn load_unified(path: &Path) -> Result<KiraCoreConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    let cfg: KiraCoreConfig = toml::from_str(&raw)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(cfg)
}

#[derive(Debug, Deserialize, Default)]
struct LegacyHooksFile {
    #[serde(default)]
    hooks: Vec<HookConfig>,
}

fn load_legacy_hooks(repo_root: &Path) -> Vec<HookConfig> {
    let path = repo_root.join(".kira").join("hooks.toml");
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path)
        .map_err(anyhow::Error::from)
        .and_then(|raw| toml::from_str::<LegacyHooksFile>(&raw).map_err(anyhow::Error::from))
    {
        Ok(file) => file.hooks,
        Err(e) => {
            warn!(path = %path.display(), error = ?e, "legacy hooks.toml parse failed");
            Vec::new()
        }
    }
}

fn load_legacy_mcp(repo_root: &Path) -> McpConfig {
    let project = repo_root.join(".kira").join("mcp.toml");
    let global = std::env::var("HOME")
        .ok()
        .map(|h| std::path::PathBuf::from(h).join(".config/kira/mcp.toml"));

    let path_opt = if project.exists() {
        Some(project)
    } else if let Some(g) = global.as_ref() {
        if g.exists() { Some(g.clone()) } else { None }
    } else {
        None
    };

    let Some(path) = path_opt else {
        return McpConfig::default();
    };
    match std::fs::read_to_string(&path)
        .map_err(anyhow::Error::from)
        .and_then(|raw| toml::from_str::<McpConfig>(&raw).map_err(anyhow::Error::from))
    {
        Ok(c) => c,
        Err(e) => {
            warn!(path = %path.display(), error = ?e, "legacy mcp.toml parse failed");
            McpConfig::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_unified_with_both_sections() {
        let dir = tempdir_unique("kira-core-test");
        let path = dir.join("kira-core.toml");
        let toml = r#"
[[hooks]]
event = "pre_tool_use"
command = ["sh", "-c", "echo hi"]

[mcp.servers.demo]
command = "echo"
args = ["hello"]
"#;
        std::fs::create_dir_all(&dir).unwrap();
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(toml.as_bytes()).unwrap();

        let cfg = KiraCoreConfig::load_from_repo(&dir);
        assert_eq!(cfg.hooks.len(), 1);
        assert_eq!(cfg.mcp.servers.len(), 1);
        assert!(cfg.mcp.servers.contains_key("demo"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_falls_back_to_legacy_when_unified_missing() {
        let dir = tempdir_unique("kira-legacy-test");
        let kira_dir = dir.join(".kira");
        std::fs::create_dir_all(&kira_dir).unwrap();
        let hooks_path = kira_dir.join("hooks.toml");
        std::fs::write(
            &hooks_path,
            r#"
[[hooks]]
event = "post_tool_use"
command = ["true"]
"#,
        )
        .unwrap();

        let cfg = KiraCoreConfig::load_from_repo(&dir);
        assert_eq!(cfg.hooks.len(), 1);
        assert!(cfg.mcp.servers.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_empty_when_nothing_present() {
        let dir = tempdir_unique("kira-empty-test");
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = KiraCoreConfig::load_from_repo(&dir);
        assert!(cfg.hooks.is_empty());
        assert!(cfg.mcp.servers.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn tempdir_unique(prefix: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "{prefix}-{}",
            uuid::Uuid::now_v7()
        ));
        let _ = std::fs::create_dir_all(&p);
        p
    }
}
