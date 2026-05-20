//! Kira core daemon entrypoint.
//!
//! Binds to `127.0.0.1:7777` by default (configurable via CLI / env).
//! Exposes `/v1/*` HTTP+SSE API consumed by the Bun+Ink REPL client.

use anyhow::{Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use futures::StreamExt;
use std::convert::Infallible;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tower_http::trace::TraceLayer;
use tracing::{error, info, instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Kira core daemon — HTTP+SSE on `127.0.0.1:7777`.
#[derive(Parser, Debug)]
#[command(name = "kira-cored", version, about)]
struct Cli {
    /// Bind address (default 127.0.0.1:7777).
    #[arg(long, default_value = "127.0.0.1:7777")]
    bind: SocketAddr,

    /// Log format: "pretty" (dev) or "json" (prod / VPS).
    #[arg(long, default_value = "pretty", env = "KIRA_LOG_FORMAT")]
    log_format: String,
}

// ============================================================================
// App state
// ============================================================================

#[derive(Clone)]
struct AppState {
    dashscope: Arc<Client<OpenAIConfig>>,
}

impl AppState {
    fn new() -> Result<Self> {
        let api_key = std::env::var("DASHSCOPE_API_KEY")
            .context("DASHSCOPE_API_KEY missing — set it in .env or environment")?;
        let config = OpenAIConfig::new()
            .with_api_base("https://dashscope-intl.aliyuncs.com/compatible-mode/v1")
            .with_api_key(api_key);
        Ok(Self {
            dashscope: Arc::new(Client::with_config(config)),
        })
    }
}

// ============================================================================
// Error type (axum-compatible)
// ============================================================================

/// Wraps `anyhow::Error` so axum can return it as HTTP 500 with JSON body.
struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        error!(error = ?self.0, "request failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": self.0.to_string(),
                "chain": format!("{:#}", self.0),
            })),
        )
            .into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

// ============================================================================
// /v1/health
// ============================================================================

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
    uptime_secs: u64,
}

static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

async fn health() -> Json<HealthResponse> {
    let uptime = START_TIME
        .get()
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_secs: uptime,
    })
}

// ============================================================================
// /v1/chat (non-streaming — SSE comes Day 5)
// ============================================================================

#[derive(Deserialize, Debug)]
struct ChatRequest {
    /// Tier hint — for now ignored, just uses qwen-plus. Router lands Day 6+.
    #[serde(default = "default_tier")]
    tier: String,
    /// Override model explicitly (for testing). If set, overrides tier mapping.
    model: Option<String>,
    messages: Vec<ChatMessage>,
}

