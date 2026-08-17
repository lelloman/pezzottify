//! End-to-end tests for authentication endpoints
//!
//! Tests login, logout, session management, and authentication requirements.

mod common;

use common::{TestClient, TestServer, ADMIN_PASS, ADMIN_USER, ARTIST_1_ID, TEST_PASS, TEST_USER};
use reqwest::StatusCode;

#[tokio::test]
async fn test_login_sets_consistent_session_and_csrf_cookie_policy() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.login(TEST_USER, TEST_PASS).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let cookies: Vec<_> = response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap())
        .collect();
    assert_eq!(cookies.len(), 2);

    let session = cookies
        .iter()
        .find(|cookie| cookie.starts_with("session_token="))
        .expect("missing session cookie");
    assert!(session.contains("HttpOnly"));
    assert!(session.contains("SameSite=Lax"));
    assert!(session.contains("Path=/"));
    assert!(session.contains("Max-Age=604800"));

    let csrf = cookies
        .iter()
        .find(|cookie| cookie.starts_with("csrf_token="))
        .expect("missing CSRF cookie");
    assert!(!csrf.contains("HttpOnly"));
    assert!(csrf.contains("SameSite=Lax"));
    assert!(csrf.contains("Path=/"));
    assert!(csrf.contains("Max-Age=604800"));
}

#[tokio::test]
async fn test_cookie_authenticated_logout_requires_csrf_and_post() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());
    assert_eq!(
        client.login(TEST_USER, TEST_PASS).await.status(),
        StatusCode::CREATED
    );

    let response = client
        .client
        .post_without_csrf(format!("{}/v1/auth/logout", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let response = client
        .client
        .post_without_csrf(format!("{}/v1/auth/logout", server.base_url))
        .header("X-CSRF-Token", "attacker-controlled-token")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let response = client
        .client
        .get(format!("{}/v1/auth/logout", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);

    let response = client.logout().await;
    assert_eq!(response.status(), StatusCode::OK);
    let expired: Vec<_> = response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap())
        .collect();
    assert_eq!(expired.len(), 2);
    assert!(expired.iter().all(|cookie| cookie.contains("Max-Age=0")));
    assert!(expired.iter().all(|cookie| cookie.contains("Path=/")));
}

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
    // 401 Unauthorized - session was cleared by logout
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_endpoint_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    // Try to access protected endpoint without logging in
    let response = client.get_artist(ARTIST_1_ID).await;

    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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

// ==================== Device Entity Integration Tests ====================

#[tokio::test]
async fn test_login_with_device_info() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    // Login with device info
    let response = client
        .login_with_device(TEST_USER, TEST_PASS, "integration-test-device-uuid")
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    // Verify we can access protected endpoint
    let response = client.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_multiple_logins_same_device_reuse_record() {
    let server = TestServer::spawn().await;

    // First login with device
    let client1 = TestClient::new(server.base_url.clone());
    let response1 = client1
        .login_with_device(TEST_USER, TEST_PASS, "reuse-device-uuid")
        .await;
    assert_eq!(response1.status(), StatusCode::CREATED);

    // Logout
    client1.logout().await;

    // Second login with same device UUID
    let client2 = TestClient::new(server.base_url.clone());
    let response2 = client2
        .login_with_device(TEST_USER, TEST_PASS, "reuse-device-uuid")
        .await;
    assert_eq!(response2.status(), StatusCode::CREATED);

    // Both should succeed, device should be reused (not create duplicate)
    // We verify this works by checking we can make authenticated requests
    let response = client2.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_device_persists_across_logout_login() {
    let server = TestServer::spawn().await;
    let device_uuid = "persist-device-uuid";

    // First login
    let client = TestClient::new(server.base_url.clone());
    let response = client
        .login_with_device(TEST_USER, TEST_PASS, device_uuid)
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    // Logout
    let response = client.logout().await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify session is cleared
    let response = client.get_artist(ARTIST_1_ID).await;
    // 401 Unauthorized - session was cleared by logout
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Login again with same device
    let response = client
        .login_with_device(TEST_USER, TEST_PASS, device_uuid)
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    // Should work again
    let response = client.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_different_devices_for_same_user() {
    let server = TestServer::spawn().await;

    // Login from device 1
    let client1 = TestClient::new(server.base_url.clone());
    let response = client1
        .login_with_device(TEST_USER, TEST_PASS, "device-alpha")
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    // Login from device 2 (different device, same user)
    let client2 = TestClient::new(server.base_url.clone());
    let response = client2
        .login_with_device(TEST_USER, TEST_PASS, "device-beta")
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    // Both devices should work independently
    let response = client1.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = client2.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Logout from device 1 shouldn't affect device 2
    client1.logout().await;

    let response = client1.get_artist(ARTIST_1_ID).await;
    // 401 Unauthorized - session was cleared by logout
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response = client2.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
}
