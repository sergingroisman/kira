//! Azure DevOps implementation of [`BoardProvider`].
//!
//! A thin wrapper: every call delegates to the existing functions in
//! [`crate::ado`] — no API logic is duplicated here.

use std::future::Future;
use std::pin::Pin;

use crate::ado;
use crate::board::BoardProvider;
use crate::error::Error;
use crate::work_item::WorkItem;

/// Board name accepted by [`crate::board::for_board`].
pub const BOARD_NAME: &str = "azure";

/// Azure DevOps board provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoBoard {
    pat: String,
    organization: String,
    project: String,
    /// Base URL override (tests with a fake server on 127.0.0.1; `None` means
    /// the real `https://dev.azure.com`).
    endpoint: Option<String>,
}

impl AdoBoard {
    /// Creates a provider for the real Azure DevOps service.
    pub fn new(pat: &str, organization: &str, project: &str) -> Self {
        Self {
            pat: pat.to_string(),
            organization: organization.to_string(),
            project: project.to_string(),
            endpoint: None,
        }
    }

    /// Creates a provider against an explicit endpoint (used by the tests with
    /// a fake HTTP server on 127.0.0.1).
    pub fn with_endpoint(endpoint: &str, pat: &str, organization: &str, project: &str) -> Self {
        Self {
            pat: pat.to_string(),
            organization: organization.to_string(),
            project: project.to_string(),
            endpoint: Some(endpoint.trim_end_matches('/').to_string()),
        }
    }
}

impl BoardProvider for AdoBoard {
    fn work_item(
        &self,
        id: i32,
    ) -> Pin<Box<dyn Future<Output = Result<WorkItem, Error>> + Send + '_>> {
        Box::pin(ado::get_work_item_with(
            self.endpoint.as_deref(),
            &self.pat,
            &self.organization,
            &self.project,
            id,
        ))
    }

    fn project_names(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, Error>> + Send + '_>> {
        Box::pin(ado::list_project_names_with(
            self.endpoint.as_deref(),
            &self.pat,
            &self.organization,
        ))
    }
}
