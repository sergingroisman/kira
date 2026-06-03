//! DetectiveRunner — star topology: 6 lens agents in parallel → consolidator.
//!
//! Implements the Detective universal code reviewer. Each lens is a read-only
//! judge-tier sub-agent. The consolidator merges findings into a final verdict
//! (`pass | warn | block`). Language and framework agnostic.

use anyhow::{Context, Result};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{instrument, warn};
use uuid::Uuid;

use crate::proto::agent::{AgentKind, AgentStatus, SpawnAgentRequest};
use crate::proto::detective::{
    DetectiveFinding, DetectiveHighlight, DetectiveLens, DetectiveSummary, DetectiveVerdict,
    RunDetectiveRequest, RunDetectiveResponse,
};
use super::AgentManager;

const LENS_TIMEOUT_SECS: u64 = 180;
const POLL_MS: u64 = 500;

pub struct DetectiveRunner {
    pub agents: AgentManager,
}

impl DetectiveRunner {
    pub fn new(agents: AgentManager) -> Self {
        Self { agents }
    }

    /// Run the full Detective review. Launches lens agents in parallel,
    /// waits for all, then runs the consolidator.
    #[instrument(skip(self, req), fields(lenses = req.lenses.len()))]
    pub async fn run(&self, req: RunDetectiveRequest) -> Result<RunDetectiveResponse> {
        let run_id = Uuid::now_v7().to_string();

        // Resolve diff.
        let diff = match req.diff {
            Some(d) => d,
            None => self.git_diff(req.worktree_id.as_deref()).await?,
        };

        if diff.trim().is_empty() {
            return Ok(empty_response(run_id));
        }

        // Build context block for each lens.
        let context = build_context(&diff, req.spec_path.as_deref(), req.tasks_path.as_deref());

        // Spawn all lens agents in parallel.
        let mut handles: Vec<(DetectiveLens, String)> = Vec::new();
        for lens in &req.lenses {
            let task = format!("{}\n\n---\n\n{}", lens.prompt_text(), context);
            match self.agents.spawn(SpawnAgentRequest {
                kind: AgentKind::Detective,
                task,
                tier: None,
                worktree_id: req.worktree_id.clone(),
                base: None,
                budget_usd: None,
                max_iters: Some(6),
                yolo_mode: Some(true), // read-only, no file writes
                background: Some(true),
                timeout_secs: Some(LENS_TIMEOUT_SECS),
            }).await {
                Ok(a) => handles.push((*lens, a.id)),
                Err(e) => warn!(lens = ?lens, error = %e, "lens agent spawn failed"),
            }
        }

        // Poll all lens agents to completion.
        let mut raw_outputs: Vec<(DetectiveLens, serde_json::Value)> = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(LENS_TIMEOUT_SECS + 30);
        for (lens, agent_id) in handles {
            let uuid = match Uuid::parse_str(&agent_id) {
                Ok(u) => u,
                Err(_) => continue,
            };
            loop {
                if Instant::now() > deadline {
                    warn!(lens = ?lens, "lens agent timeout");
                    break;
                }
                let Some(a) = self.agents.get(uuid) else { break };
                if matches!(a.status, AgentStatus::Completed | AgentStatus::Failed | AgentStatus::Stopped | AgentStatus::Cancelled | AgentStatus::Stale) {
                    // Extract JSON from the agent's `result` field (done("...") output).
                    if let Some(output) = a.result.as_deref().and_then(extract_json_from_str) {
                        raw_outputs.push((lens, output));
                    } else {
                        warn!(lens = ?lens, "lens produced no parseable JSON output");
                    }
                    break;
                }
                sleep(Duration::from_millis(POLL_MS)).await;
            }
        }

        // Consolidate findings.
        consolidate(run_id, raw_outputs, req.min_confidence)
    }

