//! CompletionGate cascade (Spektor P4 / Kira G4).
//!
//! 5 levels ordered cheapest → most expensive. Short-circuits on first fail.
//!
//!   L1 StructuralValidation — free: simple sanity checks on output text
//!      (non-empty, no obvious truncation, balanced braces if looks like code)
//!   L2 FileIntegrity         — free: touched_paths ⊆ owned_paths (when both given)
//!   L3 ChangeSufficiency     — free: actual_loc within [0.3, 3.0] × estimated_loc
//!   L4 EmbeddingFaithfulness — ~$0.00007: cosine(embed(spec), embed(output)) >= 0.55
//!   L5 JudgeLlm              — ~$0.10: heavy-tier judge returns score >= 7/10
//!
//! Config is loaded from `tokens.toml` at startup via `GateConfig`.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, instrument};

use crate::llm::embed_cache::EmbedCache;
use crate::llm::types::ChatMessageUnified;
use crate::llm::LlmProvider;
use crate::proto::gate::{GateKind, GateOutcome, GateResult, RunGatesRequest, RunGatesResponse};
use crate::router::Router;

/// Gate-level thresholds (loaded from tokens.toml).
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct GateConfig {
    #[serde(default = "default_faithfulness_pass")]
    pub faithfulness_pass_threshold: f64,
    /// 3-zona escalate threshold: faithfulness ≥ this → high confidence,
    /// skip Judge L5 (saves $0.10 per gate run). Mid-zone [pass, escalate)
    /// always escalates to Judge.
    #[serde(default = "default_faithfulness_escalate")]
    pub faithfulness_escalate_threshold: f64,
    #[serde(default = "default_judge_pass")]
    pub judge_pass_score: f64,
    #[serde(default = "default_change_suff_min")]
    pub change_suff_min: f64,
    #[serde(default = "default_change_suff_max")]
    pub change_suff_max: f64,
    #[serde(default = "default_meta_review_coverage")]
    pub meta_review_coverage_min: f64,
}

fn default_faithfulness_pass() -> f64 { 0.55 }
fn default_faithfulness_escalate() -> f64 { 0.80 }
fn default_judge_pass() -> f64 { 7.0 }
fn default_change_suff_min() -> f64 { 0.3 }
fn default_change_suff_max() -> f64 { 3.0 }
fn default_meta_review_coverage() -> f64 { 100.0 }

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            faithfulness_pass_threshold: 0.55,
            faithfulness_escalate_threshold: 0.80,
            judge_pass_score: 7.0,
            change_suff_min: 0.3,
            change_suff_max: 3.0,
            meta_review_coverage_min: 100.0,
        }
    }
}

const EMBED_MODEL: &str = "text-embedding-v3";

const JUDGE_PROMPT: &str = "You are Kira's gate judge. \
Given a SPEC and an OUTPUT, score how well OUTPUT meets SPEC on a 0-10 scale. \
Output STRICT JSON only: {\"score\": <integer 0-10>, \"rationale\": \"<one sentence>\"}. \
No prose, no markdown fences. \
Rules: be evidence-based; NO flattery, NO hedging. \
If OUTPUT is missing or unrelated to SPEC, score 0. \
NEVER invent details about OUTPUT — score only what is literally present. \
Bias mitigation: longer != better; ignore verbosity, judge content match.";

