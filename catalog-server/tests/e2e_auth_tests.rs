//! End-to-end tests for authentication endpoints
//!
//! Tests login, logout, session management, and authentication requirements.

mod common;

use common::{TestClient, TestServer, ADMIN_PASS, ADMIN_USER, ARTIST_1_ID, TEST_PASS, TEST_USER};
use reqwest::StatusCode;

#[tokio::test]
async fn test_login_with_valid_credentials() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.login(TEST_USER, TEST_PASS).await;

    assert_eq!(response.status(), StatusCode::CREATED);

    // Verify session cookie is set
    // (reqwest client automatically handles cookies)
}

#[tokio::test]
async fn test_login_with_invalid_password() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.login(TEST_USER, "wrong_password").await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_login_with_nonexistent_user() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.login("nonexistent_user", "password").await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_logout_clears_session() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    // Login first
    let response = client.login(TEST_USER, TEST_PASS).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    // Verify we can access protected endpoint
    let response = client.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Logout
    let response = client.logout().await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify we can no longer access protected endpoint
    let response = client.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_protected_endpoint_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    // Try to access protected endpoint without logging in
    let response = client.get_artist(ARTIST_1_ID).await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_admin_user_can_login() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.login(ADMIN_USER, ADMIN_PASS).await;

    assert_eq!(response.status(), StatusCode::CREATED);

    // Verify admin can access protected endpoints
    let response = client.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_session_persists_across_requests() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    // Login
    let response = client.login(TEST_USER, TEST_PASS).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    // Make multiple requests with same client (session should persist)
    for _ in 0..5 {
        let response = client.get_artist(ARTIST_1_ID).await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn test_unauthenticated_statics_endpoint() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    // Statics endpoint should work without authentication
    let response = client.get_statics().await;

    assert_eq!(response.status(), StatusCode::OK);

    // Verify response contains expected fields
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body.get("uptime").is_some());
    assert!(body.get("hash").is_some());
}