    async fn git_diff(&self, _worktree_id: Option<&str>) -> Result<String> {
        // Run `git diff HEAD` in the current directory.
        let output = tokio::process::Command::new("git")
            .args(["diff", "HEAD"])
            .output()
            .await
            .context("git diff HEAD failed")?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

fn build_context(diff: &str, spec_path: Option<&str>, tasks_path: Option<&str>) -> String {
    let mut ctx = format!("## Unified Diff\n\n```diff\n{diff}\n```\n");
    if let Some(path) = spec_path {
        if let Ok(content) = std::fs::read_to_string(path) {
            ctx.push_str(&format!("\n## spec.md\n\n```\n{content}\n```\n"));
        }
    }
    if let Some(path) = tasks_path {
        if let Ok(content) = std::fs::read_to_string(path) {
            ctx.push_str(&format!("\n## tasks.json\n\n```json\n{content}\n```\n"));
        }
    }
    ctx
}

fn extract_json_from_str(content: &str) -> Option<serde_json::Value> {
    // Try extracting JSON from ```json ... ``` block first.
    if let Some(start) = content.find("```json") {
        let after = &content[start + 7..];
        if let Some(end) = after.find("```") {
            if let Ok(v) = serde_json::from_str(after[..end].trim()) {
                return Some(v);
            }
        }
    }
    // Try raw JSON (starts with `{`).
    let trimmed = content.trim();
    if trimmed.starts_with('{') {
        if let Ok(v) = serde_json::from_str(trimmed) {
            return Some(v);
        }
    }
    None
}

fn consolidate(
    run_id: String,
    raw: Vec<(DetectiveLens, serde_json::Value)>,
    _min_confidence: f64,
) -> Result<RunDetectiveResponse> {
    let mut all_findings: Vec<DetectiveFinding> = Vec::new();
    let mut highlights: Vec<DetectiveHighlight> = Vec::new();
    let mut raw_values: Vec<serde_json::Value> = Vec::new();
    let mut by_lens: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut by_severity: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for (lens, value) in raw {
        let lens_name = format!("{lens:?}").to_lowercase();
        raw_values.push(value.clone());

        if let Some(h) = value.get("highlight").and_then(|v| v.as_str()) {
            highlights.push(DetectiveHighlight { lens: lens_name.clone(), text: h.to_string() });
        }

        let findings = value.get("findings").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let count = findings.len();
        by_lens.insert(lens_name.clone(), count);

        for f in findings {
            let severity = f.get("severity").and_then(|v| v.as_str()).unwrap_or("low").to_string();
            *by_severity.entry(severity.clone()).or_insert(0) += 1;
            all_findings.push(DetectiveFinding {
                severity,
                lenses: vec![lens_name.clone()],
                path: f.get("path").and_then(|v| v.as_str()).map(|s| s.to_string()),
                line: f.get("line").and_then(|v| v.as_u64()).map(|n| n as u32),
                title: f.get("title").and_then(|v| v.as_str()).unwrap_or("(untitled)").to_string(),
                description: f.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                recommendation: f.get("recommendation").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            });
        }
    }

    // Sort: critical > high > medium > low
    let sev_rank = |s: &str| match s { "critical" => 0, "high" => 1, "medium" => 2, _ => 3 };
    all_findings.sort_by_key(|f| sev_rank(&f.severity));

    // Deduplicate same {path, line ±3} across lenses.
    let mut deduped: Vec<DetectiveFinding> = Vec::new();
    'outer: for mut f in all_findings {
        for existing in &mut deduped {
            if existing.path == f.path {
                let same_line = match (existing.line, f.line) {
                    (Some(a), Some(b)) => (a as i32 - b as i32).abs() <= 3,
                    _ => existing.title == f.title,
                };
                if same_line {
                    existing.lenses.append(&mut f.lenses);
                    continue 'outer;
                }
            }
        }
        deduped.push(f);
    }

    // Verdict.
    let has_critical = by_severity.get("critical").copied().unwrap_or(0) > 0;
    let has_security_high = deduped.iter().any(|f| f.severity == "high" && f.lenses.contains(&"security".to_string()));
    let has_regression_high = deduped.iter().any(|f| f.severity == "high" && f.lenses.contains(&"regression".to_string()));
    let (verdict, verdict_reason) = if has_critical || has_security_high || has_regression_high {
        (DetectiveVerdict::Block, "Critical or high-severity security/regression finding present.".to_string())
    } else if deduped.is_empty() {
        (DetectiveVerdict::Pass, "No findings from any lens.".to_string())
    } else {
        (DetectiveVerdict::Warn, format!("{} finding(s) — review recommended.", deduped.len()))
    };

    Ok(RunDetectiveResponse {
        run_id,
        verdict,
        verdict_reason,
        summary: DetectiveSummary {
            total_findings: deduped.len(),
            by_lens,
            by_severity,
        },
        findings: deduped,
        highlights,
        uncovered_files: vec![],
        raw_lens_outputs: raw_values,
    })
}

fn empty_response(run_id: String) -> RunDetectiveResponse {
    RunDetectiveResponse {
        run_id,
        verdict: DetectiveVerdict::Pass,
        verdict_reason: "Empty diff — nothing to review.".to_string(),
        summary: DetectiveSummary {
            total_findings: 0,
            by_lens: std::collections::HashMap::new(),
            by_severity: std::collections::HashMap::new(),
        },
        findings: vec![],
        highlights: vec![],
        uncovered_files: vec![],
        raw_lens_outputs: vec![],
    }
}
