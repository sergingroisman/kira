//! Per-agent context-window snapshot — wire type.
//!
//! Mirrors the Claude Code "Janela de contexto" panel (categories +
//! percentages + autocompact buffer + free space) so the REPL can render
//! the same breakdown regardless of which LLM backs the agent. KIRA is
//! LLM-agnostic, but each model has its own `context_limit` (Qwen2.5:3b
//! has 32k, deepseek-v4-pro has 128k, etc.). The snapshot resolves the
//! limit from `tokens.toml [models.<name>].context_limit` and reports
//! percentages against that ceiling.
//!
//! v0 (this commit): type + endpoint shape locked. Real per-category
//! counters wire in later passes — for now everything except `messages`
//! is zero and `free_space` carries the entire unused budget.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Single category breakdown row — `name` matches the labels in the UI
/// reference (Messages, System tools, Skills, MCP tools, Custom agents,
/// Memory files, System prompt, MCP tools (deferred), System tools
/// (deferred), Autocompact buffer, Free space).
#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/context_window.ts")]
pub struct ContextCategory {
    pub name: String,
    pub tokens: u64,
    /// Percentage of `context_limit`, in `[0.0, 100.0]`.
    pub pct: f64,
    /// Optional secondary count — e.g. "208" tool definitions or "9"
    /// custom agents in the reference UI. `None` when the row has no
    /// natural per-item count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_count: Option<u32>,
    /// Whether this category is currently *active* (counts against the
    /// live window) or *deferred* (registered but loaded on demand —
    /// MCP tools off, optional skills). The Claude Code panel renders
    /// deferred rows below active ones with a different color.
    #[serde(default)]
    pub deferred: bool,
}

/// Full snapshot returned by `GET /v1/agents/{id}/context`.
#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/context_window.ts")]
pub struct ContextWindowSnapshot {
    pub agent_id: String,
    /// Model used by the agent on its last iteration. Drives `context_limit`.
    pub model: String,
    /// Hard limit for the model (e.g. 131_072 for 128k, 32_768 for Qwen 3b).
    pub context_limit: u64,
    /// Sum of `categories[*].tokens` across non-deferred categories.
    pub used: u64,
    /// `context_limit - used` (saturating at 0).
    pub free: u64,
    /// `used / context_limit * 100` rounded to two decimals.
    pub used_pct: f64,
    /// Same order as the reference UI: active rows first, then deferred,
    /// then the synthetic `Free space` row at the end. Always non-empty.
    pub categories: Vec<ContextCategory>,
    /// RFC3339 timestamp of the iteration this snapshot was computed for.
    pub computed_at: String,
}

impl ContextWindowSnapshot {
    /// Build a snapshot from raw per-category token counts. Computes
    /// `used`, `free`, `used_pct`, and appends a synthetic `Free space`
    /// row. Caller controls the order of `categories`; we just append.
    pub fn build(
        agent_id: String,
        model: String,
        context_limit: u64,
        mut categories: Vec<ContextCategory>,
        computed_at: String,
    ) -> Self {
        // Recompute pct so caller can pass tokens-only and not worry.
        let lim = context_limit.max(1) as f64;
        for c in &mut categories {
            c.pct = round2(c.tokens as f64 / lim * 100.0);
        }
        let used: u64 = categories
            .iter()
            .filter(|c| !c.deferred)
            .map(|c| c.tokens)
            .sum();
        let free = context_limit.saturating_sub(used);
        categories.push(ContextCategory {
            name: "Free space".to_string(),
            tokens: free,
            pct: round2(free as f64 / lim * 100.0),
            item_count: None,
            deferred: false,
        });
        Self {
            agent_id,
            model,
            context_limit,
            used,
            free,
            used_pct: round2(used as f64 / lim * 100.0),
            categories,
            computed_at,
        }
    }
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(name: &str, tokens: u64, deferred: bool) -> ContextCategory {
        ContextCategory {
            name: name.into(),
            tokens,
            pct: 0.0, // overwritten by build()
            item_count: None,
            deferred,
        }
    }

    #[test]
    fn build_computes_used_and_free() {
        let snap = ContextWindowSnapshot::build(
            "a".into(),
            "deepseek-v4-flash".into(),
            1000,
            vec![cat("Messages", 400, false), cat("System tools", 100, false)],
            "ts".into(),
        );
        assert_eq!(snap.used, 500);
        assert_eq!(snap.free, 500);
        assert!((snap.used_pct - 50.0).abs() < 0.01);
        let free_row = snap.categories.last().unwrap();
        assert_eq!(free_row.name, "Free space");
        assert_eq!(free_row.tokens, 500);
    }

    #[test]
    fn build_excludes_deferred_from_used() {
        let snap = ContextWindowSnapshot::build(
            "a".into(),
            "qwen2.5:3b".into(),
            10_000,
            vec![
                cat("Messages", 2000, false),
                cat("MCP tools (deferred)", 1500, true),
                cat("System tools (deferred)", 500, true),
            ],
            "ts".into(),
        );
        assert_eq!(snap.used, 2000, "deferred categories must NOT count");
        assert_eq!(snap.free, 8000);
    }

    #[test]
    fn build_caps_free_at_zero_on_overflow() {
        let snap = ContextWindowSnapshot::build(
            "a".into(),
            "tiny".into(),
            1000,
            vec![cat("Messages", 1500, false)],
            "ts".into(),
        );
        assert_eq!(snap.free, 0);
    }

    #[test]
    fn build_recomputes_pct_on_each_category() {
        let snap = ContextWindowSnapshot::build(
            "a".into(),
            "m".into(),
            2000,
            vec![cat("Messages", 500, false)],
            "ts".into(),
        );
        let msgs = snap.categories.iter().find(|c| c.name == "Messages").unwrap();
        assert!((msgs.pct - 25.0).abs() < 0.01);
    }

    #[test]
    fn build_appends_exactly_one_free_row() {
        let snap = ContextWindowSnapshot::build(
            "a".into(),
            "m".into(),
            1000,
            vec![cat("Messages", 100, false), cat("Skills", 50, false)],
            "ts".into(),
        );
        let free_rows: Vec<_> = snap.categories.iter().filter(|c| c.name == "Free space").collect();
        assert_eq!(free_rows.len(), 1);
    }
}
