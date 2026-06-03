//! Integration tests for message submission and approval round-trip.
//!
//! Task 11.2 — validates Requirements 4.1, 4.2, 4.6, 7.1, 7.3, 7.4

use std::time::Duration;

use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, method, path},
};

use kira_cli::config::{Config, TimeConfig};
use kira_cli::core_client::CoreClient;
use kira_cli::messages::ApprovalRequest;

/// Helper: build a Config pointing at the given mock server URL.
fn test_config(base_url: &str) -> Config {
    Config {
        core_url: base_url.to_string(),
        time: TimeConfig {
            health_timeout_secs: 5,
            approval_timeout_secs: 120,
            request_timeout_secs: 10,
            health_interval_secs: 30,
        },
        ..Config::default()
    }
}

// ─── Sub-task 1: Mock POST /v1/chat returning 202 with agent_id ──────────────
// Validates: Requirements 4.1, 4.2

/// Test: POST /v1/chat returns 202 with agent_id → CoreClient returns ChatAccepted.
#[tokio::test]
async fn chat_submit_returns_accepted_with_agent_id() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/yolo"))
        .respond_with(
            ResponseTemplate::new(202)
                .set_body_json(serde_json::json!({"agent_id": "test-agent-123"})),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let cfg = test_config(&mock_server.uri());
    let client = CoreClient::new(&cfg).expect("client construction should succeed");

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        client.chat_submit(&[("user".to_string(), "hello world".to_string())], "standard"),
    )
    .await
    .expect("should not timeout")
    .expect("chat_submit should succeed");

    assert_eq!(result.agent_id, "test-agent-123");
}

// ─── Sub-task 2: Verify MessageAccepted received, poller spawned ─────────────
// Validates: Requirements 4.2, 4.6

