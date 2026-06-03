//! Kira core daemon entrypoint.
//!
//! Binds to `127.0.0.1:7777` by default (configurable via CLI / env).
//! Delegates to `kira_core::run`, which builds `AppState` and mounts the full
//! `/v1/*` HTTP+SSE router (cognition, agents, guardrails, memory, workflow,
//! waves). This binary previously served only `/v1/health`, which left the
//! TUI unable to reach `/v1/events/stream` (Layers [DISCONNECTED]).

mod acp;
mod audit;

use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
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

    // Health uptime anchor (read by the /v1/health handler in server::health).
    kira_core::server::health::START_TIME
        .set(std::time::Instant::now())
        .ok();

    // Boot the full daemon: AppState + server::router (all /v1/* + SSE).
    kira_core::run(cli.bind).await
}
