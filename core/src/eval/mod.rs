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

use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestUserMessageArgs,
};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, instrument, warn};

use crate::gates::cosine_for_eval;
use crate::llm::embed_cache::EmbedCache;
use crate::proto::eval::{EvalSample, ModelSummary, RunEvalRequest, RunEvalResponse};
use crate::router::Router;

const JUDGE_TIER: &str = "heavy";
const JUDGE_PROMPT: &str = "You are an eval judge. Given SPEC and OUTPUT, \
score how well OUTPUT meets SPEC on 0-10 scale. \
Output STRICT JSON only: {\"score\": <integer>, \"rationale\": \"<1 sentence>\"}. \
No prose, no markdown. NO flattery, NO hedging. \
If OUTPUT is unrelated to SPEC, score 0.";

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
    let user_msg = match ChatCompletionRequestUserMessageArgs::default()
        .content(spec.to_string())
        .build()
    {
        Ok(m) => m,
        Err(e) => return error_sample(task_id, model, format!("build user msg: {e}")),
    };
    let messages: Vec<ChatCompletionRequestMessage> = vec![user_msg.into()];

    // Direct chat with explicit model override. Tier hint ignored when model is set.
    let (response, _provider, model_used) = match router
        .route_chat_with_tools("standard", Some(model), messages, Vec::new())
        .await
    {
        Ok(t) => t,
        Err(e) => return error_sample(task_id, model, format!("router: {e}")),
    };
    let duration_ms = start.elapsed().as_millis() as u64;

    let usage = response.usage.clone();
    let choice = response.choices.into_iter().next();
    let output = choice
        .and_then(|c| c.message.content)
        .unwrap_or_default();
    let (tokens_in, tokens_out) = usage
        .as_ref()
        .map(|u| (u.prompt_tokens, u.completion_tokens))
        .unwrap_or((0, 0));
    let cost_usd = price_per_million(&model_used)
        .map(|(i, o)| (f64::from(tokens_in) * i + f64::from(tokens_out) * o) / 1_000_000.0)
        .unwrap_or(0.0);

    // Faithfulness via embedding (free with cache hit).
    let faithfulness = if skip_embedding || output.is_empty() {
        None
    } else {
        cosine_for_eval(router, embed_cache, spec, &output).await
    };

    // Judge via Heavy LLM.
    let (judge_score, judge_rationale) = if skip_judge || output.is_empty() {
        (None, None)
    } else {
        judge_sample(router, spec, &output).await
    };

    let gates_pass = match (faithfulness, judge_score) {
        (Some(f), Some(s)) => f >= 0.55 && s >= 7.0,
        (Some(f), None) => f >= 0.55,
        (None, Some(s)) => s >= 7.0,
        (None, None) => !output.is_empty(),
    };

    EvalSample {
        task_id: task_id.to_string(),
        model: model_used,
        output,
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
    use async_openai::types::ChatCompletionRequestSystemMessageArgs;

    let messages: Vec<ChatCompletionRequestMessage> = vec![
        match ChatCompletionRequestSystemMessageArgs::default()
            .content(JUDGE_PROMPT)
            .build()
        {
            Ok(m) => m.into(),
            Err(_) => return (None, None),
        },
        match ChatCompletionRequestUserMessageArgs::default()
            .content(format!("SPEC:\n{spec}\n\nOUTPUT:\n{output}\n\nReturn JSON."))
            .build()
        {
            Ok(m) => m.into(),
            Err(_) => return (None, None),
        },
    ];
    let (resp, _, _) = match router
        .route_chat_with_tools(JUDGE_TIER, None, messages, Vec::new())
        .await
    {
        Ok(t) => t,
        Err(e) => {
            warn!(error = ?e, "judge call failed");
            return (None, None);
        }
    };
    let content = resp
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .unwrap_or_default();
    let parsed: Option<serde_json::Value> = serde_json::from_str(content.trim()).ok().or_else(|| {
        // Try inside fences.
        let s = content.trim();
        let start = s.find('{')?;
        let end = s.rfind('}')?;
        serde_json::from_str(&s[start..=end]).ok()
    });
    match parsed {
        Some(v) => (
            v.get("score").and_then(|s| s.as_f64()),
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
        "qwen-max" => Some((1.60, 6.40)),
        "qwen-plus" => Some((0.40, 1.20)),
        "qwen-flash" => Some((0.15, 0.60)),
        "deepseek-v4-pro" => Some((0.43, 0.87)),
        "deepseek-v4-flash" => Some((0.14, 0.28)),
        _ => None,
    }
}

fn summarize(samples: &[EvalSample]) -> Vec<ModelSummary> {
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
