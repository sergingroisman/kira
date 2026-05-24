//! Security primitives — OWASP-aligned controls.
//!
//! SC1 prompt_guard, SC3 cmd_allowlist, SC4 path_guard, SC5 secrets_filter.
//! v0 ships SC3. v0.5 fills the rest.

pub mod cmd_allowlist;
pub mod path_guard;
pub mod ssrf_guard;

/// File I/O caps (used by `/v1/tool/file_{read,write}`).
pub const FILE_READ_MAX_BYTES_DEFAULT: usize = 5 * 1024 * 1024; // 5 MiB
pub const FILE_WRITE_MAX_BYTES_HARD: usize = 5 * 1024 * 1024; // 5 MiB
