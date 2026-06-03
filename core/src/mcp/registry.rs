//! MCP server registry — manages multiple MCP client connections.
//!
//! Lifecycle:
//! 1. Load config from `.kira/mcp.toml`
//! 2. Connect to all `auto_connect=true` servers on daemon startup
//! 3. Aggregate tools from all connected servers into a unified catalog
//! 4. Route tool calls to the correct server by namespaced name

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::client::McpClient;
use super::types::{McpConfig, McpToolEntry};
use crate::llm::types::ToolDef;

/// Thread-safe registry of MCP server connections.
#[derive(Clone)]
pub struct McpRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

struct RegistryInner {
    /// Connected clients keyed by server name.
    clients: HashMap<String, McpClient>,
    /// Config for reconnection.
    config: McpConfig,
}

impl McpRegistry {
    /// Create an empty registry (no servers connected).
    pub fn empty() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                clients: HashMap::new(),
                config: McpConfig::default(),
            })),
        }
    }

    /// Load config and connect to all `auto_connect` servers.
    pub async fn from_config(config: McpConfig) -> Self {
        let registry = Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                clients: HashMap::new(),
                config: config.clone(),
            })),
        };

        for (name, server_cfg) in &config.servers {
            if !server_cfg.auto_connect {
                info!(server = %name, "MCP server skipped (auto_connect=false)");
                continue;
            }
            if let Err(e) = registry.connect(name.clone(), server_cfg.clone()).await {
                error!(
                    server = %name,
                    error = ?e,
                    "failed to connect MCP server — continuing without it"
                );
            }
        }

        registry
    }

    /// Load config from filesystem paths (project-level, then global fallback).
    pub async fn load_from_paths(repo_root: &Path) -> Self {
        // B2 — prefer unified kira-core.toml when present.
        let unified = repo_root.join("kira-core.toml");
        if unified.exists() {
            let cfg = crate::config::runtime::KiraCoreConfig::load_from_repo(repo_root);
            info!(
                servers = cfg.mcp.servers.len(),
                "MCP config loaded from kira-core.toml"
            );
            return Self::from_config(cfg.mcp).await;
        }

        let project_config = repo_root.join(".kira/mcp.toml");
        let global_config = dirs_path().map(|d| d.join("mcp.toml"));

        let config = if project_config.exists() {
            match load_config_file(&project_config) {
                Ok(c) => {
                    info!(path = %project_config.display(), "MCP config loaded (legacy .kira/mcp.toml)");
                    c
                }
                Err(e) => {
                    warn!(
                        path = %project_config.display(),
                        error = ?e,
                        "MCP config parse failed — no servers"
                    );
                    McpConfig::default()
                }
            }
        } else if let Some(ref gp) = global_config {
            if gp.exists() {
                match load_config_file(gp) {
                    Ok(c) => {
                        info!(path = %gp.display(), "MCP config loaded (global)");
                        c
                    }
                    Err(e) => {
                        warn!(path = %gp.display(), error = ?e, "MCP global config failed");
                        McpConfig::default()
                    }
                }
            } else {
                McpConfig::default()
            }
        } else {
            McpConfig::default()
        };

        Self::from_config(config).await
    }

    /// Connect to a specific MCP server.
    pub async fn connect(
        &self,
        name: String,
        config: super::types::McpServerConfig,
    ) -> Result<()> {
        let client = McpClient::connect(name.clone(), config).await?;
        let tool_count = client.tools().len();

        let mut inner = self.inner.write().await;
        inner.clients.insert(name.clone(), client);

        info!(
            server = %name,
            tools = tool_count,
            "MCP server registered"
        );
        Ok(())
    }

    /// Disconnect a specific server.
    pub async fn disconnect(&self, name: &str) -> bool {
        let mut inner = self.inner.write().await;
        if let Some(mut client) = inner.clients.remove(name) {
            client.disconnect().await;
            info!(server = %name, "MCP server disconnected");
            true
        } else {
            false
        }
    }

    /// Get all tools from all connected servers as Kira `ToolDef`s.
    /// These can be appended to `agent_tools()`.
    pub async fn all_tool_defs(&self) -> Vec<ToolDef> {
        let inner = self.inner.read().await;
        inner
            .clients
            .values()
            .flat_map(super::client::McpClient::tools)
            .map(mcp_tool_to_tool_def)
            .collect()
    }

    /// Get all MCP tool entries (with risk metadata).
    pub async fn all_tool_entries(&self) -> Vec<McpToolEntry> {
        let inner = self.inner.read().await;
        inner
            .clients
            .values()
            .flat_map(|c| c.tools().to_vec())
            .collect()
    }

    /// Route a tool call to the correct MCP server.
    ///
    /// `namespaced_name` is `mcp_{server}_{tool}`.
    /// Returns the sanitized response string.
    pub async fn call_tool(
        &self,
        namespaced_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String> {
        // Parse server name and tool name from the namespaced name.
        let (server_name, tool_name) = parse_namespaced(namespaced_name)
            .with_context(|| format!("invalid MCP tool name: {namespaced_name}"))?;

        let inner = self.inner.read().await;
        let client = inner
            .clients
            .get(&server_name)
            .with_context(|| format!("MCP server not connected: {server_name}"))?;

        client.call_tool(&tool_name, arguments).await
    }

    /// Check if a tool name belongs to an MCP server.
    pub async fn is_mcp_tool(&self, name: &str) -> bool {
        name.starts_with("mcp_")
    }

    /// Get the risk level of an MCP tool by its namespaced name.
    pub async fn tool_risk(&self, namespaced_name: &str) -> Option<super::types::ToolRisk> {
        let inner = self.inner.read().await;
        for client in inner.clients.values() {
            for tool in client.tools() {
                if tool.namespaced_name == namespaced_name {
                    return Some(tool.risk);
                }
            }
        }
        None
    }

    /// List connected server names.
    pub async fn connected_servers(&self) -> Vec<String> {
        let inner = self.inner.read().await;
        inner.clients.keys().cloned().collect()
    }

    /// Disconnect all servers.
    pub async fn shutdown(&self) {
        let mut inner = self.inner.write().await;
        for (name, mut client) in inner.clients.drain() {
            info!(server = %name, "shutting down MCP server");
            client.disconnect().await;
        }
    }
}

