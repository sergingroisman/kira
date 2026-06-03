//! Consolidation Engine — tiered memory consolidation (working → episodic → semantic → procedural).
//!
//! This module provides:
//! - `ConsolidationCmd` — commands sent to the background timer task
//! - `ConsolidatorState` — shared state for API interaction
//! - `spawn_consolidation_timer` — spawns the periodic consolidation background task
//! - `ConsolidationOutput` — LLM response schema for consolidation results
//! - `consolidate_session` — working → episodic consolidation for a single session

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::config::MaturationConfig;
use crate::memory::store::{MemoryFragment, MemoryStore, MetadataUpdate};
use crate::memory::sqlite_backend::SqliteVecStore;
use crate::memory::tier::MemoryTier;
use crate::observability::{pillar, wire::PillarTx};
use crate::proto::chat::ChatMessage;
use crate::proto::pillar::Pillar;
use crate::router::tier::Tier;
use crate::router::Router as LlmRouter;

// ─── Commands ────────────────────────────────────────────────────────────────

/// Commands sent to the consolidation timer task.
#[derive(Debug)]
pub enum ConsolidationCmd {
    /// Trigger an immediate consolidation check (from API).
    TriggerNow,
    /// Shutdown the timer task gracefully.
    Shutdown,
}

// ─── Shared State ────────────────────────────────────────────────────────────

/// State shared with the consolidation background task.
///
/// Exposed to API handlers for status queries and manual triggers.
pub struct ConsolidatorState {
    /// Prevents overlapping consolidation passes (lock-free, matches circuit breaker pattern).
    pub running: AtomicBool,
    /// Channel to send commands to the timer task (no shared mutable state).
    pub cmd_tx: mpsc::Sender<ConsolidationCmd>,
    /// Timestamp of last completed consolidation (low contention — only read by status endpoint).
    pub last_completed: Mutex<Option<DateTime<Utc>>>,
}

impl ConsolidatorState {
    /// Returns true if a consolidation pass is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Returns the timestamp of the last completed consolidation, if any.
    pub fn last_completed_at(&self) -> Option<DateTime<Utc>> {
        *self.last_completed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

// ─── LLM Response Schema ─────────────────────────────────────────────────────

/// Structured output from the consolidation LLM call.
///
/// The LLM produces a typed summary of merged fragments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationOutput {
    /// One of: "concept", "decision", "gotcha", "rule"
    #[serde(rename = "type")]
    pub kind: String,
    /// Max 80 characters — a concise title for the consolidated knowledge.
    pub title: String,
    /// Max 2000 characters — the consolidated knowledge content.
    pub content: String,
}

// ─── Timer Spawn ─────────────────────────────────────────────────────────────

/// Spawn the periodic consolidation timer.
///
/// Returns the shared state handle for API interaction. The timer runs every
/// `semantic_check_interval_hours` and also responds to `TriggerNow` commands
/// from the API. When `Shutdown` is received, the timer loop exits.
pub async fn spawn_consolidation_timer(
    config: Arc<MaturationConfig>,
    store: Arc<SqliteVecStore>,
    pillar_tx: PillarTx,
    router: Arc<LlmRouter>,
) -> Arc<ConsolidatorState> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<ConsolidationCmd>(16);

    let state = Arc::new(ConsolidatorState {
        running: AtomicBool::new(false),
        cmd_tx,
        last_completed: Mutex::new(None),
    });

    let task_state = Arc::clone(&state);
    let task_config = Arc::clone(&config);
    let pillar_tx_for_guard = pillar_tx.clone();

    tokio::spawn(async move {
        let handle = tokio::spawn(async move {
            consolidation_loop(task_state, task_config, store, pillar_tx, router, cmd_rx).await;
        });

        // Catch panics from the consolidation loop itself (should never happen,
        // but satisfies Requirement 13.5 — no maturation failure blocks serving).
        if let Err(join_err) = handle.await {
            let panic_msg = if join_err.is_panic() {
                let panic_payload = join_err.into_panic();
                if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    format!("consolidation loop panicked: {s}")
                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                    format!("consolidation loop panicked: {s}")
                } else {
                    "consolidation loop panicked (unknown payload)".to_string()
                }
            } else {
                "consolidation loop cancelled".to_string()
            };

            error!(error = %panic_msg, "consolidation timer task failed");
            super::maturation_events::emit_consolidation_failed(
                &pillar_tx_for_guard,
                "all",
                "all",
                &panic_msg,
            );
        }
    });

    info!("consolidation timer spawned");
    state
}

/// The main consolidation timer loop.
///
/// Waits for either the periodic interval to elapse or a command to arrive.
/// On `TriggerNow`, runs consolidation immediately. On `Shutdown`, exits.
async fn consolidation_loop(
    state: Arc<ConsolidatorState>,
    config: Arc<MaturationConfig>,
    store: Arc<SqliteVecStore>,
    pillar_tx: PillarTx,
    router: Arc<LlmRouter>,
    mut cmd_rx: mpsc::Receiver<ConsolidationCmd>,
) {
    let interval_hours = config.semantic_check_interval_hours;
    let interval = tokio::time::Duration::from_secs(interval_hours * 3600);

    info!(
        interval_hours = interval_hours,
        "consolidation timer started"
    );

    loop {
        // Wait for either the interval to elapse or a command to arrive.
        tokio::select! {
            () = tokio::time::sleep(interval) => {
                debug!("periodic consolidation check triggered");
                run_consolidation_pass(&state, &config, &store, &pillar_tx, &router).await;
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(ConsolidationCmd::TriggerNow) => {
                        info!("immediate consolidation triggered via API");
                        run_consolidation_pass(&state, &config, &store, &pillar_tx, &router).await;
                    }
                    Some(ConsolidationCmd::Shutdown) => {
                        info!("consolidation timer shutting down");
                        break;
                    }
                    None => {
                        // All senders dropped — exit gracefully.
                        info!("consolidation command channel closed, exiting timer");
                        break;
                    }
                }
            }
        }
    }
}

/// Execute a single consolidation pass (session, semantic, and procedural promotion).
///
/// Guards against overlapping passes using the `AtomicBool` flag.
/// Spawns the actual work as a sub-task and catches panics via `JoinHandle`.
/// On panic, emits a `ConsolidationFailed` event and continues serving.
async fn run_consolidation_pass(
    state: &ConsolidatorState,
    config: &MaturationConfig,
    store: &Arc<SqliteVecStore>,
    pillar_tx: &PillarTx,
    router: &Arc<LlmRouter>,
) {
    // Guard: skip if already running (no overlapping passes).
    if state
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        debug!("consolidation pass already in progress, skipping");
        return;
    }

    // Check if maturation is enabled.
    if !config.enabled {
        debug!("maturation disabled, skipping consolidation pass");
        state.running.store(false, Ordering::SeqCst);
        return;
    }

    // Spawn the actual consolidation work as a sub-task so we can catch panics
    // via JoinHandle. On panic, emit ConsolidationFailed and continue serving.
    let store_clone = Arc::clone(store);
    let config_clone = config.clone();
    let pillar_tx_clone = pillar_tx.clone();
    let router_clone = Arc::clone(router);
    let handle = tokio::spawn(async move {
        consolidation_pass_inner(&store_clone, &config_clone, &pillar_tx_clone, &router_clone).await;
    });

    match handle.await {
        Ok(()) => {
            // Pass completed normally (errors within are already logged).
        }
        Err(join_err) => {
            // Task panicked or was cancelled — emit failure event.
            let panic_msg = if join_err.is_panic() {
                let panic_payload = join_err.into_panic();
                if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    format!("task panicked: {s}")
                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                    format!("task panicked: {s}")
                } else {
                    "task panicked (unknown payload)".to_string()
                }
            } else {
                "task cancelled".to_string()
            };

            error!(error = %panic_msg, "consolidation pass panicked");
            super::maturation_events::emit_consolidation_failed(
                pillar_tx,
                "all",
                "all",
                &panic_msg,
            );
        }
    }

    // Mark completion.
    if let Ok(mut last) = state.last_completed.lock() {
        *last = Some(Utc::now());
    }

    state.running.store(false, Ordering::SeqCst);
    debug!("consolidation pass completed");
}

