//! Pluggable board providers.
//!
//! KIRA reads work items through the [`BoardProvider`] trait so the hosting
//! board can be swapped later (this phase: Azure DevOps; planned: Jira,
//! Trello). [`for_board`] is the single factory: it picks the implementation
//! from a board name (default from `KIRA_BOARD`, itself defaulting to
//! `"azure"`) and refuses unknown names with an actionable error.
//!
//! Implementations are read-only: they fetch work items and list projects and
//! never mutate anything on the board.

pub mod azure;

use std::future::Future;
use std::pin::Pin;

use crate::error::Error;
use crate::work_item::WorkItem;

/// A hosting board able to fetch work items and list projects.
///
/// The futures are boxed so the trait stays object-safe: `main` holds one
/// `Box<dyn BoardProvider>` chosen by [`for_board`].
pub trait BoardProvider: Send + Sync {
    /// Fetches a single work item by id (read-only).
    fn work_item(
        &self,
        id: i32,
    ) -> Pin<Box<dyn Future<Output = Result<WorkItem, Error>> + Send + '_>>;

    /// Lists the names of all projects visible to the credential (read-only).
    fn project_names(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, Error>> + Send + '_>>;
}

/// Environment variable holding the board name (default: `azure`).
pub const BOARD_ENV: &str = "KIRA_BOARD";

/// The only board implemented in this phase.
pub const DEFAULT_BOARD: &str = "azure";

/// Reads the board name from `KIRA_BOARD`, falling back to [`DEFAULT_BOARD`].
pub fn name_from_env() -> String {
    std::env::var(BOARD_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BOARD.to_string())
}

/// Builds the board provider for `name`.
///
/// Unknown names produce an actionable error — no silent fallback.
pub fn for_board(
    name: &str,
    pat: &str,
    organization: &str,
    project: &str,
) -> Result<Box<dyn BoardProvider>, Error> {
    match name.trim().to_lowercase().as_str() {
        azure::BOARD_NAME => Ok(Box::new(azure::AdoBoard::new(pat, organization, project))),
        other => Err(Error::UnknownBoard {
            name: other.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_builds_azure_board_regardless_of_case() {
        for name in ["azure", "Azure", "AZURE", " azure "] {
            let board = for_board(name, "pat", "org", "proj");
            assert!(
                board.is_ok(),
                "board {name:?} deve ser aceito: {err:?}",
                err = board.err()
            );
        }
    }

    #[test]
    fn factory_refuses_unknown_board_with_actionable_error() {
        let Err(err) = for_board("jira", "pat", "org", "proj") else {
            panic!("board desconhecido deve ser recusado");
        };
        let message = err.to_string();
        assert!(message.contains("jira"), "mensagem: {message}");
        assert!(
            message.contains("ainda não é suportado"),
            "mensagem: {message}"
        );
        assert!(
            message.contains("Azure DevOps"),
            "mensagem deve apontar o MVP: {message}"
        );
        assert!(message.contains("KIRA_BOARD=azure"), "mensagem: {message}");
    }

    /// Guard against the parallel test-runner race on the shared BOARD_ENV.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn name_from_env_defaults_to_azure() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        unsafe {
            std::env::remove_var(BOARD_ENV);
        }
        assert_eq!(name_from_env(), DEFAULT_BOARD);
    }

    #[test]
    fn name_from_env_reads_the_override() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        unsafe {
            std::env::set_var(BOARD_ENV, "jira");
        }
        assert_eq!(name_from_env(), "jira");
        unsafe {
            std::env::remove_var(BOARD_ENV);
        }
    }
}
