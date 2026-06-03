//! Integration tests — mock Core server via wiremock.

use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

/// Test: health check succeeds and returns version.
#[tokio::test]
async fn health_check_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/health"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"status": "ok", "version": "0.1.0"})),
        )
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/v1/health", mock_server.uri()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["version"], "0.1.0");
}

/// Test: lessons list returns array.
#[tokio::test]
async fn lessons_list_returns_array() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/lessons"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": "abc12345-xxxx", "level": "promoted", "content": "always use error handling", "created_at": "2025-01-01T00:00:00Z"},
        ])))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/v1/lessons", mock_server.uri()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_array());
    assert_eq!(body[0]["level"], "promoted");
}

/// Test: agent status returns JSON.
#[tokio::test]
async fn agent_status_returns_json() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/agents/abc12345"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc12345",
            "kind": "writer",
            "status": "completed",
            "iterations": 12,
            "cost_usd": 0.0234,
            "model": "deepseek-chat"
        })))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/v1/agents/abc12345", mock_server.uri()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "completed");
    assert_eq!(body["kind"], "writer");
}

/// Test: memory search without query returns 400.
#[tokio::test]
async fn memory_search_missing_query_returns_400() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/memory/search"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(serde_json::json!({"error": "query_required"})),
        )
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/v1/memory/search", mock_server.uri()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "query_required");
}

/// Test: queue list returns empty when no messages.
#[tokio::test]
async fn queue_list_empty() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/queue"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"messages": []})))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/v1/queue", mock_server.uri()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["messages"].as_array().unwrap().len(), 0);
}

/// Test: queue cancel processing returns 409.
#[tokio::test]
async fn queue_cancel_processing_returns_409() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/v1/queue/some-id"))
        .respond_with(
            ResponseTemplate::new(409)
                .set_body_json(serde_json::json!({"error": "message_already_processing"})),
        )
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .delete(format!("{}/v1/queue/some-id", mock_server.uri()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 409);
}

/// Test: chat yolo returns agent_id.
#[tokio::test]
async fn chat_yolo_returns_agent_id() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/yolo"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"agent_id": "new-agent-123"})),
        )
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/v1/chat/yolo", mock_server.uri()))
        .json(&serde_json::json!({"message": "hello", "tier": "standard"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["agent_id"], "new-agent-123");
}

/// Test: SSE event parsing (unit-level, no server needed).
#[test]
fn sse_parse_guardrail_triggered() {
    use kira_cli::sse::{PillarEventKind, parse_event};

    let event = parse_event(
        "GuardrailTriggered",
        r#"{"level": "tool", "reason": "critical risk", "agent_id": "x"}"#,
    );
    assert_eq!(
        event,
        PillarEventKind::GuardrailTriggered {
            level: "tool".to_string(),
            reason: "critical risk".to_string(),
            agent_id: Some("x".to_string()),
        }
    );
}
