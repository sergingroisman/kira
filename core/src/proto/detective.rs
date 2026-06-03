//! Detective — universal multi-dimension code review wire types.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Request to run a Detective review.
#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/detective.ts")]
pub struct RunDetectiveRequest {
    /// Unified diff to review. If not provided, Detective will run `git diff HEAD` in the worktree.
    #[serde(default)]
    pub diff: Option<String>,
    /// Worktree to run in (for context files: AGENTS.md, spec.md, tasks.json).
    #[serde(default)]
    pub worktree_id: Option<String>,
    /// Lenses to run. Defaults to all 6.
    #[serde(default = "all_lenses")]
    pub lenses: Vec<DetectiveLens>,
    /// Minimum confidence threshold (0.0–1.0). Defaults to 0.8.
    #[serde(default = "default_confidence")]
    pub min_confidence: f64,
    /// Optional spec.md path (for the requirements lens).
    #[serde(default)]
    pub spec_path: Option<String>,
    /// Optional tasks.json path (for the requirements lens).
    #[serde(default)]
    pub tasks_path: Option<String>,
}

fn all_lenses() -> Vec<DetectiveLens> {
    vec![
        DetectiveLens::Security,
        DetectiveLens::Requirements,
        DetectiveLens::Tests,
        DetectiveLens::Architecture,
        DetectiveLens::Regression,
        DetectiveLens::Performance,
    ]
}

fn default_confidence() -> f64 { 0.8 }

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/detective.ts")]
#[serde(rename_all = "snake_case")]
pub enum DetectiveLens {
    Security,
    Requirements,
    Tests,
    Architecture,
    Regression,
    Performance,
}

impl DetectiveLens {
    pub const fn prompt_key(self) -> &'static str {
        match self {
            Self::Security     => "detective/security.md",
            Self::Requirements => "detective/requirements.md",
            Self::Tests        => "detective/tests.md",
            Self::Architecture => "detective/architecture.md",
            Self::Regression   => "detective/regression.md",
            Self::Performance  => "detective/performance.md",
        }
    }

    pub const fn prompt_text(self) -> &'static str {
        match self {
            Self::Security     => include_str!("../agents/prompts/detective/security.md"),
            Self::Requirements => include_str!("../agents/prompts/detective/requirements.md"),
            Self::Tests        => include_str!("../agents/prompts/detective/tests.md"),
            Self::Architecture => include_str!("../agents/prompts/detective/architecture.md"),
            Self::Regression   => include_str!("../agents/prompts/detective/regression.md"),
            Self::Performance  => include_str!("../agents/prompts/detective/performance.md"),
        }
    }
}

/// Consolidated Detective review response.
#[derive(Serialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/detective.ts")]
pub struct RunDetectiveResponse {
    pub run_id: String,
    pub verdict: DetectiveVerdict,
    pub verdict_reason: String,
    pub summary: DetectiveSummary,
    pub findings: Vec<DetectiveFinding>,
    pub highlights: Vec<DetectiveHighlight>,
    pub uncovered_files: Vec<String>,
    pub raw_lens_outputs: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/detective.ts")]
#[serde(rename_all = "snake_case")]
pub enum DetectiveVerdict {
    Pass,
    Warn,
    Block,
}

#[derive(Serialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/detective.ts")]
pub struct DetectiveSummary {
    pub total_findings: usize,
    pub by_lens: std::collections::HashMap<String, usize>,
    pub by_severity: std::collections::HashMap<String, usize>,
}

#[derive(Serialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/detective.ts")]
pub struct DetectiveFinding {
    pub severity: String,
    pub lenses: Vec<String>,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub title: String,
    pub description: String,
    pub recommendation: String,
}

#[derive(Serialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/detective.ts")]
pub struct DetectiveHighlight {
    pub lens: String,
    pub text: String,
}
