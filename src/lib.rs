//! KIRA — Knowledge-Informed Remediation Agent.
//!
//! Library behind the `kira` binary: read-only access to Azure DevOps work
//! items. All use of the generated `azure_devops_rust_api` crate is isolated
//! in [`ado`].

pub mod ado;
pub mod board;
pub mod credentials;
pub mod doctor;
pub mod error;
pub mod format;
pub mod guard;
pub mod html;
pub mod llm;
pub mod redact;
pub mod scan;
pub mod setup;
pub mod store;
pub mod triage;
pub mod work_item;