/// Convert `McpToolEntry` → Kira `ToolDef` for LLM function-calling.
fn mcp_tool_to_tool_def(entry: &McpToolEntry) -> ToolDef {
    let risk_note = match entry.risk {
        super::types::ToolRisk::Low => "",
        super::types::ToolRisk::High => " [HIGH RISK — may write files or send requests]",
        super::types::ToolRisk::Critical => {
            " [CRITICAL RISK — destructive/irreversible action]"
        }
    };

    ToolDef {
        name: entry.namespaced_name.clone(),
        description: format!(
            "[MCP:{server}] {desc}{risk}",
            server = entry.server_name,
            desc = entry.description,
            risk = risk_note,
        ),
        parameters: entry.input_schema.clone(),
        strict: false,
    }
}

/// Parse `mcp_{server}_{tool}` → `(server, tool)`.
///
/// Server names cannot contain underscores (enforced at config load).
/// Tool names may contain underscores.
fn parse_namespaced(name: &str) -> Option<(String, String)> {
    let rest = name.strip_prefix("mcp_")?;
    let underscore_pos = rest.find('_')?;
    let server = rest[..underscore_pos].to_string();
    let tool = rest[underscore_pos + 1..].to_string();
    if server.is_empty() || tool.is_empty() {
        return None;
    }
    Some((server, tool))
}

fn load_config_file(path: &Path) -> Result<McpConfig> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("parse {}", path.display()))
}

fn dirs_path() -> Option<std::path::PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| std::path::PathBuf::from(h).join(".config/kira"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_namespaced_basic() {
        let (s, t) = parse_namespaced("mcp_brain_retrieve").unwrap();
        assert_eq!(s, "brain");
        assert_eq!(t, "retrieve");
    }

    #[test]
    fn parse_namespaced_tool_with_underscore() {
        let (s, t) = parse_namespaced("mcp_brain_get_document").unwrap();
        assert_eq!(s, "brain");
        assert_eq!(t, "get_document");
    }

    #[test]
    fn parse_namespaced_invalid() {
        assert!(parse_namespaced("bash").is_none());
        assert!(parse_namespaced("mcp_").is_none());
        assert!(parse_namespaced("mcp__tool").is_none());
    }

    #[test]
    fn tool_def_conversion() {
        use super::super::types::ToolRisk;
        let entry = McpToolEntry {
            server_name: "brain".into(),
            tool_name: "search".into(),
            namespaced_name: "mcp_brain_search".into(),
            description: "Search knowledge base".into(),
            input_schema: serde_json::json!({"type": "object"}),
            risk: ToolRisk::Low,
        };
        let def = mcp_tool_to_tool_def(&entry);
        assert_eq!(def.name, "mcp_brain_search");
        assert!(def.description.contains("[MCP:brain]"));
    }
}
