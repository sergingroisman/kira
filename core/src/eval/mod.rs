//! Eval framework — run N×M (tasks × models) comparison + judge + faithfulness.
//!
//! For each (task, model):
//!   1. Direct chat completion with explicit `model_override`.
//!   2. Record output, tokens, cost, duration.
//!   3. Optionally run embedding faithfulness (cosine spec↔output).
//!   4. Optionally run judge LLM with rubric.
//!   5. Aggregate per-model summary at the end.
//!
//! Sequential by default to keep budget bounded. v0.5+ may add parallelism.

use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, instrument, warn};

use crate::gates::cosine_for_eval;
use crate::llm::types::{ChatMessageUnified, ChatParams};
use crate::llm::embed_cache::EmbedCache;
use crate::proto::eval::{EvalSample, ModelSummary, RunEvalRequest, RunEvalResponse};
use crate::router::Router;

const JUDGE_TIER: &str = "heavy";
const JUDGE_PROMPT: &str = crate::gates::JUDGE_PROMPT;

#[instrument(skip(router, embed_cache, req), fields(n_tasks = req.tasks.len(), n_models = req.models.len()))]
pub async fn run_eval(
    router: &Router,
    embed_cache: &EmbedCache,
    req: &RunEvalRequest,
) -> RunEvalResponse {
    let started = Instant::now();
    let mut samples: Vec<EvalSample> = Vec::new();

    for task in &req.tasks {
        let models_for_task = task.models.clone().unwrap_or_else(|| req.models.clone());
        for model in &models_for_task {
            let sample = run_one_sample(
                router,
                embed_cache,
                &task.id,
                &task.spec,
                model,
                req.skip_embedding,
                req.skip_judge,
            )
            .await;
            samples.push(sample);
        }
    }

    let summary_by_model = summarize(&samples);
    let total_cost_usd: f64 = samples.iter().map(|s| s.cost_usd).sum();
    let total_duration_ms = started.elapsed().as_millis() as u64;

    info!(
        n_samples = samples.len(),
        total_cost_usd,
        total_duration_ms,
        "eval finished"
    );

    RunEvalResponse {
        samples,
        summary_by_model,
        total_cost_usd,
        total_duration_ms,
        n_tasks: req.tasks.len() as u32,
        n_models: req.models.len() as u32,
    }
}

async fn run_one_sample(
    router: &Router,
    embed_cache: &EmbedCache,
    task_id: &str,
    spec: &str,
    model: &str,
    skip_embedding: bool,
    skip_judge: bool,
) -> EvalSample {
    let start = Instant::now();
    let messages: Vec<ChatMessageUnified> = vec![ChatMessageUnified::User {
        content: spec.to_string(),
    }];

    // Direct chat with explicit model override. Tier hint ignored when model is set.
    let (output, _provider, model_used) = match router
        .route_chat_with_tools("standard", Some(model), messages, Vec::new(), None)
        .await
    {
        Ok(t) => t,
        Err(e) => return error_sample(task_id, model, format!("router: {e}")),
    };
    let duration_ms = start.elapsed().as_millis() as u64;

    let (tokens_in, tokens_out) = (output.tokens_in, output.tokens_out);
    let cost_usd = price_per_million(&model_used)
        .map_or(0.0, |(i, o)| f64::from(tokens_in).mul_add(i, f64::from(tokens_out) * o) / 1_000_000.0);

    // Faithfulness via embedding (free with cache hit).
    let faithfulness = if skip_embedding || output.content.is_empty() {
        None
    } else {
        cosine_for_eval(router, embed_cache, spec, &output.content).await
    };

    // Judge via Heavy LLM.
    let (judge_score, judge_rationale) = if skip_judge || output.content.is_empty() {
        (None, None)
    } else {
        judge_sample(router, spec, &output.content).await
    };

    let gates_pass = match (faithfulness, judge_score) {
        (Some(f), Some(s)) => f >= 0.55 && s >= 7.0,
        (Some(f), None) => f >= 0.55,
        (None, Some(s)) => s >= 7.0,
        (None, None) => !output.content.is_empty(),
    };

    EvalSample {
        task_id: task_id.to_string(),
        model: model_used,
        output: output.content,
        tokens_in,
        tokens_out,
        cost_usd,
        duration_ms,
        judge_score,
        judge_rationale,
        faithfulness,
        gates_pass,
        error: None,
    }
}

