//! Kira core daemon entrypoint.
//!
//! Thin binary: parse CLI, init tracing, delegate to [`kira_core::run`].

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

    kira_core::server::health::START_TIME
        .set(std::time::Instant::now())
        .ok();

    kira_core::run(cli.bind).await
}
