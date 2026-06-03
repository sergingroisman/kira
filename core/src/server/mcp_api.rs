//! MCP status endpoints.
//!
//! - `GET /v1/mcp/servers` — list connected MCP servers + tools

use axum::{extract::State, Json};
use serde::Serialize;

use crate::mcp::types::ToolRisk;
use super::AppState;

#[derive(Serialize)]
pub struct McpStatusResponse {
    pub servers: Vec<ServerInfo>,
    pub total_tools: usize,
}

#[derive(Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub tools: Vec<ToolInfo>,
}

#[derive(Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub namespaced_name: String,
    pub description: String,
    pub risk: String,
}

pub async fn servers(State(state): State<AppState>) -> Json<McpStatusResponse> {
    let entries = state.mcp.all_tool_entries().await;
    let connected = state.mcp.connected_servers().await;

    let mut servers: Vec<ServerInfo> = Vec::new();
    for server_name in &connected {
        let tools: Vec<ToolInfo> = entries
            .iter()
            .filter(|t| &t.server_name == server_name)
            .map(|t| ToolInfo {
                name: t.tool_name.clone(),
                namespaced_name: t.namespaced_name.clone(),
                description: t.description.clone(),
                risk: match t.risk {
                    ToolRisk::Low => "low".into(),
                    ToolRisk::High => "high".into(),
                    ToolRisk::Critical => "critical".into(),
                },
            })
            .collect();
        servers.push(ServerInfo {
            name: server_name.clone(),
            tools,
        });
    }

    let total_tools = entries.len();
    Json(McpStatusResponse {
        servers,
        total_tools,
    })
}
