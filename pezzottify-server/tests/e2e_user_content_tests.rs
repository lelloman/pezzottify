//! End-to-end tests for user content endpoints
//!
//! Tests liked content and playlist functionality.

mod common;

use common::{TestClient, TestServer, ALBUM_1_ID, ARTIST_1_ID, TRACK_1_ID, TRACK_2_ID, TRACK_3_ID};
use reqwest::StatusCode;

// =============================================================================
// Liked Content Tests
// =============================================================================

#[tokio::test]
async fn test_like_and_unlike_track() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Like a track
    let response = client.add_liked_content("track", TRACK_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");

    // Verify it appears in liked tracks
    let response = client.get_liked_content("track").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let liked: Vec<String> = response.json().await.unwrap();
    assert!(liked.contains(&TRACK_1_ID.to_string()));

    // Unlike the track
    let response = client.remove_liked_content("track", TRACK_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify it's no longer in liked tracks
    let response = client.get_liked_content("track").await;
    assert_eq!(response.status(), StatusCode::OK);
    let liked: Vec<String> = response.json().await.unwrap();
    assert!(!liked.contains(&TRACK_1_ID.to_string()));
}

#[tokio::test]
async fn test_like_album() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Like an album
    let response = client.add_liked_content("album", ALBUM_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify it appears in liked albums
    let response = client.get_liked_content("album").await;
    assert_eq!(response.status(), StatusCode::OK);
    let liked: Vec<String> = response.json().await.unwrap();
    assert!(liked.contains(&ALBUM_1_ID.to_string()));
}

#[tokio::test]
async fn test_like_artist() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Like an artist
    let response = client.add_liked_content("artist", ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify it appears in liked artists
    let response = client.get_liked_content("artist").await;
    assert_eq!(response.status(), StatusCode::OK);
    let liked: Vec<String> = response.json().await.unwrap();
    assert!(liked.contains(&ARTIST_1_ID.to_string()));
}

#[tokio::test]
async fn test_like_multiple_tracks() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Like multiple tracks
    client.add_liked_content("track", TRACK_1_ID).await;
    client.add_liked_content("track", TRACK_2_ID).await;
    client.add_liked_content("track", TRACK_3_ID).await;

    // Verify all appear in liked tracks
    let response = client.get_liked_content("track").await;
    assert_eq!(response.status(), StatusCode::OK);
    let liked: Vec<String> = response.json().await.unwrap();
    assert!(liked.contains(&TRACK_1_ID.to_string()));
    assert!(liked.contains(&TRACK_2_ID.to_string()));
    assert!(liked.contains(&TRACK_3_ID.to_string()));
}

#[tokio::test]
async fn concurrent_user_writes_are_all_committed_and_visible() {
    let server = TestServer::spawn().await;
    let first = TestClient::authenticated_with_device(server.base_url.clone(), "writer-1").await;
    let second = TestClient::authenticated_with_device(server.base_url.clone(), "writer-2").await;
    let third = TestClient::authenticated_with_device(server.base_url.clone(), "writer-3").await;

    let (first_response, second_response, third_response) = tokio::join!(
        first.add_liked_content("track", TRACK_1_ID),
        second.add_liked_content("track", TRACK_2_ID),
        third.add_liked_content("track", TRACK_3_ID),
    );
    assert_eq!(first_response.status(), StatusCode::OK);
    assert_eq!(second_response.status(), StatusCode::OK);
    assert_eq!(third_response.status(), StatusCode::OK);

    let response = first.get_liked_content("track").await;
    assert_eq!(response.status(), StatusCode::OK);
    let liked: Vec<String> = response.json().await.unwrap();
    assert!(liked.contains(&TRACK_1_ID.to_string()));
    assert!(liked.contains(&TRACK_2_ID.to_string()));
    assert!(liked.contains(&TRACK_3_ID.to_string()));
}

#[tokio::test]
async fn test_get_liked_content_invalid_type() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Request invalid content type
    let response = client.get_liked_content("invalid").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_liked_content_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    // Try to like content without authentication
    let response = client.add_liked_content("track", TRACK_1_ID).await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// =============================================================================
// Playlist Tests
// =============================================================================

#[tokio::test]
async fn test_create_playlist() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a playlist
    let response = client
        .create_playlist("My Playlist", vec![TRACK_1_ID, TRACK_2_ID])
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Response should contain the playlist ID
    let playlist_id: String = response.json().await.unwrap();
    assert!(!playlist_id.is_empty());
}

#[tokio::test]
async fn test_get_playlists() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a playlist first
    let response = client
        .create_playlist("Test Playlist", vec![TRACK_1_ID])
        .await;
    let playlist_id: String = response.json().await.unwrap();

    // Get all playlists
    let response = client.get_playlists().await;
    assert_eq!(response.status(), StatusCode::OK);
    let playlists: Vec<String> = response.json().await.unwrap();
    assert!(playlists.contains(&playlist_id));
}

