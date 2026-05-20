//! Security primitives — OWASP-aligned controls.
//!
//! SC1 prompt_guard, SC3 cmd_allowlist, SC4 path_guard, SC5 secrets_filter.
//! v0 ships SC3. v0.5 fills the rest.

pub mod cmd_allowlist;