/// Run the cascade. Returns aggregate report + per-level details.
/// `embed_cache` is optional — when present, level 4 reuses cached vectors.
/// `cfg` provides thresholds (loaded from tokens.toml).
#[instrument(skip(router, embed_cache, req), fields(skip_embed = req.skip_embedding, skip_judge = req.skip_judge))]
pub async fn run_cascade(
    router: &Router,
    embed_cache: Option<&EmbedCache>,
    req: &RunGatesRequest,
    gate_cfg: GateConfig,
) -> RunGatesResponse {
    let started = Instant::now();
    let mut results: Vec<GateResult> = Vec::new();

    // L1 StructuralValidation — heuristic first (always), then optional real
    // lint when worktree_path is provided.
    let r = gate_structural(&req.output);
    let outcome = r.outcome;
    results.push(r);
    if outcome == GateOutcome::Fail {
        return finalize(results, started);
    }
    if !req.skip_structural_lint && !req.worktree_path.is_empty() {
        let r = gate_structural_lint(&req.worktree_path).await;
        let outcome = r.outcome;
        results.push(r);
        if outcome == GateOutcome::Fail {
            return finalize(results, started);
        }
    }

    // L2 FileIntegrity
    let r = gate_file_integrity(&req.touched_paths, &req.owned_paths);
    let outcome = r.outcome;
    results.push(r);
    if outcome == GateOutcome::Fail {
        return finalize(results, started);
    }

    // L3 ChangeSufficiency
    let r = gate_change_sufficiency(req.estimated_loc, req.actual_loc);
    let outcome = r.outcome;
    results.push(r);
    if outcome == GateOutcome::Fail {
        return finalize(results, started);
    }

    // L4 EmbeddingFaithfulness
    // Tracks faithfulness similarity to enable L5 short-circuit on
    // high-confidence outputs (3-zone classification: fail / mid / high).
    let mut faith_sim: Option<f64> = None;
    if req.skip_embedding {
        results.push(skipped(GateKind::EmbeddingFaithfulness));
    } else {
        let r =
            gate_embedding_faithfulness(router, embed_cache, &req.spec, &req.output, gate_cfg).await;
        let outcome = r.outcome;
        faith_sim = r.score;
        results.push(r);
        if outcome == GateOutcome::Fail {
            return finalize(results, started);
        }
    }

    // L5 JudgeLlm — skip on high-confidence faithfulness (saves $0.10).
    let high_confidence_skip = faith_sim
        .is_some_and(|s| s >= gate_cfg.faithfulness_escalate_threshold);
    if req.skip_judge || high_confidence_skip {
        if high_confidence_skip {
            results.push(GateResult {
                gate: GateKind::JudgeLlm,
                outcome: GateOutcome::Skipped,
                score: faith_sim,
                reason: Some(format!(
                    "skipped — faithfulness {:.3} ≥ escalate {:.2} (high confidence)",
                    faith_sim.unwrap_or(0.0),
                    gate_cfg.faithfulness_escalate_threshold,
                )),
                suggestion: None,
                duration_ms: 0,
            });
        } else {
            results.push(skipped(GateKind::JudgeLlm));
        }
    } else {
        let r = gate_judge_llm(router, &req.spec, &req.output, gate_cfg).await;
        let outcome = r.outcome;
        results.push(r);
        if outcome == GateOutcome::Fail {
            return finalize(results, started);
        }
    }

    // L6 MetaReview (anti-hallucination, TRUST-inspired)
    if req.skip_meta_review || req.haystack.is_empty() {
        results.push(skipped(GateKind::MetaReview));
    } else {
        let r = gate_meta_review(&req.output, &req.haystack);
        results.push(r);
    }

    finalize(results, started)
}

// ============================================================================
// L1 — Structural validation (free)
// ============================================================================

