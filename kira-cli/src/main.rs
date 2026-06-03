#![forbid(unsafe_code)]

use kira_cli::config;
use kira_cli::core_client;

mod tui;

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "kira", about = "Kira harness CLI", version)]
struct Cli {
    /// Core daemon URL (overrides KIRA_CORE_URL and tokens.toml)
    #[arg(long, env = "KIRA_CORE_URL")]
    core_url: Option<String>,

    /// LLM tier to use
    #[arg(long, value_parser = ["heavy", "standard", "light"], default_value = "standard")]
    tier: String,

    /// Execution mode
    #[arg(long, value_parser = ["yolo", "plan"], default_value = "yolo")]
    mode: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Send a message non-interactively and print the result
    Chat { message: String },
    /// Agent management
    Agent {
        #[command(subcommand)]
        cmd: AgentCommands,
    },
    /// Lesson management
    Lessons {
        #[command(subcommand)]
        cmd: LessonCommands,
    },
    /// Start a workflow run (sdd, gsd2, …)
    Workflow {
        /// Goal description
        #[arg(long)]
        goal: String,
        /// Workflow id (default: sdd)
        #[arg(long, default_value = "sdd")]
        workflow: String,
        /// Kind hint: feature, bug_fix, docs, refactor
        #[arg(long)]
        kind: Option<String>,
    },
    /// List available workflows
    Workflows,
    /// Run Detective multi-lens code review
    Detective {
        /// Diff to review (omit to auto-run `git diff HEAD`)
        #[arg(long)]
        diff: Option<String>,
        /// Worktree id for context (AGENTS.md, spec artifacts)
        #[arg(long)]
        worktree: Option<String>,
        /// Path to spec.md for requirements lens
        #[arg(long)]
        spec: Option<String>,
        /// Path to tasks.json for requirements lens
        #[arg(long)]
        tasks: Option<String>,
        /// Output format: summary (default) or json
        #[arg(long, value_parser = ["summary", "json"], default_value = "summary")]
        format: String,
    },
}

#[derive(clap::Subcommand, Debug)]
enum AgentCommands {
    /// Spawn a new agent
    Spawn {
        #[arg(long, value_parser = ["writer", "deployer", "judge", "detective"])]
        kind: String,
        #[arg(long)]
        task: String,
    },
    /// Get agent status
    Status { id: String },
}

#[derive(clap::Subcommand, Debug)]
enum LessonCommands {
    /// List all lessons
    List {
        #[arg(long, value_parser = ["table", "json"], default_value = "table")]
        format: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config from tokens.toml
    let cfg = config::Config::load(cli.core_url.as_deref())?;

    // Init tracing (non-TUI mode: log to stderr; TUI mode: suppress to avoid corrupting screen)
    if cli.command.is_some() {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("kira_cli=info".parse()?),
            )
            .init();
    }

    match cli.command {
        Some(cmd) => {
            // Non-interactive mode
            let client = core_client::CoreClient::new(&cfg)?;
            run_command(cmd, client, &cli.tier).await
        }
        None => {
            // TUI mode
            tui::run(cfg, cli.tier, cli.mode).await
        }
    }
}

async fn run_command(cmd: Commands, client: core_client::CoreClient, tier: &str) -> Result<()> {
    match cmd {
        Commands::Chat { message } => {
            let result = client.chat_yolo(&message, tier).await?;
            println!("{result}");
        }
        Commands::Agent { cmd } => match cmd {
            AgentCommands::Spawn { kind, task } => {
                let id = client.agent_spawn(&kind, &task, tier).await?;
                println!("{id}");
            }
            AgentCommands::Status { id } => {
                let status = client.agent_status(&id).await?;
                println!("{status}");
            }
        },
        Commands::Lessons { cmd } => match cmd {
            LessonCommands::List { format } => {
                let lessons = client.lessons_list().await?;
                if format.as_str() == "json" {
                    println!("{}", serde_json::to_string_pretty(&lessons)?);
                } else {
                    println!("{:<10} {:<8} {:<82} CREATED", "ID", "LEVEL", "CONTENT");
                    println!("{}", "-".repeat(120));
                    for l in &lessons {
                        println!(
                            "{:<10} {:<8} {:<82} {}",
                            l.id.chars().take(8).collect::<String>(),
                            l.level,
                            l.content.chars().take(80).collect::<String>(),
                            l.created_at
                        );
                    }
                }
            }
        },
        Commands::Workflow { goal, workflow, kind } => {
            let run = client.workflow_start(&goal, &workflow, kind.as_deref(), false).await?;
            println!("run: {}", run.id);
            println!("workflow: {} | phase: {}", run.workflow_id, run.current_phase_id);
            println!("goal: {}", run.goal);
        }
        Commands::Workflows => {
            let ids = client.workflow_ids().await?;
            if ids.is_empty() {
                println!("no workflows loaded");
            } else {
                println!("available workflows:");
                for id in &ids {
                    println!("  {id}");
                }
            }
        }
        Commands::Detective { diff, worktree, spec, tasks, format } => {
            eprintln!("detective: running 6-lens review…");
            let result = client.detective_run(
                diff.as_deref(),
                worktree.as_deref(),
                spec.as_deref(),
                tasks.as_deref(),
            ).await?;

            if format.as_str() == "json" {
                println!("{}", serde_json::to_string_pretty(&result)?);
                return Ok(());
            }

            // Summary format
            let verdict_icon = match result.verdict.as_str() {
                "pass"  => "✅",
                "warn"  => "⚠️ ",
                "block" => "🚨",
                _       => "❓",
            };
            println!("{verdict_icon} verdict: {} — {}", result.verdict.to_uppercase(), result.verdict_reason);
            println!("findings: {} total", result.summary.total_findings);
            for (k, v) in &result.summary.by_severity {
                if *v > 0 { println!("  {k}: {v}"); }
            }
            if !result.findings.is_empty() {
                println!("\nfindings:");
                for f in &result.findings {
                    let loc = match (&f.path, f.line) {
                        (Some(p), Some(l)) => format!("{p}:{l}"),
                        (Some(p), None) => p.clone(),
                        _ => "(no location)".to_string(),
                    };
                    println!("  [{sev}] {loc} — {title}", sev = f.severity, loc = loc, title = f.title);
                    println!("    → {}", f.recommendation);
                }
            }
            if !result.highlights.is_empty() {
                println!("\nhighlights:");
                for h in &result.highlights {
                    println!("  [{}] {}", h.lens, h.text);
                }
            }
        }
    }
    Ok(())
}
