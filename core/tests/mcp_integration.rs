//! Integration test: MCP client connects to @modelcontextprotocol/server-everything,
//! discovers tools, and calls "echo".
//!
//! Requires: npx available in PATH.
//! Skip: set MCP_SKIP_INTEGRATION=1 to skip.

use kira_core::mcp::client::McpClient;
use kira_core::mcp::types::McpServerConfig;
use std::collections::HashMap;

fn should_skip() -> bool {
    std::env::var("MCP_SKIP_INTEGRATION").is_ok()
        || std::process::Command::new("npx")
            .arg("--version")
            .output()
            .is_err()
}

#[tokio::test]
async fn mcp_connect_and_list_tools() {
    if should_skip() {
        eprintln!("SKIP: MCP integration test (npx unavailable or MCP_SKIP_INTEGRATION set)");
        return;
    }

    let config = McpServerConfig {
        command: "npx".into(),
        args: vec![
            "-y".into(),
            "@modelcontextprotocol/server-everything".into(),
        ],
        env: HashMap::new(),
        auto_connect: true,
        risk_override: None,
        max_response_bytes: 50 * 1024,
    };

    let client = McpClient::connect("everything".into(), config)
        .await
        .expect("failed to connect to MCP server-everything");

    // Should have discovered tools.
    let tools = client.tools();
    assert!(!tools.is_empty(), "expected at least 1 tool from server-everything");

    // Should have "echo" tool.
    let echo = tools.iter().find(|t| t.tool_name == "echo");
    assert!(echo.is_some(), "expected 'echo' tool; got: {:?}",
        tools.iter().map(|t| &t.tool_name).collect::<Vec<_>>());

    // Namespaced correctly.
    let echo = echo.unwrap();
    assert_eq!(echo.namespaced_name, "mcp_everything_echo");
    assert_eq!(echo.server_name, "everything");

    eprintln!("MCP tools discovered: {}", tools.len());
    for t in tools {
        eprintln!("  {} (risk={:?})", t.namespaced_name, t.risk);
    }
}

#[tokio::test]
async fn mcp_call_echo_tool() {
    if should_skip() {
        eprintln!("SKIP: MCP integration test");
        return;
    }

    let config = McpServerConfig {
        command: "npx".into(),
        args: vec![
            "-y".into(),
            "@modelcontextprotocol/server-everything".into(),
        ],
        env: HashMap::new(),
        auto_connect: true,
        risk_override: None,
        max_response_bytes: 50 * 1024,
    };

    let client = McpClient::connect("everything".into(), config)
        .await
        .expect("failed to connect");

    let result = client
        .call_tool("echo", serde_json::json!({"message": "hello from kira"}))
        .await
        .expect("echo tool call failed");

    // Should contain our message in the response.
    assert!(
        result.contains("hello from kira"),
        "expected 'hello from kira' in response, got: {result}"
    );

    // Should be wrapped in MCP_RESPONSE delimiters.
    assert!(
        result.contains("<MCP_RESPONSE"),
        "expected MCP_RESPONSE delimiter in: {result}"
    );

    eprintln!("Echo response:\n{result}");
}
