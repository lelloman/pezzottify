//! End-to-end tests for Download Manager API
//!
//! Tests for `/v1/download/*` endpoints.
//!
//! Covers authorization, disabled-manager behavior, and enabled queue lifecycle contracts.

mod common;

use common::{TestClient, TestServer, ALBUM_1_ID, ALBUM_1_TITLE, ARTIST_1_NAME};

#[tokio::test]
async fn enabled_manager_preserves_queue_limits_audit_and_delete_contracts() {
    let server = TestServer::builder().with_download_manager().spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let limits = client.download_limits().await;
    assert_eq!(limits.status(), 200);
    let limits: serde_json::Value = limits.json().await.unwrap();
    assert_eq!(limits["can_request"], true);

    let created = client
        .download_request_album(ALBUM_1_ID, ALBUM_1_TITLE, ARTIST_1_NAME)
        .await;
    assert_eq!(created.status(), 200);
    let created: serde_json::Value = created.json().await.unwrap();
    let request_id = created["request_id"].as_str().unwrap();
    assert!(!request_id.is_empty());
    assert_eq!(created["status"], "PENDING");

    let duplicate = client
        .download_request_album(ALBUM_1_ID, ALBUM_1_TITLE, ARTIST_1_NAME)
        .await;
    assert_eq!(duplicate.status(), 400);

    let mine: serde_json::Value = client.download_my_requests().await.json().await.unwrap();
    assert_eq!(mine["requests"].as_array().unwrap().len(), 1);
    assert_eq!(mine["requests"][0]["id"], request_id);
    assert_eq!(mine["requests"][0]["content_id"], ALBUM_1_ID);

    let stats: serde_json::Value = client.download_admin_stats().await.json().await.unwrap();
    assert_eq!(stats["queue"]["pending"], 1);

    let requests: serde_json::Value = client.download_admin_requests().await.json().await.unwrap();
    assert_eq!(requests.as_array().unwrap().len(), 1);
    assert_eq!(requests[0]["id"], request_id);

    let audit: serde_json::Value = client
        .download_admin_audit_item(request_id)
        .await
        .json()
        .await
        .unwrap();
    assert!(audit["total_count"].as_u64().unwrap() >= 1);

    let deleted = client.download_admin_delete(request_id).await;
    assert_eq!(deleted.status(), 200);
    let requests: serde_json::Value = client.download_admin_requests().await.json().await.unwrap();
    assert!(requests.as_array().unwrap().is_empty());
}

// ============================================================================
// User Endpoint Authorization Tests
// ============================================================================

#[tokio::test]
async fn test_download_limits_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.download_limits().await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_download_my_requests_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.download_my_requests().await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_download_request_album_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client
        .download_request_album("test-album-id", "Test Album", "Test Artist")
        .await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), 401);
}

// ============================================================================
// Admin Endpoint Authorization Tests
// ============================================================================

#[tokio::test]
async fn test_download_admin_stats_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.download_admin_stats().await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_download_admin_stats_rejects_non_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.download_admin_stats().await;
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_download_admin_failed_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.download_admin_failed().await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_download_admin_failed_rejects_non_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.download_admin_failed().await;
    assert_eq!(response.status(), 403);
}

#[tokio::test]
#[ignore = "The legacy admin activity route is not implemented"]
async fn test_download_admin_activity_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.download_admin_activity().await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), 401);
}

#[tokio::test]
#[ignore = "The legacy admin activity route is not implemented"]
async fn test_download_admin_activity_rejects_non_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.download_admin_activity().await;
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_download_admin_requests_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.download_admin_requests().await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_download_admin_requests_rejects_non_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.download_admin_requests().await;
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_download_admin_retry_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.download_admin_retry("test-id").await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_download_admin_retry_rejects_non_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.download_admin_retry("test-id").await;
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_download_admin_audit_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.download_admin_audit().await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_download_admin_audit_rejects_non_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.download_admin_audit().await;
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_download_admin_audit_item_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.download_admin_audit_item("test-id").await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_download_admin_audit_item_rejects_non_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.download_admin_audit_item("test-id").await;
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_download_admin_audit_user_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.download_admin_audit_user("test-user").await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_download_admin_audit_user_rejects_non_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.download_admin_audit_user("test-user").await;
    assert_eq!(response.status(), 403);
}

// ============================================================================
// User Endpoints Permission Tests
// ============================================================================
// Note: User download endpoints require RequestContent permission.
// Regular authenticated users (without this permission) get 403 Forbidden.
// Admin users have RequestContent permission implicitly.

#[tokio::test]
async fn test_download_limits_rejects_user_without_request_content_permission() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.download_limits().await;
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_download_my_requests_rejects_user_without_request_content_permission() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.download_my_requests().await;
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_download_request_album_rejects_user_without_request_content_permission() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client
        .download_request_album("test-album-id", "Test Album", "Test Artist")
        .await;
    assert_eq!(response.status(), 403);
}

// ============================================================================
// No Download Manager Configured Tests (Admin has RequestContent permission)
// ============================================================================

#[tokio::test]
async fn test_download_limits_returns_503_when_not_configured() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.download_limits().await;
    assert_eq!(response.status(), 503);
}

#[tokio::test]
async fn test_download_my_requests_returns_503_when_not_configured() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.download_my_requests().await;
    assert_eq!(response.status(), 503);
}

#[tokio::test]
async fn test_download_request_album_returns_503_when_not_configured() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client
        .download_request_album("test-album-id", "Test Album", "Test Artist")
        .await;
    assert_eq!(response.status(), 503);
}

// ============================================================================
// No Download Manager Configured Tests (Admin Endpoints)
// ============================================================================

#[tokio::test]
async fn test_download_admin_stats_returns_503_when_not_configured() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.download_admin_stats().await;
    assert_eq!(response.status(), 503);
}

#[tokio::test]
async fn test_download_admin_failed_returns_503_when_not_configured() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.download_admin_failed().await;
    assert_eq!(response.status(), 503);
}

#[tokio::test]
#[ignore = "The legacy admin activity route is not implemented"]
async fn test_download_admin_activity_returns_503_when_not_configured() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.download_admin_activity().await;
    assert_eq!(response.status(), 503);
}

#[tokio::test]
async fn test_download_admin_requests_returns_503_when_not_configured() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.download_admin_requests().await;
    assert_eq!(response.status(), 503);
}

#[tokio::test]
async fn test_download_admin_retry_returns_503_when_not_configured() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.download_admin_retry("test-id").await;
    assert_eq!(response.status(), 503);
}

#[tokio::test]
async fn test_download_admin_audit_returns_503_when_not_configured() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.download_admin_audit().await;
    assert_eq!(response.status(), 503);
}

#[tokio::test]
async fn test_download_admin_audit_item_returns_503_when_not_configured() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.download_admin_audit_item("test-id").await;
    assert_eq!(response.status(), 503);
}

#[tokio::test]
async fn test_download_admin_audit_user_returns_503_when_not_configured() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.download_admin_audit_user("test-user").await;
    assert_eq!(response.status(), 503);
}