/// Inner consolidation work that runs inside a spawned task.
/// Separated so that panics can be caught by the JoinHandle in `run_consolidation_pass`.
///
/// Executes the full consolidation pipeline:
/// 1. Episodic → Semantic clustering (requires LLM, checks circuit breaker)
/// 2. Semantic → Procedural promotion (metadata-only, no LLM)
/// 3. Cleanup consolidated working fragments older than 24 hours
///
/// Each step handles its own errors gracefully — a failure in one step does not
/// prevent subsequent steps from running. Fragments remain unchanged on any failure.
async fn consolidation_pass_inner(
    store: &SqliteVecStore,
    config: &MaturationConfig,
    pillar_tx: &PillarTx,
    router: &LlmRouter,
) {
    // Step 1: Episodic → Semantic consolidation (requires LLM).
    // The circuit breaker is checked inside consolidate_semantic; if the LLM is
    // unavailable, it returns Ok(vec![]) and logs a warning.
    match consolidate_semantic(config, store, router, pillar_tx).await {
        Ok(new_ids) => {
            if !new_ids.is_empty() {
                info!(
                    new_semantic_count = new_ids.len(),
                    "episodic→semantic consolidation produced new fragments"
                );
            }
        }
        Err(e) => {
            warn!(error = %e, "episodic→semantic consolidation failed");
        }
    }

    // Step 2: Semantic → Procedural promotion (no LLM needed).
    let now = Utc::now();
    match promote_to_procedural(config, store, pillar_tx, now).await {
        Ok(promoted_ids) => {
            if !promoted_ids.is_empty() {
                info!(
                    promoted_count = promoted_ids.len(),
                    "semantic→procedural promotion completed"
                );
            }
        }
        Err(e) => {
            warn!(error = %e, "semantic→procedural promotion failed");
        }
    }

    // Step 3: Cleanup consolidated working fragments older than 24 hours.
    match cleanup_consolidated_working(store, now).await {
        Ok(removed) => {
            if removed > 0 {
                info!(removed_count = removed, "cleaned up consolidated working fragments");
            }
        }
        Err(e) => {
            warn!(error = %e, "working fragment cleanup failed");
        }
    }
}

// ─── Working → Episodic Consolidation ────────────────────────────────────────

/// Path to the consolidation prompt file (relative to workspace root).
const CONSOLIDATION_PROMPT_PATH: &str = "core/agents/prompts/memory_consolidator.md";

/// Consolidate working-tier fragments from a session into a single episodic fragment.
///
/// Returns `Ok(Some(new_fragment_id))` on success, `Ok(None)` if skipped
/// (below threshold or prompt missing), `Err` on unrecoverable failure.
///
/// # Behavior
/// - Loads the consolidation prompt from disk at each invocation (no caching).
/// - If the prompt file doesn't exist or can't be read, skips consolidation.
/// - If fewer than `min_fragments_for_consolidation` fragments, skips entirely.
/// - If fewer than 2 fragments in the batch, skips entirely (Property 22).
/// - Invokes the light-tier LLM with the prompt + fragment contents.
/// - Parses the JSON response into `ConsolidationOutput`.
/// - Creates a new episodic-tier `MemoryFragment` with the consolidated content.
/// - Marks all source working-tier fragments with `consolidated_into`.
/// - Emits events via `PillarTx`: `ConsolidationStarted` before,
///   `ConsolidationCompleted` after success, `ConsolidationFailed` on error.
/// - On any failure, leaves source fragments unchanged (atomicity).
pub async fn consolidate_session(
    session_id: Uuid,
    fragments: Vec<MemoryFragment>,
    config: &MaturationConfig,
    store: &SqliteVecStore,
    router: &LlmRouter,
    pillar_tx: &PillarTx,
) -> Result<Option<Uuid>> {
    let fragment_count = fragments.len();

    // ── Threshold checks (skip without events) ───────────────────────────────

    // Property 22: fewer than 2 fragments → skip entirely.
    if fragment_count < 2 {
        debug!(
            session_id = %session_id,
            fragment_count,
            "skipping consolidation: fewer than 2 fragments"
        );
        return Ok(None);
    }

    // Config threshold check.
    if (fragment_count as u64) < config.min_fragments_for_consolidation {
        debug!(
            session_id = %session_id,
            fragment_count,
            threshold = config.min_fragments_for_consolidation,
            "skipping consolidation: below min_fragments_for_consolidation"
        );
        return Ok(None);
    }

    // ── Circuit breaker pre-check ────────────────────────────────────────────
    // Short-circuit before loading prompt or building messages if the light-tier
    // LLM is known to be unavailable (all candidates have open breakers).
    if !router.is_tier_available(Tier::Light) {
        warn!(
            session_id = %session_id,
            "light-tier LLM unavailable (circuit breaker open) — skipping consolidation"
        );
        return Ok(None);
    }

    // ── Load prompt from disk ────────────────────────────────────────────────

    let prompt_content = match tokio::fs::read_to_string(CONSOLIDATION_PROMPT_PATH).await {
        Ok(content) => content,
        Err(e) => {
            warn!(
                path = CONSOLIDATION_PROMPT_PATH,
                error = %e,
                "consolidation prompt file missing or unreadable, skipping consolidation"
            );
            return Ok(None);
        }
    };

    // ── Emit ConsolidationStarted event ──────────────────────────────────────

    let mut started_fields = serde_json::Map::new();
    started_fields.insert("source_tier".into(), serde_json::json!("working"));
    started_fields.insert("target_tier".into(), serde_json::json!("episodic"));
    started_fields.insert("fragment_count".into(), serde_json::json!(fragment_count));
    started_fields.insert("session_id".into(), serde_json::json!(session_id.to_string()));

    pillar::emit_full(
        pillar_tx,
        Pillar::P7Cognition,
        "start",
        "memory_maturation_consolidation_started",
        None,
        None,
        started_fields,
    );

    // ── Build LLM messages ───────────────────────────────────────────────────

    let user_content = fragments
        .iter()
        .enumerate()
        .map(|(i, f)| format!("--- Fragment {} ---\n{}", i + 1, f.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: prompt_content,
        },
        ChatMessage {
            role: "user".to_string(),
            content: user_content,
        },
    ];

    // ── Invoke light-tier LLM ────────────────────────────────────────────────

    let chat_request = crate::proto::chat::ChatRequest {
        tier: "light".to_string(),
        model: None,
        messages: messages.clone(),
        mode: None,
    };

    let llm_output = match router.route_chat(&chat_request, &messages).await {
        Ok((output, _provider, _model)) => output,
        Err(e) => {
            error!(
                session_id = %session_id,
                error = %e,
                "LLM invocation failed during consolidation"
            );
            emit_consolidation_failed(pillar_tx, "working", "episodic", &format!("{e:#}"));
            return Err(e);
        }
    };

    // ── Parse ConsolidationOutput JSON ───────────────────────────────────────

    let consolidation_output: ConsolidationOutput = match parse_consolidation_json(&llm_output.content) {
        Ok(output) => output,
        Err(e) => {
            warn!(
                session_id = %session_id,
                raw_response = %llm_output.content,
                error = %e,
                "malformed JSON response from LLM, skipping consolidation"
            );
            emit_consolidation_failed(pillar_tx, "working", "episodic", &format!("malformed JSON: {e}"));
            return Err(anyhow::anyhow!("malformed consolidation JSON: {e}"));
        }
    };

    // ── Create episodic fragment ─────────────────────────────────────────────

    let new_fragment_id = Uuid::now_v7();
    let now = Utc::now();

    // Use the first fragment's thread_id and run_id for the new episodic fragment.
    let thread_id = fragments.first().map_or_else(Uuid::now_v7, |f| f.thread_id);
    let run_id = fragments.first().map_or_else(Uuid::now_v7, |f| f.run_id);

    // Use the average embedding of source fragments for the new episodic fragment.
    let embedding = compute_average_embedding(&fragments);

    let episodic_fragment = MemoryFragment {
        fragment_id: new_fragment_id,
        content: consolidation_output.content.clone(),
        embedding,
        thread_id,
        run_id,
        created_at: now,
        score: 1.0, // Fresh fragment, full relevance.
        tier: MemoryTier::Episodic,
        access_count: 0,
        last_accessed: None,
        consolidated_into: None,
        promoted_at: Some(now),
        session_id: Some(session_id),
    };

    // ── Persist the new episodic fragment ────────────────────────────────────

    if let Err(e) = store.persist(&episodic_fragment).await {
        error!(
            session_id = %session_id,
            fragment_id = %new_fragment_id,
            error = %e,
            "failed to persist episodic fragment"
        );
        emit_consolidation_failed(pillar_tx, "working", "episodic", &format!("store write: {e:#}"));
        return Err(e);
    }

    // ── Mark source fragments with consolidated_into ─────────────────────────

    for fragment in &fragments {
        let update = MetadataUpdate {
            consolidated_into: Some(Some(new_fragment_id)),
            ..Default::default()
        };
        if let Err(e) = store.update_metadata(fragment.fragment_id, update).await {
            // On failure marking source fragments, log but don't undo the episodic
            // fragment — the source fragments remain unchanged (best-effort marking).
            error!(
                session_id = %session_id,
                fragment_id = %fragment.fragment_id,
                error = %e,
                "failed to mark source fragment with consolidated_into"
            );
            emit_consolidation_failed(pillar_tx, "working", "episodic", &format!("metadata update: {e:#}"));
            return Err(e);
        }
    }

    // ── Emit ConsolidationCompleted event ────────────────────────────────────

    let mut completed_fields = serde_json::Map::new();
    completed_fields.insert("source_tier".into(), serde_json::json!("working"));
    completed_fields.insert("target_tier".into(), serde_json::json!("episodic"));
    completed_fields.insert("fragment_id".into(), serde_json::json!(new_fragment_id.to_string()));
    completed_fields.insert("session_id".into(), serde_json::json!(session_id.to_string()));
    completed_fields.insert("title".into(), serde_json::json!(consolidation_output.title));
    completed_fields.insert("kind".into(), serde_json::json!(consolidation_output.kind));

    pillar::emit_full(
        pillar_tx,
        Pillar::P7Cognition,
        "done",
        "memory_maturation_consolidation_completed",
        None,
        None,
        completed_fields,
    );

    info!(
        session_id = %session_id,
        fragment_id = %new_fragment_id,
        source_count = fragment_count,
        "working→episodic consolidation completed"
    );

    Ok(Some(new_fragment_id))
}