fn default_tier() -> String {
    "standard".to_string()
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatResponse {
    content: String,
    model: String,
    tokens_in: u32,
    tokens_out: u32,
    finish_reason: Option<String>,
}

#[instrument(skip(state, req), fields(tier = %req.tier, model))]
async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, AppError> {
    // Map tier → model (v0 hardcoded; M01 Router takes over Sem 4-5).
    let model = req.model.clone().unwrap_or_else(|| match req.tier.as_str() {
        "heavy" => "qwen-max".to_string(),
        "light" => "qwen-flash".to_string(),
        _ => "qwen-plus".to_string(), // standard default
    });
    tracing::Span::current().record("model", &model);

    // Convert wire format → async_openai types.
    let messages: Vec<ChatCompletionRequestMessage> = req
        .messages
        .iter()
        .map(|m| -> Result<ChatCompletionRequestMessage> {
            // v0: tudo vira UserMessage. SystemMessage/AssistantMessage virá quando precisar.
            let msg = ChatCompletionRequestUserMessageArgs::default()
                .content(m.content.clone())
                .build()?;
            Ok(msg.into())
        })
        .collect::<Result<_>>()?;

    let request = CreateChatCompletionRequestArgs::default()
        .model(&model)
        .messages(messages)
        .build()?;

    let response = state
        .dashscope
        .chat()
        .create(request)
        .await
        .context("DashScope chat completion failed")?;

    let choice = response.choices.into_iter().next().context("no choices")?;
    let usage = response.usage.context("no usage returned")?;

    Ok(Json(ChatResponse {
        content: choice.message.content.unwrap_or_default(),
        model: response.model,
        tokens_in: usage.prompt_tokens,
        tokens_out: usage.completion_tokens,
        finish_reason: choice.finish_reason.map(|r| format!("{r:?}")),
    }))
}

// ============================================================================
// /v1/chat/stream (SSE — Day 5)
// ============================================================================
//
// Wire format: each event is a named SSE event.
//   event: token  data: {"delta":"..."}
//   event: done   data: {"model":"...","tokens_in":N,"tokens_out":N,"finish_reason":"..."}
//   event: error  data: {"message":"..."}
//
// Client (Bun) consumes via EventSource(`http://127.0.0.1:7777/v1/chat/stream`)
// — but EventSource is GET-only. For now POST + manual SSE parsing on client side.
// Alternative: use `fetch` with ReadableStream in Bun. Easy enough.

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamEvent {
    Token { delta: String },
    Done { model: String, finish_reason: Option<String> },
    Error { message: String },
}

#[instrument(skip(state, req), fields(tier = %req.tier, model))]
async fn chat_stream(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, AppError> {
    let model = req.model.clone().unwrap_or_else(|| match req.tier.as_str() {
        "heavy" => "qwen-max".to_string(),
        "light" => "qwen-flash".to_string(),
        _ => "qwen-plus".to_string(),
    });
    tracing::Span::current().record("model", &model);

    let messages: Vec<ChatCompletionRequestMessage> = req
        .messages
        .iter()
        .map(|m| -> Result<ChatCompletionRequestMessage> {
            let msg = ChatCompletionRequestUserMessageArgs::default()
                .content(m.content.clone())
                .build()?;
            Ok(msg.into())
        })
        .collect::<Result<_>>()?;

    let request = CreateChatCompletionRequestArgs::default()
        .model(&model)
        .messages(messages)
        .stream(true)
        .build()?;

    let upstream = state
        .dashscope
        .chat()
        .create_stream(request)
        .await
        .context("DashScope create_stream failed")?;

    let model_clone = model.clone();
    let sse_stream = upstream.flat_map(move |chunk_result| {
        let model = model_clone.clone();
        let events: Vec<Result<Event, Infallible>> = match chunk_result {
            Ok(chunk) => {
                let choice = chunk.choices.first();
                let delta = choice
                    .and_then(|c| c.delta.content.as_ref())
                    .cloned()
                    .unwrap_or_default();
                let finish = choice.and_then(|c| c.finish_reason.map(|r| format!("{r:?}")));

                let mut events = Vec::new();
                if !delta.is_empty() {
                    let payload = StreamEvent::Token { delta };
                    events.push(Ok(Event::default()
                        .event("token")
                        .data(serde_json::to_string(&payload).unwrap_or_default())));
                }
                if let Some(reason) = finish {
                    let payload = StreamEvent::Done {
                        model: model.clone(),
                        finish_reason: Some(reason),
                    };
                    events.push(Ok(Event::default()
                        .event("done")
                        .data(serde_json::to_string(&payload).unwrap_or_default())));
                }
                events
            }
            Err(e) => {
                let payload = StreamEvent::Error {
                    message: e.to_string(),
                };
                vec![Ok(Event::default()
                    .event("error")
                    .data(serde_json::to_string(&payload).unwrap_or_default()))]
            }
        };
        futures::stream::iter(events)
    });

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::new()))
}

// ============================================================================
// /v1/tool/bash (Day 6) — SC3 OS Command Injection Defense
// ============================================================================
//
// Security controls applied:
//   1. Executable allowlist (only these may run)
//   2. Argument regex (alphanumeric + safe punctuation only)
//   3. NO shell expansion (tokio::process::Command, separate args, never sh -c)
//   4. Timeout (hard cap, default 30s, max 300s)
//   5. Output size cap (truncate beyond N bytes)
//
// Forbidden metacharacters in args: & | ; $ > < \ ! ' " ( ) backtick

use once_cell::sync::Lazy;
use regex::Regex;
use std::time::Duration;

/// Executables permitted in the `bash` tool. Tighten per agent kind in v0.5.
const BASH_ALLOWLIST: &[&str] = &[
    "ls", "cat", "head", "tail", "grep", "find", "git", "cargo", "bun", "npm",
    "pnpm", "node", "rg", "sd", "wc", "diff", "stat", "echo", "pwd", "which",
];

/// Args must match this — alphanumeric, dots, slashes, dashes, common safe chars.
static SAFE_ARG_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z0-9_./@:=,\-+]+$").expect("hardcoded regex"));

const BASH_TIMEOUT_DEFAULT_SECS: u64 = 30;
const BASH_TIMEOUT_MAX_SECS: u64 = 300;
const BASH_OUTPUT_MAX_BYTES: usize = 256 * 1024; // 256 KiB cap

