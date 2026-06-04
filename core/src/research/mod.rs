//! Dynamic research planning (v1) — preflight + parallel fan-out.
//!
//! A reactive "search → fetch → search → fetch" loop inside a Writer agent
//! burns iterations (observed: 12 iters, mostly 404s from guessed URLs, then
//! Failed with no output). This module front-loads the decision:
//!
//! 1. **Preflight** (1 cheap LLM call): decide WHAT to search (`queries`) and
//!    HOW MANY sources to read (`max_sources`) — scaled to the goal's breadth.
//! 2. **Search** every query in parallel; collect REAL result URLs (dedup, cap).
//! 3. **Fetch** those URLs in parallel (`web_fetch` is a deterministic tool — no
//!    LLM/agent per source). Aggregate into research notes.
//!
//! The caller then hands the notes to a single Writer to synthesize, with no
//! web tools. Net cost: ~2 LLM calls + parallel I/O, vs a 12-iter spiral.

use anyhow::{Context, Result};
use futures::future::join_all;
use serde::Deserialize;
use tracing::{debug, warn};

use crate::config::WebToolsConfig;
use crate::llm::types::ChatMessageUnified;
use crate::router::Router;
use crate::web::search::{self, SearchOpts};
use crate::web::fetch;

const PLANNER_PROMPT: &str = include_str!("planner_prompt.md");

/// Hard ceiling on sources regardless of the plan (cost guard).
const MAX_SOURCES_CAP: usize = 8;
/// Hard ceiling on distinct search queries.
const MAX_QUERIES: usize = 5;
/// Default sources when the planner omits / zeroes the field.
const DEFAULT_SOURCES: usize = 4;
/// Per-source content cap (chars) to bound the synthesis prompt size.
const PER_SOURCE_CHARS: usize = 4000;
/// Results requested per query before dedup/cap.
const RESULTS_PER_QUERY: u32 = 5;

/// A research plan emitted by the preflight LLM call.
#[derive(Deserialize, Default, Debug, Clone)]
pub struct ResearchPlan {
    #[serde(default)]
    pub queries: Vec<String>,
    #[serde(default)]
    pub max_sources: usize,
}

/// Ask the planner model WHAT to search + HOW MANY sources. `standard` tier
/// (capable + cheap); the local 3B emits malformed JSON too often.
pub async fn plan_research(router: &Router, goal: &str) -> Result<ResearchPlan> {
    let messages = vec![
        ChatMessageUnified::System { content: PLANNER_PROMPT.to_string() },
        ChatMessageUnified::User {
            content: format!("GOAL: {goal}\n\nEmit the JSON now."),
        },
    ];
    let (output, _provider, model) = router
        .route_chat_with_tools("standard", None, messages, Vec::new(), None)
        .await
        .context("research planner LLM call failed")?;
    debug!(%model, chars = output.content.len(), "research plan response");
    Ok(parse_plan(&output.content))
}

/// Parse + sanitize the planner JSON. Tolerant of fences / noise; clamps to
/// the hard caps. Malformed → empty plan (caller skips research).
fn parse_plan(raw: &str) -> ResearchPlan {
    let cleaned = strip_code_fences(raw.trim());
    let mut plan: ResearchPlan = match serde_json::from_str(&cleaned) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "research plan JSON parse failed — empty plan");
            return ResearchPlan::default();
        }
    };
    plan.queries.retain(|q| !q.trim().is_empty());
    plan.queries.truncate(MAX_QUERIES);
    if plan.max_sources == 0 {
        plan.max_sources = DEFAULT_SOURCES;
    }
    plan.max_sources = plan.max_sources.min(MAX_SOURCES_CAP);
    plan
}