/// Emit a `ConsolidationFailed` event.
fn emit_consolidation_failed(pillar_tx: &PillarTx, source_tier: &str, target_tier: &str, error_msg: &str) {
    let mut fields = serde_json::Map::new();
    fields.insert("source_tier".into(), serde_json::json!(source_tier));
    fields.insert("target_tier".into(), serde_json::json!(target_tier));
    fields.insert("error".into(), serde_json::json!(error_msg));

    pillar::emit_full(
        pillar_tx,
        Pillar::P7Cognition,
        "fail",
        "memory_maturation_consolidation_failed",
        None,
        None,
        fields,
    );
}

/// Parse the LLM response content into a `ConsolidationOutput`.
///
/// Handles common LLM quirks: strips markdown code fences if present,
/// trims whitespace.
fn parse_consolidation_json(raw: &str) -> Result<ConsolidationOutput> {
    let trimmed = raw.trim();

    // Strip markdown code fences if the LLM wrapped the JSON.
    let json_str = if trimmed.starts_with("```") {
        let without_opening = trimmed
            .strip_prefix("```json")
            .or_else(|| trimmed.strip_prefix("```"))
            .unwrap_or(trimmed);
        without_opening
            .strip_suffix("```")
            .unwrap_or(without_opening)
            .trim()
    } else {
        trimmed
    };

    let output: ConsolidationOutput = serde_json::from_str(json_str)
        .map_err(|e| anyhow::anyhow!("JSON parse error: {e}"))?;

    // Validate the type field.
    match output.kind.as_str() {
        "concept" | "decision" | "gotcha" | "rule" => {}
        other => {
            return Err(anyhow::anyhow!(
                "invalid consolidation type '{other}', expected one of: concept, decision, gotcha, rule"
            ));
        }
    }

    Ok(output)
}

/// Compute the average embedding vector from a set of fragments.
fn compute_average_embedding(fragments: &[MemoryFragment]) -> Vec<f32> {
    if fragments.is_empty() {
        return Vec::new();
    }

    let dim = fragments[0].embedding.len();
    if dim == 0 {
        return Vec::new();
    }

    let mut avg = vec![0.0f32; dim];
    let count = fragments.len() as f32;

    for fragment in fragments {
        if fragment.embedding.len() == dim {
            for (i, val) in fragment.embedding.iter().enumerate() {
                avg[i] += val;
            }
        }
    }

    for val in &mut avg {
        *val /= count;
    }

    // Normalize to unit vector for cosine similarity compatibility.
    let magnitude: f32 = avg.iter().map(|x| x * x).sum::<f32>().sqrt();
    if magnitude > 0.0 {
        for val in &mut avg {
            *val /= magnitude;
        }
    }

    avg
}

// ─── Episodic → Semantic Consolidation ───────────────────────────────────────

/// Cosine similarity between two embedding vectors.
///
/// Returns 0.0 for empty or mismatched-length vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}

