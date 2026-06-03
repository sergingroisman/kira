//! MCP (Model Context Protocol) client integration.
//!
//! Kira as MCP **client** — connects to external MCP servers via stdio
//! transport, discovers their tools, and makes them available to agents
//! alongside built-in tools (bash, `file_read`, `file_write`, etc).
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐     stdio     ┌──────────────────┐
//! │  Kira Agent  │──────────────│  MCP Server A     │
//! │  (tool loop) │              │  (sergin-brain)   │
//! │              │     stdio     ├──────────────────┤
//! │              │──────────────│  MCP Server B     │
//! │              │              │  (filesystem)     │
//! └─────────────┘              └──────────────────┘
//! ```
//!
//! ## Security (SC7)
//!
//! - **Schema pinning**: SHA-256 hash at connect, warn on drift
//! - **Param sanitization**: blocklist regex before sending to servers
//! - **Response sanitization**: `<MCP_RESPONSE>` delimiters + truncation
//! - **Risk classification**: LOW/HIGH/CRITICAL per tool
//! - **Env isolation**: child processes get only configured env vars + PATH/HOME
//!
//! ## Config
//!
//! `.kira/mcp.toml`:
//! ```toml
//! [servers.brain]
//! command = "node"
//! args = ["dist/index.js"]
//! env = { BRAIN_DB = "/path/to/db" }
//!
//! [servers.filesystem]
//! command = "npx"
//! args = ["-y", "@anthropic/mcp-fs"]
//! risk_override = "high"
//! ```

pub mod client;
pub mod registry;
pub mod safety;
pub mod types;

pub use registry::McpRegistry;
pub use types::{McpConfig, ToolRisk};
