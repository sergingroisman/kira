//! MCP configuration and type definitions.
//!
//! Config is loaded from `.kira/mcp.toml` (project-level) or
//! `~/.config/kira/mcp.toml` (global fallback).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level MCP configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct McpConfig {
    /// Named MCP servers to connect to on daemon startup.
    #[serde(default)]
    pub servers: HashMap<String, McpServerConfig>,
}

/// Configuration for a single MCP server.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerConfig {
    /// Command to spawn the server process (e.g. "npx", "python3", "node").
    pub command: String,

    /// Arguments to pass after the command.
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables to set for the child process.
    /// Isolated per server — does NOT inherit from parent process.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Whether to connect on daemon startup (default: true).
    #[serde(default = "default_true")]
    pub auto_connect: bool,

    /// Risk override for all tools from this server.
    /// If unset, tools are classified individually by schema analysis.
    pub risk_override: Option<ToolRisk>,

    /// Maximum response size in bytes before truncation (default: 50KB).
    #[serde(default = "default_max_response_bytes")]
    pub max_response_bytes: usize,
}

/// Tool risk classification (SC2/SC7) — re-export of the canonical type
/// defined in `crate::safety::risk` so MCP tools and built-in tools share
/// a single risk vocabulary.
pub use crate::safety::risk::ToolRisk;

/// A tool discovered from an MCP server, enriched with Kira metadata.
#[derive(Debug, Clone)]
pub struct McpToolEntry {
    /// Server name from config (e.g. "sergin-brain").
    pub server_name: String,
    /// Original tool name from the MCP server.
    pub tool_name: String,
    /// Kira-namespaced name: `mcp_{server}_{tool}` to avoid collisions.
    pub namespaced_name: String,
    /// Tool description from server.
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
    /// Risk classification.
    pub risk: ToolRisk,
}

/// Schema snapshot for rug-pull detection (SC7).
#[derive(Debug, Clone)]
pub struct SchemaSnapshot {
    /// SHA-256 hex digest of the canonical JSON representation of all tool schemas.
    pub hash: String,
    /// Timestamp when the snapshot was taken.
    pub taken_at: chrono::DateTime<chrono::Utc>,
    /// Number of tools in the snapshot.
    pub tool_count: usize,
}

const fn default_true() -> bool {
    true
}

const fn default_max_response_bytes() -> usize {
    50 * 1024
}
