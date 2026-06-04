//! `POST /v1/chat` (non-streaming) + `POST /v1/chat/stream` (SSE) + `POST /v1/chat/yolo` (direct agent spawn).

use anyhow::Result;
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures::StreamExt;
use std::convert::Infallible;
use std::sync::Arc;
use tracing::{instrument, warn};

use crate::llm::types::StreamChunk;
use crate::llm::LlmProvider;
use crate::proto::agent::{AgentKind, SpawnAgentRequest};
use crate::proto::chat::{ChatRequest, ChatResponse, StreamEvent, YoloResponse};
use crate::router::tier::{Candidate, Tier};
use crate::router::Router;
use crate::server::{error::AppError, AppState};

// ============================================================================
// Non-streaming /v1/chat — delegates to Router.route_chat (handles fallback)
// ============================================================================

#[instrument(skip(state, req), fields(tier = %req.tier))]
pub async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, AppError> {
    let (output, _provider, model) = state.router.route_chat(&req, &req.messages).await?;

    Ok(Json(ChatResponse {
        content: output.content,
        model,
        tokens_in: output.tokens_in,
        tokens_out: output.tokens_out,
        finish_reason: output.finish_reason,
    }))
}

// ============================================================================
// Streaming /v1/chat/stream
// ============================================================================
//
// For streaming the fallback chain is trickier: once bytes have been sent to
// the client, we can't switch providers mid-stream. v0 strategy: try the first
// non-broken candidate; if its `create_stream` setup call fails, try the next.
// Once `create_stream` succeeds we commit to that provider; downstream stream
// errors surface to the client.

