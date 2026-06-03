//! Security primitives — OWASP-aligned controls.
//!
//! SC1 `prompt_guard`, SC3 `cmd_allowlist`, SC4 `path_guard`, SC5 `secrets_filter`.
//! v0 ships SC3. v0.5 fills the rest.

pub mod approval;
pub mod cmd_allowlist;
pub mod delimiters;
pub mod input;
pub mod judge_caller;
pub mod output;
pub mod path_guard;
pub mod permission;
pub mod pipeline;
pub mod risk;
pub mod secrets_filter;
pub mod ssrf_guard;
pub mod tool;

/// File I/O caps (used by `/v1/tool/file_{read,write}`).
pub const FILE_READ_MAX_BYTES_DEFAULT: usize = 5 * 1024 * 1024; // 5 MiB
pub const FILE_WRITE_MAX_BYTES_HARD: usize = 5 * 1024 * 1024; // 5 MiB
