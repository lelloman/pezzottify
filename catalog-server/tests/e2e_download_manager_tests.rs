//! End-to-end tests for Download Manager API
//!
//! Tests for `/v1/download/*` endpoints.
//!
//! Note: The current test server doesn't have a download manager configured,
//! so these tests verify proper error handling when no download manager is available.
//! Tests for actual download operations are in the unit tests.

mod common;

use common::{TestClient, TestServer};

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
async fn test_download_admin_activity_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.download_admin_activity().await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), 401);
}

#[tokio::test]
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
