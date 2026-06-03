//! Workflow loader + executor + consolidator (TRUST-inspired).
//!
//! Reads `.kira/workflows/<name>.yaml`, spawns reviewer agents in parallel
//! (one per dimension), runs the L1-L6 gate cascade per output to filter
//! hallucinations, and consolidates the surviving findings into a
//! markdown report.
//!
//! Scope v0.5: only the `pr-review` workflow shape is supported (multiple
//! `Reviewer` agents emitting findings JSON). Generic workflow execution
//! (writer→judge→deployer chains) lives in the Pilot/Orquestrador refactor.

pub mod consolidator;

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, instrument, warn};
use uuid::Uuid;

use crate::agents::AgentManager;
use crate::gates::run_cascade;
use crate::llm::embed_cache::EmbedCache;
use crate::proto::agent::{AgentKind, AgentStatus, SpawnAgentRequest};
use crate::proto::gate::{GateOutcome, RunGatesRequest};
use crate::proto::review::{AgentReviewOutcome, RunReviewResponse};
use crate::router::Router;

const POLL_INTERVAL_MS: u64 = 500;
const DEFAULT_TIMEOUT_SECS: u64 = 300;

#[derive(Deserialize, Debug, Clone)]
pub struct Workflow {
    pub version: u32,
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub grounding: GroundingSpec,
    pub agents: Vec<WorkflowAgent>,
    #[serde(default)]
    pub gates: GatesSpec,
    #[serde(default)]
    pub consolidator: ConsolidatorSpec,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct GroundingSpec {
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub optional: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct WorkflowAgent {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub dimension: String,
    #[serde(default)]
    pub tier: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct GatesSpec {
    #[serde(default)]
    pub cascade: CascadeSpec,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CascadeSpec {
    #[serde(default = "default_true")]
    pub skip_embedding: bool,
    #[serde(default = "default_true")]
    pub skip_judge: bool,
    #[serde(default)]
    pub skip_meta_review: bool,
}

impl Default for CascadeSpec {
    fn default() -> Self {
        Self { skip_embedding: true, skip_judge: true, skip_meta_review: false }
    }
}

const fn default_true() -> bool { true }

#[derive(Deserialize, Debug, Clone, Default)]
pub struct ConsolidatorSpec {
    #[serde(default)]
    pub group_by: String,
    #[serde(default)]
    pub include_highlights: bool,
}

/// Load a workflow YAML from disk and parse it.
pub fn load(path: &Path) -> Result<Workflow> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read workflow: {}", path.display()))?;
    let wf: Workflow = serde_yaml::from_str(&raw)
        .with_context(|| format!("parse workflow yaml: {}", path.display()))?;
    if wf.version != 1 {
        anyhow::bail!("workflow version {} not supported (need 1)", wf.version);
    }
    Ok(wf)
}

#[derive(Clone)]
pub struct ReviewExecutor {
    pub repo_root: PathBuf,
    pub agents: AgentManager,
    pub router: Router,
    pub embed_cache: EmbedCache,
}

/// Execute a `pr-review`-shaped workflow.
///
/// - Captures `git diff base...head` as the haystack for L6 meta-review.
/// - Spawns one Reviewer agent per `agents[]` entry, passing the dimension
///   in the task prompt.
/// - Polls each agent until terminal state or timeout.
/// - Runs the gate cascade on each agent's `result` with the diff as haystack.
/// - Filters out findings that fail meta-review.
/// - Writes consolidated `REVIEW.md` to `.kira/runs/<run_id>/REVIEW.md`.
#[instrument(skip(executor, workflow), fields(head = %head, workflow_id = %workflow.id))]
pub async fn execute_pr_review(
    executor: &ReviewExecutor,
    workflow: &Workflow,
    head: &str,
    base: &str,
    timeout_secs: u64,
) -> Result<RunReviewResponse> {
    let started = Instant::now();
    let run_id = Uuid::now_v7().to_string();
    info!(run_id = %run_id, "pr-review run starting");

    // 1. Capture diff.
    let diff = crate::worktree::git::diff_three_dot(&executor.repo_root, base, head)
        .await
        .context("git diff capture failed")?;
    if diff.trim().is_empty() {
        anyhow::bail!("empty diff: {base}...{head} — nothing to review");
    }

    // 2. Spawn all reviewer agents in parallel.
    let mut spawn_handles: Vec<tokio::task::JoinHandle<(WorkflowAgent, anyhow::Result<String>)>> =
        Vec::new();
    for wa in &workflow.agents {
        let agents = executor.agents.clone();
        let wa_clone = wa.clone();
        let diff_clone = diff.clone();
        spawn_handles.push(tokio::spawn(async move {
            let task = build_reviewer_task(&wa_clone.dimension, &diff_clone);
            let res = agents
                .spawn(SpawnAgentRequest {
                    kind: AgentKind::Reviewer,
                    task,
                    tier: None,
                    worktree_id: None,
                    base: None,
                    budget_usd: None,
                    // Closed-world prompt should converge in <8 turns, but
                    // give headroom for retries on tool errors.
                    max_iters: Some(15),
                    yolo_mode: None,
                    background: None,
                    timeout_secs: None,
                })
                .await
                .map(|a| a.id);
            (wa_clone, res)
        }));
    }

    let mut pending: Vec<(WorkflowAgent, String)> = Vec::new();
    let mut outcomes: Vec<AgentReviewOutcome> = Vec::new();
    for h in spawn_handles {
        match h.await {
            Ok((wa, Ok(agent_id))) => pending.push((wa, agent_id)),
            Ok((wa, Err(e))) => outcomes.push(spawn_failed(&wa, &format!("{e}"))),
            Err(e) => warn!(error = ?e, "reviewer spawn join failed"),
        }
    }

    // 3. Poll until each agent terminates.
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let cascade_cfg = workflow.gates.cascade.clone();
    let mut findings_jsons: Vec<(WorkflowAgent, String, Option<String>)> = Vec::new(); // (agent_meta, agent_id, result_json)
    for (wa, agent_id) in pending {
        let uuid = match Uuid::parse_str(&agent_id) {
            Ok(u) => u,
            Err(e) => {
                outcomes.push(spawn_failed(&wa, &format!("invalid uuid: {e}")));
                continue;
            }
        };
        let mut last_status = AgentStatus::Spawning;
        let mut result_text: Option<String> = None;
        let err_text: Option<String> = loop {
            if Instant::now() > deadline {
                break Some("timeout".into());
            }
            let Some(a) = executor.agents.get(uuid) else {
                break Some("agent vanished".into());
            };
            last_status = a.status;
            if is_terminal(a.status) {
                result_text = a.result.clone();
                break a.error.clone();
            }
            sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
        };
        findings_jsons.push((wa.clone(), agent_id.clone(), result_text.clone()));
        // Push a placeholder outcome here; populated post-gate below.
        outcomes.push(AgentReviewOutcome {
            dimension: wa.dimension.clone(),
            agent_id: agent_id.clone(),
            status: format!("{last_status:?}").to_lowercase(),
            findings_kept: 0,
            findings_dropped: 0,
            gate_overall: "skipped".into(),
            gate_failed_at: None,
            duration_ms: 0,
            error: err_text,
        });
    }

    // 4. Run L1-L6 gates on each output. Filter findings by meta-review verdict.
    let mut consolidated: Vec<consolidator::Finding> = Vec::new();
    for (wa, agent_id, maybe_output) in findings_jsons {
        let Some(output) = maybe_output else { continue };
        let Some(outcome_slot) = outcomes.iter_mut().find(|o| o.agent_id == agent_id) else {
            continue;
        };
        let req = RunGatesRequest {
            spec: format!("Review dimension: {} (must follow reviewer.md schema)", wa.dimension),
            output: output.clone(),
            owned_paths: Vec::new(),
            touched_paths: Vec::new(),
            estimated_loc: None,
            actual_loc: None,
            skip_embedding: cascade_cfg.skip_embedding,
            skip_judge: cascade_cfg.skip_judge,
            haystack: diff.clone(),
            skip_meta_review: cascade_cfg.skip_meta_review,
            worktree_path: String::new(),
            skip_structural_lint: true,
            writer_model: None,
        };
        let gate_resp = run_cascade(
            &executor.router,
            Some(&executor.embed_cache),
            &req,
            crate::gates::GateConfig::default(),
        )
        .await;
        outcome_slot.gate_overall = format!("{:?}", gate_resp.overall).to_lowercase();
        outcome_slot.gate_failed_at =
            gate_resp.failed_at.map(|g| format!("{g:?}").to_lowercase());
        outcome_slot.duration_ms = gate_resp.duration_ms;

        if gate_resp.overall == GateOutcome::Pass {
            // Parse + collect findings.
            match consolidator::parse_findings(&output, &wa.dimension) {
                Ok(parsed) => {
                    outcome_slot.findings_kept = parsed.findings.len() as u32;
                    consolidated.extend(parsed.findings);
                }
                Err(e) => {
                    warn!(dimension = %wa.dimension, error = ?e, "parse findings failed");
                    outcome_slot.error = Some(format!("parse: {e}"));
                }
            }
        } else {
            // Gate failed — drop everything from this agent.
            if let Ok(parsed) = consolidator::parse_findings(&output, &wa.dimension) {
                outcome_slot.findings_dropped = parsed.findings.len() as u32;
            }
        }
    }

    // 5. Write REVIEW.md to .kira/runs/<run_id>/REVIEW.md
    let run_dir = executor.repo_root.join(".kira").join("runs").join(&run_id);
    std::fs::create_dir_all(&run_dir).context("create run dir")?;
    let md = consolidator::render_review_md(&run_id, head, base, &outcomes, &consolidated);
    let review_md_path = run_dir.join("REVIEW.md");
    std::fs::write(&review_md_path, md).context("write REVIEW.md")?;

    let total = consolidated.len() as u32;
    Ok(RunReviewResponse {
        run_id,
        head: head.to_string(),
        base: base.to_string(),
        workflow: workflow.id.clone(),
        agents: outcomes,
        review_md_path: review_md_path.display().to_string(),
        total_findings: total,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn build_reviewer_task(dimension: &str, diff: &str) -> String {
    // qwen-max input cap is 30720 tokens (~120KB). Stay well under.
    const DIFF_CAP_BYTES: usize = 60_000;
    let truncated = if diff.len() > DIFF_CAP_BYTES {
        format!("{}\n\n[diff truncated at {DIFF_CAP_BYTES} bytes]\n", &diff[..DIFF_CAP_BYTES])
    } else {
        diff.to_string()
    };
    let changed = changed_files_from_diff(&truncated);
    let files_block = if changed.is_empty() {
        "(no recognized file headers)".to_string()
    } else {
        changed.join("\n")
    };

    format!(
        "Dimension: {dimension}\n\n\
         You are reviewing the unified diff below. CLOSED-WORLD RULES:\n\
         1. The DIFF is the COMPLETE artifact. Do NOT call file_read on paths\n\
            that do not appear in the diff. If a path is not listed in\n\
            CHANGED_FILES, it does not exist for this review.\n\
         2. file_read is allowed ONLY for paths in CHANGED_FILES (to see\n\
            context lines outside the hunk window).\n\
         3. Do NOT invent file paths from memory or training data.\n\
         4. Every evidence_quote MUST be a literal substring of the DIFF.\n\
         5. Emit your final JSON via done(...) in 3-5 tool turns max.\n\
            Stop reading files once you have evidence — the diff is enough.\n\n\
         OUTPUT SCHEMA: see reviewer.md (STRICT JSON inside done()).\n\
         If you have no findings, emit findings:[] with coverage.pct_evaluated=100.\n\n\
         CHANGED_FILES:\n{files_block}\n\n\
         DIFF (base...head):\n{truncated}"
    )
}

/// Parse `diff --git a/x b/x` headers from a unified diff into a sorted unique list.
fn changed_files_from_diff(diff: &str) -> Vec<String> {
    let mut set = std::collections::BTreeSet::new();
    for line in diff.lines() {
        // `diff --git a/path b/path`
        if let Some(rest) = line.strip_prefix("diff --git ")
            && let Some(b_idx) = rest.find(" b/") {
                let b = &rest[b_idx + 3..];
                set.insert(b.to_string());
                continue;
            }
        // Fallback: `+++ b/path` (handles renames).
        if let Some(p) = line.strip_prefix("+++ b/")
            && p != "/dev/null" {
                set.insert(p.to_string());
            }
    }
    set.into_iter().collect()
}

fn spawn_failed(wa: &WorkflowAgent, err: &str) -> AgentReviewOutcome {
    AgentReviewOutcome {
        dimension: wa.dimension.clone(),
        agent_id: String::new(),
        status: "spawn_failed".into(),
        findings_kept: 0,
        findings_dropped: 0,
        gate_overall: "skipped".into(),
        gate_failed_at: None,
        duration_ms: 0,
        error: Some(err.into()),
    }
}

const fn is_terminal(s: AgentStatus) -> bool {
    matches!(
        s,
        AgentStatus::Completed
            | AgentStatus::Failed
            | AgentStatus::Stopped
            | AgentStatus::Cancelled
            | AgentStatus::Stale
    )
}

/// Convenience: default workflow path for pr-review.
pub fn default_workflow_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".kira/workflows/pr-review.yaml")
}

/// Cap on bytes returned from `diff_three_dot` before passing to reviewers.
/// Exposed for tests.
pub const DEFAULT_TIMEOUT: u64 = DEFAULT_TIMEOUT_SECS;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_workflow() {
        let yaml = "
version: 1
id: pr-review
description: test
grounding: {}
agents:
  - id: security
    kind: reviewer
    dimension: security
  - id: perf
    kind: reviewer
    dimension: performance
gates:
  cascade:
    skip_embedding: true
    skip_judge: true
    skip_meta_review: false
";
        let wf: Workflow = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(wf.version, 1);
        assert_eq!(wf.agents.len(), 2);
        assert_eq!(wf.agents[0].dimension, "security");
        assert!(!wf.gates.cascade.skip_meta_review);
    }

    #[test]
    fn rejects_wrong_version() {
        let yaml = "version: 99\nid: x\nagents: []\n";
        let tmp = std::env::temp_dir().join("kira-wf-bad.yaml");
        std::fs::write(&tmp, yaml).unwrap();
        let err = load(&tmp).unwrap_err();
        assert!(format!("{err}").contains("version 99"));
    }

    #[test]
    fn build_reviewer_task_includes_dimension() {
        let t = build_reviewer_task("security", "diff text");
        assert!(t.contains("Dimension: security"));
        assert!(t.contains("diff text"));
    }

    #[test]
    fn changed_files_from_diff_basic() {
        let d = "\
diff --git a/foo.rs b/foo.rs
index e69de29..4b825dc 100644
--- a/foo.rs
+++ b/foo.rs
@@ -0,0 +1 @@
+x
diff --git a/bar/baz.ts b/bar/baz.ts
+++ b/bar/baz.ts
";
        let files = changed_files_from_diff(d);
        assert_eq!(files, vec!["bar/baz.ts".to_string(), "foo.rs".to_string()]);
    }

    #[test]
    fn closed_world_prompt_lists_files() {
        let d = "diff --git a/a.rs b/a.rs\n+++ b/a.rs\n+let x = 1;\n";
        let t = build_reviewer_task("security", d);
        assert!(t.contains("CHANGED_FILES:"));
        assert!(t.contains("a.rs"));
        assert!(t.contains("CLOSED-WORLD RULES"));
    }
}
