//! Integration tests for startup sequence and SSE reconnection.
//!
//! Uses `wiremock` to mock `/v1/health` and `/v1/events/stream`.
//! Validates Requirements: 2.1, 2.2, 2.3, 3.1, 3.4, 3.5

use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use kira_cli::config::{Config, TimeConfig};
use kira_cli::core_client::CoreClient;
use kira_cli::health::spawn_health_checker;
use kira_cli::messages::UiMessage;
use kira_cli::sse::spawn_listener;

/// Helper: build a Config pointing at the given mock server URL.
fn config_for(base_url: &str) -> Config {
    Config {
        core_url: base_url.to_string(),
        time: TimeConfig {
            health_timeout_secs: 5,
            approval_timeout_secs: 120,
            request_timeout_secs: 30,
            health_interval_secs: 30,
        },
        ..Config::default()
    }
}

/// Collect messages from the channel until timeout, returning all received.
async fn collect_messages(rx: &mut mpsc::Receiver<UiMessage>, timeout: Duration) -> Vec<UiMessage> {
    let mut messages = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(msg)) => messages.push(msg),
            _ => break,
        }
    }
    messages
}

/// Collect messages until a predicate is satisfied or timeout expires.
async fn collect_until(
    rx: &mut mpsc::Receiver<UiMessage>,
    timeout: Duration,
    predicate: impl Fn(&[UiMessage]) -> bool,
) -> Vec<UiMessage> {
    let mut messages = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if predicate(&messages) {
            break;
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(msg)) => messages.push(msg),
            _ => break,
        }
    }
    messages
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 1: health OK → SSE spawned → CoreConnected state
// Validates: Requirements 2.1, 2.2, 3.1
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_ok_then_sse_spawned_produces_core_connected() {
    let mock_server = MockServer::start().await;

    // Mock health endpoint returning OK
    Mock::given(method("GET"))
        .and(path("/v1/health"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"status": "ok", "version": "0.1.0"})),
        )
        .mount(&mock_server)
        .await;

    // Mock SSE endpoint returning a heartbeat event then keeping connection open
    Mock::given(method("GET"))
        .and(path("/v1/events/stream"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("Content-Type", "text/event-stream")
                .set_body_string("event: Heartbeat\ndata: {\"agent_id\": \"test\"}\n\n"),
        )
        .mount(&mock_server)
        .await;

    let cfg = config_for(&mock_server.uri());
    let client = CoreClient::new(&cfg).unwrap();

    let (tx, mut rx) = mpsc::channel::<UiMessage>(256);
    let cancel = CancellationToken::new();

    // Step 1: Spawn health checker (short interval for test speed)
    let _health_handle = spawn_health_checker(tx.clone(), client.clone(), 1, cancel.clone());

    // Step 2: Wait for CoreConnected from health checker
    let messages = collect_until(&mut rx, Duration::from_secs(5), |msgs| {
        msgs.iter()
            .any(|m| matches!(m, UiMessage::CoreConnected { .. }))
    })
    .await;

    assert!(
        messages
            .iter()
            .any(|m| matches!(m, UiMessage::CoreConnected { version } if version == "0.1.0")),
        "Expected CoreConnected with version 0.1.0, got: {messages:?}"
    );

    // Step 3: After health OK, spawn SSE listener (simulating event loop behavior)
    let _sse_handle = spawn_listener(tx.clone(), client.clone(), cancel.clone());

    // Step 4: Wait for SSE CoreConnected (SSE sends CoreConnected on successful connection)
    let sse_messages = collect_until(&mut rx, Duration::from_secs(5), |msgs| {
        msgs.iter()
            .any(|m| matches!(m, UiMessage::CoreConnected { .. }))
    })
    .await;

    assert!(
        sse_messages
            .iter()
            .any(|m| matches!(m, UiMessage::CoreConnected { .. })),
        "Expected CoreConnected from SSE listener, got: {sse_messages:?}"
    );

    // Cleanup
    cancel.cancel();
    tokio::time::sleep(Duration::from_millis(100)).await;
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 2: health fail → Disconnected → periodic retry → eventual connect
// Validates: Requirements 2.2, 2.3
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_fail_then_retry_produces_disconnected_then_connected() {
    let mock_server = MockServer::start().await;

    // Initially, health returns 503 (Core unavailable)
    let fail_mock = Mock::given(method("GET"))
        .and(path("/v1/health"))
        .respond_with(ResponseTemplate::new(503).set_body_string("service unavailable"))
        .expect(1..)
        .mount_as_scoped(&mock_server)
        .await;

    let cfg = config_for(&mock_server.uri());
    let client = CoreClient::new(&cfg).unwrap();

    let (tx, mut rx) = mpsc::channel::<UiMessage>(256);
    let cancel = CancellationToken::new();

    // Spawn health checker with 1s interval for fast testing
    let _health_handle = spawn_health_checker(tx.clone(), client.clone(), 1, cancel.clone());

    // Wait for CoreDisconnected
    let messages = collect_until(&mut rx, Duration::from_secs(3), |msgs| {
        msgs.iter()
            .any(|m| matches!(m, UiMessage::CoreDisconnected { .. }))
    })
    .await;

    assert!(
        messages
            .iter()
            .any(|m| matches!(m, UiMessage::CoreDisconnected { .. })),
        "Expected CoreDisconnected on health failure, got: {messages:?}"
    );

    // Now switch health to return OK (drop the scoped mock to remove the 503 response)
    drop(fail_mock);

    Mock::given(method("GET"))
        .and(path("/v1/health"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"status": "ok", "version": "0.2.0"})),
        )
        .mount(&mock_server)
        .await;

    // Wait for CoreConnected (health checker retries periodically)
    let recovery_messages = collect_until(&mut rx, Duration::from_secs(5), |msgs| {
        msgs.iter()
            .any(|m| matches!(m, UiMessage::CoreConnected { .. }))
    })
    .await;

    assert!(
        recovery_messages
            .iter()
            .any(|m| matches!(m, UiMessage::CoreConnected { version } if version == "0.2.0")),
        "Expected CoreConnected after recovery, got: {recovery_messages:?}"
    );

    // Cleanup
    cancel.cancel();
    tokio::time::sleep(Duration::from_millis(100)).await;
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 3: SSE drops → backoff → reconnect → backoff resets
// Validates: Requirements 3.1, 3.4, 3.5
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn sse_drops_then_reconnects_with_backoff() {
    let mock_server = MockServer::start().await;

    // First SSE connection: send one event then close the stream (simulating a drop)
    // The mock will respond to the first request, then the second request will also succeed
    // (simulating reconnection after drop).
    Mock::given(method("GET"))
        .and(path("/v1/events/stream"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("Content-Type", "text/event-stream")
                .set_body_string("event: QueueDepthChanged\ndata: {\"depth\": 3}\n\n"),
        )
        .expect(2..)
        .mount(&mock_server)
        .await;

    let cfg = config_for(&mock_server.uri());
    let client = CoreClient::new(&cfg).unwrap();

    let (tx, mut rx) = mpsc::channel::<UiMessage>(256);
    let cancel = CancellationToken::new();

    let start = tokio::time::Instant::now();

    // Spawn SSE listener
    let _sse_handle = spawn_listener(tx.clone(), client.clone(), cancel.clone());

    // The SSE listener should:
    // 1. Connect → receive CoreConnected + PillarEvent(QueueDepthChanged)
    // 2. Stream ends → CoreDisconnected
    // 3. Backoff (1s) → reconnect → CoreConnected + PillarEvent again
    //
    // We wait for at least 2 CoreConnected messages (initial + reconnect)
    let messages = collect_until(&mut rx, Duration::from_secs(10), |msgs| {
        let connected_count = msgs
            .iter()
            .filter(|m| matches!(m, UiMessage::CoreConnected { .. }))
            .count();
        connected_count >= 2
    })
    .await;

    let elapsed = start.elapsed();

    // Verify we got at least 2 CoreConnected (initial connect + reconnect)
    let connected_count = messages
        .iter()
        .filter(|m| matches!(m, UiMessage::CoreConnected { .. }))
        .count();
    assert!(
        connected_count >= 2,
        "Expected at least 2 CoreConnected (initial + reconnect), got {connected_count}. Messages: {messages:?}"
    );

    // Verify we got at least 1 CoreDisconnected (stream drop)
    let disconnected_count = messages
        .iter()
        .filter(|m| matches!(m, UiMessage::CoreDisconnected { .. }))
        .count();
    assert!(
        disconnected_count >= 1,
        "Expected at least 1 CoreDisconnected (stream drop), got {disconnected_count}. Messages: {messages:?}"
    );

    // Verify we got PillarEvent(QueueDepthChanged) at least once
    let pillar_count = messages
        .iter()
        .filter(|m| matches!(m, UiMessage::PillarEvent(_)))
        .count();
    assert!(
        pillar_count >= 1,
        "Expected at least 1 PillarEvent, got {pillar_count}. Messages: {messages:?}"
    );

    // Verify backoff: the reconnect should have taken at least ~1s (initial backoff)
    // The second CoreConnected should arrive after the first disconnect + 1s backoff
    assert!(
        elapsed >= Duration::from_millis(900),
        "Expected at least ~1s for backoff, but elapsed was {elapsed:?}"
    );

    // Cleanup
    cancel.cancel();
    tokio::time::sleep(Duration::from_millis(100)).await;
}
