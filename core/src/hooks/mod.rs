//! Hook runner — declarative subprocess hooks fired on lifecycle events.
//!
//! Config lives at `<repo>/.kira/hooks.toml`. Missing file = no hooks.
//!
//! Schema:
//! ```toml
//! [[hooks]]
//! event = "pre_tool_use"
//! matcher.tool = "bash"
//! matcher.regex = "rm -rf"
//! command = ["sh", "-c", "echo 'BLOCKED: dangerous rm' >&2 && exit 1"]
//! timeout_secs = 5
//! ```
//!
//! v0 semantics: exit code 0 → allow; exit code != 0 on `PreToolUse` → block.
//! Stdout JSON parsing (`action/message/modified_input`) is v0.5.

use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, info, instrument, warn};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    SessionStart,
    PreToolUse,
    PostToolUse,
    WorktreeSpawn,
    WorktreeDestroy,
    BudgetWarn,
    BudgetStop,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Matcher {
    /// Match a specific tool name (e.g. "bash", "`file_write`").
    #[serde(default)]
    pub tool: Option<String>,
    /// Regex applied to the joined argument string for `PreToolUse` / `PostToolUse`.
    #[serde(default)]
    pub regex: Option<String>,
    /// Glob applied to `path` field of `file_read/file_write` tools.
    #[serde(default)]
    pub path_glob: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HookConfig {
    pub event: HookEvent,
    #[serde(default)]
    pub matcher: Matcher,
    pub command: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

const fn default_timeout() -> u64 {
    5
}

#[derive(Debug, Deserialize, Default)]
struct HooksFile {
    #[serde(default)]
    hooks: Vec<HookConfig>,
}

#[derive(Clone)]
pub struct HookRunner {
    hooks: std::sync::Arc<Vec<HookConfig>>,
}

#[derive(Debug, Clone)]
pub enum HookOutcome {
    Allowed,
    Blocked { reason: String, hook_idx: usize },
}

/// Context passed to hooks. Serialized as env vars before exec.
#[derive(Debug, Clone, Default)]
pub struct HookContext {
    pub tool_name: Option<String>,
    pub tool_args: Option<String>, // joined json or argv
    pub path: Option<String>,
    pub agent_id: Option<String>,
    pub worktree_id: Option<String>,
    pub extras: HashMap<String, String>,
}

impl HookRunner {
    /// Load hooks from `<repo_root>/.kira/hooks.toml`. Missing file = no hooks.
    ///
    /// Prefers the unified `kira-core.toml` when present (B2).
    pub fn load(repo_root: &PathBuf) -> Result<Self> {
        let unified = repo_root.join("kira-core.toml");
        if unified.exists() {
            let cfg = crate::config::runtime::KiraCoreConfig::load_from_repo(repo_root);
            info!(count = cfg.hooks.len(), "hooks loaded from kira-core.toml");
            return Ok(Self {
                hooks: std::sync::Arc::new(cfg.hooks),
            });
        }
        let path = repo_root.join(".kira").join("hooks.toml");
        if !path.exists() {
            info!(path = %path.display(), "no hooks.toml — hook runner disabled");
            return Ok(Self {
                hooks: std::sync::Arc::new(Vec::new()),
            });
        }
        let raw = std::fs::read_to_string(&path)?;
        let parsed: HooksFile = toml::from_str(&raw)?;
        info!(
            path = %path.display(),
            count = parsed.hooks.len(),
            "hooks loaded (legacy .kira/hooks.toml)"
        );
        Ok(Self {
            hooks: std::sync::Arc::new(parsed.hooks),
        })
    }

    pub fn empty() -> Self {
        Self {
            hooks: std::sync::Arc::new(Vec::new()),
        }
    }

    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    fn matches(h: &HookConfig, event: HookEvent, ctx: &HookContext) -> bool {
        if h.event != event {
            return false;
        }
        if let Some(t) = &h.matcher.tool
            && ctx.tool_name.as_deref() != Some(t.as_str()) {
                return false;
            }
        if let Some(re) = &h.matcher.regex {
            let blob = ctx.tool_args.as_deref().unwrap_or("");
            match regex::Regex::new(re) {
                Ok(r) => {
                    if !r.is_match(blob) {
                        return false;
                    }
                }
                Err(e) => {
                    warn!(error = ?e, regex = %re, "invalid hook regex — skipping");
                    return false;
                }
            }
        }
        if let Some(glob) = &h.matcher.path_glob {
            let path = ctx.path.as_deref().unwrap_or("");
            match globset::Glob::new(glob) {
                Ok(g) => {
                    if !g.compile_matcher().is_match(path) {
                        return false;
                    }
                }
                Err(e) => {
                    warn!(error = ?e, glob = %glob, "invalid hook glob — skipping");
                    return false;
                }
            }
        }
        true
    }

    #[instrument(skip(self, ctx), fields(event = ?event, tool = ?ctx.tool_name))]
    pub async fn dispatch(&self, event: HookEvent, ctx: HookContext) -> HookOutcome {
        for (idx, h) in self.hooks.iter().enumerate() {
            if !Self::matches(h, event, &ctx) {
                continue;
            }
            let outcome = run_one(h, idx, &ctx).await;
            // For PreToolUse, the first blocker short-circuits.
            if matches!(outcome, HookOutcome::Blocked { .. }) && event == HookEvent::PreToolUse {
                return outcome;
            }
        }
        HookOutcome::Allowed
    }
}

#[instrument(skip(h, ctx), fields(idx, exe = %h.command.first().cloned().unwrap_or_default()))]
async fn run_one(h: &HookConfig, idx: usize, ctx: &HookContext) -> HookOutcome {
    let Some((exe, args)) = h.command.split_first() else {
        warn!("hook command empty — skipping");
        return HookOutcome::Allowed;
    };
    let timeout = Duration::from_secs(h.timeout_secs.max(1).min(60));

    let mut cmd = Command::new(exe);
    cmd.args(args)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", std::env::var("HOME").unwrap_or_default())
        .env("KIRA_HOOK_EVENT", format!("{:?}", h.event))
        .env(
            "KIRA_HOOK_TOOL",
            ctx.tool_name.clone().unwrap_or_default(),
        )
        .env(
            "KIRA_HOOK_ARGS",
            ctx.tool_args.clone().unwrap_or_default(),
        )
        .env("KIRA_HOOK_PATH", ctx.path.clone().unwrap_or_default())
        .env(
            "KIRA_HOOK_AGENT_ID",
            ctx.agent_id.clone().unwrap_or_default(),
        )
        .env(
            "KIRA_HOOK_WORKTREE_ID",
            ctx.worktree_id.clone().unwrap_or_default(),
        );
    for (k, v) in &ctx.extras {
        cmd.env(format!("KIRA_HOOK_EXTRA_{}", k.to_ascii_uppercase()), v);
    }
    cmd.kill_on_drop(true);

    debug!("running hook");
    let output = match tokio::time::timeout(timeout, cmd.output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            warn!(error = ?e, "hook spawn failed — treating as allow");
            return HookOutcome::Allowed;
        }
        Err(_) => {
            warn!(timeout_secs = h.timeout_secs, "hook timed out — treating as block");
            return HookOutcome::Blocked {
                reason: format!("hook #{idx} timed out after {}s", h.timeout_secs),
                hook_idx: idx,
            };
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let reason = if stderr.trim().is_empty() {
            format!("hook #{idx} exited {:?}", output.status.code())
        } else {
            format!("hook #{idx}: {}", stderr.trim())
        };
        return HookOutcome::Blocked {
            reason,
            hook_idx: idx,
        };
    }

    HookOutcome::Allowed
}
