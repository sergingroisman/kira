//! Eval framework wire types — compare N models against M tasks.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/eval.ts")]
pub struct EvalTask {
    pub id: String,
    /// The spec/prompt sent to the model.
    pub spec: String,
    /// Optional reference answer or expected topic (used by judge for context).
    #[serde(default)]
    pub expected_topic: Option<String>,
    /// Optional per-task model override (otherwise uses request's `models`).
    #[serde(default)]
    pub models: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/eval.ts")]
pub struct RunEvalRequest {
    pub tasks: Vec<EvalTask>,
    /// Models to evaluate. Each task runs against each model (cartesian).
    /// Can be overridden per-task via `EvalTask.models`.
    pub models: Vec<String>,
    /// Skip the embedding faithfulness gate (saves embed calls).
    #[serde(default)]
    pub skip_embedding: bool,
    /// Skip the judge gate (saves heavy LLM calls).
    #[serde(default)]
    pub skip_judge: bool,
}

#[derive(Serialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/eval.ts")]
pub struct EvalSample {
    pub task_id: String,
    pub model: String,
    pub output: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_usd: f64,
    pub duration_ms: u64,
    /// 0-10 from judge (None when skip_judge or call failed).
    pub judge_score: Option<f64>,
    pub judge_rationale: Option<String>,
    /// Cosine similarity spec↔output (None when skip_embedding).
    pub faithfulness: Option<f64>,
    /// True if all enabled gates passed.
    pub gates_pass: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/eval.ts")]
pub struct ModelSummary {
    pub model: String,
    pub n_samples: u32,
    pub avg_judge_score: Option<f64>,
    pub avg_faithfulness: Option<f64>,
    pub total_cost_usd: f64,
    pub avg_duration_ms: f64,
    pub pass_rate: f64, // 0..1
}

#[derive(Serialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/eval.ts")]
pub struct RunEvalResponse {
    pub samples: Vec<EvalSample>,
    pub summary_by_model: Vec<ModelSummary>,
    pub total_cost_usd: f64,
    pub total_duration_ms: u64,
    pub n_tasks: u32,
    pub n_models: u32,
}
