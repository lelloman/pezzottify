//! End-to-end tests for permission-based access control
//!
//! Tests that different user roles have appropriate access to endpoints.

mod common;

use common::{TestClient, TestServer, ARTIST_1_ID, TRACK_1_ID};
use reqwest::StatusCode;
use serde_json::json;

// =============================================================================
// Unauthenticated Access Tests
// =============================================================================

#[tokio::test]
async fn test_unauthenticated_cannot_access_catalog() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.get_artist(ARTIST_1_ID).await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_unauthenticated_cannot_access_catalog_stats() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.get_catalog_stats().await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_unauthenticated_cannot_stream() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.stream_track(TRACK_1_ID).await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_unauthenticated_cannot_like_content() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.add_liked_content("track", TRACK_1_ID).await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_unauthenticated_cannot_create_playlist() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.create_playlist("Test", vec![TRACK_1_ID]).await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_unauthenticated_can_access_statics() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    // Statics/home endpoint should be accessible without auth
    let response = client.get_statics().await;
    assert_eq!(response.status(), StatusCode::OK);
}

// =============================================================================
// Regular User Access Tests
// =============================================================================

#[tokio::test]
async fn test_regular_user_can_access_catalog() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_regular_user_can_access_catalog_stats() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_catalog_stats().await;
    assert!(
        response.status() == StatusCode::OK || response.status() == StatusCode::SERVICE_UNAVAILABLE,
        "Expected OK or SERVICE_UNAVAILABLE, got {}",
        response.status()
    );
}

#[tokio::test]
async fn test_regular_user_can_stream() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.stream_track(TRACK_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_regular_user_can_like_content() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.add_liked_content("track", TRACK_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_regular_user_can_create_playlist() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client
        .create_playlist("My Playlist", vec![TRACK_1_ID])
        .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_regular_user_cannot_edit_catalog() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Try to create an artist (requires EditCatalog permission)
    let response = client
        .client
        .post(format!("{}/v1/content/artist", server.base_url))
        .json(&json!({
            "id": "new-artist",
            "name": "New Artist",
            "genres": [],
            "activity_periods": []
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_regular_user_cannot_delete_catalog_item() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Try to delete an artist (requires EditCatalog permission)
    let response = client
        .client
        .delete(format!(
            "{}/v1/content/artist/{}",
            server.base_url, ARTIST_1_ID
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =============================================================================
// Admin User Access Tests
// =============================================================================

#[tokio::test]
async fn test_admin_can_access_catalog() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_admin_can_create_catalog_item() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Create a new artist
    let response = client
        .client
        .post(format!("{}/v1/content/artist", server.base_url))
        .json(&json!({
            "id": "new-artist-123",
            "name": "New Test Artist",
            "genres": ["rock"],
            "activity_periods": []
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_admin_can_update_catalog_item() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Update an existing artist from the test catalog
    // The test catalog has artist R1 ("The Test Band")
    let response = client
        .client
        .put(format!(
            "{}/v1/content/artist/{}",
            server.base_url, ARTIST_1_ID
        ))
        .json(&json!({
            "id": ARTIST_1_ID,
            "name": "Updated Artist Name",
            "genres": ["rock"],
            "activity_periods": []
        }))
        .send()
        .await
        .unwrap();

    // Admin should be able to update (not get FORBIDDEN)
    // 200 = success, 400 = validation error (not a permission issue)
    assert!(
        response.status() == StatusCode::OK || response.status() == StatusCode::BAD_REQUEST,
        "Expected OK or BAD_REQUEST (for validation), got FORBIDDEN which would indicate permission issue"
    );
}

#[tokio::test]
async fn test_admin_can_delete_catalog_item() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // First create an item to delete
    let response = client
        .client
        .post(format!("{}/v1/content/artist", server.base_url))
        .json(&json!({
            "id": "artist-to-delete",
            "name": "Artist To Delete",
            "genres": [],
            "activity_periods": []
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Now delete it
    let response = client
        .client
        .delete(format!(
            "{}/v1/content/artist/artist-to-delete",
            server.base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

// =============================================================================
// Cross-User Access Tests
// =============================================================================

#[tokio::test]
async fn test_user_cannot_access_other_users_playlist() {
    let server = TestServer::spawn().await;

    // Create playlist as regular user
    let client1 = TestClient::authenticated(server.base_url.clone()).await;
    let response = client1
        .create_playlist("Private Playlist", vec![TRACK_1_ID])
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let playlist_id: String = response.json().await.unwrap();

    // Try to access as admin user (different user)
    let client2 = TestClient::authenticated_admin(server.base_url.clone()).await;
    let response = client2.get_playlist(&playlist_id).await;

    // Should not find the playlist or be forbidden (belongs to different user)
    assert!(
        response.status() == StatusCode::NOT_FOUND || response.status() == StatusCode::FORBIDDEN,
        "Expected NOT_FOUND or FORBIDDEN, got {}",
        response.status()
    );
}

#[tokio::test]
async fn test_user_cannot_delete_other_users_playlist() {
    let server = TestServer::spawn().await;

    // Create playlist as regular user
    let client1 = TestClient::authenticated(server.base_url.clone()).await;
    let response = client1
        .create_playlist("Private Playlist", vec![TRACK_1_ID])
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let playlist_id: String = response.json().await.unwrap();

    // Try to delete as admin user (different user)
    let client2 = TestClient::authenticated_admin(server.base_url.clone()).await;
    let response = client2.delete_playlist(&playlist_id).await;

    // Should fail (not owner)
    assert!(
        response.status() == StatusCode::NOT_FOUND
            || response.status() == StatusCode::FORBIDDEN
            || response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

// =============================================================================
// Session Management Tests
// =============================================================================

#[tokio::test]
async fn test_logout_revokes_access() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Verify access works
    let response = client.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Logout
    let response = client.logout().await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify access is revoked
    let response = client.get_artist(ARTIST_1_ID).await;
    // 401 Unauthorized - session was cleared by logout
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_invalid_credentials_denied() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.login("nonexistent", "wrongpassword").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_wrong_password_denied() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.login(common::TEST_USER, "wrongpassword").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
