//! End-to-end tests for Admin Jobs API
//!
//! Tests for `/v1/admin/jobs/*` endpoints.
//!
//! Note: The current test server doesn't have a scheduler configured,
//! so these tests verify proper error handling when no scheduler is available.
//! Tests for actual job operations are in the unit tests.

mod common;

use common::{TestClient, TestServer};

// ============================================================================
// Authorization Tests
// ============================================================================

// Note: Admin endpoints return 403 for both unauthenticated and non-admin users
// because the authorization check runs before the authentication check is fully
// evaluated. This is acceptable behavior - the key point is that the endpoint
// is protected.

#[tokio::test]
async fn test_list_jobs_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.admin_list_jobs().await;
    // 401 Unauthorized is returned for unauthenticated requests
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_list_jobs_rejects_non_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.admin_list_jobs().await;
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_get_job_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.admin_get_job("test_job").await;
    // 401 Unauthorized is returned for unauthenticated requests
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_get_job_rejects_non_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.admin_get_job("test_job").await;
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_trigger_job_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.admin_trigger_job("test_job").await;
    // 401 Unauthorized is returned for unauthenticated requests
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_trigger_job_rejects_non_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.admin_trigger_job("test_job").await;
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_get_job_history_rejects_unauthenticated() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.admin_get_job_history("test_job", 10).await;
    // 401 Unauthorized is returned for unauthenticated requests
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_get_job_history_rejects_non_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.admin_get_job_history("test_job", 10).await;
    assert_eq!(response.status(), 403);
}

// ============================================================================
// No Scheduler Configured Tests
// ============================================================================

#[tokio::test]
async fn test_list_jobs_returns_503_when_no_scheduler() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.admin_list_jobs().await;
    // Should return 503 Service Unavailable when scheduler is not configured
    assert_eq!(response.status(), 503);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("not available"));
}

#[tokio::test]
async fn test_get_job_returns_503_when_no_scheduler() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.admin_get_job("test_job").await;
    assert_eq!(response.status(), 503);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("not available"));
}

#[tokio::test]
async fn test_trigger_job_returns_503_when_no_scheduler() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.admin_trigger_job("test_job").await;
    assert_eq!(response.status(), 503);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("not available"));
}

#[tokio::test]
async fn test_get_job_history_returns_503_when_no_scheduler() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.admin_get_job_history("test_job", 10).await;
    assert_eq!(response.status(), 503);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("not available"));
}
