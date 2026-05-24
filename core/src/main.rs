//! Kira core entrypoint.
//!
//! Subcommands:
//!   `serve`   — HTTP+SSE daemon on `127.0.0.1:7777` (default)
//!   `dataset` — export training data from audit logs

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod dataset;

#[derive(Parser, Debug)]
#[command(name = "kira-cored", version, about = "Kira core daemon + CLI tools")]
struct Cli {
    /// Log format: "pretty" (dev) or "json" (prod / VPS).
    #[arg(long, default_value = "pretty", env = "KIRA_LOG_FORMAT", global = true)]
    log_format: String,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start the HTTP+SSE daemon.
    Serve {
        /// Bind address.
        #[arg(long, default_value = "127.0.0.1:7777")]
        bind: SocketAddr,
    },
    /// Export training datasets from audit logs.
    Dataset {
        #[command(subcommand)]
        action: DatasetAction,
    },
}

#[derive(Subcommand, Debug)]
enum DatasetAction {
    /// Export judge decisions as LLaMA-Factory JSONL.
    Export {
        /// Filter: only judge decisions.
        #[arg(long)]
        judge: bool,

        /// Only include entries after this ISO date (e.g. 2026-01-01).
        #[arg(long)]
        since: Option<String>,

        /// Max entries to export.
        #[arg(long)]
        count: Option<usize>,

        /// Only include entries with human_label != null.
        #[arg(long)]
        labeled_only: bool,

        /// Output file (default: stdout).
        #[arg(short, long)]
        output: Option<String>,
    },
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

    match cli.command {
        Some(Command::Dataset { action }) => dataset::run(action),
        Some(Command::Serve { bind }) => {
            kira_core::server::health::START_TIME
                .set(std::time::Instant::now())
                .ok();
            kira_core::run(bind).await
        }
        None => {
            let bind: SocketAddr = "127.0.0.1:7777".parse()?;
            kira_core::server::health::START_TIME
                .set(std::time::Instant::now())
                .ok();
            kira_core::run(bind).await
        }
    }
}
