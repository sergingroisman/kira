use std::env;
use std::error::Error as StdError;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use kira::ado;
use kira::error::Error;
use kira::format;

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
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Inspect { id, project } => match inspect(id, project).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("erro: {err}");
                let mut source = StdError::source(&err);
                while let Some(cause) = source {
                    eprintln!("  causado por: {cause}");
                    source = cause.source();
                }
                ExitCode::FAILURE
            }
        },
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

fn read_env(name: &str, error: Error) -> Result<String, Error> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or(error)
}
