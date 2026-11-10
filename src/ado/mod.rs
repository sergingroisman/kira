//! Azure DevOps REST access.
//!
//! All use of the generated `azure_devops_rust_api` crate is isolated in this
//! module; the rest of the crate only sees [`WorkItem`]. Read-only: no
//! operation here ever mutates a work item.

use azure_devops_rust_api::{Credential, core, wit};

use crate::error::Error;
use crate::work_item::WorkItem;

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
    let credential = Credential::from_pat(pat);
    let client = wit::ClientBuilder::new(credential).build();
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
/// Used by `kira doctor` to validate the credentials and the project name
/// without needing a work item id.
pub async fn list_project_names(pat: &str, organization: &str) -> Result<Vec<String>, Error> {
    let credential = Credential::from_pat(pat);
    let client = core::ClientBuilder::new(credential).build();
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
