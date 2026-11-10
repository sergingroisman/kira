use std::env;
use std::error::Error as StdError;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use kira::ado;
use kira::doctor;
use kira::error::Error;
use kira::format;
use kira::llm::ollama;
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
        /// Azure DevOps project name (defaults to $AZURE_DEVOPS_PROJECT)
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

async fn inspect(id: i32, project_flag: Option<String>) -> Result<(), Error> {
    let pat = read_env("AZURE_DEVOPS_PAT", Error::MissingPat)?;
    let organization = read_env("AZURE_DEVOPS_ORG", Error::MissingOrg)?;
    let project = match project_flag {
        Some(project) => project,
        None => read_env("AZURE_DEVOPS_PROJECT", Error::MissingProject)?,
    };
    let item = ado::get_work_item(&pat, &organization, &project, id).await?;
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
    let pat = read_env("AZURE_DEVOPS_PAT", Error::MissingPat)?;
    let organization = read_env("AZURE_DEVOPS_ORG", Error::MissingOrg)?;
    let project = read_env("AZURE_DEVOPS_PROJECT", Error::MissingProject)?;
    triage::triage_live(&pat, &organization, &project, id, base_url, model).await
}

fn read_env(name: &str, error: Error) -> Result<String, Error> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or(error)
}