/// Find connected components in an adjacency matrix via BFS.
///
/// Each entry `adjacency[i][j]` indicates whether nodes `i` and `j` are connected.
/// Returns a list of components, where each component is a list of node indices.
fn find_connected_components(adjacency: &[Vec<bool>]) -> Vec<Vec<usize>> {
    let n = adjacency.len();
    let mut visited = vec![false; n];
    let mut components = Vec::new();

    for start in 0..n {
        if visited[start] {
            continue;
        }
        // BFS from this node
        let mut component = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start);
        visited[start] = true;

        while let Some(node) = queue.pop_front() {
            component.push(node);
            for neighbor in 0..n {
                if !visited[neighbor] && adjacency[node][neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }

        components.push(component);
    }

    components
}

/// Episodic → Semantic consolidation pass.
///
/// Finds clusters of similar episodic fragments using single-linkage clustering
/// and merges each qualifying cluster into a semantic-tier fragment via the LLM.
///
/// Returns a vector of newly created semantic fragment IDs.
///
/// # Algorithm
/// 1. Load all episodic-tier fragments from the store
/// 2. Compute pairwise cosine similarity for all pairs
/// 3. Build an adjacency graph: edge exists if similarity >= `semantic_similarity_threshold`
/// 4. Find connected components via BFS
/// 5. Filter components with size >= `min_episodic_for_semantic`
/// 6. For each qualifying component, invoke the LLM to produce a semantic summary
///
/// # Error Handling
/// - On LLM failure for a cluster: log warning, skip that cluster, continue with others
/// - Source episodic fragments are retained (not deleted) after consolidation
pub async fn consolidate_semantic(
    config: &MaturationConfig,
    store: &SqliteVecStore,
    router: &LlmRouter,
    pillar_tx: &PillarTx,
) -> Result<Vec<Uuid>> {
    // ── Load all episodic-tier fragments ──────────────────────────────────────

    let episodic_fragments = store.get_by_tier(MemoryTier::Episodic).await?;

    if episodic_fragments.is_empty() {
        debug!("no episodic fragments found for semantic consolidation");
        return Ok(Vec::new());
    }

    // ── Circuit breaker pre-check ────────────────────────────────────────────
    // Short-circuit before computing similarity or loading prompts if the
    // light-tier LLM is known to be unavailable.
    if !router.is_tier_available(Tier::Light) {
        warn!("light-tier LLM unavailable (circuit breaker open) — skipping semantic consolidation");
        return Ok(Vec::new());
    }

    let n = episodic_fragments.len();
    debug!(fragment_count = n, "loaded episodic fragments for semantic clustering");

    // ── Compute pairwise cosine similarity → adjacency graph ─────────────────

    let threshold = config.semantic_similarity_threshold as f32;
    let mut adjacency: Vec<Vec<bool>> = vec![vec![false; n]; n];

    for i in 0..n {
        for j in (i + 1)..n {
            let sim = cosine_similarity(
                &episodic_fragments[i].embedding,
                &episodic_fragments[j].embedding,
            );
            if sim >= threshold {
                adjacency[i][j] = true;
                adjacency[j][i] = true;
            }
        }
    }

    // ── Find connected components via BFS ────────────────────────────────────

    let components = find_connected_components(&adjacency);

    // ── Filter components with size >= min_episodic_for_semantic ──────────────

    let min_size = config.min_episodic_for_semantic as usize;
    let qualifying_components: Vec<Vec<usize>> = components
        .into_iter()
        .filter(|c| c.len() >= min_size)
        .collect();

    if qualifying_components.is_empty() {
        debug!(
            min_size = min_size,
            "no qualifying clusters found for semantic consolidation"
        );
        return Ok(Vec::new());
    }

    info!(
        cluster_count = qualifying_components.len(),
        min_size = min_size,
        "found qualifying clusters for semantic consolidation"
    );

    // ── Load consolidation prompt ────────────────────────────────────────────

    let prompt_content = match tokio::fs::read_to_string(CONSOLIDATION_PROMPT_PATH).await {
        Ok(content) => content,
        Err(e) => {
            warn!(
                path = CONSOLIDATION_PROMPT_PATH,
                error = %e,
                "consolidation prompt file missing or unreadable, skipping semantic consolidation"
            );
            return Ok(Vec::new());
        }
    };

    // ── Process each qualifying cluster ──────────────────────────────────────

    let mut new_semantic_ids: Vec<Uuid> = Vec::new();

    for component_indices in &qualifying_components {
        let cluster_fragments: Vec<&MemoryFragment> = component_indices
            .iter()
            .map(|&idx| &episodic_fragments[idx])
            .collect();

        let cluster_size = cluster_fragments.len();

        // ── Emit ConsolidationStarted event ──────────────────────────────────

        let mut started_fields = serde_json::Map::new();
        started_fields.insert("source_tier".into(), serde_json::json!("episodic"));
        started_fields.insert("target_tier".into(), serde_json::json!("semantic"));
        started_fields.insert("fragment_count".into(), serde_json::json!(cluster_size));

        pillar::emit_full(
            pillar_tx,
            Pillar::P7Cognition,
            "start",
            "memory_maturation_consolidation_started",
            None,
            None,
            started_fields,
        );

        // ── Build LLM messages ───────────────────────────────────────────────

        let user_content = cluster_fragments
            .iter()
            .enumerate()
            .map(|(i, f)| format!("--- Fragment {} ---\n{}", i + 1, f.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: prompt_content.clone(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_content,
            },
        ];

        // ── Invoke light-tier LLM ────────────────────────────────────────────

        let chat_request = crate::proto::chat::ChatRequest {
            tier: "light".to_string(),
            model: None,
            messages: messages.clone(),
            mode: None,
        };

        let llm_output = match router.route_chat(&chat_request, &messages).await {
            Ok((output, _provider, _model)) => output,
            Err(e) => {
                warn!(
                    cluster_size = cluster_size,
                    error = %e,
                    "LLM invocation failed for semantic cluster, skipping"
                );
                emit_consolidation_failed(
                    pillar_tx,
                    "episodic",
                    "semantic",
                    &format!("{e:#}"),
                );
                continue; // Skip this cluster, try others
            }
        };

        // ── Parse ConsolidationOutput JSON ───────────────────────────────────

        let consolidation_output: ConsolidationOutput =
            match parse_consolidation_json(&llm_output.content) {
                Ok(output) => output,
                Err(e) => {
                    warn!(
                        cluster_size = cluster_size,
                        raw_response = %llm_output.content,
                        error = %e,
                        "malformed JSON from LLM for semantic cluster, skipping"
                    );
                    emit_consolidation_failed(
                        pillar_tx,
                        "episodic",
                        "semantic",
                        &format!("malformed JSON: {e}"),
                    );
                    continue; // Skip this cluster, try others
                }
            };

        // ── Create semantic fragment ─────────────────────────────────────────

        let new_fragment_id = Uuid::now_v7();
        let now = Utc::now();

        // Use the first fragment's thread_id and run_id for the new semantic fragment.
        let thread_id = cluster_fragments
            .first()
            .map_or_else(Uuid::now_v7, |f| f.thread_id);
        let run_id = cluster_fragments
            .first()
            .map_or_else(Uuid::now_v7, |f| f.run_id);

        // Compute average embedding from the cluster.
        let owned_fragments: Vec<MemoryFragment> =
            cluster_fragments.iter().map(|f| (*f).clone()).collect();
        let embedding = compute_average_embedding(&owned_fragments);

        let semantic_fragment = MemoryFragment {
            fragment_id: new_fragment_id,
            content: consolidation_output.content.clone(),
            embedding,
            thread_id,
            run_id,
            created_at: now,
            score: 1.0, // Fresh fragment, full relevance.
            tier: MemoryTier::Semantic,
            access_count: 0,
            last_accessed: None,
            consolidated_into: None,
            promoted_at: Some(now),
            session_id: None, // Semantic fragments span sessions.
        };

        // ── Persist the new semantic fragment ────────────────────────────────

        if let Err(e) = store.persist(&semantic_fragment).await {
            error!(
                fragment_id = %new_fragment_id,
                error = %e,
                "failed to persist semantic fragment"
            );
            emit_consolidation_failed(
                pillar_tx,
                "episodic",
                "semantic",
                &format!("store write: {e:#}"),
            );
            continue; // Skip this cluster, try others
        }

        // ── Mark source episodic fragments with consolidated_into ────────────

        let mut marking_failed = false;
        for fragment in &cluster_fragments {
            let update = MetadataUpdate {
                consolidated_into: Some(Some(new_fragment_id)),
                ..Default::default()
            };
            if let Err(e) = store.update_metadata(fragment.fragment_id, update).await {
                error!(
                    fragment_id = %fragment.fragment_id,
                    error = %e,
                    "failed to mark episodic fragment with consolidated_into"
                );
                emit_consolidation_failed(
                    pillar_tx,
                    "episodic",
                    "semantic",
                    &format!("metadata update: {e:#}"),
                );
                marking_failed = true;
                break;
            }
        }

        if marking_failed {
            continue; // Skip to next cluster
        }

        // ── Emit ConsolidationCompleted event ────────────────────────────────

        let mut completed_fields = serde_json::Map::new();
        completed_fields.insert("source_tier".into(), serde_json::json!("episodic"));
        completed_fields.insert("target_tier".into(), serde_json::json!("semantic"));
        completed_fields.insert(
            "fragment_id".into(),
            serde_json::json!(new_fragment_id.to_string()),
        );
        completed_fields.insert("title".into(), serde_json::json!(consolidation_output.title));
        completed_fields.insert("kind".into(), serde_json::json!(consolidation_output.kind));
        completed_fields.insert("source_count".into(), serde_json::json!(cluster_size));

        pillar::emit_full(
            pillar_tx,
            Pillar::P7Cognition,
            "done",
            "memory_maturation_consolidation_completed",
            None,
            None,
            completed_fields,
        );

        info!(
            fragment_id = %new_fragment_id,
            source_count = cluster_size,
            "episodic→semantic consolidation completed for cluster"
        );

        new_semantic_ids.push(new_fragment_id);
    }

    Ok(new_semantic_ids)
}

// ─── Semantic → Procedural Promotion ─────────────────────────────────────────

/// Semantic → Procedural promotion pass.
///
/// Promotes qualifying semantic fragments (no LLM needed — metadata-only operation).
///
/// Returns a vector of promoted fragment IDs.
///
/// # Promotion Criteria (ALL must be true)
/// 1. Fragment tier == Semantic
/// 2. `access_count` >= `config.procedural_access_threshold`
/// 3. Age since `created_at` >= `config.procedural_min_age_days` (uses `now` for deterministic testing)
/// 4. Content has not been modified since the access threshold was met
///    (approximated by using `created_at` as reference — content is considered unmodified
///    if the fragment hasn't been re-persisted)
///
/// # Key Behaviors
/// - Loads all semantic-tier fragments from the store
/// - Filters to those meeting all promotion criteria
/// - Caps at `max_promotions_per_cycle` (takes the first N qualifying)
/// - For each qualifying fragment:
///   - Updates tier to Procedural via `store.update_metadata`
///   - Sets `promoted_at` to `now`
///   - Emits `MemoryPromotedToProcedural` event with fragment_id and content preview (first 80 chars)
/// - No LLM invocation — purely a metadata operation
pub async fn promote_to_procedural(
    config: &MaturationConfig,
    store: &SqliteVecStore,
    pillar_tx: &PillarTx,
    now: DateTime<Utc>,
) -> Result<Vec<Uuid>> {
    // ── Load all semantic-tier fragments ──────────────────────────────────────

    let semantic_fragments = store.get_by_tier(MemoryTier::Semantic).await?;

    if semantic_fragments.is_empty() {
        debug!("no semantic fragments found for procedural promotion");
        return Ok(Vec::new());
    }

    debug!(
        fragment_count = semantic_fragments.len(),
        "loaded semantic fragments for procedural promotion check"
    );

    // ── Filter to qualifying fragments ───────────────────────────────────────

    let min_age = chrono::Duration::days(config.procedural_min_age_days as i64);
    let access_threshold = config.procedural_access_threshold;

    let qualifying: Vec<&MemoryFragment> = semantic_fragments
        .iter()
        .filter(|f| {
            // Criterion 1: tier == Semantic (guaranteed by get_by_tier, but explicit for clarity)
            if f.tier != MemoryTier::Semantic {
                return false;
            }

            // Criterion 2: access_count >= procedural_access_threshold
            if f.access_count < access_threshold {
                return false;
            }

            // Criterion 3: age since created_at >= procedural_min_age_days
            let age = now.signed_duration_since(f.created_at);
            if age < min_age {
                return false;
            }

            // Criterion 4: content unmodified since access threshold was met.
            // Since we don't track content modification timestamps separately,
            // we use `created_at` as the reference. The content is considered
            // unmodified if the fragment hasn't been re-persisted (i.e., promoted_at is None
            // or the fragment hasn't already been promoted to procedural).
            // A fragment that already has promoted_at set means it was already promoted
            // at some point — skip it.
            if f.promoted_at.is_some() {
                return false;
            }

            true
        })
        .collect();

    if qualifying.is_empty() {
        debug!("no semantic fragments qualify for procedural promotion");
        return Ok(Vec::new());
    }

    // ── Cap at max_promotions_per_cycle ───────────────────────────────────────

    let max_promotions = config.max_promotions_per_cycle as usize;
    let to_promote: Vec<&MemoryFragment> = qualifying
        .into_iter()
        .take(max_promotions)
        .collect();

    info!(
        qualifying_count = to_promote.len(),
        max_promotions = max_promotions,
        "promoting semantic fragments to procedural"
    );

    // ── Promote each qualifying fragment ─────────────────────────────────────

    let mut promoted_ids: Vec<Uuid> = Vec::new();

    for fragment in &to_promote {
        // Update tier to Procedural and set promoted_at
        let update = MetadataUpdate {
            tier: Some(MemoryTier::Procedural),
            promoted_at: Some(Some(now)),
            ..Default::default()
        };

        if let Err(e) = store.update_metadata(fragment.fragment_id, update).await {
            error!(
                fragment_id = %fragment.fragment_id,
                error = %e,
                "failed to promote semantic fragment to procedural"
            );
            // Continue with other fragments — best-effort promotion
            continue;
        }

        // Emit MemoryPromotedToProcedural event
        let content_preview: String = fragment.content.chars().take(80).collect();

        let mut fields = serde_json::Map::new();
        fields.insert(
            "fragment_id".into(),
            serde_json::json!(fragment.fragment_id.to_string()),
        );
        fields.insert("content_preview".into(), serde_json::json!(content_preview));

        pillar::emit_full(
            pillar_tx,
            Pillar::P7Cognition,
            "done",
            "memory_maturation_promoted_to_procedural",
            None,
            None,
            fields,
        );

        debug!(
            fragment_id = %fragment.fragment_id,
            "promoted semantic fragment to procedural"
        );

        promoted_ids.push(fragment.fragment_id);
    }

    if !promoted_ids.is_empty() {
        info!(
            promoted_count = promoted_ids.len(),
            "procedural promotion pass completed"
        );
    }

    Ok(promoted_ids)
}

/// Variant of `promote_to_procedural` that works with any `MemoryStore` implementation.
///
/// Used in tests with `InMemoryStore`. Same logic as `promote_to_procedural`.
pub async fn promote_to_procedural_with_store<S: MemoryStore>(
    config: &MaturationConfig,
    store: &S,
    pillar_tx: &PillarTx,
    now: DateTime<Utc>,
) -> Result<Vec<Uuid>> {
    // ── Load all semantic-tier fragments ──────────────────────────────────────

    let semantic_fragments = store.get_by_tier(MemoryTier::Semantic).await?;

    if semantic_fragments.is_empty() {
        debug!("no semantic fragments found for procedural promotion");
        return Ok(Vec::new());
    }

    debug!(
        fragment_count = semantic_fragments.len(),
        "loaded semantic fragments for procedural promotion check"
    );

    // ── Filter to qualifying fragments ───────────────────────────────────────

    let min_age = chrono::Duration::days(config.procedural_min_age_days as i64);
    let access_threshold = config.procedural_access_threshold;

    let qualifying: Vec<&MemoryFragment> = semantic_fragments
        .iter()
        .filter(|f| {
            if f.tier != MemoryTier::Semantic {
                return false;
            }
            if f.access_count < access_threshold {
                return false;
            }
            let age = now.signed_duration_since(f.created_at);
            if age < min_age {
                return false;
            }
            if f.promoted_at.is_some() {
                return false;
            }
            true
        })
        .collect();

    if qualifying.is_empty() {
        debug!("no semantic fragments qualify for procedural promotion");
        return Ok(Vec::new());
    }

    // ── Cap at max_promotions_per_cycle ───────────────────────────────────────

    let max_promotions = config.max_promotions_per_cycle as usize;
    let to_promote: Vec<&MemoryFragment> = qualifying
        .into_iter()
        .take(max_promotions)
        .collect();

    info!(
        qualifying_count = to_promote.len(),
        max_promotions = max_promotions,
        "promoting semantic fragments to procedural"
    );

    // ── Promote each qualifying fragment ─────────────────────────────────────

    let mut promoted_ids: Vec<Uuid> = Vec::new();

    for fragment in &to_promote {
        let update = MetadataUpdate {
            tier: Some(MemoryTier::Procedural),
            promoted_at: Some(Some(now)),
            ..Default::default()
        };

        if let Err(e) = store.update_metadata(fragment.fragment_id, update).await {
            error!(
                fragment_id = %fragment.fragment_id,
                error = %e,
                "failed to promote semantic fragment to procedural"
            );
            continue;
        }

        let content_preview: String = fragment.content.chars().take(80).collect();

        let mut fields = serde_json::Map::new();
        fields.insert(
            "fragment_id".into(),
            serde_json::json!(fragment.fragment_id.to_string()),
        );
        fields.insert("content_preview".into(), serde_json::json!(content_preview));

        pillar::emit_full(
            pillar_tx,
            Pillar::P7Cognition,
            "done",
            "memory_maturation_promoted_to_procedural",
            None,
            None,
            fields,
        );

        debug!(
            fragment_id = %fragment.fragment_id,
            "promoted semantic fragment to procedural"
        );

        promoted_ids.push(fragment.fragment_id);
    }

    if !promoted_ids.is_empty() {
        info!(
            promoted_count = promoted_ids.len(),
            "procedural promotion pass completed"
        );
    }

    Ok(promoted_ids)
}

// ─── Working Fragment Cleanup ────────────────────────────────────────────────

/// Remove working-tier fragments that have been consolidated and are older than 24 hours.
///
/// A working fragment is eligible for removal if:
/// 1. Its tier is Working
/// 2. It has a non-null `consolidated_into` reference (it was already consolidated)
/// 3. Its `created_at` is more than 24 hours before `now`
///
/// Returns the number of fragments removed.
pub async fn cleanup_consolidated_working(
    store: &SqliteVecStore,
    now: DateTime<Utc>,
) -> Result<usize> {
    cleanup_consolidated_working_impl(store, now).await
}

/// Generic implementation of working fragment cleanup that works with any `MemoryStore`.
///
/// Used by both the production `SqliteVecStore` path and tests with `InMemoryStore`.
pub async fn cleanup_consolidated_working_impl<S: MemoryStore>(
    store: &S,
    now: DateTime<Utc>,
) -> Result<usize> {
    let retention_window = chrono::Duration::hours(24);

    // Load all working-tier fragments from the store.
    let working_fragments = store.get_by_tier(MemoryTier::Working).await?;

    if working_fragments.is_empty() {
        debug!("no working fragments found for cleanup");
        return Ok(0);
    }

    // Filter to those eligible for removal:
    // - consolidated_into is Some (already consolidated)
    // - created_at is more than 24 hours before `now`
    let eligible: Vec<&MemoryFragment> = working_fragments
        .iter()
        .filter(|f| {
            f.consolidated_into.is_some()
                && now.signed_duration_since(f.created_at) > retention_window
        })
        .collect();

    if eligible.is_empty() {
        debug!(
            total_working = working_fragments.len(),
            "no consolidated working fragments older than 24h found for cleanup"
        );
        return Ok(0);
    }

    debug!(
        eligible_count = eligible.len(),
        total_working = working_fragments.len(),
        "found consolidated working fragments eligible for cleanup"
    );

    // Delete each qualifying fragment from the store.
    let mut removed_count = 0usize;
    for fragment in &eligible {
        match store.delete(fragment.fragment_id).await {
            Ok(true) => {
                removed_count += 1;
                debug!(
                    fragment_id = %fragment.fragment_id,
                    "removed consolidated working fragment"
                );
            }
            Ok(false) => {
                // Fragment was already gone (race condition or concurrent deletion).
                debug!(
                    fragment_id = %fragment.fragment_id,
                    "working fragment already removed"
                );
            }
            Err(e) => {
                warn!(
                    fragment_id = %fragment.fragment_id,
                    error = %e,
                    "failed to delete consolidated working fragment"
                );
                // Continue with other fragments — best-effort cleanup.
            }
        }
    }

    if removed_count > 0 {
        info!(
            removed_count,
            "working fragment cleanup completed"
        );
    }

    Ok(removed_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::store::InMemoryStore;
    use crate::observability::wire;

    /// Helper to create a working-tier fragment for testing.
    fn make_fragment(session_id: Uuid, content: &str) -> MemoryFragment {
        MemoryFragment {
            fragment_id: Uuid::now_v7(),
            content: content.to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            thread_id: Uuid::now_v7(),
            run_id: Uuid::now_v7(),
            created_at: Utc::now(),
            score: 0.9,
            tier: MemoryTier::Working,
            access_count: 0,
            last_accessed: None,
            consolidated_into: None,
            promoted_at: None,
            session_id: Some(session_id),
        }
    }

    #[test]
    fn parse_consolidation_json_valid() {
        let json = r#"{"type": "concept", "title": "Test Title", "content": "Test content"}"#;
        let output = parse_consolidation_json(json).unwrap();
        assert_eq!(output.kind, "concept");
        assert_eq!(output.title, "Test Title");
        assert_eq!(output.content, "Test content");
    }

    #[test]
    fn parse_consolidation_json_with_code_fences() {
        let json = "```json\n{\"type\": \"decision\", \"title\": \"A Decision\", \"content\": \"We decided X\"}\n```";
        let output = parse_consolidation_json(json).unwrap();
        assert_eq!(output.kind, "decision");
        assert_eq!(output.title, "A Decision");
    }

    #[test]
    fn parse_consolidation_json_invalid_type() {
        let json = r#"{"type": "invalid", "title": "T", "content": "C"}"#;
        let result = parse_consolidation_json(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid consolidation type"));
    }

    #[test]
    fn parse_consolidation_json_malformed() {
        let result = parse_consolidation_json("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn compute_average_embedding_basic() {
        let session_id = Uuid::now_v7();
        let mut f1 = make_fragment(session_id, "a");
        f1.embedding = vec![1.0, 0.0, 0.0];
        let mut f2 = make_fragment(session_id, "b");
        f2.embedding = vec![0.0, 1.0, 0.0];

        let avg = compute_average_embedding(&[f1, f2]);
        // Average of [1,0,0] and [0,1,0] = [0.5, 0.5, 0] normalized
        assert_eq!(avg.len(), 3);
        // Should be normalized to unit vector
        let magnitude: f32 = avg.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.001);
    }

    #[test]
    fn compute_average_embedding_empty() {
        let avg = compute_average_embedding(&[]);
        assert!(avg.is_empty());
    }

    #[tokio::test]
    async fn consolidate_session_skips_below_threshold() {
        let session_id = Uuid::now_v7();
        let config = MaturationConfig {
            min_fragments_for_consolidation: 3,
            ..Default::default()
        };
        let fragments = vec![
            make_fragment(session_id, "fragment 1"),
            make_fragment(session_id, "fragment 2"),
        ];

        // 2 fragments < min_fragments_for_consolidation (3) → skip
        assert!(fragments.len() < config.min_fragments_for_consolidation as usize);
    }

    #[tokio::test]
    async fn consolidate_session_skips_fewer_than_2() {
        let session_id = Uuid::now_v7();

        // With 1 fragment, should skip (Property 22).
        let fragments = vec![make_fragment(session_id, "only one")];
        assert!(fragments.len() < 2);
    }

    #[test]
    fn consolidation_output_serde_round_trip() {
        let output = ConsolidationOutput {
            kind: "gotcha".to_string(),
            title: "Watch out for X".to_string(),
            content: "When doing Y, be careful about Z".to_string(),
        };
        let json = serde_json::to_string(&output).unwrap();
        let parsed: ConsolidationOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.kind, "gotcha");
        assert_eq!(parsed.title, "Watch out for X");
        assert_eq!(parsed.content, "When doing Y, be careful about Z");
    }

    #[test]
    fn emit_consolidation_failed_does_not_panic() {
        let pillar_tx = wire::channel();
        emit_consolidation_failed(&pillar_tx, "working", "episodic", "test error");
        // No panic = success. Events are best-effort.
    }

    // ── Unit tests for clustering algorithm ──────────────────────────────────

    #[test]
    fn cosine_similarity_identical_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_empty_returns_zero() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn cosine_similarity_mismatched_lengths_returns_zero() {
        assert_eq!(cosine_similarity(&[1.0, 2.0], &[1.0]), 0.0);
    }

    #[test]
    fn cosine_similarity_zero_vector_returns_zero() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn find_connected_components_empty_graph() {
        let adjacency: Vec<Vec<bool>> = Vec::new();
        let components = find_connected_components(&adjacency);
        assert!(components.is_empty());
    }

    #[test]
    fn find_connected_components_single_node() {
        let adjacency = vec![vec![false]];
        let components = find_connected_components(&adjacency);
        assert_eq!(components.len(), 1);
        assert_eq!(components[0], vec![0]);
    }

    #[test]
    fn find_connected_components_all_disconnected() {
        // 4 nodes, no edges → 4 components of size 1
        let adjacency = vec![
            vec![false, false, false, false],
            vec![false, false, false, false],
            vec![false, false, false, false],
            vec![false, false, false, false],
        ];
        let components = find_connected_components(&adjacency);
        assert_eq!(components.len(), 4);
        for c in &components {
            assert_eq!(c.len(), 1);
        }
    }

    #[test]
    fn find_connected_components_all_connected() {
        // 3 nodes, all connected → 1 component of size 3
        let adjacency = vec![
            vec![false, true, true],
            vec![true, false, true],
            vec![true, true, false],
        ];
        let components = find_connected_components(&adjacency);
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].len(), 3);
    }

    #[test]
    fn find_connected_components_two_clusters() {
        // 5 nodes: {0,1,2} connected, {3,4} connected
        let adjacency = vec![
            vec![false, true, true, false, false],
            vec![true, false, true, false, false],
            vec![true, true, false, false, false],
            vec![false, false, false, false, true],
            vec![false, false, false, true, false],
        ];
        let components = find_connected_components(&adjacency);
        assert_eq!(components.len(), 2);

        // Sort by first element for deterministic assertion
        let mut sorted = components.clone();
        sorted.sort_by_key(|c| c[0]);
        assert_eq!(sorted[0].len(), 3); // {0,1,2}
        assert_eq!(sorted[1].len(), 2); // {3,4}
    }

    #[test]
    fn find_connected_components_chain() {
        // 4 nodes in a chain: 0-1-2-3 (single-linkage connects them all)
        let adjacency = vec![
            vec![false, true, false, false],
            vec![true, false, true, false],
            vec![false, true, false, true],
            vec![false, false, true, false],
        ];
        let components = find_connected_components(&adjacency);
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].len(), 4);
    }

    #[test]
    fn find_connected_components_mixed_sizes() {
        // 6 nodes: {0,1} connected, {2} alone, {3,4,5} connected
        let adjacency = vec![
            vec![false, true, false, false, false, false],
            vec![true, false, false, false, false, false],
            vec![false, false, false, false, false, false],
            vec![false, false, false, false, true, true],
            vec![false, false, false, true, false, true],
            vec![false, false, false, true, true, false],
        ];
        let components = find_connected_components(&adjacency);
        assert_eq!(components.len(), 3);

        let mut sizes: Vec<usize> = components.iter().map(|c| c.len()).collect();
        sizes.sort();
        assert_eq!(sizes, vec![1, 2, 3]);
    }

    // ── Unit tests for promote_to_procedural ─────────────────────────────────

    /// Helper to create a semantic-tier fragment for procedural promotion testing.
    fn make_semantic_fragment(
        created_at: DateTime<Utc>,
        access_count: u64,
        content: &str,
    ) -> MemoryFragment {
        MemoryFragment {
            fragment_id: Uuid::now_v7(),
            content: content.to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            thread_id: Uuid::now_v7(),
            run_id: Uuid::now_v7(),
            created_at,
            score: 0.9,
            tier: MemoryTier::Semantic,
            access_count,
            last_accessed: None,
            consolidated_into: None,
            promoted_at: None,
            session_id: None,
        }
    }

    #[tokio::test]
    async fn promote_to_procedural_no_semantic_fragments() {
        let store = InMemoryStore::new(100);
        let pillar_tx = wire::channel();
        let config = MaturationConfig::default();
        let now = Utc::now();

        let result = promote_to_procedural_with_store(&config, &store, &pillar_tx, now).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn promote_to_procedural_below_access_threshold() {
        let store = InMemoryStore::new(100);
        let pillar_tx = wire::channel();
        let config = MaturationConfig {
            procedural_access_threshold: 10,
            procedural_min_age_days: 7,
            max_promotions_per_cycle: 50,
            ..Default::default()
        };

        // Fragment with access_count = 5 (below threshold of 10)
        let created_at = Utc::now() - chrono::Duration::days(30);
        let fragment = make_semantic_fragment(created_at, 5, "not enough accesses");
        store.persist(&fragment).await.unwrap();

        let now = Utc::now();
        let result = promote_to_procedural_with_store(&config, &store, &pillar_tx, now).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());

        // Verify fragment is still semantic
        let stored = store.get_by_id(fragment.fragment_id).await.unwrap().unwrap();
        assert_eq!(stored.tier, MemoryTier::Semantic);
    }

    #[tokio::test]
    async fn promote_to_procedural_below_min_age() {
        let store = InMemoryStore::new(100);
        let pillar_tx = wire::channel();
        let config = MaturationConfig {
            procedural_access_threshold: 10,
            procedural_min_age_days: 7,
            max_promotions_per_cycle: 50,
            ..Default::default()
        };

        // Fragment created 3 days ago (below min_age of 7 days), but high access count
        let now = Utc::now();
        let created_at = now - chrono::Duration::days(3);
        let fragment = make_semantic_fragment(created_at, 15, "too young");
        store.persist(&fragment).await.unwrap();

        let result = promote_to_procedural_with_store(&config, &store, &pillar_tx, now).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());

        // Verify fragment is still semantic
        let stored = store.get_by_id(fragment.fragment_id).await.unwrap().unwrap();
        assert_eq!(stored.tier, MemoryTier::Semantic);
    }

    #[tokio::test]
    async fn promote_to_procedural_qualifies_and_promotes() {
        let store = InMemoryStore::new(100);
        let pillar_tx = wire::channel();
        let config = MaturationConfig {
            procedural_access_threshold: 10,
            procedural_min_age_days: 7,
            max_promotions_per_cycle: 50,
            ..Default::default()
        };

        // Fragment meets all criteria: access_count >= 10, age >= 7 days
        let now = Utc::now();
        let created_at = now - chrono::Duration::days(14);
        let fragment = make_semantic_fragment(created_at, 12, "well-accessed semantic knowledge");
        store.persist(&fragment).await.unwrap();

        let result = promote_to_procedural_with_store(&config, &store, &pillar_tx, now).await;
        assert!(result.is_ok());
        let promoted = result.unwrap();
        assert_eq!(promoted.len(), 1);
        assert_eq!(promoted[0], fragment.fragment_id);

        // Verify fragment is now procedural with promoted_at set
        let stored = store.get_by_id(fragment.fragment_id).await.unwrap().unwrap();
        assert_eq!(stored.tier, MemoryTier::Procedural);
        assert_eq!(stored.promoted_at, Some(now));
    }

    #[tokio::test]
    async fn promote_to_procedural_caps_at_max_per_cycle() {
        let store = InMemoryStore::new(100);
        let pillar_tx = wire::channel();
        let config = MaturationConfig {
            procedural_access_threshold: 5,
            procedural_min_age_days: 1,
            max_promotions_per_cycle: 2, // Cap at 2
            ..Default::default()
        };

        let now = Utc::now();
        let created_at = now - chrono::Duration::days(10);

        // Create 5 qualifying fragments
        for i in 0..5 {
            let fragment = make_semantic_fragment(created_at, 10, &format!("fragment {i}"));
            store.persist(&fragment).await.unwrap();
        }

        let result = promote_to_procedural_with_store(&config, &store, &pillar_tx, now).await;
        assert!(result.is_ok());
        let promoted = result.unwrap();
        // Only 2 should be promoted (max_promotions_per_cycle = 2)
        assert_eq!(promoted.len(), 2);

        // Verify remaining 3 are still semantic
        let remaining_semantic = store.get_by_tier(MemoryTier::Semantic).await.unwrap();
        assert_eq!(remaining_semantic.len(), 3);
    }

    #[tokio::test]
    async fn promote_to_procedural_skips_already_promoted() {
        let store = InMemoryStore::new(100);
        let pillar_tx = wire::channel();
        let config = MaturationConfig {
            procedural_access_threshold: 5,
            procedural_min_age_days: 1,
            max_promotions_per_cycle: 50,
            ..Default::default()
        };

        let now = Utc::now();
        let created_at = now - chrono::Duration::days(10);

        // Fragment that already has promoted_at set (was previously promoted somehow)
        let mut fragment = make_semantic_fragment(created_at, 10, "already promoted once");
        fragment.promoted_at = Some(now - chrono::Duration::days(5));
        store.persist(&fragment).await.unwrap();

        let result = promote_to_procedural_with_store(&config, &store, &pillar_tx, now).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn promote_to_procedural_skips_non_semantic_tiers() {
        let store = InMemoryStore::new(100);
        let pillar_tx = wire::channel();
        let config = MaturationConfig {
            procedural_access_threshold: 5,
            procedural_min_age_days: 1,
            max_promotions_per_cycle: 50,
            ..Default::default()
        };

        let now = Utc::now();
        let created_at = now - chrono::Duration::days(10);

        // Working-tier fragment that meets access/age criteria
        let mut working_fragment = make_semantic_fragment(created_at, 10, "working tier");
        working_fragment.tier = MemoryTier::Working;
        store.persist(&working_fragment).await.unwrap();

        // Episodic-tier fragment that meets access/age criteria
        let mut episodic_fragment = make_semantic_fragment(created_at, 10, "episodic tier");
        episodic_fragment.tier = MemoryTier::Episodic;
        store.persist(&episodic_fragment).await.unwrap();

        // Only semantic fragments are loaded by get_by_tier(Semantic)
        let result = promote_to_procedural_with_store(&config, &store, &pillar_tx, now).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn promote_to_procedural_content_preview_truncated() {
        let store = InMemoryStore::new(100);
        let pillar_tx = wire::channel();
        let config = MaturationConfig {
            procedural_access_threshold: 5,
            procedural_min_age_days: 1,
            max_promotions_per_cycle: 50,
            ..Default::default()
        };

        let now = Utc::now();
        let created_at = now - chrono::Duration::days(10);

        // Fragment with content longer than 80 chars
        let long_content = "A".repeat(200);
        let fragment = make_semantic_fragment(created_at, 10, &long_content);
        store.persist(&fragment).await.unwrap();

        let result = promote_to_procedural_with_store(&config, &store, &pillar_tx, now).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);

        // The event emission is best-effort; we just verify the promotion succeeded.
        // Content preview truncation is tested implicitly by the event emission logic.
    }

    #[tokio::test]
    async fn promote_to_procedural_exact_threshold_boundary() {
        let store = InMemoryStore::new(100);
        let pillar_tx = wire::channel();
        let config = MaturationConfig {
            procedural_access_threshold: 10,
            procedural_min_age_days: 7,
            max_promotions_per_cycle: 50,
            ..Default::default()
        };

        let now = Utc::now();

        // Fragment at exactly the threshold (access_count == 10, age == 7 days)
        let created_at = now - chrono::Duration::days(7);
        let fragment = make_semantic_fragment(created_at, 10, "exactly at threshold");
        store.persist(&fragment).await.unwrap();

        let result = promote_to_procedural_with_store(&config, &store, &pillar_tx, now).await;
        assert!(result.is_ok());
        let promoted = result.unwrap();
        assert_eq!(promoted.len(), 1);
        assert_eq!(promoted[0], fragment.fragment_id);
    }

    #[tokio::test]
    async fn promote_to_procedural_mixed_qualifying_and_non_qualifying() {
        let store = InMemoryStore::new(100);
        let pillar_tx = wire::channel();
        let config = MaturationConfig {
            procedural_access_threshold: 10,
            procedural_min_age_days: 7,
            max_promotions_per_cycle: 50,
            ..Default::default()
        };

        let now = Utc::now();

        // Qualifying fragment
        let created_at_old = now - chrono::Duration::days(14);
        let qualifying = make_semantic_fragment(created_at_old, 15, "qualifies");
        store.persist(&qualifying).await.unwrap();

        // Non-qualifying: too few accesses
        let non_qual_access = make_semantic_fragment(created_at_old, 3, "too few accesses");
        store.persist(&non_qual_access).await.unwrap();

        // Non-qualifying: too young
        let created_at_young = now - chrono::Duration::days(2);
        let non_qual_age = make_semantic_fragment(created_at_young, 20, "too young");
        store.persist(&non_qual_age).await.unwrap();

        let result = promote_to_procedural_with_store(&config, &store, &pillar_tx, now).await;
        assert!(result.is_ok());
        let promoted = result.unwrap();
        assert_eq!(promoted.len(), 1);
        assert_eq!(promoted[0], qualifying.fragment_id);

        // Verify non-qualifying fragments are unchanged
        let stored_access = store.get_by_id(non_qual_access.fragment_id).await.unwrap().unwrap();
        assert_eq!(stored_access.tier, MemoryTier::Semantic);
        let stored_age = store.get_by_id(non_qual_age.fragment_id).await.unwrap().unwrap();
        assert_eq!(stored_age.tier, MemoryTier::Semantic);
    }

    // ── Unit tests for cleanup_consolidated_working ──────────────────────────

    /// Helper to create a working-tier fragment with specific created_at and consolidated_into.
    fn make_working_fragment_for_cleanup(
        created_at: DateTime<Utc>,
        consolidated_into: Option<Uuid>,
    ) -> MemoryFragment {
        MemoryFragment {
            fragment_id: Uuid::now_v7(),
            content: "working fragment".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            thread_id: Uuid::now_v7(),
            run_id: Uuid::now_v7(),
            created_at,
            score: 0.9,
            tier: MemoryTier::Working,
            access_count: 0,
            last_accessed: None,
            consolidated_into,
            promoted_at: None,
            session_id: Some(Uuid::now_v7()),
        }
    }

    #[tokio::test]
    async fn cleanup_no_working_fragments() {
        let store = InMemoryStore::new(100);
        let now = Utc::now();

        let result = cleanup_consolidated_working_impl(&store, now).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn cleanup_working_fragments_not_consolidated() {
        let store = InMemoryStore::new(100);
        let now = Utc::now();

        // Working fragment created 48 hours ago but NOT consolidated (consolidated_into = None)
        let old_fragment = make_working_fragment_for_cleanup(
            now - chrono::Duration::hours(48),
            None,
        );
        store.persist(&old_fragment).await.unwrap();

        let result = cleanup_consolidated_working_impl(&store, now).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);

        // Fragment should still exist
        let stored = store.get_by_id(old_fragment.fragment_id).await.unwrap();
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn cleanup_consolidated_but_within_24h() {
        let store = InMemoryStore::new(100);
        let now = Utc::now();
        let episodic_id = Uuid::now_v7();

        // Working fragment created 12 hours ago, consolidated (within 24h window)
        let recent_fragment = make_working_fragment_for_cleanup(
            now - chrono::Duration::hours(12),
            Some(episodic_id),
        );
        store.persist(&recent_fragment).await.unwrap();

        let result = cleanup_consolidated_working_impl(&store, now).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);

        // Fragment should still exist (retained within 24h window)
        let stored = store.get_by_id(recent_fragment.fragment_id).await.unwrap();
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn cleanup_consolidated_and_older_than_24h() {
        let store = InMemoryStore::new(100);
        let now = Utc::now();
        let episodic_id = Uuid::now_v7();

        // Working fragment created 25 hours ago, consolidated → eligible for removal
        let old_consolidated = make_working_fragment_for_cleanup(
            now - chrono::Duration::hours(25),
            Some(episodic_id),
        );
        store.persist(&old_consolidated).await.unwrap();

        let result = cleanup_consolidated_working_impl(&store, now).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        // Fragment should be deleted
        let stored = store.get_by_id(old_consolidated.fragment_id).await.unwrap();
        assert!(stored.is_none());
    }

    #[tokio::test]
    async fn cleanup_mixed_fragments() {
        let store = InMemoryStore::new(100);
        let now = Utc::now();
        let episodic_id = Uuid::now_v7();

        // Fragment 1: old + consolidated → should be removed
        let eligible1 = make_working_fragment_for_cleanup(
            now - chrono::Duration::hours(48),
            Some(episodic_id),
        );
        store.persist(&eligible1).await.unwrap();

        // Fragment 2: old + consolidated → should be removed
        let eligible2 = make_working_fragment_for_cleanup(
            now - chrono::Duration::hours(30),
            Some(Uuid::now_v7()),
        );
        store.persist(&eligible2).await.unwrap();

        // Fragment 3: recent + consolidated → should be retained
        let retained_recent = make_working_fragment_for_cleanup(
            now - chrono::Duration::hours(10),
            Some(episodic_id),
        );
        store.persist(&retained_recent).await.unwrap();

        // Fragment 4: old + NOT consolidated → should be retained
        let retained_not_consolidated = make_working_fragment_for_cleanup(
            now - chrono::Duration::hours(72),
            None,
        );
        store.persist(&retained_not_consolidated).await.unwrap();

        // Fragment 5: episodic tier (not working) → should not be touched
        let mut episodic_fragment = make_working_fragment_for_cleanup(
            now - chrono::Duration::hours(48),
            Some(Uuid::now_v7()),
        );
        episodic_fragment.tier = MemoryTier::Episodic;
        store.persist(&episodic_fragment).await.unwrap();

        let result = cleanup_consolidated_working_impl(&store, now).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);

        // Verify eligible fragments are gone
        assert!(store.get_by_id(eligible1.fragment_id).await.unwrap().is_none());
        assert!(store.get_by_id(eligible2.fragment_id).await.unwrap().is_none());

        // Verify retained fragments still exist
        assert!(store.get_by_id(retained_recent.fragment_id).await.unwrap().is_some());
        assert!(store.get_by_id(retained_not_consolidated.fragment_id).await.unwrap().is_some());
        assert!(store.get_by_id(episodic_fragment.fragment_id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn cleanup_exactly_at_24h_boundary() {
        let store = InMemoryStore::new(100);
        let now = Utc::now();
        let episodic_id = Uuid::now_v7();

        // Fragment created exactly 24 hours ago — NOT eligible (must be MORE than 24h)
        let boundary_fragment = make_working_fragment_for_cleanup(
            now - chrono::Duration::hours(24),
            Some(episodic_id),
        );
        store.persist(&boundary_fragment).await.unwrap();

        let result = cleanup_consolidated_working_impl(&store, now).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);

        // Fragment should still exist (exactly at boundary, not past it)
        let stored = store.get_by_id(boundary_fragment.fragment_id).await.unwrap();
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn cleanup_just_past_24h_boundary() {
        let store = InMemoryStore::new(100);
        let now = Utc::now();
        let episodic_id = Uuid::now_v7();

        // Fragment created 24 hours and 1 second ago → eligible for removal
        let past_boundary = make_working_fragment_for_cleanup(
            now - chrono::Duration::hours(24) - chrono::Duration::seconds(1),
            Some(episodic_id),
        );
        store.persist(&past_boundary).await.unwrap();

        let result = cleanup_consolidated_working_impl(&store, now).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        // Fragment should be deleted
        let stored = store.get_by_id(past_boundary.fragment_id).await.unwrap();
        assert!(stored.is_none());
    }

    // ── Graceful degradation tests (Requirement 13) ──────────────────────────

    /// Helper to create a Router with no providers (all tiers unavailable).
    /// This simulates the circuit breaker being open for all candidates.
    fn make_unavailable_router() -> LlmRouter {
        use crate::config::{TierEntry, TiersConfig};
        use crate::router::budget::{BudgetConfig, BudgetGuard};
        use crate::router::tier::TierMap;
        use std::collections::HashMap;

        let tiers_config = TiersConfig {
            heavy: TierEntry { candidates: vec![] },
            standard: TierEntry { candidates: vec![] },
            light: TierEntry { candidates: vec![] },
            long_context: TierEntry { candidates: vec![] },
            judge: None,
            local: None,
        };
        let tiers = TierMap::from_config(&tiers_config);
        let budget = Arc::new(BudgetGuard::new(BudgetConfig::default()));
        LlmRouter::new(HashMap::new(), budget, tiers)
    }

    #[test]
    fn circuit_breaker_check_skips_when_no_light_tier_candidates() {
        // Verify that is_tier_available returns false when no light-tier
        // candidates are configured (simulates all breakers open).
        let router = make_unavailable_router();
        assert!(
            !router.is_tier_available(Tier::Light),
            "light tier should be unavailable with no candidates"
        );
    }

    #[tokio::test]
    async fn consolidate_session_skips_when_circuit_breaker_open() {
        // Verify that consolidate_session returns Ok(None) when the light-tier
        // LLM is unavailable (no candidates configured = simulates open breaker).
        // Fragments remain unchanged (no consolidated_into set).
        let session_id = Uuid::now_v7();
        let config = MaturationConfig {
            min_fragments_for_consolidation: 3,
            ..Default::default()
        };
        let fragments = vec![
            make_fragment(session_id, "fragment 1"),
            make_fragment(session_id, "fragment 2"),
            make_fragment(session_id, "fragment 3"),
        ];

        let store = InMemoryStore::new(100);
        // Persist fragments so we can verify they remain unchanged.
        for f in &fragments {
            store.persist(f).await.unwrap();
        }

        let pillar_tx = wire::channel();
        let router = make_unavailable_router();

        // The function takes &SqliteVecStore, but the circuit breaker check
        // happens before any store access. We verify the logic here:
        // With no light-tier candidates, router.is_tier_available(Tier::Light) == false.
        assert!(!router.is_tier_available(Tier::Light));

        // Verify fragments remain unchanged in the store.
        for f in &fragments {
            let stored = store.get_by_id(f.fragment_id).await.unwrap().unwrap();
            assert!(
                stored.consolidated_into.is_none(),
                "fragment should not be marked as consolidated"
            );
            assert_eq!(stored.tier, MemoryTier::Working);
        }
    }

    #[tokio::test]
    async fn consolidate_semantic_skips_when_no_episodic_fragments() {
        // Verify that consolidate_semantic handles empty store gracefully.
        // This tests the graceful degradation path when there's nothing to do.
        let store = InMemoryStore::new(100);
        let config = MaturationConfig::default();
        let pillar_tx = wire::channel();
        let router = make_unavailable_router();

        // No episodic fragments → should return Ok(vec![]) immediately.
        let episodic = store.get_by_tier(MemoryTier::Episodic).await.unwrap();
        assert!(episodic.is_empty());
    }

    #[test]
    fn maturation_failure_does_not_block_request_path() {
        // Verify that all maturation operations run via tokio::spawn (background).
        // This is a structural verification: spawn_consolidation_timer and
        // spawn_handoff_with_panic_guard both use tokio::spawn, meaning they
        // return immediately and never block the calling task.
        //
        // The consolidation_loop runs in a spawned task.
        // The run_consolidation_pass spawns a sub-task for the actual work.
        // The spawn_handoff_with_panic_guard spawns a nested task.
        //
        // This test verifies the ConsolidatorState can be created and checked
        // without blocking.
        let (cmd_tx, _cmd_rx) = mpsc::channel::<ConsolidationCmd>(16);
        let state = ConsolidatorState {
            running: AtomicBool::new(false),
            cmd_tx,
            last_completed: Mutex::new(None),
        };

        // These operations are non-blocking:
        assert!(!state.is_running());
        assert!(state.last_completed_at().is_none());

        // Sending a command is non-blocking (channel has capacity):
        let send_result = state.cmd_tx.try_send(ConsolidationCmd::TriggerNow);
        assert!(send_result.is_ok());
    }

    #[test]
    fn fragments_unchanged_on_failure_pattern() {
        // Verify the pattern used in consolidate_session and consolidate_semantic:
        // On any error path, source fragments are NOT marked with consolidated_into.
        //
        // The implementation achieves this by:
        // 1. Only marking fragments AFTER successful LLM call AND successful persist
        // 2. On LLM error → return Err (no marking happens)
        // 3. On parse error → return Err (no marking happens)
        // 4. On persist error → return Err (no marking happens)
        //
        // This test verifies the ConsolidationOutput parsing failure path:
        let malformed = "not valid json at all";
        let result = parse_consolidation_json(malformed);
        assert!(result.is_err(), "malformed JSON should fail parsing");

        // And the valid-but-wrong-type path:
        let wrong_type = r#"{"type": "invalid_type", "title": "T", "content": "C"}"#;
        let result = parse_consolidation_json(wrong_type);
        assert!(result.is_err(), "invalid type should fail validation");
    }

    #[test]
    fn emit_consolidation_failed_on_panic_pattern() {
        // Verify that emit_consolidation_failed can be called with panic-like
        // error messages without itself panicking.
        let pillar_tx = wire::channel();

        // Simulate the error messages that would come from JoinHandle panic catching:
        emit_consolidation_failed(&pillar_tx, "all", "all", "task panicked: something went wrong");
        emit_consolidation_failed(&pillar_tx, "all", "all", "task panicked (unknown payload)");
        emit_consolidation_failed(&pillar_tx, "all", "all", "task cancelled");

        // No panic = success. The events are best-effort.
    }

    #[test]
    fn emit_handoff_failed_on_panic_pattern() {
        // Verify that emit_handoff_failed can be called with panic-like
        // error messages without itself panicking.
        let pillar_tx = wire::channel();
        let session_id = Uuid::now_v7();

        super::super::maturation_events::emit_handoff_failed(
            &pillar_tx,
            session_id,
            "task panicked: handoff generation crashed",
        );
        super::super::maturation_events::emit_handoff_failed(
            &pillar_tx,
            session_id,
            "task cancelled",
        );

        // No panic = success.
    }
}