fn gate_structural(output: &str) -> GateResult {
    let start = Instant::now();
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return GateResult {
            gate: GateKind::StructuralValidation,
            outcome: GateOutcome::Fail,
            score: Some(0.0),
            reason: Some("output is empty".into()),
            suggestion: Some("Producer must emit at least some content.".into()),
            duration_ms: ms(start),
        };
    }
    // Detect obvious truncation markers.
    if trimmed.ends_with("...") || trimmed.ends_with("…") {
        return GateResult {
            gate: GateKind::StructuralValidation,
            outcome: GateOutcome::Fail,
            score: Some(0.0),
            reason: Some("output ends with truncation marker".into()),
            suggestion: Some("Increase max_tokens or split into smaller artifacts.".into()),
            duration_ms: ms(start),
        };
    }
    // Light balance check on common brace pairs.
    let open = trimmed.matches(|c: char| matches!(c, '{' | '[' | '(')).count();
    let close = trimmed.matches(|c: char| matches!(c, '}' | ']' | ')')).count();
    if open.abs_diff(close) > 2 {
        return GateResult {
            gate: GateKind::StructuralValidation,
            outcome: GateOutcome::Fail,
            score: Some(0.0),
            reason: Some(format!("brace mismatch: open={open} close={close}")),
            suggestion: Some("Likely truncation or malformed code.".into()),
            duration_ms: ms(start),
        };
    }
    GateResult {
        gate: GateKind::StructuralValidation,
        outcome: GateOutcome::Pass,
        score: None,
        reason: None,
        suggestion: None,
        duration_ms: ms(start),
    }
}

// ============================================================================
// L1b — Real project lint/typecheck (cargo / bun) when worktree available.
// ============================================================================
//
// Detects project type by marker file:
//   - `Cargo.toml`   → `cargo check --message-format=short --offline` (60s cap)
//   - `package.json` → `bunx tsc --noEmit` if `tsconfig.json` present, else skip
//
// Skipped silently if no markers. Fails fast on first non-zero exit with
// stderr tail as `reason`.

async fn gate_structural_lint(worktree_path: &str) -> GateResult {
    use tokio::process::Command;
    use tokio::time::{timeout, Duration};

    let start = Instant::now();
    let root = std::path::Path::new(worktree_path);
    if !root.exists() {
        return GateResult {
            gate: GateKind::StructuralValidation,
            outcome: GateOutcome::Skipped,
            score: None,
            reason: Some(format!("worktree path not found: {worktree_path}")),
            suggestion: None,
            duration_ms: ms(start),
        };
    }

    // Pick lint command by marker.
    let (cmd, args, label): (&str, Vec<&str>, &str) = if root.join("Cargo.toml").exists() {
        ("cargo", vec!["check", "--message-format=short", "--offline"], "cargo check")
    } else if root.join("tsconfig.json").exists() && root.join("package.json").exists() {
        ("bunx", vec!["tsc", "--noEmit"], "bunx tsc --noEmit")
    } else {
        return GateResult {
            gate: GateKind::StructuralValidation,
            outcome: GateOutcome::Skipped,
            score: None,
            reason: Some("no Cargo.toml or tsconfig.json — no lint applicable".into()),
            suggestion: None,
            duration_ms: ms(start),
        };
    };

    let fut = Command::new(cmd)
        .args(&args)
        .current_dir(root)
        .kill_on_drop(true)
        .output();

    let output = match timeout(Duration::from_secs(60), fut).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            return GateResult {
                gate: GateKind::StructuralValidation,
                outcome: GateOutcome::Skipped,
                score: None,
                reason: Some(format!("{label} spawn failed: {e}")),
                suggestion: None,
                duration_ms: ms(start),
            };
        }
        Err(_) => {
            return GateResult {
                gate: GateKind::StructuralValidation,
                outcome: GateOutcome::Fail,
                score: None,
                reason: Some(format!("{label} timed out after 60s")),
                suggestion: Some(
                    "Lint took too long — fix incremental build or skip via skip_structural_lint."
                        .into(),
                ),
                duration_ms: ms(start),
            };
        }
    };

    if output.status.success() {
        GateResult {
            gate: GateKind::StructuralValidation,
            outcome: GateOutcome::Pass,
            score: None,
            reason: Some(format!("{label}: clean")),
            suggestion: None,
            duration_ms: ms(start),
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail = stderr
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        GateResult {
            gate: GateKind::StructuralValidation,
            outcome: GateOutcome::Fail,
            score: None,
            reason: Some(format!("{label} exit={:?}; tail:\n{tail}", output.status.code())),
            suggestion: Some(
                "Fix the lint/typecheck errors before next gate runs. Cheapest fix → most leverage."
                    .into(),
            ),
            duration_ms: ms(start),
        }
    }
}

