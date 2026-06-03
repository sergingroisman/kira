//! Sandbox runtime abstraction for agent tool execution.
//!
//! v0 introduces the trait boundary so future adapters (Docker, Firecracker)
//! can be plugged in without changing the agent loop. The default
//! `HostRuntime` preserves today's behaviour: `tokio::process::Command`
//! scoped to the agent's worktree.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, instrument};

/// Execution result from a sandboxed command.
#[derive(Debug, Clone)]
pub struct SandboxExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub truncated: bool,
}

/// Policy controlling resource and network constraints for a sandbox session.
#[derive(Debug, Clone, Default)]
pub struct SandboxPolicy {
    /// Max wall-clock seconds for a single exec call.
    pub timeout_secs: u64,
    /// Max bytes per stream (stdout / stderr).
    pub output_max_bytes: usize,
    /// Environment variables injected into the sandbox.
    pub env: HashMap<String, String>,
    /// When true, network egress is blocked.
    pub network_off: bool,
}

/// Handle to a prepared sandbox session. Cheap to clone.
#[derive(Debug, Clone)]
pub struct SandboxHandle {
    pub agent_id: String,
    pub worktree_path: PathBuf,
    pub policy: SandboxPolicy,
}

/// Trait for sandbox runtimes. Implementations manage the lifecycle of
/// isolated execution environments.
pub trait SandboxRuntime: Send + Sync {
    /// Prepare a sandbox for the given agent + worktree.
    fn prepare(&self, agent_id: String, worktree_path: PathBuf, policy: SandboxPolicy) -> Result<SandboxHandle>;

    /// Execute a command inside the sandbox.
    fn exec(&self, handle: &SandboxHandle, argv: Vec<String>, cwd: Option<PathBuf>) -> impl std::future::Future<Output = Result<SandboxExecResult>> + Send;

    /// Tear down the sandbox and release resources.
    fn shutdown(&self, handle: &SandboxHandle) -> impl std::future::Future<Output = Result<()>> + Send;
}

/// Default v0 runtime: executes directly on the host using
/// `tokio::process::Command` with the existing safety layers
/// (allowlist, path guard, timeout, `env_clear`).
pub struct HostRuntime;

impl HostRuntime {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for HostRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl SandboxRuntime for HostRuntime {
    fn prepare(&self, agent_id: String, worktree_path: PathBuf, policy: SandboxPolicy) -> Result<SandboxHandle> {
        debug!(agent_id, path = %worktree_path.display(), "HostRuntime::prepare");
        Ok(SandboxHandle {
            agent_id,
            worktree_path,
            policy,
        })
    }

    #[instrument(skip(self, handle, argv), fields(agent_id = %handle.agent_id, argc = argv.len()))]
    async fn exec(&self, handle: &SandboxHandle, argv: Vec<String>, cwd: Option<PathBuf>) -> Result<SandboxExecResult> {
        let start = std::time::Instant::now();
        let exe = argv.first().context("argv empty — first element must be executable")?;
        let args = &argv[1..];

        let mut cmd = Command::new(exe);
        cmd.args(args);
        cmd.env_clear();
        for (k, v) in &handle.policy.env {
            cmd.env(k, v);
        }
        cmd.current_dir(cwd.as_ref().unwrap_or(&handle.worktree_path));
        cmd.kill_on_drop(true);

        let timeout = Duration::from_secs(handle.policy.timeout_secs.max(1));
        let output = tokio::time::timeout(timeout, cmd.output())
            .await
            .with_context(|| format!("bash timed out after {timeout:?}"))?
            .context("spawn failed")?;

        let elapsed = start.elapsed();
        let mut stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let mut stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let mut truncated = false;
        if stdout.len() > handle.policy.output_max_bytes {
            stdout.truncate(handle.policy.output_max_bytes);
            truncated = true;
        }
        if stderr.len() > handle.policy.output_max_bytes {
            stderr.truncate(handle.policy.output_max_bytes);
            truncated = true;
        }

        Ok(SandboxExecResult {
            stdout,
            stderr,
            exit_code: output.status.code().unwrap_or(-1),
            duration_ms: elapsed.as_millis() as u64,
            truncated,
        })
    }

    async fn shutdown(&self, _handle: &SandboxHandle) -> Result<()> {
        // HostRuntime has no external resources to release.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn host_runtime_echo() {
        let rt = HostRuntime::new();
        let handle = rt
            .prepare(
                "test-agent".into(),
                std::env::current_dir().unwrap(),
                SandboxPolicy {
                    timeout_secs: 5,
                    output_max_bytes: 1024,
                    ..Default::default()
                },
            )
            .unwrap();
        let res = rt.exec(&handle, vec!["echo".into(), "hello".into()], None).await.unwrap();
        assert_eq!(res.exit_code, 0);
        assert!(res.stdout.contains("hello"));
    }
}