async fn judge_sample(
    router: &Router,
    spec: &str,
    output: &str,
) -> (Option<f64>, Option<String>) {
    let messages: Vec<ChatMessageUnified> = vec![
        ChatMessageUnified::System {
            content: JUDGE_PROMPT.to_string(),
        },
        ChatMessageUnified::User {
            content: format!("SPEC:\n{spec}\n\nOUTPUT:\n{output}\n\nReturn JSON."),
        },
    ];
    let judge_params = ChatParams::judge_deterministic();
    let (resp, _, _) = match router
        .route_chat_with_tools(JUDGE_TIER, None, messages, Vec::new(), Some(judge_params))
        .await
    {
        Ok(t) => t,
        Err(e) => {
            warn!(error = ?e, "judge call failed");
            return (None, None);
        }
    };
    let content = resp.content;
    let parsed: Option<serde_json::Value> = serde_json::from_str(content.trim()).ok().or_else(|| {
        // Try inside fences.
        let s = content.trim();
        let start = s.find('{')?;
        let end = s.rfind('}')?;
        serde_json::from_str(&s[start..=end]).ok()
    });
    match parsed {
        Some(v) => (
            v.get("score").and_then(serde_json::Value::as_f64),
            v.get("rationale")
                .and_then(|r| r.as_str())
                .map(str::to_string),
        ),
        None => (None, None),
    }
}

fn error_sample(task_id: &str, model: &str, err: String) -> EvalSample {
    EvalSample {
        task_id: task_id.to_string(),
        model: model.to_string(),
        output: String::new(),
        tokens_in: 0,
        tokens_out: 0,
        cost_usd: 0.0,
        duration_ms: 0,
        judge_score: None,
        judge_rationale: None,
        faithfulness: None,
        gates_pass: false,
        error: Some(err),
    }
}

fn price_per_million(model: &str) -> Option<(f64, f64)> {
    match model {
        "deepseek-v4-pro" => Some((0.43, 0.87)),
        "deepseek-v4-flash" => Some((0.14, 0.28)),
        "gpt-4o" => Some((2.50, 10.00)),
        "gpt-4o-mini" => Some((0.15, 0.60)),
        _ => None,
    }
}