// ============================================================================
// L2 — File integrity (free)
// ============================================================================

fn gate_file_integrity(touched: &[String], owned: &[String]) -> GateResult {
    let start = Instant::now();
    if owned.is_empty() || touched.is_empty() {
        // Nothing to check — pass by default (no ownership context).
        return GateResult {
            gate: GateKind::FileIntegrity,
            outcome: GateOutcome::Pass,
            score: None,
            reason: Some("no ownership context provided".into()),
            suggestion: None,
            duration_ms: ms(start),
        };
    }
    // Build globsets from owned patterns.
    let mut globs = globset::GlobSetBuilder::new();
    let mut bad_pattern: Option<String> = None;
    for p in owned {
        match globset::Glob::new(p) {
            Ok(g) => {
                globs.add(g);
            }
            Err(e) => {
                bad_pattern = Some(format!("{p}: {e}"));
                break;
            }
        }
    }
    if let Some(bad) = bad_pattern {
        return GateResult {
            gate: GateKind::FileIntegrity,
            outcome: GateOutcome::Fail,
            score: None,
            reason: Some(format!("invalid owned glob: {bad}")),
            suggestion: Some("Fix the owned_paths globs.".into()),
            duration_ms: ms(start),
        };
    }
    let set = match globs.build() {
        Ok(s) => s,
        Err(e) => {
            return GateResult {
                gate: GateKind::FileIntegrity,
                outcome: GateOutcome::Fail,
                score: None,
                reason: Some(format!("globset build: {e}")),
                suggestion: None,
                duration_ms: ms(start),
            };
        }
    };
    let off_scope: Vec<&String> = touched.iter().filter(|p| !set.is_match(p)).collect();
    if off_scope.is_empty() {
        GateResult {
            gate: GateKind::FileIntegrity,
            outcome: GateOutcome::Pass,
            score: None,
            reason: None,
            suggestion: None,
            duration_ms: ms(start),
        }
    } else {
        GateResult {
            gate: GateKind::FileIntegrity,
            outcome: GateOutcome::Fail,
            score: None,
            reason: Some(format!(
                "touched paths outside owned scope: {}",
                off_scope.iter().take(3).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            )),
            suggestion: Some("Either expand ownership or limit writes to owned globs.".into()),
            duration_ms: ms(start),
        }
    }
}

// ============================================================================
// L3 — Change sufficiency (free)
// ============================================================================

fn gate_change_sufficiency(estimated: Option<u32>, actual: Option<u32>) -> GateResult {
    const CHANGE_SUFF_MIN: f64 = 0.3;
    const CHANGE_SUFF_MAX: f64 = 3.0;
    let start = Instant::now();
    let (Some(e), Some(a)) = (estimated, actual) else {
        return GateResult {
            gate: GateKind::ChangeSufficiency,
            outcome: GateOutcome::Pass,
            score: None,
            reason: Some("no LOC estimate / actual provided".into()),
            suggestion: None,
            duration_ms: ms(start),
        };
    };
    if e == 0 {
        return GateResult {
            gate: GateKind::ChangeSufficiency,
            outcome: GateOutcome::Pass,
            score: None,
            reason: Some("estimated_loc=0".into()),
            suggestion: None,
            duration_ms: ms(start),
        };
    }
    let ratio = f64::from(a) / f64::from(e);
    let outcome = if (CHANGE_SUFF_MIN..=CHANGE_SUFF_MAX).contains(&ratio) {
        GateOutcome::Pass
    } else {
        GateOutcome::Fail
    };
    GateResult {
        gate: GateKind::ChangeSufficiency,
        outcome,
        score: Some(ratio),
        reason: Some(format!(
            "actual {a} vs estimated {e} → ratio {ratio:.2} (need [{min}, {max}])",
            min = CHANGE_SUFF_MIN,
            max = CHANGE_SUFF_MAX,
        )),
        suggestion: if outcome == GateOutcome::Fail {
            Some(if ratio < CHANGE_SUFF_MIN {
                "Diff is suspiciously small — agent may have done nothing.".into()
            } else {
                "Diff is suspiciously large — scope creep or wrong change.".into()
            })
        } else {
            None
        },
        duration_ms: ms(start),
    }
}

