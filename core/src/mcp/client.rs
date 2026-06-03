//! Single MCP server client wrapper.
//!
//! Wraps `rmcp` client with Kira-specific lifecycle management:
//! - Spawn child process via stdio transport
//! - Discover tools on connect
//! - Schema pinning (SC7)
//! - Tool call routing with param sanitization

use anyhow::{Context, Result};
use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use std::collections::HashMap;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use super::safety::{classify_tool_risk, compute_schema_hash, sanitize_response};
use super::types::{McpServerConfig, McpToolEntry, SchemaSnapshot};

/// A connected MCP server with its tool catalog.
pub struct McpClient {
    /// Server name from config.
    name: String,
    /// Config used to spawn this server.
    config: McpServerConfig,
    /// The rmcp service handle. `None` if disconnected.
    service: Option<rmcp::service::RunningService<rmcp::RoleClient, ()>>,
    /// Discovered tools.
    tools: Vec<McpToolEntry>,
    /// Schema snapshot taken at connect time (SC7 rug-pull detection).
    schema_snapshot: Option<SchemaSnapshot>,
}

impl McpClient {
    /// Connect to an MCP server by spawning its child process.
    pub async fn connect(name: String, config: McpServerConfig) -> Result<Self> {
        info!(
            server = %name,
            command = %config.command,
            args = ?config.args,
            "connecting to MCP server"
        );

        let env_vars: HashMap<String, String> = config.env.clone();
        let cmd_str = config.command.clone();
        let args_clone = config.args.clone();

        let transport = TokioChildProcess::new(
            Command::new(&cmd_str).configure(move |cmd| {
                for arg in &args_clone {
                    cmd.arg(arg);
                }
                // Isolated env: only pass configured vars + minimal PATH/HOME.
                cmd.env_clear();
                if let Ok(path) = std::env::var("PATH") {
                    cmd.env("PATH", path);
                }
                if let Ok(home) = std::env::var("HOME") {
                    cmd.env("HOME", home);
                }
                for (k, v) in &env_vars {
                    cmd.env(k, v);
                }
            }),
        )
        .context("failed to spawn MCP server child process")?;

        let service = ().serve(transport).await.with_context(|| {
            format!("failed to initialize MCP session with server '{name}'")
        })?;

        let peer_info = service.peer_info();
        info!(
            server = %name,
            peer = ?peer_info,
            "MCP server connected"
        );

        let mut client = Self {
            name,
            config,
            service: Some(service),
            tools: Vec::new(),
            schema_snapshot: None,
        };

        // Discover tools and pin schema.
        client.refresh_tools().await?;

        Ok(client)
    }

    /// Discover tools from the connected server and update the catalog.
    async fn refresh_tools(&mut self) -> Result<()> {
        let service = self
            .service
            .as_ref()
            .context("not connected")?;

        let raw_tools = service
            .list_all_tools()
            .await
            .context("failed to list MCP tools")?;

        let risk_override = self.config.risk_override;

        self.tools = raw_tools
            .into_iter()
            .map(|t| {
                let tool_name = t.name.to_string();
                let description = t.description.clone().unwrap_or_default().to_string();
                let input_schema: serde_json::Value =
                    serde_json::to_value(&t.input_schema).unwrap_or_default();
                let risk = risk_override
                    .unwrap_or_else(|| classify_tool_risk(&tool_name, &description, &input_schema));
                let namespaced = format!("mcp_{}_{}", self.name, tool_name);

                McpToolEntry {
                    server_name: self.name.clone(),
                    tool_name,
                    namespaced_name: namespaced,
                    description,
                    input_schema,
                    risk,
                }
            })
            .collect();

        // Pin schema on first discovery.
        let snapshot = compute_schema_hash(&self.tools);
        info!(
            server = %self.name,
            tool_count = self.tools.len(),
            schema_hash = %snapshot.hash,
            "MCP tools discovered and schema pinned"
        );
        self.schema_snapshot = Some(snapshot);

        Ok(())
    }

    /// Get the discovered tools.
    pub fn tools(&self) -> &[McpToolEntry] {
        &self.tools
    }

    /// Get the server name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Call a tool on this server.
    ///
    /// `tool_name` is the original (non-namespaced) name.
    /// `arguments` is the JSON params object.
    ///
    /// Returns the sanitized response string.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String> {
        let service = self
            .service
            .as_ref()
            .context("MCP server not connected")?;

        debug!(
            server = %self.name,
            tool = %tool_name,
            "calling MCP tool"
        );

        let mut params = CallToolRequestParams::new(tool_name.to_string());
        if let serde_json::Value::Object(map) = arguments {
            params = params.with_arguments(map);
        }

        let result = service
            .call_tool(params)
            .await
            .with_context(|| format!("MCP tool call failed: {tool_name}"))?;

        // Extract text content from the result.
        // Content = Annotated<RawContent>. We extract .raw and match on variant.
        let mut output_parts: Vec<String> = Vec::new();
        for content in &result.content {
            match &content.raw {
                rmcp::model::RawContent::Text(t) => {
                    output_parts.push(t.text.clone());
                }
                rmcp::model::RawContent::Image(_) => {
                    output_parts.push("[image content omitted]".into());
                }
                rmcp::model::RawContent::Audio(_) => {
                    output_parts.push("[audio content omitted]".into());
                }
                other => {
                    // Resource, ResourceLink — serialize as JSON for transparency.
                    output_parts.push(format!("{other:?}"));
                }
            }
        }
        let raw_output = output_parts.join("\n");

        // Sanitize: wrap in delimiters, truncate.
        let sanitized =
            sanitize_response(&self.name, &raw_output, self.config.max_response_bytes);

        Ok(sanitized)
    }

    /// Check for schema drift since initial connection (SC7 rug-pull detection).
    pub async fn check_schema_drift(&self) -> Result<bool> {
        if let Some(ref snapshot) = self.schema_snapshot {
            Ok(super::safety::check_schema_drift(snapshot, &self.tools))
        } else {
            Ok(false)
        }
    }

    /// Gracefully disconnect from the server.
    pub async fn disconnect(&mut self) {
        if let Some(service) = self.service.take() {
            info!(server = %self.name, "disconnecting MCP server");
            if let Err(e) = service.cancel().await {
                error!(
                    server = %self.name,
                    error = ?e,
                    "error disconnecting MCP server"
                );
            }
        }
        self.tools.clear();
        self.schema_snapshot = None;
    }

    /// Whether the client is connected.
    pub const fn is_connected(&self) -> bool {
        self.service.is_some()
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if self.service.is_some() {
            warn!(
                server = %self.name,
                "McpClient dropped while still connected — child process will be killed"
            );
        }
    }
}
