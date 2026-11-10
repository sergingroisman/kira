use std::error::Error as StdError;
use std::io::BufWriter;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use kira::board::{self, BoardProvider};
use kira::credentials;
use kira::doctor;
use kira::error::Error;
use kira::format;
use kira::llm::ollama;
use kira::setup;
use kira::triage;

/// KIRA — Knowledge-Informed Remediation Agent (*Keep It Running, Always*).
#[derive(Debug, Parser)]
#[command(
    name = "kira",
    version,
    about = "KIRA — Knowledge-Informed Remediation Agent (*Keep It Running, Always*)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print a readable summary of an Azure DevOps work item
    Inspect {
        /// Work item id
        id: i32,
        /// Azure DevOps project name (defaults to $AZURE_DEVOPS_PROJECT or config.toml)
        #[arg(long)]
        project: Option<String>,
    },
    /// Diagnose the Azure DevOps configuration of this machine
    Doctor,
    /// Suggest a triage for a work item using the local LLM (read-only)
    Triage {
        /// Work item id (live path; requires the Azure DevOps env vars)
        id: Option<i32>,
        /// Read the work item from a local JSON file (offline path; no PAT)
        #[arg(long)]
        file: Option<PathBuf>,
        /// Ollama model tag (overrides KIRA_MODEL)
        #[arg(long)]
        model: Option<String>,
    },
    /// Save the Azure DevOps configuration and the PAT in the OS keyring
    Setup {
        /// Azure DevOps organization name (skips the prompt)
        #[arg(long)]
        org: Option<String>,
        /// Azure DevOps project name (skips the prompt)
        #[arg(long)]
        project: Option<String>,
        /// Remove the saved PAT from the keyring and exit
        #[arg(long)]
        reset: bool,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Inspect { id, project } => match inspect(id, project).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                print_error(&err);
                ExitCode::FAILURE
            }
        },
        Command::Doctor => doctor::run().await,
        Command::Triage { id, file, model } => triage_command(id, file, model).await,
        Command::Setup {
            org,
            project,
            reset,
        } => setup_command(org, project, reset).await,
    }
}

fn print_error(err: &Error) {
    eprintln!("erro: {err}");
    let mut source = StdError::source(err);
    while let Some(cause) = source {
        eprintln!("  causado por: {cause}");
        source = cause.source();
    }
}

/// Resolves org/project/PAT (flags > env > keyring/config.toml) and builds the
/// board provider selected by `KIRA_BOARD` (default `azure`).
fn resolve_board(project_override: Option<&str>) -> Result<Box<dyn BoardProvider>, Error> {
    let config = credentials::resolve_azure_config(project_override)?;
    board::for_board(
        &board::name_from_env(),
        &config.pat,
        &config.organization,
        &config.project,
    )
}

async fn inspect(id: i32, project_flag: Option<String>) -> Result<(), Error> {
    let board = resolve_board(project_flag.as_deref())?;
    let item = board.work_item(id).await?;
    print!("{}", format::format_work_item(&item));
    Ok(())
}

/// Source of the work item to triage: a live Azure DevOps id or a local file.
enum TriageSource {
    Id(i32),
    File(PathBuf),
}

fn triage_source(id: Option<i32>, file: Option<PathBuf>) -> Result<TriageSource, String> {
    match (id, file) {
        (Some(id), None) => Ok(TriageSource::Id(id)),
        (None, Some(path)) => Ok(TriageSource::File(path)),
        (Some(_), Some(_)) => {
            Err("informe apenas uma origem: o id do work item OU --file <caminho.json>".to_string())
        }
        (None, None) => Err(
            "informe a origem do work item: o id (caminho vivo, precisa do Azure configurado) \
             OU --file <caminho.json> (offline, sem PAT)"
                .to_string(),
        ),
    }
}

async fn triage_command(
    id: Option<i32>,
    file: Option<PathBuf>,
    model_flag: Option<String>,
) -> ExitCode {
    let source = match triage_source(id, file) {
        Ok(source) => source,
        Err(message) => {
            eprintln!("erro: {message}");
            eprintln!("uso: kira triage <work-item-id>  |  kira triage --file <caminho.json>");
            return ExitCode::FAILURE;
        }
    };
    let base_url = ollama::base_url_from_env();
    let model = model_flag.unwrap_or_else(ollama::model_from_env);
    let result = match source {
        TriageSource::Id(id) => triage_live(&base_url, &model, id).await,
        TriageSource::File(path) => triage::triage_file(&base_url, &model, &path).await,
    };
    match result {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            print_error(&err);
            ExitCode::FAILURE
        }
    }
}

async fn triage_live(base_url: &str, model: &str, id: i32) -> Result<String, Error> {
    let board = resolve_board(None)?;
    triage::triage_live(board.as_ref(), id, base_url, model).await
}

async fn setup_command(org: Option<String>, project: Option<String>, reset: bool) -> ExitCode {
    // Fail fast (before the user creates a token) when there is nowhere to
    // save it; on machines without a keyring the env var remains the path.
    let store = match credentials::open_default_store() {
        Ok(store) => store,
        Err(err) => {
            print_error(&err);
            return ExitCode::FAILURE;
        }
    };
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut input = stdin.lock();
    let mut output = BufWriter::new(stdout.lock());
    setup::run(
        &setup::SetupOptions {
            org,
            project,
            reset,
        },
        store.as_ref(),
        credentials::config_path().as_deref(),
        &setup::env_lookup,
        &mut input,
        &mut output,
        &setup::default_board_factory,
        &setup::open_url_in_browser,
    )
    .await
}
