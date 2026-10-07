//! End-to-end tests for user settings endpoints
//!
//! Tests GET/PUT operations on user settings with typed settings.
//! Note: DELETE endpoint has been removed per the refactor plan.

mod common;

use common::{TestClient, TestServer, TEST_PASS, TEST_USER};
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn test_get_settings_empty_initially() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_user_settings().await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    let settings = body.get("settings").unwrap().as_array().unwrap();
    assert!(settings.is_empty());
}

#[tokio::test]
async fn test_get_settings_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.get_user_settings().await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_update_notify_whatsnew_true() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Set notify_whatsnew to true using typed setting
    let body = json!({
        "settings": [
            { "key": "notify_whatsnew", "value": true }
        ]
    });
    let response = client.update_user_settings_json(body).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify the setting was saved
    let response = client.get_user_settings().await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    let settings = body.get("settings").unwrap().as_array().unwrap();
    assert_eq!(settings.len(), 1);
    assert_eq!(settings[0]["key"], "notify_whatsnew");
    assert_eq!(settings[0]["value"], true);
}

#[tokio::test]
async fn test_update_notify_whatsnew_false() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Set notify_whatsnew to true first
    let body = json!({
        "settings": [
            { "key": "notify_whatsnew", "value": true }
        ]
    });
    client.update_user_settings_json(body).await;

    // Now set it to false
    let body = json!({
        "settings": [
            { "key": "notify_whatsnew", "value": false }
        ]
    });
    let response = client.update_user_settings_json(body).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify the setting was updated
    let response = client.get_user_settings().await;
    let body: serde_json::Value = response.json().await.unwrap();
    let settings = body.get("settings").unwrap().as_array().unwrap();
    assert_eq!(settings.len(), 1);
    assert_eq!(settings[0]["key"], "notify_whatsnew");
    assert_eq!(settings[0]["value"], false);
}

#[tokio::test]
async fn test_update_unknown_setting_key_returns_error() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Try to set an unknown setting key - should get 422 (Unprocessable Entity)
    let body = json!({
        "settings": [
            { "key": "unknown_key", "value": "some_value" }
        ]
    });
    let response = client.update_user_settings_json(body).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_invalid_value_type_returns_error() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Try to set notify_whatsnew with a string instead of bool
    let body = json!({
        "settings": [
            { "key": "notify_whatsnew", "value": "yes" }
        ]
    });
    let response = client.update_user_settings_json(body).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_settings_persist_across_sessions() {
    let server = TestServer::spawn().await;

    // User sets a value
    let client = TestClient::authenticated(server.base_url.clone()).await;
    let body = json!({
        "settings": [
            { "key": "notify_whatsnew", "value": true }
        ]
    });
    client.update_user_settings_json(body).await;

    // Logout
    client.logout().await;

    // Login again and verify settings persisted
    let client_again = TestClient::new(server.base_url.clone());
    client_again.login(TEST_USER, TEST_PASS).await;

    let response = client_again.get_user_settings().await;
    let body: serde_json::Value = response.json().await.unwrap();
    let settings = body.get("settings").unwrap().as_array().unwrap();
    assert_eq!(settings.len(), 1);
    assert_eq!(settings[0]["key"], "notify_whatsnew");
    assert_eq!(settings[0]["value"], true);
}

#[tokio::test]
async fn test_update_settings_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let body = json!({
        "settings": [
            { "key": "notify_whatsnew", "value": true }
        ]
    });
    let response = client.update_user_settings_json(body).await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_empty_settings_update_succeeds() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Empty settings array should succeed
    let body = json!({
        "settings": []
    });
    let response = client.update_user_settings_json(body).await;
    assert_eq!(response.status(), StatusCode::OK);
}
