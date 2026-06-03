//! Parse reviewer findings JSON and render REVIEW.md.
//!
//! Deterministic — no LLM call. Inputs are the per-agent `result` strings
//! (reviewer.md schema) plus a list of `AgentReviewOutcome`s. Output is a
//! markdown report grouped by severity in TRUST/Fakeflix order.

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::proto::review::AgentReviewOutcome;

#[derive(Debug, Clone, Deserialize)]
pub struct Finding {
    #[serde(default)]
    pub dimension: String,
    pub rule_id: String,
    #[serde(default)]
    pub rule_source: String,
    pub file: String,
    #[serde(default)]
    pub line_start: u32,
    #[serde(default)]
    pub line_end: u32,
    pub severity: String,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    pub claim: String,
    #[serde(default)]
    pub evidence_quote: String,
    #[serde(default)]
    pub recommendation: String,
}

const fn default_confidence() -> f64 { 0.0 }

#[derive(Debug, Clone, Deserialize)]
pub struct ReviewerOutput {
    #[serde(default)]
    pub dimension: String,
    #[serde(default)]
    pub highlight: String,
    #[serde(default)]
    pub findings: Vec<Finding>,
}

/// Parse a reviewer's JSON result. Tolerant to surrounding prose; extracts
/// the outermost JSON object. Attaches the dimension to every finding.
pub fn parse_findings(raw: &str, dimension: &str) -> Result<ReviewerOutput> {
    let json_str = if raw.trim_start().starts_with('{') {
        raw.trim().to_string()
    } else {
        extract_json_blob(raw).context("no JSON object found in reviewer output")?
    };
    let mut out: ReviewerOutput =
        serde_json::from_str(&json_str).context("parse reviewer JSON")?;
    for f in &mut out.findings {
        if f.dimension.is_empty() {
            f.dimension = dimension.to_string();
        }
    }
    if out.dimension.is_empty() {
        out.dimension = dimension.to_string();
    }
    Ok(out)
}

fn extract_json_blob(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end > start { Some(s[start..=end].to_string()) } else { None }
}

const SEVERITY_ORDER: &[&str] = &[
    "security",
    "critical",
    "performance",
    "warning",
    "suggestion",
];

const SEVERITY_EMOJI: &[(&str, &str)] = &[
    ("security", "🔒"),
    ("critical", "🚨"),
    ("performance", "⚡"),
    ("warning", "⚠️"),
    ("suggestion", "💡"),
];

fn emoji_for(sev: &str) -> &'static str {
    SEVERITY_EMOJI
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(sev))
        .map_or("•", |(_, e)| *e)
}