/// Run every query in parallel, collect deduped result URLs up to
/// `plan.max_sources`. Returns `(url, title)` pairs.
pub async fn gather_urls(cfg: &WebToolsConfig, plan: &ResearchPlan) -> Vec<(String, String)> {
    let opts = SearchOpts {
        max_results: Some(RESULTS_PER_QUERY),
        language: None,
    };
    let futs = plan
        .queries
        .iter()
        .map(|q| async { search::search(cfg, q, &opts).await.unwrap_or_default() });
    let batches = join_all(futs).await;

    let mut seen = std::collections::HashSet::new();
    let mut urls = Vec::new();
    for batch in batches {
        for r in batch {
            if seen.insert(r.url.clone()) {
                urls.push((r.url, r.title));
                if urls.len() >= plan.max_sources {
                    return urls;
                }
            }
        }
    }
    urls
}

/// Fetch all URLs in parallel, aggregate into a single notes markdown.
/// Failed fetches are skipped silently (best-effort).
pub async fn fetch_notes(cfg: &WebToolsConfig, urls: Vec<(String, String)>) -> String {
    let futs = urls.into_iter().map(|(url, title)| async move {
        match fetch::fetch(cfg, &url).await {
            Ok(r) => {
                let body: String = r.markdown.chars().take(PER_SOURCE_CHARS).collect();
                Some(format!("## {title}\nSource: {url}\n\n{body}\n"))
            }
            Err(_) => None,
        }
    });
    join_all(futs)
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n---\n")
}

/// Full research pass: plan → search → fetch → notes. Empty string if the
/// planner produced no queries (caller should fall back to direct answering).
pub async fn run_research(router: &Router, cfg: &WebToolsConfig, goal: &str) -> Result<String> {
    let plan = plan_research(router, goal).await?;
    if plan.queries.is_empty() {
        debug!("research: empty plan, skipping");
        return Ok(String::new());
    }
    let urls = gather_urls(cfg, &plan).await;
    debug!(sources = urls.len(), queries = plan.queries.len(), "research: fetching sources");
    Ok(fetch_notes(cfg, urls).await)
}

/// Strip a leading/trailing ```code fence``` if present (LLMs love them).
fn strip_code_fences(s: &str) -> String {
    let s = s.trim();
    if !s.starts_with("```") {
        return s.to_string();
    }
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() < 2 {
        return s.to_string();
    }
    let start = usize::from(lines[0].starts_with("```"));
    let end = if *lines.last().unwrap_or(&"") == "```" {
        lines.len() - 1
    } else {
        lines.len()
    };
    lines[start..end].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_plan() {
        let raw = r#"{"queries": ["a", "b"], "max_sources": 5}"#;
        let p = parse_plan(raw);
        assert_eq!(p.queries, vec!["a", "b"]);
        assert_eq!(p.max_sources, 5);
    }

    #[test]
    fn strips_code_fence() {
        let raw = "```json\n{\"queries\": [\"x\"], \"max_sources\": 2}\n```";
        let p = parse_plan(raw);
        assert_eq!(p.queries, vec!["x"]);
        assert_eq!(p.max_sources, 2);
    }

    #[test]
    fn clamps_sources_to_cap() {
        let raw = r#"{"queries": ["a"], "max_sources": 99}"#;
        assert_eq!(parse_plan(raw).max_sources, MAX_SOURCES_CAP);
    }

    #[test]
    fn defaults_sources_when_zero() {
        let raw = r#"{"queries": ["a"], "max_sources": 0}"#;
        assert_eq!(parse_plan(raw).max_sources, DEFAULT_SOURCES);
    }

    #[test]
    fn caps_queries_and_drops_blanks() {
        let raw = r#"{"queries": ["a","","b","c","d","e","f"], "max_sources": 3}"#;
        let p = parse_plan(raw);
        assert_eq!(p.queries.len(), MAX_QUERIES);
        assert!(!p.queries.iter().any(|q| q.is_empty()));
    }

    #[test]
    fn malformed_json_is_empty_plan() {
        let p = parse_plan("not json");
        assert!(p.queries.is_empty());
    }
}