#[instrument(skip(state, req), fields(tier = %req.tier, model))]
pub async fn chat_stream(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, AppError> {
    state.router.budget.check()?;

    let candidates = candidates_for_request(&state.router, &req);
    let mut last_err: Option<anyhow::Error> = None;

    for cand in candidates {
        let breaker = state.router.breaker_for_public(&cand.key());
        if breaker.is_open() {
            warn!(candidate = %cand.key(), "skipping — breaker Open");
            continue;
        }
        let Some(provider) = provider_clone(&state.router, &cand.provider) else {
            continue;
        };

        tracing::Span::current().record("model", &cand.model);

        match provider.chat_stream(&cand.model, &req.messages).await {
            Ok(upstream) => {
                breaker.record_success();
                let model = cand.model.clone();
                let sse_stream = upstream.filter_map(move |chunk_result| {
                    let model = model.clone();
                    async move {
                        match chunk_result {
                            Ok(chunk) => Some(render_stream_chunk(chunk, &model)),
                            Err(e) => Some(error_event(&e.to_string())),
                        }
                    }
                }).flat_map(futures::stream::iter);
                return Ok(Sse::new(sse_stream).keep_alive(KeepAlive::new()));
            }
            Err(e) => {
                warn!(candidate = %cand.key(), error = ?e, "stream setup failed, trying next");
                breaker.record_failure();
                last_err = Some(e);
                continue;
            }
        }
    }

    Err(last_err
        .unwrap_or_else(|| anyhow::anyhow!("all candidates exhausted (tier {})", req.tier))
        .into())
}

// ============================================================================
// Helpers
// ============================================================================

fn candidates_for_request(router: &Router, req: &ChatRequest) -> Vec<Candidate> {
    if let Some(model) = req.model.as_deref() {
        return router
            .known_providers()
            .into_iter()
            .map(|p| Candidate::new(p, model.to_string()))
            .collect();
    }
    router.tier_candidates(Tier::from_str_lossy(&req.tier))
}

fn provider_clone(router: &Router, name: &str) -> Option<Arc<LlmProvider>> {
    router.provider_handle(name)
}

fn render_stream_chunk(chunk: StreamChunk, model: &str) -> Vec<Result<Event, Infallible>> {
    match chunk {
        StreamChunk::Token(delta) if delta.is_empty() => vec![],
        StreamChunk::Token(delta) => {
            let payload = StreamEvent::Token { delta };
            vec![Ok(Event::default()
                .event("token")
                .data(serde_json::to_string(&payload).unwrap_or_default()))]
        }
        StreamChunk::Finish { reason } => {
            let payload = StreamEvent::Done {
                model: model.to_string(),
                finish_reason: reason,
            };
            vec![Ok(Event::default()
                .event("done")
                .data(serde_json::to_string(&payload).unwrap_or_default()))]
        }
        StreamChunk::Error(message) => {
            let payload = StreamEvent::Error { message };
            vec![Ok(Event::default()
                .event("error")
                .data(serde_json::to_string(&payload).unwrap_or_default()))]
        }
    }
}

fn error_event(message: &str) -> Vec<Result<Event, Infallible>> {
    let payload = StreamEvent::Error {
        message: message.to_string(),
    };
    vec![Ok(Event::default()
        .event("error")
        .data(serde_json::to_string(&payload).unwrap_or_default()))]
}

// ============================================================================
// YOLO mode — /v1/chat/yolo
// ============================================================================

/// Conceptual-chat heuristic (H6/AP5, 2026-05-25): does the user message
/// look like a *question* the model can answer from its own knowledge, vs
/// a *coding task* that needs the full agent loop with tools and worktree?
///
/// Returns true when no action keywords are present in PT or EN. Errs on the
/// side of "yes, conceptual" — false positives cost a fast route (cheap),
/// false negatives spawn a full agent (expensive, the old default).
fn looks_conceptual(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    // Action verbs that imply file/code changes or shell work. PT + EN.
    const ACTIONS: &[&str] = &[
        // EN
        "implement", "create", "build", "make", "add ", "remove", "delete",
        "fix", "refactor", "rename", "edit", "update", "modify", "change ",
        "run ", "execute", "test ", "deploy", "install", "setup", "configure",
        "generate", "write ", "patch", "merge", "commit", "push ", "pull ",
        "rebase", "checkout", "git ", "cargo ", "bun ", "npm ", "pnpm ",
        "open ", "read ", "search ", "grep", "find ",
        // PT — indicative / infinitive
        "implementa", "implementar", "cria", "criar", "adiciona", "adicionar",
        "remove", "remover", "deleta", "deletar", "corrige", "corrigir",
        "refatora", "renomeia", "edita", "editar", "atualiza", "atualizar",
        "modifica", "muda ", "roda ", "executa", "testa ", "instala",
        "configura", "gera ", "escreve ", "abre ", "lê ", "busca ", "procura",
        // PT — imperative (how users actually phrase commands: "crie X", "rode Y")
        "crie", "faça", "faca", "adicione", "remova", "apague", "corrija",
        "refatore", "renomeie", "edite", "atualize", "modifique", "rode",
        "gere", "escreva", "abra", "leia", "busque", "procure", "instale",
        "configure", "mude", "salve", "grave", "implemente", "construa",
        "monte", "rode os", "rascunhe",
    ];
    !ACTIONS.iter().any(|kw| lower.contains(kw))
}

/// A research-heavy task (study a tool, learn install/usage, survey community
/// use). Such tasks benefit from a research preflight (plan queries + parallel
/// fetch) instead of a reactive search loop that burns iterations on 404s.
fn looks_research(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    const SIGNALS: &[&str] = &[
        // PT
        "estud", "pesquis", "documenta", "como instalar", "como usar",
        "casos de uso", "como os outros", "como funciona", "tutorial",
        "levantar", "investiga",
        // EN
        "study", "research", "how to install", "how to use", "getting started",
        "user stories", "use cases", "community", "review", "survey", "learn ",
        "investigate", "overview",
    ];
    SIGNALS.iter().any(|kw| lower.contains(kw))
}

#[instrument(skip(state, req), fields(yolo = true))]
pub async fn chat_yolo(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<YoloResponse>, AppError> {
    let mut task = String::new();
    let mut last_user_only = String::new();
    for msg in &req.messages {
        match msg.role.as_str() {
            "user" => {
                task.push_str(&format!("User: {}\n", msg.content));
                last_user_only = msg.content.clone();
            }
            "assistant" => {
                let preview: String = msg.content.chars().take(500).collect();
                task.push_str(&format!("Previous agent output: {preview}\n"));
            }
            _ => {}
        }
    }
    let user_message = task.trim().to_string();

    if user_message.is_empty() {
        return Err(AppError::bad_request("no user message for yolo"));
    }

    let tier = req.tier.clone();

    // Route (H6/AP5): conceptual question → read-only Explorer; action verb →
    // full Writer. Both spawn an agent so the loop emits events (Heartbeat,
    // P7 cost, pillar layers) that populate the TUI sidebar + Layers view, and
    // the answer streams back via the agent poller. The Explorer has tools
    // (file_read/web_search/web_fetch) but cannot write — so it *can* research
    // before answering "what is X?" without the cost/risk of a full Writer.
    let conceptual = looks_conceptual(&last_user_only);
    let (kind, mut route) = if conceptual {
        (AgentKind::Explorer, "lite")
    } else {
        (AgentKind::Writer, "full")
    };

    // Research preflight: for research-heavy Writer tasks, plan queries + fetch
    // sources in parallel UP FRONT, then hand the agent the notes. Avoids the
    // reactive 12-iter search spiral (guessed URLs → 404 → Failed, no output).
    let mut user_message = user_message;
    if !conceptual && looks_research(&last_user_only) {
        let cfg = crate::config::WebToolsConfig::default();
        match crate::research::run_research(&state.router, &cfg, &last_user_only).await {
            Ok(notes) if !notes.trim().is_empty() => {
                tracing::info!(bytes = notes.len(), "yolo: research preflight attached notes");
                user_message = format!(
                    "{user_message}\n\n--- RESEARCH NOTES (pre-fetched for you; \
                     synthesize the deliverable FROM THESE. Do NOT web_search or \
                     web_fetch again — the research is done) ---\n{notes}"
                );
                route = "research";
            }
            Ok(_) => tracing::info!("yolo: research preflight produced no notes — proceeding plain"),
            Err(e) => tracing::warn!(error = %e, "yolo: research preflight failed — proceeding plain"),
        }
    }

    // Honour the requested tier for conceptual Q&A. The local 3B model
    // (qwen2.5:3b) is too weak to understand user questions ("O que é MCP e
    // ACP?" → garbage answers, tool misuse) — "barato primeiro" downgraded
    // quality below usefulness. `standard` (deepseek-chat) is cheap *and*
    // capable, and the read-only Explorer toolset keeps cost bounded.
    let agent_tier = tier.clone();

    let cfg_iters = state
        .tokens
        .as_ref()
        .map_or(5, |t| t.concurrency.max_agent_iters_yolo);
    // Cap the lite route to keep cheap questions cheap — but 4 was too tight:
    // a couple of failed tool calls (dead web_search, 403 fetches) burned the
    // whole budget before the model could answer. 8 leaves room; the
    // force-final-answer fallback in spawn.rs guarantees a response either way.
    // Conceptual Q&A: a few steps suffice. Action tasks (Writer) often need to
    // research AND write multiple files — 5 was too tight (web research alone
    // burned the budget before the first file_write). Budget cap is the
    // real backstop.
    let max_iters = if conceptual {
        cfg_iters.max(8)
    } else {
        cfg_iters.max(12)
    };

    // Wall-clock safety net. Without it (`timeout_secs: None`) a slow or
    // looping model runs until max_iters with no time bound — observed as an
    // agent "running" for 460s+ with no answer. The cancel mechanism already
    // exists in spawn.rs; it only fires when timeout_secs is set. Conceptual
    // (lite) questions should be quick; action tasks get a longer ceiling.
    let timeout_secs = Some(if conceptual { 120 } else { 600 });

    let agent_req = SpawnAgentRequest {
        kind,
        task: user_message,
        tier: Some(agent_tier.clone()),
        worktree_id: None,
        base: None,
        budget_usd: None,
        max_iters: Some(max_iters),
        yolo_mode: Some(true),
        background: None,
        timeout_secs,
    };

    tracing::info!(%route, tier = %agent_tier, "yolo: spawning {kind:?} agent");
    let agent = state.agents.spawn(agent_req).await?;

    Ok(Json(YoloResponse {
        agent_id: agent.id.clone(),
        kind: format!("{:?}", agent.kind).to_lowercase(),
        status: format!("{:?}", agent.status).to_lowercase(),
        message: None,
        route: route.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::looks_conceptual;

    #[test]
    fn questions_are_conceptual() {
        assert!(looks_conceptual("A memória do KIRA tem quantas camadas?"));
        assert!(looks_conceptual("How does the harness work?"));
        assert!(looks_conceptual("o que é SDD?"));
        assert!(looks_conceptual("quem é responsável pelo Pilot?"));
    }

    #[test]
    fn coding_tasks_are_not_conceptual() {
        assert!(!looks_conceptual("implementa um endpoint /health"));
        assert!(!looks_conceptual("fix the bug in spawn.rs"));
        assert!(!looks_conceptual("refactor the router module"));
        assert!(!looks_conceptual("rode cargo test"));
        assert!(!looks_conceptual("create a new file foo.rs"));
        assert!(!looks_conceptual("grep all matches of foo"));
    }
}