pub(crate) fn summarize(samples: &[EvalSample]) -> Vec<ModelSummary> {
    let mut by_model: HashMap<String, Vec<&EvalSample>> = HashMap::new();
    for s in samples {
        by_model.entry(s.model.clone()).or_default().push(s);
    }
    let mut out: Vec<ModelSummary> = by_model
        .into_iter()
        .map(|(model, group)| {
            let n = group.len() as u32;
            let n_f = f64::from(n);
            let scores: Vec<f64> = group.iter().filter_map(|s| s.judge_score).collect();
            let faiths: Vec<f64> = group.iter().filter_map(|s| s.faithfulness).collect();
            let total_cost: f64 = group.iter().map(|s| s.cost_usd).sum();
            let total_ms: u64 = group.iter().map(|s| s.duration_ms).sum();
            let passes = group.iter().filter(|s| s.gates_pass).count() as u32;
            ModelSummary {
                model,
                n_samples: n,
                avg_judge_score: if scores.is_empty() {
                    None
                } else {
                    Some(scores.iter().sum::<f64>() / scores.len() as f64)
                },
                avg_faithfulness: if faiths.is_empty() {
                    None
                } else {
                    Some(faiths.iter().sum::<f64>() / faiths.len() as f64)
                },
                total_cost_usd: total_cost,
                avg_duration_ms: total_ms as f64 / n_f,
                pass_rate: f64::from(passes) / n_f,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.avg_judge_score
            .unwrap_or(0.0)
            .partial_cmp(&a.avg_judge_score.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(model: &str, judge: Option<f64>, faith: Option<f64>, cost: f64, ms: u64, gates_pass: bool) -> EvalSample {
        EvalSample {
            task_id: "t1".into(),
            model: model.into(),
            output: "out".into(),
            tokens_in: 100,
            tokens_out: 50,
            cost_usd: cost,
            duration_ms: ms,
            judge_score: judge,
            judge_rationale: None,
            faithfulness: faith,
            gates_pass,
            error: None,
        }
    }

    #[test]
    fn summarize_empty_returns_empty() {
        assert!(summarize(&[]).is_empty());
    }

    #[test]
    fn summarize_groups_by_model() {
        let samples = vec![
            sample("m1", Some(8.0), Some(0.9), 0.01, 100, true),
            sample("m1", Some(6.0), Some(0.7), 0.02, 200, false),
            sample("m2", Some(9.0), Some(0.95), 0.05, 300, true),
        ];
        let out = summarize(&samples);
        assert_eq!(out.len(), 2);
        // Sorted by avg_judge_score desc: m2 (9.0) before m1 (7.0)
        assert_eq!(out[0].model, "m2");
        assert_eq!(out[1].model, "m1");
    }

    #[test]
    fn summarize_averages_judge_and_faithfulness() {
        let samples = vec![
            sample("m1", Some(8.0), Some(0.8), 0.0, 0, true),
            sample("m1", Some(6.0), Some(0.6), 0.0, 0, true),
        ];
        let out = summarize(&samples);
        let m1 = out.iter().find(|s| s.model == "m1").unwrap();
        assert!((m1.avg_judge_score.unwrap() - 7.0).abs() < 1e-9);
        assert!((m1.avg_faithfulness.unwrap() - 0.7).abs() < 1e-9);
        assert_eq!(m1.n_samples, 2);
    }

    #[test]
    fn summarize_pass_rate_handles_partial() {
        let samples = vec![
            sample("m1", Some(8.0), None, 0.0, 0, true),
            sample("m1", Some(7.0), None, 0.0, 0, false),
            sample("m1", Some(6.0), None, 0.0, 0, true),
            sample("m1", Some(5.0), None, 0.0, 0, false),
        ];
        let out = summarize(&samples);
        let m1 = &out[0];
        assert!((m1.pass_rate - 0.5).abs() < 1e-9);
    }

    #[test]
    fn summarize_treats_none_judge_as_skipped_not_zero() {
        // Per workshop "barato primeiro": a None score (skipped) MUST NOT
        // poison the average — only present scores count. Else cheap-judge
        // skip would tank a model's apparent quality.
        let samples = vec![
            sample("m1", Some(9.0), None, 0.0, 0, true),
            sample("m1", None, None, 0.0, 0, true),
            sample("m1", None, None, 0.0, 0, true),
        ];
        let out = summarize(&samples);
        let m1 = &out[0];
        assert!((m1.avg_judge_score.unwrap() - 9.0).abs() < 1e-9);
        assert_eq!(m1.avg_faithfulness, None);
    }

    #[test]
    fn summarize_total_cost_sums_per_model() {
        let samples = vec![
            sample("m1", None, None, 0.01, 0, true),
            sample("m1", None, None, 0.02, 0, true),
            sample("m2", None, None, 0.10, 0, true),
        ];
        let out = summarize(&samples);
        let m1 = out.iter().find(|s| s.model == "m1").unwrap();
        let m2 = out.iter().find(|s| s.model == "m2").unwrap();
        assert!((m1.total_cost_usd - 0.03).abs() < 1e-9);
        assert!((m2.total_cost_usd - 0.10).abs() < 1e-9);
    }
}
