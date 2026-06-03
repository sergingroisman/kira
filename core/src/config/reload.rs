//! SIGHUP hot-reload for `[memory.maturation]` configuration.
//!
//! On SIGHUP:
//! 1. Re-read `tokens.toml` from disk.
//! 2. Parse the `[memory.maturation]` section.
//! 3. Validate ranges (out-of-range → defaults + warn).
//! 4. Swap the shared `MaturationConfig` atomically via `ArcSwap`.
//! 5. In-progress consolidation passes continue with the config snapshot
//!    captured at their start (they hold an `Arc<MaturationConfig>` clone).
//!
//! If `tokens.toml` is syntactically invalid or unreadable, the current
//! in-memory config is retained and a warning is logged.

use arc_swap::ArcSwap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

use super::MaturationConfig;

/// Shared, atomically-swappable maturation config.
///
/// Readers call `.load()` to get an `Arc<MaturationConfig>` snapshot.
/// The SIGHUP handler calls `.store()` to replace the config atomically.
/// In-progress operations that already loaded a snapshot are unaffected.
pub type SharedMaturationConfig = Arc<ArcSwap<MaturationConfig>>;

/// Create a new `SharedMaturationConfig` from an initial config value.
pub fn shared_config(config: MaturationConfig) -> SharedMaturationConfig {
    Arc::new(ArcSwap::from_pointee(config))
}

/// Attempt to reload `MaturationConfig` from `tokens.toml` at the given path.
///
/// Returns `Some(config)` on success, `None` on failure (with warning logged).
/// On failure, the caller should retain the current config unchanged.
pub fn reload_maturation_config(tokens_path: &Path) -> Option<MaturationConfig> {
    // 1. Read the file.
    let content = match std::fs::read_to_string(tokens_path) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                path = %tokens_path.display(),
                error = %e,
                "SIGHUP reload: failed to read tokens.toml — retaining current config"
            );
            return None;
        }
    };

    // 2. Parse as TOML. We only need the [memory.maturation] section,
    //    but we parse the full Tokens struct to reuse existing validation.
    let tokens: super::Tokens = match toml::from_str(&content) {
        Ok(t) => t,
        Err(e) => {
            warn!(
                path = %tokens_path.display(),
                error = %e,
                "SIGHUP reload: tokens.toml is syntactically invalid — retaining current config"
            );
            return None;
        }
    };

    // 3. Validate ranges (replaces out-of-range with defaults + warn).
    let config = tokens.memory.maturation.validated();

    Some(config)
}

/// Resolve the path to `tokens.toml` using the same search order as startup.
///
/// Returns the first existing candidate path, or `None` if not found.
pub fn find_tokens_path() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("tokens.toml"),
        PathBuf::from("../tokens.toml"),
    ];
    for path in &candidates {
        if path.exists() {
            return Some(path.clone());
        }
    }
    // Try CWD-based path.
    if let Ok(mut cwd) = std::env::current_dir() {
        cwd.push("tokens.toml");
        if cwd.exists() {
            return Some(cwd);
        }
    }
    None
}

/// Spawn the SIGHUP signal handler task.
///
/// On each SIGHUP:
/// - Reloads `tokens.toml` and extracts `[memory.maturation]`.
/// - On success: atomically swaps the shared config and logs info.
/// - On failure (invalid TOML, unreadable file): retains current config, logs warning.
///
/// In-progress consolidation passes are unaffected because they hold their own
/// `Arc<MaturationConfig>` snapshot taken at pass start.
#[cfg(unix)]
pub fn spawn_sighup_handler(shared: SharedMaturationConfig) {
    tokio::spawn(async move {
        let mut signal = match tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::hangup(),
        ) {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "failed to register SIGHUP handler — hot-reload disabled");
                return;
            }
        };

        info!("SIGHUP handler registered for maturation config hot-reload");

        loop {
            signal.recv().await;
            info!("SIGHUP received — reloading maturation config from tokens.toml");

            let Some(tokens_path) = find_tokens_path() else {
                warn!("SIGHUP reload: tokens.toml not found — retaining current config");
                continue;
            };

            if let Some(new_config) = reload_maturation_config(&tokens_path) {
                let enabled_str = if new_config.enabled { "enabled" } else { "disabled" };
                info!(
                    enabled = enabled_str,
                    decay_factor = new_config.decay_factor,
                    interval_hours = new_config.semantic_check_interval_hours,
                    "maturation config reloaded successfully via SIGHUP"
                );
                shared.store(Arc::new(new_config));
            }
            // On None: warning already logged by reload_maturation_config.
        }
    });
}

