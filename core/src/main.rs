//! Kira core daemon entrypoint.
//!
//! Binds to `127.0.0.1:7777` by default (configurable via CLI / env).
//! Exposes `/v1/*` HTTP+SSE API consumed by the Bun+Ink REPL client.

use anyhow::{Context, Result};
use axum::{routing::get, Json, Router};
use clap::Parser;
use serde::Serialize;
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing::info;
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

fn init_tracing(format: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("kira_core=debug,tower_http=info,info"));

    let registry = tracing_subscriber::registry().with(filter);

    match format {
        "json" => {
            registry
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        }
        _ => {
            registry
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present (dev). Silent failure in prod.
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();
    init_tracing(&cli.log_format);

    START_TIME.set(std::time::Instant::now()).ok();

    let app = Router::new()
        .route("/v1/health", get(health))
        .layer(TraceLayer::new_for_http());

    info!(addr = %cli.bind, "kira-cored starting");

    let listener = tokio::net::TcpListener::bind(&cli.bind)
        .await
        .with_context(|| format!("failed to bind {}", cli.bind))?;

    axum::serve(listener, app)
        .await
        .context("axum::serve exited unexpectedly")?;

    Ok(())
}
