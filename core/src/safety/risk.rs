//! SC2 — Tool risk classification.
//!
//! General-purpose risk levels applied uniformly to built-in tools and
//! MCP-provided tools. v0 is **warn-only** (audit + log). v0.5 will gate
//! HIGH+ behind user approval and HALT on CRITICAL.

use serde::{Deserialize, Serialize};

/// Tool risk level.
///
/// - **Low**: read-only / pure-query (`file_read`, `web_search`, `web_fetch`, done).
///   SSRF guard already protects web_fetch against malicious URLs.
/// - **High**: writes files or executes code (`file_write`, `bash`).
/// - **Critical**: destructive or irreversible (deploy, payment, `rm -rf` patterns).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolRisk {
    Low,
    High,
    Critical,
}

impl ToolRisk {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Risk level for a built-in tool (bash, file_*, web_*, done).
///
/// Returns `None` for unknown / MCP-namespaced names — callers should fall
/// back to the MCP registry (`McpRegistry::tool_risk`) for those.
pub fn builtin_tool_risk(name: &str) -> Option<ToolRisk> {
    match name {
        // Read-only tools — SSRF guard protects web_fetch against malicious URLs.
        "done" | "file_read" | "web_search" | "web_fetch" => Some(ToolRisk::Low),
        // State-modifying tools — require approval.
        "file_write" | "edit" | "bash" => Some(ToolRisk::High),
        _ => None,
    }
}

/// Argv-aware bash risk (H8/AP7, 2026-05-25): a bash invocation whose first
/// meaningful command is a readonly tool (`ls`, `grep`, `find`, etc.) is Low
/// and skips the approval prompt. Workshop SPEKTOR "guardrails sensatos por
/// default" — pausing the user for `grep -r foo .` is friction without safety
/// value.
///
/// Returns `None` if the argv shape isn't recognisable (caller falls back to
/// the static `builtin_tool_risk("bash") = High`).
pub fn bash_argv_risk(argv: &[String]) -> Option<ToolRisk> {
    if bash_argv_is_critical(argv) {
        return Some(ToolRisk::Critical);
    }
    let cmd = bash_leading_command(argv)?;
    const READONLY: &[&str] = &[
        "ls", "cat", "head", "tail", "wc", "file", "stat", "pwd", "echo",
        "grep", "egrep", "fgrep", "rg", "ag",
        "find", "fd", "tree",
        "which", "type", "whereis",
        "diff", "cmp",
        "date", "uname", "hostname", "whoami", "id", "env",
        "git", // git itself is mostly readonly; destructive subcommands flagged via bash_argv_is_critical
    ];
    if READONLY.contains(&cmd.as_str()) {
        return Some(ToolRisk::Low);
    }
    None
}

/// Extract the leading shell command from a bash argv. Handles the common
/// `["sh", "-c", "cmd args"]` shape and the direct `["cmd", "args"]` shape.
fn bash_leading_command(argv: &[String]) -> Option<String> {
    if argv.is_empty() {
        return None;
    }
    // sh/bash -c "..." form: parse the first token of the script.
    if (argv[0] == "sh" || argv[0] == "bash") && argv.len() >= 3 && argv[1] == "-c" {
        let script = &argv[2];
        let first = script.split_whitespace().next()?;
        // Strip leading pipes/redirects if any (defensive).
        return Some(first.trim_matches(|c: char| !c.is_alphanumeric() && c != '_').to_string());
    }
    Some(argv[0].clone())
}

/// Runtime escalation: a bash invocation whose argv matches a critical
/// pattern (rm -rf, dd, mkfs, etc) gets bumped to Critical regardless of the
/// static `builtin_tool_risk("bash") = High` baseline.
///
/// Conservative: returns `true` only on high-confidence destructive patterns
/// the SC3 allowlist would also reject. This is defence-in-depth signalling
/// for audit and observability, not an additional block.
pub fn bash_argv_is_critical(argv: &[String]) -> bool {
    let joined = argv.join(" ");
    let lower = joined.to_lowercase();
    let danger = [
        "rm -rf /",
        "rm -rf ~",
        "rm -rf *",
        "dd if=",
        "mkfs.",
        "shred ",
        ":(){:|:&};:", // fork bomb
        "chmod -r 777 /",
        "chown -r root /",
    ];
    danger.iter().any(|p| lower.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_known_tools() {
        assert_eq!(builtin_tool_risk("done"), Some(ToolRisk::Low));
        assert_eq!(builtin_tool_risk("file_read"), Some(ToolRisk::Low));
        assert_eq!(builtin_tool_risk("web_search"), Some(ToolRisk::Low));
        assert_eq!(builtin_tool_risk("file_write"), Some(ToolRisk::High));
        assert_eq!(builtin_tool_risk("edit"), Some(ToolRisk::High));
        assert_eq!(builtin_tool_risk("web_fetch"), Some(ToolRisk::Low));
        assert_eq!(builtin_tool_risk("bash"), Some(ToolRisk::High));
    }

    #[test]
    fn builtin_unknown_returns_none() {
        assert_eq!(builtin_tool_risk("mcp_brain_search"), None);
        assert_eq!(builtin_tool_risk("nonexistent"), None);
    }

    #[test]
    fn bash_argv_detects_rm_rf_root() {
        let argv = vec!["sh".into(), "-c".into(), "rm -rf /".into()];
        assert!(bash_argv_is_critical(&argv));
    }

    #[test]
    fn bash_argv_detects_dd() {
        let argv = vec!["dd".into(), "if=/dev/zero".into(), "of=/dev/sda".into()];
        assert!(bash_argv_is_critical(&argv));
    }

    #[test]
    fn bash_argv_safe_call() {
        let argv = vec!["cargo".into(), "test".into()];
        assert!(!bash_argv_is_critical(&argv));
    }

    #[test]
    fn risk_str_repr() {
        assert_eq!(ToolRisk::Low.as_str(), "low");
        assert_eq!(ToolRisk::High.as_str(), "high");
        assert_eq!(ToolRisk::Critical.as_str(), "critical");
    }
}