#[tokio::test]
async fn test_get_playlist_by_id() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a playlist
    let response = client
        .create_playlist("My Test Playlist", vec![TRACK_1_ID, TRACK_2_ID])
        .await;
    let playlist_id: String = response.json().await.unwrap();

    // Get the playlist by ID
    let response = client.get_playlist(&playlist_id).await;
    assert_eq!(response.status(), StatusCode::OK);

    let playlist: serde_json::Value = response.json().await.unwrap();
    assert_eq!(playlist["name"], "My Test Playlist");
    let tracks = playlist["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 2);
}

#[tokio::test]
async fn test_update_playlist_name() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a playlist
    let response = client
        .create_playlist("Original Name", vec![TRACK_1_ID])
        .await;
    let playlist_id: String = response.json().await.unwrap();

    // Update the name
    let response = client
        .update_playlist(&playlist_id, Some("New Name"), None)
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify the name changed
    let response = client.get_playlist(&playlist_id).await;
    let playlist: serde_json::Value = response.json().await.unwrap();
    assert_eq!(playlist["name"], "New Name");
}

#[tokio::test]
async fn test_update_playlist_tracks() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a playlist with one track
    let response = client.create_playlist("Test", vec![TRACK_1_ID]).await;
    let playlist_id: String = response.json().await.unwrap();

    // Update to have different tracks
    let response = client
        .update_playlist(&playlist_id, None, Some(vec![TRACK_2_ID, TRACK_3_ID]))
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify tracks changed
    let response = client.get_playlist(&playlist_id).await;
    let playlist: serde_json::Value = response.json().await.unwrap();
    let tracks = playlist["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 2);
}

#[tokio::test]
async fn test_delete_playlist() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a playlist
    let response = client.create_playlist("To Delete", vec![TRACK_1_ID]).await;
    let playlist_id: String = response.json().await.unwrap();

    // Delete it
    let response = client.delete_playlist(&playlist_id).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify it's gone
    let response = client.get_playlist(&playlist_id).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_missing_playlist_returns_stable_error_without_sync_event() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.delete_playlist("missing-playlist").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let header_request_id = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["code"], "playlist_not_found");
    assert_eq!(body["message"], "Playlist not found");
    assert_eq!(body["request_id"], header_request_id);

    let response = client.get_sync_events(0).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["current_seq"], 0);
    assert!(body["events"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_add_tracks_to_playlist() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a playlist with one track
    let response = client.create_playlist("Test", vec![TRACK_1_ID]).await;
    let playlist_id: String = response.json().await.unwrap();

    // Add more tracks
    let response = client
        .add_tracks_to_playlist(&playlist_id, vec![TRACK_2_ID, TRACK_3_ID])
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify tracks were added
    let response = client.get_playlist(&playlist_id).await;
    let playlist: serde_json::Value = response.json().await.unwrap();
    let tracks = playlist["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 3);
}

#[tokio::test]
async fn missing_track_add_does_not_mutate_playlist_or_append_sync_event() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;
    let response = client.create_playlist("Test", vec![]).await;
    let playlist_id: String = response.json().await.unwrap();

    let before: serde_json::Value = client.get_sync_events(0).await.json().await.unwrap();
    let current_seq = before["current_seq"].as_i64().unwrap();

    let response = client
        .add_tracks_to_playlist(&playlist_id, vec!["missing-track"])
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["code"], "invalid_request");

    let playlist: serde_json::Value = client
        .get_playlist(&playlist_id)
        .await
        .json()
        .await
        .unwrap();
    assert!(playlist["tracks"].as_array().unwrap().is_empty());
    let after: serde_json::Value = client
        .get_sync_events(current_seq)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(after["current_seq"], current_seq);
    assert!(after["events"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_remove_tracks_from_playlist() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a playlist with multiple tracks
    let response = client
        .create_playlist("Test", vec![TRACK_1_ID, TRACK_2_ID, TRACK_3_ID])
        .await;
    let playlist_id: String = response.json().await.unwrap();

    // Remove the middle track (position 1)
    let response = client
        .remove_tracks_from_playlist(&playlist_id, vec![1])
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify track was removed
    let response = client.get_playlist(&playlist_id).await;
    let playlist: serde_json::Value = response.json().await.unwrap();
    let tracks = playlist["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 2);
}

#[tokio::test]
async fn test_create_empty_playlist() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create an empty playlist
    let response = client.create_playlist("Empty Playlist", vec![]).await;
    assert_eq!(response.status(), StatusCode::OK);

    let playlist_id: String = response.json().await.unwrap();

    // Verify it exists and is empty
    let response = client.get_playlist(&playlist_id).await;
    let playlist: serde_json::Value = response.json().await.unwrap();
    let tracks = playlist["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 0);
}

#[tokio::test]
async fn test_playlist_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    // Try to create playlist without authentication
    let response = client.create_playlist("Test", vec![TRACK_1_ID]).await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_nonexistent_playlist() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_playlist("nonexistent-playlist-id").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["code"], "playlist_not_found");
}