// ============================================================================
// L4 — Embedding faithfulness (~$0.00007)
// ============================================================================

async fn gate_embedding_faithfulness(
    router: &Router,
    embed_cache: Option<&EmbedCache>,
    spec: &str,
    output: &str,
    gate_cfg: GateConfig,
) -> GateResult {
    let start = Instant::now();
    let Some(provider) = router.provider_handle("dashscope") else {
        return GateResult {
            gate: GateKind::EmbeddingFaithfulness,
            outcome: GateOutcome::Skipped,
            score: None,
            reason: Some("dashscope not configured — embedding gate skipped".into()),
            suggestion: None,
            duration_ms: ms(start),
        };
    };
    match embed_pair(&provider, embed_cache, spec, output).await {
        Ok((a, b)) => {
            let sim = cosine(&a, &b);
            // 3-zone classification (TRUST-aligned):
            //   sim < pass          → Fail fast (don't waste Judge LLM)
            //   pass ≤ sim < escal  → Pass, but force Judge L5 to confirm
            //   sim ≥ escal         → Pass with high confidence (Judge optional)
            let pass = gate_cfg.faithfulness_pass_threshold;
            let esc = gate_cfg.faithfulness_escalate_threshold;
            let outcome = if sim < pass {
                GateOutcome::Fail
            } else {
                GateOutcome::Pass
            };
            let zone = if sim < pass {
                "fail_fast"
            } else if sim < esc {
                "escalate_to_judge"
            } else {
                "high_confidence"
            };
            GateResult {
                gate: GateKind::EmbeddingFaithfulness,
                outcome,
                score: Some(sim),
                reason: Some(format!(
                    "cosine(spec, output)={sim:.3} · zone={zone} (pass {pass:.2}, escalate {esc:.2})",
                )),
                suggestion: match outcome {
                    GateOutcome::Fail => Some(
                        "Output semantically distant from spec — likely wrong content.".into(),
                    ),
                    GateOutcome::Pass if sim < esc => Some(
                        "Mid-zone similarity — Judge L5 will confirm.".into(),
                    ),
                    _ => None,
                },
                duration_ms: ms(start),
            }
        }
        Err(e) => GateResult {
            gate: GateKind::EmbeddingFaithfulness,
            outcome: GateOutcome::Skipped,
            score: None,
            reason: Some(format!("embed call failed: {e}")),
            suggestion: None,
            duration_ms: ms(start),
        },
    }
}

async fn embed_pair(
    provider: &Arc<LlmProvider>,
    cache: Option<&EmbedCache>,
    a: &str,
    b: &str,
) -> Result<(Vec<f32>, Vec<f32>)> {
    let inputs = vec![a.to_string(), b.to_string()];
    let vecs = match cache {
        Some(c) => c
            .embed_or_compute(provider, EMBED_MODEL, inputs)
            .await
            .context("embed_or_compute failed")?,
        None => provider
            .embed(EMBED_MODEL, inputs)
            .await
            .context("embed call failed")?,
    };
    if vecs.len() != 2 {
        anyhow::bail!("expected 2 vectors, got {}", vecs.len());
    }
    let mut it = vecs.into_iter();
    let a = it.next().unwrap();
    let b = it.next().unwrap();
    Ok((a, b))
}