#[derive(Deserialize, Debug)]
struct BashRequest {
    /// First element = executable name (must be in allowlist).
    /// Subsequent elements = arguments (must match SAFE_ARG_RE).
    /// Example: `["git", "status", "--short"]`
    argv: Vec<String>,

    #[serde(default)]
    cwd: Option<String>,

    #[serde(default = "default_bash_timeout")]
    timeout_secs: u64,
}

fn default_bash_timeout() -> u64 {
    BASH_TIMEOUT_DEFAULT_SECS
}

#[derive(Serialize)]
struct BashResponse {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    duration_ms: u128,
    truncated: bool,
}

/// Validate argv against allowlist + regex. Returns (executable, args) or error.
fn validate_argv(argv: &[String]) -> Result<(&str, Vec<&str>)> {
    let (exe, args) = argv
        .split_first()
        .context("argv empty — first element must be executable")?;

    if !BASH_ALLOWLIST.contains(&exe.as_str()) {
        anyhow::bail!(
            "executable '{}' not in allowlist (SC3). Allowed: {:?}",
            exe,
            BASH_ALLOWLIST
        );
    }

    for (i, a) in args.iter().enumerate() {
        if !SAFE_ARG_RE.is_match(a) {
            anyhow::bail!(
                "arg #{} '{}' contains forbidden chars (SC3 regex {})",
                i,
                a,
                SAFE_ARG_RE.as_str()
            );
        }
    }

    Ok((exe.as_str(), args.iter().map(String::as_str).collect()))
}

#[instrument(skip(req), fields(exe, args_count = req.argv.len() - 1))]
async fn tool_bash(Json(req): Json<BashRequest>) -> Result<Json<BashResponse>, AppError> {
    let timeout = req.timeout_secs.min(BASH_TIMEOUT_MAX_SECS);

    let (exe, args) = validate_argv(&req.argv)?;
    tracing::Span::current().record("exe", exe);

    let mut cmd = tokio::process::Command::new(exe);
    cmd.args(&args);
    cmd.env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", std::env::var("HOME").unwrap_or_default())
        .env("USER", std::env::var("USER").unwrap_or_default())
        .env("LANG", std::env::var("LANG").unwrap_or_else(|_| "C.UTF-8".into()));
    if let Some(cwd) = req.cwd.as_ref() {
        let canonical = std::fs::canonicalize(cwd)
            .with_context(|| format!("cwd canonicalize failed: {cwd}"))?;
        cmd.current_dir(canonical);
    }
    cmd.kill_on_drop(true);

    let start = std::time::Instant::now();
    let output = tokio::time::timeout(Duration::from_secs(timeout), cmd.output())
        .await
        .with_context(|| format!("bash tool timed out after {timeout}s"))?
        .context("spawn failed")?;
    let elapsed = start.elapsed();

    let mut stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let mut truncated = false;
    if stdout.len() > BASH_OUTPUT_MAX_BYTES {
        stdout.truncate(BASH_OUTPUT_MAX_BYTES);
        truncated = true;
    }
    if stderr.len() > BASH_OUTPUT_MAX_BYTES {
        stderr.truncate(BASH_OUTPUT_MAX_BYTES);
        truncated = true;
    }

    Ok(Json(BashResponse {
        stdout,
        stderr,
        exit_code: output.status.code(),
        duration_ms: elapsed.as_millis(),
        truncated,
    }))
}

// ============================================================================
// Bootstrap
// ============================================================================

fn init_tracing(format: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("kira_core=debug,tower_http=info,info"));

    let registry = tracing_subscriber::registry().with(filter);

    match format {
        "json" => registry
            .with(tracing_subscriber::fmt::layer().json())
            .init(),
        _ => registry
            .with(tracing_subscriber::fmt::layer().pretty())
            .init(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();
    init_tracing(&cli.log_format);

    START_TIME.set(std::time::Instant::now()).ok();

    let state = AppState::new()?;

    let app = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/chat", post(chat))
        .route("/v1/chat/stream", post(chat_stream))
        .route("/v1/tool/bash", post(tool_bash))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    info!(addr = %cli.bind, "kira-cored starting");

    let listener = tokio::net::TcpListener::bind(&cli.bind)
        .await
        .with_context(|| format!("failed to bind {}", cli.bind))?;

    axum::serve(listener, app)
        .await
        .context("axum::serve exited unexpectedly")?;

    Ok(())
}
