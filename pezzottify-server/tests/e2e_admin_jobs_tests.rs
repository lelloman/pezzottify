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
async fn test_job_controls_reject_unauthenticated_and_non_admin_users() {
    let server = TestServer::builder().with_scheduler().spawn().await;

    let anonymous = TestClient::new(server.base_url.clone());
    assert_eq!(anonymous.admin_get_job_controls().await.status(), 401);
    assert_eq!(
        anonymous
            .admin_set_global_job_pause(true, false)
            .await
            .status(),
        401
    );

    let regular_user = TestClient::authenticated(server.base_url.clone()).await;
    assert_eq!(regular_user.admin_get_job_controls().await.status(), 403);
    assert_eq!(
        regular_user
            .admin_set_global_job_pause(true, false)
            .await
            .status(),
        403
    );
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

// ============================================================================
// Configured Scheduler Contract Tests
// ============================================================================

#[tokio::test]
async fn configured_scheduler_lists_registered_jobs() {
    let server = TestServer::builder().with_scheduler().spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.admin_list_jobs().await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"jobs": []})
    );
}

#[tokio::test]
async fn configured_scheduler_preserves_missing_job_contracts() {
    let server = TestServer::builder().with_scheduler().spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    assert_eq!(client.admin_get_job("missing").await.status(), 404);
    assert_eq!(client.admin_trigger_job("missing").await.status(), 404);
    let history = client.admin_get_job_history("missing", 10).await;
    assert_eq!(history.status(), 200);
    assert_eq!(
        history.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"history": []})
    );
}

#[tokio::test]
async fn configured_scheduler_exposes_and_persists_global_pause_control() {
    let server = TestServer::builder().with_scheduler().spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let initial = client.admin_get_job_controls().await;
    assert_eq!(initial.status(), 200);
    assert_eq!(
        initial.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({
            "global_paused": false,
            "paused_resource_classes": [],
            "paused_jobs": [],
        })
    );

    let paused = client.admin_set_global_job_pause(true, false).await;
    assert_eq!(paused.status(), 200);
    assert_eq!(
        paused.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({
            "global_paused": true,
            "paused_resource_classes": [],
            "paused_jobs": [],
        })
    );

    let persisted = server
        .server_store
        .get_state("background_jobs.pause_state.v1")
        .unwrap()
        .expect("pause state must be persisted");
    let persisted: serde_json::Value = serde_json::from_str(&persisted).unwrap();
    assert_eq!(persisted["global_paused"], true);

    let resumed = client.admin_set_global_job_pause(false, false).await;
    assert_eq!(resumed.status(), 200);
    assert_eq!(
        client
            .admin_get_job_controls()
            .await
            .json::<serde_json::Value>()
            .await
            .unwrap()["global_paused"],
        false
    );
}