/// Public helper for the eval framework — computes spec↔output cosine
/// via cache when available. Returns None on any failure.
pub async fn cosine_for_eval(
    router: &Router,
    cache: &EmbedCache,
    spec: &str,
    output: &str,
) -> Option<f64> {
    let provider = router.provider_handle("dashscope")?;
    match embed_pair(&provider, Some(cache), spec, output).await {
        Ok((a, b)) => Some(cosine(&a, &b)),
        Err(_) => None,
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f64;
    let mut na = 0.0_f64;
    let mut nb = 0.0_f64;
    for i in 0..a.len() {
        dot += f64::from(a[i]) * f64::from(b[i]);
        na += f64::from(a[i]) * f64::from(a[i]);
        nb += f64::from(b[i]) * f64::from(b[i]);
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

// ============================================================================
// L5 — Judge LLM (~$0.10)
// ============================================================================

async fn gate_judge_llm(
    router: &Router,
    spec: &str,
    output: &str,
    gate_cfg: GateConfig,
) -> GateResult {
    let start = Instant::now();
    let messages = vec![
        ChatMessageUnified::System {
            content: JUDGE_PROMPT.to_string(),
        },
        ChatMessageUnified::User {
            content: format!("SPEC:\n{spec}\n\nOUTPUT:\n{output}\n\nReturn JSON."),
        },
    ];
    let (resp, _provider, model) = match router
        .route_chat_with_tools("heavy", None, messages, Vec::new())
        .await
    {
        Ok(t) => t,
        Err(e) => {
            return GateResult {
                gate: GateKind::JudgeLlm,
                outcome: GateOutcome::Skipped,
                score: None,
                reason: Some(format!("judge LLM call failed: {e}")),
                suggestion: None,
                duration_ms: ms(start),
            };
        }
    };
    let content = resp.content;
    let json: serde_json::Value = match serde_json::from_str(content.trim()) {
        Ok(v) => v,
        Err(_) => {
            // Try to extract JSON from inside fences.
            let extracted = extract_json_blob(&content).unwrap_or_default();
            match serde_json::from_str(&extracted) {
                Ok(v) => v,
                Err(e) => {
                    return GateResult {
                        gate: GateKind::JudgeLlm,
                        outcome: GateOutcome::Skipped,
                        score: None,
                        reason: Some(format!("judge returned non-JSON ({e}); model={model}")),
                        suggestion: Some("Re-run with stricter prompt or different judge model.".into()),
                        duration_ms: ms(start),
                    };
                }
            }
        }
    };
    let score = json
        .get("score")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let rationale = json
        .get("rationale")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let outcome = if score >= gate_cfg.judge_pass_score {
        GateOutcome::Pass
    } else {
        GateOutcome::Fail
    };
    debug!(score, ?outcome, "judge gate");
    GateResult {
        gate: GateKind::JudgeLlm,
        outcome,
        score: Some(score),
        reason: rationale,
        suggestion: if outcome == GateOutcome::Fail {
            Some(format!("Judge score {score} below threshold {}", gate_cfg.judge_pass_score))
        } else {
            None
        },
        duration_ms: ms(start),
    }
}

// ============================================================================
// L6 — Meta-review (anti-hallucination, TRUST-inspired)
// ============================================================================
//
// Parses `output` as reviewer JSON, walks `findings[].evidence_quote`, and
// confirms each quote is a LITERAL substring of `haystack` (case-sensitive,
// after trimming surrounding whitespace). Any quote that fails fails the gate.
// When output is not reviewer-format JSON, the gate skips (not the right
// artifact). When `findings` is empty, the gate passes (nothing to hallucinate).

fn gate_meta_review(output: &str, haystack: &str) -> GateResult {
    const META_REVIEW_COVERAGE_MIN: f64 = 100.0;
    let start = Instant::now();
    let json_str = if output.trim_start().starts_with('{') {
        output.trim().to_string()
    } else {
        extract_json_blob(output).unwrap_or_default()
    };
    let parsed: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(_) => {
            return GateResult {
                gate: GateKind::MetaReview,
                outcome: GateOutcome::Skipped,
                score: None,
                reason: Some("output is not reviewer-format JSON".into()),
                suggestion: None,
                duration_ms: ms(start),
            };
        }
    };
    let Some(findings) = parsed.get("findings").and_then(|f| f.as_array()) else {
        return GateResult {
            gate: GateKind::MetaReview,
            outcome: GateOutcome::Skipped,
            score: None,
            reason: Some("no `findings` array in output".into()),
            suggestion: None,
            duration_ms: ms(start),
        };
    };
    // Coverage matrix check (TRUST: pct_evaluated < 100 = HALT).
    if let Some(cov) = parsed.get("coverage") {
        let pct = cov.get("pct_evaluated").and_then(|v| v.as_f64()).unwrap_or(-1.0);
        if pct >= 0.0 && pct < META_REVIEW_COVERAGE_MIN {
            return GateResult {
                gate: GateKind::MetaReview,
                outcome: GateOutcome::Fail,
                score: Some(pct),
                reason: Some(format!(
                    "reviewer coverage {pct:.0}% < required {META_REVIEW_COVERAGE_MIN:.0}% (rule×file matrix incomplete)"
                )),
                suggestion: Some(
                    "Reviewer must evaluate every rule against every changed file (PASS/VIOLATION/N/A) before declaring done."
                        .into(),
                ),
                duration_ms: ms(start),
            };
        }
    }
    let total = findings.len();
    let mut hallucinated: Vec<(usize, String)> = Vec::new();
    for (i, f) in findings.iter().enumerate() {
        let Some(quote) = f.get("evidence_quote").and_then(|q| q.as_str()) else {
            hallucinated.push((i, "missing evidence_quote".into()));
            continue;
        };
        let q = quote.trim();
        if q.is_empty() {
            hallucinated.push((i, "empty evidence_quote".into()));
            continue;
        }
        if !haystack.contains(q) {
            hallucinated.push((i, format!("quote not found literally: {:.80}…", q)));
        }
    }
    let pass_count = total - hallucinated.len();
    let pct = if total == 0 { 100.0 } else { (pass_count as f64) / (total as f64) * 100.0 };
    let outcome = if hallucinated.is_empty() {
        GateOutcome::Pass
    } else {
        GateOutcome::Fail
    };
    let reason = if hallucinated.is_empty() {
        Some(format!("{total} finding(s) all evidence-grounded"))
    } else {
        let sample = hallucinated.iter().take(3)
            .map(|(i, r)| format!("[{i}] {r}"))
            .collect::<Vec<_>>().join("; ");
        Some(format!(
            "{} of {total} findings failed literal-evidence check ({pct:.0}% grounded); {sample}",
            hallucinated.len()
        ))
    };
    GateResult {
        gate: GateKind::MetaReview,
        outcome,
        score: Some(pct),
        reason,
        suggestion: if outcome == GateOutcome::Fail {
            Some(
                "Reviewer hallucinated evidence. Drop unverifiable findings; re-quote literally."
                    .into(),
            )
        } else {
            None
        },
        duration_ms: ms(start),
    }
}

fn extract_json_blob(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end > start {
        Some(s[start..=end].to_string())
    } else {
        None
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn skipped(gate: GateKind) -> GateResult {
    GateResult {
        gate,
        outcome: GateOutcome::Skipped,
        score: None,
        reason: Some("skipped by caller".into()),
        suggestion: None,
        duration_ms: 0,
    }
}

fn ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

fn finalize(results: Vec<GateResult>, started: Instant) -> RunGatesResponse {
    let failed_at = results
        .iter()
        .find(|r| r.outcome == GateOutcome::Fail)
        .map(|r| r.gate);
    let overall = if failed_at.is_some() {
        GateOutcome::Fail
    } else {
        GateOutcome::Pass
    };
    let total_ms = started.elapsed().as_millis() as u64;
    info!(?overall, ?failed_at, duration_ms = total_ms, "gates cascade finished");
    RunGatesResponse {
        results,
        overall,
        failed_at,
        duration_ms: total_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_empty_fails() {
        let r = gate_structural("   \n  ");
        assert_eq!(r.outcome, GateOutcome::Fail);
    }

    #[test]
    fn structural_truncated_fails() {
        let r = gate_structural("some long text...");
        assert_eq!(r.outcome, GateOutcome::Fail);
    }

    #[test]
    fn structural_balanced_passes() {
        let r = gate_structural("fn foo() { let x = vec![1, 2]; x.iter().sum::<i32>() }");
        assert_eq!(r.outcome, GateOutcome::Pass);
    }

    #[test]
    fn file_integrity_pass_when_owned() {
        let r = gate_file_integrity(
            &["src/content/foo.md".into()],
            &["src/content/**".into()],
        );
        assert_eq!(r.outcome, GateOutcome::Pass);
    }

    #[test]
    fn file_integrity_fail_outside_scope() {
        let r = gate_file_integrity(
            &["src/secrets/.env".into()],
            &["src/content/**".into()],
        );
        assert_eq!(r.outcome, GateOutcome::Fail);
    }

    #[test]
    fn change_sufficiency_within_range_passes() {
        let r = gate_change_sufficiency(Some(100), Some(80));
        assert_eq!(r.outcome, GateOutcome::Pass);
    }

    #[test]
    fn change_sufficiency_too_small_fails() {
        let r = gate_change_sufficiency(Some(100), Some(2));
        assert_eq!(r.outcome, GateOutcome::Fail);
    }

    #[test]
    fn change_sufficiency_no_estimate_passes() {
        let r = gate_change_sufficiency(None, Some(50));
        assert_eq!(r.outcome, GateOutcome::Pass);
    }

    #[test]
    fn cosine_orthogonal_zero() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        assert!(cosine(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn meta_review_passes_when_quotes_literal() {
        let hay = "fn foo() { let token = req.body.token; }";
        let out = r#"{"findings":[{"evidence_quote":"req.body.token","rule_id":"SEC-001"}]}"#;
        let r = gate_meta_review(out, hay);
        assert_eq!(r.outcome, GateOutcome::Pass);
    }

    #[test]
    fn meta_review_fails_on_hallucinated_quote() {
        let hay = "fn foo() { let token = req.body.token; }";
        let out = r#"{"findings":[{"evidence_quote":"jwt.verify(secret)","rule_id":"SEC-002"}]}"#;
        let r = gate_meta_review(out, hay);
        assert_eq!(r.outcome, GateOutcome::Fail);
    }

    #[test]
    fn meta_review_skips_non_reviewer_json() {
        let r = gate_meta_review("plain text not json", "anything");
        assert_eq!(r.outcome, GateOutcome::Skipped);
    }

    #[test]
    fn meta_review_skips_json_without_findings() {
        let r = gate_meta_review(r#"{"score": 8}"#, "anything");
        assert_eq!(r.outcome, GateOutcome::Skipped);
    }

    #[test]
    fn meta_review_passes_with_empty_findings() {
        let r = gate_meta_review(r#"{"findings": []}"#, "anything");
        assert_eq!(r.outcome, GateOutcome::Pass);
    }

    #[test]
    fn meta_review_fails_on_incomplete_coverage() {
        let out = r#"{"findings":[], "coverage":{"pct_evaluated": 80}}"#;
        let r = gate_meta_review(out, "anything");
        assert_eq!(r.outcome, GateOutcome::Fail);
    }

    #[test]
    fn meta_review_passes_with_full_coverage() {
        let out = r#"{"findings":[], "coverage":{"pct_evaluated": 100}}"#;
        let r = gate_meta_review(out, "anything");
        assert_eq!(r.outcome, GateOutcome::Pass);
    }

    #[test]
    fn cosine_identical_one() {
        let a = vec![1.0_f32, 2.0, 3.0];
        assert!((cosine(&a, &a) - 1.0).abs() < 1e-6);
    }
}
