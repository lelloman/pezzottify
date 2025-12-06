//! End-to-end tests for WebSocket sync broadcast
//!
//! Tests that sync events are broadcast to other devices via WebSocket.

mod common;

use common::{TestClient, TestServer, TEST_PASS, TEST_USER};
use futures::{SinkExt, StreamExt};
use http::header;
use reqwest::StatusCode;
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Helper to extract the session token from a login response cookies
async fn extract_session_token(response: reqwest::Response) -> String {
    response
        .cookies()
        .find(|c| c.name() == "session_token")
        .expect("Session token cookie not found")
        .value()
        .to_string()
}

/// Connect to WebSocket with authentication
async fn connect_ws(
    base_url: &str,
    session_token: &str,
) -> tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
> {
    // Convert http:// to ws://
    let ws_url = base_url.replace("http://", "ws://") + "/v1/ws";

    // Build request with cookie header
    let request = http::Request::builder()
        .uri(&ws_url)
        .header(header::COOKIE, format!("session_token={}", session_token))
        .header(header::HOST, "localhost")
        .header(header::CONNECTION, "Upgrade")
        .header(header::UPGRADE, "websocket")
        .header(header::SEC_WEBSOCKET_VERSION, "13")
        .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .body(())
        .expect("Failed to build WebSocket request");

    let (ws_stream, _) = connect_async(request)
        .await
        .expect("Failed to connect to WebSocket");

    ws_stream
}

/// Wait for a specific message type, timing out after duration
async fn wait_for_message(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    expected_type: &str,
    timeout_duration: Duration,
) -> Option<Value> {
    let result = timeout(timeout_duration, async {
        while let Some(Ok(msg)) = ws.next().await {
            if let Message::Text(text) = msg {
                if let Ok(json) = serde_json::from_str::<Value>(&text) {
                    if json.get("msg_type").and_then(|t| t.as_str()) == Some(expected_type) {
                        return Some(json);
                    }
                }
            }
        }
        None
    })
    .await;

    result.ok().flatten()
}

#[tokio::test]
async fn test_websocket_sync_broadcast_on_like() {
    let server = TestServer::spawn().await;

    // Create two clients for the same user but different devices
    let client1 = TestClient::new(server.base_url.clone());
    let response1 = client1
        .login_with_device(TEST_USER, TEST_PASS, "device-1-uuid")
        .await;
    assert_eq!(response1.status(), StatusCode::CREATED);
    let token1 = extract_session_token(response1).await;

    let client2 = TestClient::new(server.base_url.clone());
    let response2 = client2
        .login_with_device(TEST_USER, TEST_PASS, "device-2-uuid")
        .await;
    assert_eq!(response2.status(), StatusCode::CREATED);
    let token2 = extract_session_token(response2).await;

    // Connect device 2 to WebSocket
    let mut ws2 = connect_ws(&server.base_url, &token2).await;

    // Wait for the "connected" message on device 2
    let connected = wait_for_message(&mut ws2, "system.connected", Duration::from_secs(5)).await;
    assert!(connected.is_some(), "Should receive connected message");

    // Device 1 likes a track
    let response = client1.add_liked_content("track", "track-001").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Device 2 should receive sync event
    let sync_msg = wait_for_message(&mut ws2, "sync", Duration::from_secs(5)).await;
    assert!(sync_msg.is_some(), "Should receive sync message on device 2");

    let payload = sync_msg.unwrap();
    let event = payload.get("payload").and_then(|p| p.get("event"));
    assert!(event.is_some(), "Sync message should have event payload");

    let event = event.unwrap();
    assert_eq!(
        event.get("type").and_then(|t| t.as_str()),
        Some("content_liked")
    );
    assert_eq!(
        event
            .get("payload")
            .and_then(|p| p.get("content_id"))
            .and_then(|c| c.as_str()),
        Some("track-001")
    );

    // Cleanup
    ws2.close(None).await.ok();
}

