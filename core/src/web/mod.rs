//! Web tools — sovereign search (`SearXNG`) + safe fetch (readability + SSRF).
//!
//! Two public functions for now:
//! - `search(cfg, query, opts)` → `Vec<SearchResult>`
//! - `fetch(cfg, url)` → markdown content wrapped in `<TOOL_OUTPUT untrusted>`
//!
//! Both are best-effort: errors propagate as `anyhow`. SSRF, cap, redirect
//! limits enforced before any wire IO.

pub mod fetch;
pub mod search;

pub use fetch::{fetch, FetchResult};
pub use search::{search, SearchResult};
