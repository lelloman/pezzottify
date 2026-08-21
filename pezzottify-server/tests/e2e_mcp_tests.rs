mod common;

use common::{TestClient, TestServer, ADMIN_PASS, ADMIN_USER, TEST_PASS, TEST_USER};
use futures::{SinkExt, StreamExt};
use http::header;
use reqwest::StatusCode;
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};

type McpSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn login_token(server: &TestServer, user: &str, password: &str, device: &str) -> String {
    let client = TestClient::new(server.base_url.clone());
    let response = client.login_with_device(user, password, device).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let token = response
        .cookies()
        .find(|cookie| cookie.name() == "session_token")
        .unwrap()
        .value()
        .to_string();
    token
}

async fn connect_mcp(server: &TestServer, token: &str) -> McpSocket {
    let uri = server.base_url.replace("http://", "ws://") + "/v1/mcp";
    let request = http::Request::builder()
        .uri(uri)
        .header(header::COOKIE, format!("session_token={token}"))
        .header(header::HOST, "localhost")
        .header(header::CONNECTION, "Upgrade")
        .header(header::UPGRADE, "websocket")
        .header(header::SEC_WEBSOCKET_VERSION, "13")
        .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .body(())
        .unwrap();
    connect_async(request).await.unwrap().0
}

async fn request(socket: &mut McpSocket, value: Value) -> Value {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .unwrap();
    let message = tokio::time::timeout(std::time::Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    serde_json::from_str(message.to_text().unwrap()).unwrap()
}

async fn initialize(socket: &mut McpSocket) {
    let response = request(
        socket,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "contract-test", "version": "1"}
            }
        }),
    )
    .await;
    assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
}

#[tokio::test]
async fn mcp_requires_initialization_before_database_tools() {
    let server = TestServer::spawn().await;
    let token = login_token(&server, ADMIN_USER, ADMIN_PASS, "mcp-pre-init").await;
    let mut socket = connect_mcp(&server, &token).await;

    let response = request(
        &mut socket,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": "server.query", "arguments": {"query_type": "stats"}}
        }),
    )
    .await;
    assert_eq!(response["error"]["code"], -32600);
}

#[tokio::test]
async fn mcp_admin_database_tool_returns_catalog_and_user_stats() {
    let server = TestServer::spawn().await;
    let token = login_token(&server, ADMIN_USER, ADMIN_PASS, "mcp-admin-stats").await;
    let mut socket = connect_mcp(&server, &token).await;
    initialize(&mut socket).await;

    let response = request(
        &mut socket,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": "server.query", "arguments": {"query_type": "stats"}}
        }),
    )
    .await;
    assert!(response.get("error").is_none());
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    let stats: Value = serde_json::from_str(text).unwrap();
    // The lightweight integration fixture bypasses cardinality triggers while
    // seeding, so its cached catalog counters remain zero.
    assert_eq!(stats["catalog"]["artists"], 0);
    assert_eq!(stats["catalog"]["albums"], 0);
    assert_eq!(stats["catalog"]["tracks"], 0);
    assert_eq!(stats["users"]["total_users"], 2);
}

#[tokio::test]
async fn mcp_regular_user_can_search_but_cannot_query_server_stats() {
    let server = TestServer::spawn().await;
    let token = login_token(&server, TEST_USER, TEST_PASS, "mcp-regular").await;
    let mut socket = connect_mcp(&server, &token).await;
    initialize(&mut socket).await;

    let search = request(
        &mut socket,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": "catalog.search", "arguments": {"query": "test", "limit": 5}}
        }),
    )
    .await;
    assert!(search.get("error").is_none());

    let denied = request(
        &mut socket,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {"name": "server.query", "arguments": {"query_type": "stats"}}
        }),
    )
    .await;
    // Tools outside the caller's permission set are intentionally hidden.
    assert_eq!(denied["error"]["code"], -32601);
}