/// Test: successful chat_submit means a poller can be spawned (agent_id is valid).
/// We verify the full round-trip: submit → get agent_id → poll agent status.
#[tokio::test]
async fn chat_submit_agent_id_enables_polling() {
    let mock_server = MockServer::start().await;

    // Mock chat submission
    Mock::given(method("POST"))
        .and(path("/v1/chat/yolo"))
        .respond_with(
            ResponseTemplate::new(202)
                .set_body_json(serde_json::json!({"agent_id": "poller-agent-456"})),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    // Mock agent status endpoint (simulates what the poller would call)
    Mock::given(method("GET"))
        .and(path("/v1/agents/poller-agent-456"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "poller-agent-456",
            "kind": "writer",
            "status": "running",
            "iterations": 3,
            "cost_usd": 0.01,
            "model": "deepseek-chat"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let cfg = test_config(&mock_server.uri());
    let client = CoreClient::new(&cfg).unwrap();

    // Submit message
    let accepted = client
        .chat_submit(&[("user".to_string(), "build feature X".to_string())], "standard")
        .await
        .unwrap();
    assert_eq!(accepted.agent_id, "poller-agent-456");

    // Simulate poller: use the agent_id to check status
    let status_json = client.agent_status(&accepted.agent_id).await.unwrap();
    assert!(status_json.contains("running"));
}

/// Test: chat_submit failure (non-2xx) returns an error.
#[tokio::test]
async fn chat_submit_failure_returns_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/yolo"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"error": "internal server error"})),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let cfg = test_config(&mock_server.uri());
    let client = CoreClient::new(&cfg).unwrap();

    let result = client.chat_submit(&[("user".to_string(), "hello".to_string())], "standard").await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("500") || err_msg.contains("chat submit failed"),
        "Error should indicate failure: {err_msg}"
    );
}

// ─── Sub-task 3: Mock approval event → verify modal state → mock POST approval ─
// Validates: Requirements 7.1, 7.3

/// Test: approval round-trip — POST /v1/agents/{id}/approval with approve=true.
#[tokio::test]
async fn approval_approve_round_trip() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/agents/test-agent-123/approval"))
        .and(body_json(serde_json::json!({
            "approved": true,
            "feedback": null
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let cfg = test_config(&mock_server.uri());
    let client = CoreClient::new(&cfg).unwrap();

    tokio::time::timeout(
        Duration::from_secs(5),
        client.agent_approve("test-agent-123", true, None),
    )
    .await
    .expect("should not timeout")
    .expect("approval should succeed");
}

/// Test: approval round-trip — POST /v1/agents/{id}/approval with reject + feedback.
#[tokio::test]
async fn approval_reject_with_feedback_round_trip() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/agents/test-agent-123/approval"))
        .and(body_json(serde_json::json!({
            "approved": false,
            "feedback": "too risky"
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let cfg = test_config(&mock_server.uri());
    let client = CoreClient::new(&cfg).unwrap();

    let result = client
        .agent_approve("test-agent-123", false, Some("too risky"))
        .await;

    assert!(result.is_ok());
}

/// Test: approval endpoint failure (e.g., agent not found) returns error.
#[tokio::test]
async fn approval_endpoint_failure_returns_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/agents/unknown-agent/approval"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(serde_json::json!({"error": "agent not found"})),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let cfg = test_config(&mock_server.uri());
    let client = CoreClient::new(&cfg).unwrap();

    let result = client.agent_approve("unknown-agent", true, None).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("404") || err_msg.contains("approval failed"),
        "Error should indicate 404: {err_msg}"
    );
}

// ─── Sub-task 4: Test timeout auto-reject path ──────────────────────────────
// Validates: Requirements 7.3, 7.4

/// Test: ApprovalRequest with timeout_secs=0 is immediately expired.
#[tokio::test]
async fn approval_timeout_zero_is_immediately_expired() {
    let request = ApprovalRequest {
        agent_id: "timeout-agent-789".to_string(),
        tool_name: "bash".to_string(),
        risk_level: "CRITICAL".to_string(),
        arguments_preview: "rm -rf /".to_string(),
        timeout_secs: 0,
        received_at: tokio::time::Instant::now(),
    };

    // With timeout_secs=0, is_expired() should return true immediately
    assert!(
        request.is_expired(),
        "timeout_secs=0 should be immediately expired"
    );
}

/// Test: ApprovalRequest with large timeout is not expired.
#[tokio::test]
async fn approval_large_timeout_not_expired() {
    let request = ApprovalRequest {
        agent_id: "patient-agent-000".to_string(),
        tool_name: "file_write".to_string(),
        risk_level: "HIGH".to_string(),
        arguments_preview: "write to /etc/hosts".to_string(),
        timeout_secs: 3600,
        received_at: tokio::time::Instant::now(),
    };

    assert!(
        !request.is_expired(),
        "3600s timeout should not be expired immediately"
    );
}

/// Test: timeout auto-reject sends {approved: false, feedback: null} to Core.
/// This simulates what the TUI does when an approval times out.
#[tokio::test]
async fn timeout_auto_reject_sends_correct_payload() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/agents/timeout-agent-789/approval"))
        .and(body_json(serde_json::json!({
            "approved": false,
            "feedback": null
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let cfg = test_config(&mock_server.uri());
    let client = CoreClient::new(&cfg).unwrap();

    // Simulate the auto-reject that happens on timeout
    let request = ApprovalRequest {
        agent_id: "timeout-agent-789".to_string(),
        tool_name: "bash".to_string(),
        risk_level: "CRITICAL".to_string(),
        arguments_preview: "rm -rf /".to_string(),
        timeout_secs: 0,
        received_at: tokio::time::Instant::now(),
    };

    // Verify it's expired
    assert!(request.is_expired());

    // Send auto-reject (approved=false, feedback=None) — same as TUI would do
    let result = client.agent_approve(&request.agent_id, false, None).await;

    assert!(result.is_ok(), "auto-reject should succeed");
}

/// Test: ApprovalRequest becomes expired after waiting past timeout.
#[tokio::test]
async fn approval_expires_after_timeout_elapses() {
    // Use a very short timeout (1 second)
    let request = ApprovalRequest {
        agent_id: "expiring-agent".to_string(),
        tool_name: "bash".to_string(),
        risk_level: "HIGH".to_string(),
        arguments_preview: "echo test".to_string(),
        timeout_secs: 1,
        received_at: tokio::time::Instant::now(),
    };

    // Should not be expired yet
    assert!(!request.is_expired());

    // Wait past the timeout
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Now it should be expired
    assert!(
        request.is_expired(),
        "should be expired after waiting past timeout_secs"
    );
}