/// Render the final markdown report.
pub fn render_review_md(
    run_id: &str,
    head: &str,
    base: &str,
    outcomes: &[AgentReviewOutcome],
    findings: &[Finding],
) -> String {
    let total = findings.len();
    let total_files: std::collections::HashSet<&str> =
        findings.iter().map(|f| f.file.as_str()).collect();
    let agents_n = outcomes.len();
    let agents_passed = outcomes.iter().filter(|o| o.gate_overall == "pass").count();
    let agents_failed = outcomes.iter().filter(|o| o.gate_overall == "fail").count();

    let mut out = String::new();
    out.push_str(&format!("# Kira Review — {head} vs {base}\n\n"));
    out.push_str(&format!("> run_id: `{run_id}` · workflow: pr-review · TRUST-inspired multi-dim\n\n"));
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Value |\n|---|---|\n");
    out.push_str(&format!("| Reviewer agents | {agents_n} |\n"));
    out.push_str(&format!("| Agents passed meta-review | {agents_passed} |\n"));
    out.push_str(&format!("| Agents halted at gate | {agents_failed} |\n"));
    out.push_str(&format!("| Findings (after anti-hallucination) | {total} |\n"));
    out.push_str(&format!("| Files with findings | {} |\n\n", total_files.len()));

    // Per-agent table
    out.push_str("## Agents\n\n");
    out.push_str("| Dimension | Status | Gate | Failed at | Kept | Dropped | Duration |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for o in outcomes {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {}ms |\n",
            o.dimension,
            o.status,
            o.gate_overall,
            o.gate_failed_at.clone().unwrap_or_else(|| "—".into()),
            o.findings_kept,
            o.findings_dropped,
            o.duration_ms
        ));
    }
    out.push('\n');

    // Findings grouped by severity.
    if findings.is_empty() {
        out.push_str("## ✅ No findings\n\nNo issues survived the anti-hallucination meta-gate. Either the diff is clean or every reviewer hallucinated.\n");
        return out;
    }

    out.push_str("## Findings\n\n");
    for sev in SEVERITY_ORDER {
        let group: Vec<&Finding> = findings
            .iter()
            .filter(|f| f.severity.eq_ignore_ascii_case(sev))
            .collect();
        if group.is_empty() {
            continue;
        }
        out.push_str(&format!("### {} {} ({})\n\n", emoji_for(sev), capitalize(sev), group.len()));
        for f in group {
            out.push_str(&format!(
                "- **[`{file}:{l1}`]** [{dim}] {claim}\n",
                file = f.file,
                l1 = f.line_start,
                dim = f.dimension,
                claim = f.claim,
            ));
            if !f.rule_id.is_empty() {
                out.push_str(&format!(
                    "  - rule: `{}`{}\n",
                    f.rule_id,
                    if f.rule_source.is_empty() {
                        String::new()
                    } else {
                        format!(" · source: `{}`", f.rule_source)
                    }
                ));
            }
            if !f.evidence_quote.is_empty() {
                out.push_str(&format!("  - evidence: `{}`\n", trim_quote(&f.evidence_quote)));
            }
            if !f.recommendation.is_empty() {
                out.push_str(&format!("  - fix: {}\n", f.recommendation));
            }
            out.push_str(&format!("  - confidence: {:.2}\n", f.confidence));
        }
        out.push('\n');
    }

    out.push_str("---\n");
    out.push_str("\n> Findings without literal evidence in the diff were dropped by the meta-review gate (L6). Humans decide; AI suggests.\n");
    out
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + c.as_str(),
        None => String::new(),
    }
}

fn trim_quote(q: &str) -> String {
    let one_line = q.replace('\n', " ⏎ ");
    if one_line.len() > 120 {
        format!("{}…", one_line.chars().take(120).collect::<String>())
    } else {
        one_line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clean_json() {
        let raw = r#"{
            "dimension": "security",
            "highlight": "Inputs validated upfront",
            "findings": [
                {"rule_id":"SEC-001","rule_source":"AGENTS.md#sec","file":"a.ts","line_start":3,"line_end":3,"severity":"security","confidence":0.95,"claim":"raw query","evidence_quote":"SELECT *","recommendation":"parameterize"}
            ]
        }"#;
        let out = parse_findings(raw, "security").unwrap();
        assert_eq!(out.findings.len(), 1);
        assert_eq!(out.findings[0].rule_id, "SEC-001");
    }

    #[test]
    fn parse_with_surrounding_prose() {
        let raw = "Sure here is the JSON:\n{\"findings\":[]}\nThanks.";
        let out = parse_findings(raw, "x").unwrap();
        assert!(out.findings.is_empty());
    }

    #[test]
    fn render_no_findings() {
        let md = render_review_md("rid", "feat", "main", &[], &[]);
        assert!(md.contains("No findings"));
    }

    #[test]
    fn render_groups_by_severity() {
        let f = vec![
            Finding {
                dimension: "security".into(),
                rule_id: "SEC-001".into(),
                rule_source: "AGENTS.md".into(),
                file: "a.ts".into(),
                line_start: 1,
                line_end: 1,
                severity: "security".into(),
                confidence: 0.9,
                claim: "raw sql".into(),
                evidence_quote: "SELECT *".into(),
                recommendation: "param".into(),
            },
            Finding {
                dimension: "performance".into(),
                rule_id: "PERF-002".into(),
                rule_source: String::new(),
                file: "b.ts".into(),
                line_start: 5,
                line_end: 5,
                severity: "performance".into(),
                confidence: 0.85,
                claim: "n+1".into(),
                evidence_quote: "for (id of ids)".into(),
                recommendation: "batch".into(),
            },
        ];
        let md = render_review_md("rid", "h", "b", &[], &f);
        assert!(md.contains("🔒 Security (1)"));
        assert!(md.contains("⚡ Performance (1)"));
        assert!(md.contains("SEC-001"));
        assert!(md.contains("PERF-002"));
    }
}