/// No-op on non-Unix platforms (Windows doesn't have SIGHUP).
#[cfg(not(unix))]
pub fn spawn_sighup_handler(_shared: SharedMaturationConfig) {
    info!("SIGHUP handler not available on this platform — hot-reload disabled");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp_file(content: &str) -> PathBuf {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("kira-reload-test-{}.toml", uuid::Uuid::now_v7()));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn reload_valid_toml_returns_config() {
        let content = r#"
[time]
tool_timeout_default = 30
bash_timeout_max = 300
hook_timeout_default = 5
agent_timeout_default = 3600
agent_heartbeat_interval = 30
agent_stale_after_minutes = 5
plan_ttl_default = 3600
worktree_cleanup_after_min = 10
breaker_window = 60
breaker_open_duration = 30
heartbeat_scanner_period = 60
sse_keepalive = 15
eval_poll_interval_ms = 500
eval_default_timeout = 300
session_rate_limit_per_min = 30
approval_timeout_secs = 300

[budget]
monthly_hard_stop_usd = 50.0
warn_pct = 0.8
agent_default_usd = 1.0

[concurrency]
max_worktrees = 2
max_agent_iters = 20
max_agent_iters_yolo = 50
max_agent_iters_quick = 5
max_agent_iters_small = 10
max_agent_iters_complex = 30
breaker_failure_threshold = 5
stall_same_tool_threshold = 3
max_agents = 4

[thresholds]
faithfulness_pass_below = 0.3
faithfulness_escalate = 0.7
judge_pass_score = 0.7
change_sufficiency_min = 0.5
change_sufficiency_max = 0.9
lesson_promote_count = 3
inject_top_n = 5
embed_cache_max_entries = 10000
meta_review_coverage = 0.8

[providers]

[tiers]
heavy = { candidates = ["deepseek:deepseek-chat"] }
standard = { candidates = ["deepseek:deepseek-chat"] }
light = { candidates = ["deepseek:deepseek-chat"] }
long_context = { candidates = ["deepseek:deepseek-chat"] }

[pricing]

[models]

[paths]
state_db_mac = "~/.local/share/kira/state.redb"
state_db_linux = "~/.local/share/kira/state.redb"
worktrees_subdir = ".kira/worktrees"
mailbox_subdir = ".kira/mailbox"
runs_subdir = ".kira/runs"
prompts_dir = "core/agents/prompts"
workflows_dir = "core/workflows"
branch_prefix = "kira/"
wt_prefix = "kira-wt-"

[bind]
host_dev = "127.0.0.1"
port_default = 7777

[embedding]
model = "text-embedding-3-small"

[safety]
prompt_injection_keywords = []
prompt_injection_levenshtein = 2
bash_executable_allowlist = []
bash_forbidden_chars = []
file_read_max_bytes = 1048576
file_write_max_bytes = 1048576

[text_pipeline]
chunker_target_tokens = 512
chunker_overlap_tokens = 64
rerank_top_k = 5

[memory]
backend = "sqlite_vec"

[memory.maturation]
enabled = false
decay_factor = 0.9
"#;

        let path = write_temp_file(content);
        let result = reload_maturation_config(&path);
        let _ = std::fs::remove_file(&path);

        assert!(result.is_some());
        let config = result.unwrap();
        assert!(!config.enabled);
        assert_eq!(config.decay_factor, 0.9);
    }

    #[test]
    fn reload_invalid_toml_returns_none() {
        let path = write_temp_file("this is not valid { toml [[[");
        let result = reload_maturation_config(&path);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_none());
    }

    #[test]
    fn reload_nonexistent_file_returns_none() {
        let result = reload_maturation_config(Path::new("/nonexistent/path/tokens.toml"));
        assert!(result.is_none());
    }

    #[test]
    fn shared_config_load_and_store() {
        let initial = MaturationConfig {
            enabled: true,
            decay_factor: 0.95,
            ..Default::default()
        };
        let shared = shared_config(initial);

        // Load returns the initial config.
        let loaded = shared.load();
        assert!(loaded.enabled);
        assert_eq!(loaded.decay_factor, 0.95);

        // Store a new config.
        let new_config = MaturationConfig {
            enabled: false,
            decay_factor: 0.8,
            ..Default::default()
        };
        shared.store(Arc::new(new_config));

        // Load returns the new config.
        let loaded = shared.load();
        assert!(!loaded.enabled);
        assert_eq!(loaded.decay_factor, 0.8);
    }
}
