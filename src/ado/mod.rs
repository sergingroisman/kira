//! Azure DevOps REST access.
//!
//! All use of the generated `azure_devops_rust_api` crate is isolated in this
//! module; the rest of the crate only sees [`WorkItem`]. Read-only: no
//! operation here ever mutates a work item.
//!
//! The board seam (`src/board/`) wraps these functions: it delegates instead
//! of duplicating them.

use azure_devops_rust_api::{Credential, core, wit};

use crate::error::Error;
use crate::work_item::WorkItem;

/// Parses the endpoint override into the URL type the generated builders take.
fn parse_endpoint(endpoint: &str) -> Result<azure_core::http::Url, Error> {
    azure_core::http::Url::parse(endpoint).map_err(|err| Error::Ado(Box::new(err)))
}

/// Fetches a single work item from Azure DevOps.
///
/// The PAT is used as a basic-credential password (empty user) and is never
/// included in errors, logs or debug output.
pub async fn get_work_item(
    pat: &str,
    organization: &str,
    project: &str,
    id: i32,
) -> Result<WorkItem, Error> {
    get_work_item_with(None, pat, organization, project, id).await
}

/// [`get_work_item`] against an explicit endpoint override (used by the tests
/// with a fake server on 127.0.0.1; production passes `None` for the real
/// `https://dev.azure.com`).
pub(crate) async fn get_work_item_with(
    endpoint: Option<&str>,
    pat: &str,
    organization: &str,
    project: &str,
    id: i32,
) -> Result<WorkItem, Error> {
    let credential = Credential::from_pat(pat);
    let mut builder = wit::ClientBuilder::new(credential);
    if let Some(endpoint) = endpoint {
        builder = builder.endpoint(parse_endpoint(endpoint)?);
    }
    let client = builder.build();
    let model = client
        .work_items_client()
        .get_work_item(organization, id, project)
        .send()
        .await
        .map_err(|err| Error::Ado(Box::new(err)))?
        .into_body()
        .map_err(|err| Error::Ado(Box::new(err)))?;
    let body = serde_json::to_string(&model)?;
    WorkItem::from_api_json(&body)
}

/// Lists the names of all projects in an organization (read-only).
///
/// Used by `kira doctor` and `kira setup` to validate the credentials and the
/// project name without needing a work item id.
pub async fn list_project_names(pat: &str, organization: &str) -> Result<Vec<String>, Error> {
    list_project_names_with(None, pat, organization).await
}

/// [`list_project_names`] against an explicit endpoint override (tests only;
/// see [`get_work_item_with`]).
pub(crate) async fn list_project_names_with(
    endpoint: Option<&str>,
    pat: &str,
    organization: &str,
) -> Result<Vec<String>, Error> {
    let credential = Credential::from_pat(pat);
    let mut builder = core::ClientBuilder::new(credential);
    if let Some(endpoint) = endpoint {
        builder = builder.endpoint(parse_endpoint(endpoint)?);
    }
    let client = builder.build();
    let list = match client.projects_client().list(organization).await {
        Ok(list) => list,
        Err(err) => {
            // The PAT is never part of the error: only the status code and the
            // organization name are kept, so no response body is ever printed.
            match err.http_status().map(u16::from) {
                Some(401) => {
                    return Err(Error::AdoUnauthorized {
                        organization: organization.to_string(),
                    });
                }
                Some(status) => {
                    return Err(Error::AdoStatus {
                        status,
                        organization: organization.to_string(),
                    });
                }
                None => return Err(Error::Ado(Box::new(err))),
            }
        }
    };
    Ok(list.value.into_iter().map(|project| project.name).collect())
}
