//! SC5 — Permission ruleset for tool calls.
//!
//! v0 supports three actions per rule:
//!   - `Allow`  → execute without approval (bypasses HIGH-risk gate)
//!   - `Block`  → reject immediately (treated like CRITICAL)
//!   - `Ask`    → go through the approval gate (default for unmatched HIGH tools)
//!
//! Rules are ordered: first match wins. A default rule can be set for
//! each tool name. Wildcards use simple prefix/suffix matching (* at
//! start or end only) — enough for path globs and command prefixes.

use serde::{Deserialize, Serialize};
use tracing::debug;

/// Action taken when a rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Block,
    Ask,
}

/// A single permission rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Tool name to match. Wildcard `*` matches any tool.
    pub tool: String,
    /// Optional JSON-path-like filter on arguments.
    /// Examples:
    ///   - `"path"`          → check the `path` argument
    ///   - `"argv.0"`        → check argv[0]
    ///   - `"url"`           → check the `url` argument
    #[serde(default)]
    pub arg: Option<String>,
    /// Value pattern to match. Simple wildcards:
    ///   - `src/content/**`  → prefix match
    ///   - `*.md`            → suffix match
    ///   - `git*`            → prefix match
    #[serde(default)]
    pub pattern: Option<String>,
    pub action: RuleAction,
    /// Human-readable reason (logged when rule fires).
    pub reason: String,
}

/// Ordered list of rules. First match wins.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionRuleset {
    pub rules: Vec<PermissionRule>,
}

impl PermissionRuleset {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, rule: PermissionRule) {
        self.rules.push(rule);
    }

    /// Evaluate a tool call against the ruleset.
    /// Returns the action of the first matching rule, or `Ask` if none match.
    pub fn evaluate(&self, tool_name: &str, arguments: &str) -> RuleAction {
        for rule in &self.rules {
            if !match_tool(&rule.tool, tool_name) {
                continue;
            }
            if let Some(ref arg) = rule.arg
                && let Some(ref pat) = rule.pattern
                    && !match_arg(arguments, arg, pat) {
                        continue;
                    }
            debug!(
                tool = %tool_name,
                rule_tool = %rule.tool,
                action = ?rule.action,
                reason = %rule.reason,
                "permission rule matched"
            );
            return rule.action;
        }
        RuleAction::Ask
    }
}

fn match_tool(rule_tool: &str, actual: &str) -> bool {
    if rule_tool == "*" {
        return true;
    }
    rule_tool == actual
}

/// Extract a value from JSON arguments by a simple dotted path.
/// `path` examples: `"path"`, `"argv.0"`, `"url"`.
fn match_arg(arguments: &str, path: &str, pattern: &str) -> bool {
    let parsed: serde_json::Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let mut current = &parsed;
    for key in path.split('.') {
        if let Ok(idx) = key.parse::<usize>() {
            current = match current.get(idx) {
                Some(v) => v,
                None => return false,
            };
        } else {
            current = match current.get(key) {
                Some(v) => v,
                None => return false,
            };
        }
    }
    let value = current.as_str().unwrap_or("");
    match_wildcard(pattern, value)
}

/// Simple wildcard matching: `*` at start or end only.
fn match_wildcard(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    // Directory glob: prefix/** matches anything under that prefix.
    if pattern.contains("/**") {
        let prefix = pattern.trim_end_matches("/**");
        return value.starts_with(prefix);
    }
    if pattern.starts_with('*') && pattern.ends_with('*') && pattern.len() > 2 {
        let mid = &pattern[1..pattern.len() - 1];
        return value.contains(mid);
    }
    if pattern.starts_with('*') {
        return value.ends_with(&pattern[1..]);
    }
    if pattern.ends_with('*') {
        return value.starts_with(&pattern[..pattern.len() - 1]);
    }
    pattern == value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ruleset() -> PermissionRuleset {
        let mut rs = PermissionRuleset::new();
        rs.add(PermissionRule {
            tool: "file_write".into(),
            arg: Some("path".into()),
            pattern: Some("src/content/**".into()),
            action: RuleAction::Allow,
            reason: "content files are agent-owned".into(),
        });
        rs.add(PermissionRule {
            tool: "bash".into(),
            arg: Some("argv.0".into()),
            pattern: Some("rm*".into()),
            action: RuleAction::Block,
            reason: "rm is forbidden".into(),
        });
        rs.add(PermissionRule {
            tool: "bash".into(),
            arg: Some("argv.0".into()),
            pattern: Some("git".into()),
            action: RuleAction::Allow,
            reason: "git is safe".into(),
        });
        rs
    }

    #[test]
    fn allow_content_path() {
        let rs = ruleset();
        let action = rs.evaluate("file_write", r#"{"path":"src/content/blog/post.md"}"#);
        assert_eq!(action, RuleAction::Allow);
    }

    #[test]
    fn block_rm() {
        let rs = ruleset();
        let action = rs.evaluate("bash", r#"{"argv":["rm","-rf","/"]}"#);
        assert_eq!(action, RuleAction::Block);
    }

    #[test]
    fn allow_git() {
        let rs = ruleset();
        let action = rs.evaluate("bash", r#"{"argv":["git","status"]}"#);
        assert_eq!(action, RuleAction::Allow);
    }

    #[test]
    fn unmatched_asks() {
        let rs = ruleset();
        let action = rs.evaluate("bash", r#"{"argv":["cargo","test"]}"#);
        assert_eq!(action, RuleAction::Ask);
    }

    #[test]
    fn wildcard_tool_matches_all() {
        let mut rs = PermissionRuleset::new();
        rs.add(PermissionRule {
            tool: "*".into(),
            arg: None,
            pattern: None,
            action: RuleAction::Block,
            reason: "deny all".into(),
        });
        assert_eq!(rs.evaluate("anything", "{}"), RuleAction::Block);
    }
}
