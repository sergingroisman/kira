//! Azure DevOps REST access.
//!
//! All use of the generated `azure_devops_rust_api` crate is isolated in this
//! module; the rest of the crate only sees [`WorkItem`]. Read-only: no
//! operation here ever mutates a work item.

use azure_devops_rust_api::{Credential, wit};

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