#[tokio::test]
async fn test_websocket_sync_broadcast_on_unlike() {
    let server = TestServer::spawn().await;

    // Create two clients for the same user but different devices
    let client1 = TestClient::new(server.base_url.clone());
    let response1 = client1
        .login_with_device(TEST_USER, TEST_PASS, "device-3-uuid")
        .await;
    assert_eq!(response1.status(), StatusCode::CREATED);
    let token1 = extract_session_token(response1).await;

    let client2 = TestClient::new(server.base_url.clone());
    let response2 = client2
        .login_with_device(TEST_USER, TEST_PASS, "device-4-uuid")
        .await;
    assert_eq!(response2.status(), StatusCode::CREATED);
    let token2 = extract_session_token(response2).await;

    // Like a track first
    client1.add_liked_content("track", "track-002").await;

    // Connect device 2 to WebSocket
    let mut ws2 = connect_ws(&server.base_url, &token2).await;

    // Wait for the "connected" message
    wait_for_message(&mut ws2, "system.connected", Duration::from_secs(5)).await;

    // Device 1 unlikes the track
    let response = client1.remove_liked_content("track", "track-002").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Device 2 should receive sync event
    let sync_msg = wait_for_message(&mut ws2, "sync", Duration::from_secs(5)).await;
    assert!(
        sync_msg.is_some(),
        "Should receive sync message for unlike on device 2"
    );

    let payload = sync_msg.unwrap();
    let event = payload.get("payload").and_then(|p| p.get("event"));
    assert!(event.is_some());

    let event = event.unwrap();
    assert_eq!(
        event.get("type").and_then(|t| t.as_str()),
        Some("content_unliked")
    );

    ws2.close(None).await.ok();
}

#[tokio::test]
async fn test_websocket_sync_broadcast_on_playlist_create() {
    let server = TestServer::spawn().await;

    // Create two clients for the same user but different devices
    let client1 = TestClient::new(server.base_url.clone());
    let response1 = client1
        .login_with_device(TEST_USER, TEST_PASS, "device-5-uuid")
        .await;
    assert_eq!(response1.status(), StatusCode::CREATED);
    let token1 = extract_session_token(response1).await;

    let client2 = TestClient::new(server.base_url.clone());
    let response2 = client2
        .login_with_device(TEST_USER, TEST_PASS, "device-6-uuid")
        .await;
    assert_eq!(response2.status(), StatusCode::CREATED);
    let token2 = extract_session_token(response2).await;

    // Connect device 2 to WebSocket
    let mut ws2 = connect_ws(&server.base_url, &token2).await;

    // Wait for the "connected" message
    wait_for_message(&mut ws2, "system.connected", Duration::from_secs(5)).await;

    // Device 1 creates a playlist
    let response = client1.create_playlist("Test Playlist", vec![]).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Device 2 should receive sync event
    let sync_msg = wait_for_message(&mut ws2, "sync", Duration::from_secs(5)).await;
    assert!(
        sync_msg.is_some(),
        "Should receive sync message for playlist creation on device 2"
    );

    let payload = sync_msg.unwrap();
    let event = payload.get("payload").and_then(|p| p.get("event"));
    assert!(event.is_some());

    let event = event.unwrap();
    assert_eq!(
        event.get("type").and_then(|t| t.as_str()),
        Some("playlist_created")
    );
    assert_eq!(
        event
            .get("payload")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str()),
        Some("Test Playlist")
    );

    ws2.close(None).await.ok();
}

#[tokio::test]
async fn test_websocket_no_broadcast_to_source_device() {
    let server = TestServer::spawn().await;

    // Create a single client/device
    let client1 = TestClient::new(server.base_url.clone());
    let response1 = client1
        .login_with_device(TEST_USER, TEST_PASS, "device-7-uuid")
        .await;
    assert_eq!(response1.status(), StatusCode::CREATED);
    let token1 = extract_session_token(response1).await;

    // Connect to WebSocket
    let mut ws1 = connect_ws(&server.base_url, &token1).await;

    // Wait for the "connected" message
    wait_for_message(&mut ws1, "system.connected", Duration::from_secs(5)).await;

    // Like a track from the same device
    let response = client1.add_liked_content("track", "track-003").await;
    assert_eq!(response.status(), StatusCode::OK);

    // The source device should NOT receive its own sync event
    let sync_msg = wait_for_message(&mut ws1, "sync", Duration::from_millis(500)).await;
    assert!(
        sync_msg.is_none(),
        "Source device should NOT receive its own sync event"
    );

    ws1.close(None).await.ok();
}
