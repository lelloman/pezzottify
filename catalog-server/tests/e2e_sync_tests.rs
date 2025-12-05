//! End-to-end tests for sync API endpoints
//!
//! Tests GET /v1/sync/state and GET /v1/sync/events endpoints.

mod common;

use common::{TestClient, TestServer, TRACK_1_ID, TRACK_2_ID};
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn test_get_sync_state_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.get_sync_state().await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_sync_state_empty_user() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_sync_state().await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Check structure
    assert!(body.get("seq").is_some());
    assert!(body.get("likes").is_some());
    assert!(body.get("settings").is_some());
    assert!(body.get("playlists").is_some());
    assert!(body.get("permissions").is_some());

    // Initial state should be empty (seq 0, no likes, no settings, no playlists)
    assert_eq!(body["seq"], 0);
    assert!(body["likes"]["albums"].as_array().unwrap().is_empty());
    assert!(body["likes"]["artists"].as_array().unwrap().is_empty());
    assert!(body["likes"]["tracks"].as_array().unwrap().is_empty());
    assert!(body["settings"].as_array().unwrap().is_empty());
    assert!(body["playlists"].as_array().unwrap().is_empty());
    // Permissions should have at least some permissions for regular user
    assert!(!body["permissions"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_get_sync_state_with_liked_content() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Like some content
    let response = client.add_liked_content("track", "track-001").await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = client.add_liked_content("album", "album-001").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Get sync state
    let response = client.get_sync_state().await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Should have liked content
    let liked_tracks = body["likes"]["tracks"].as_array().unwrap();
    let liked_albums = body["likes"]["albums"].as_array().unwrap();

    assert!(liked_tracks.contains(&json!("track-001")));
    assert!(liked_albums.contains(&json!("album-001")));

    // Seq should be > 0 since we logged events
    assert!(body["seq"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn test_get_sync_state_with_settings() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Set a setting
    let settings_body = json!({
        "settings": [
            { "key": "enable_direct_downloads", "value": true }
        ]
    });
    let response = client.update_user_settings_json(settings_body).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Get sync state
    let response = client.get_sync_state().await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Should have the setting
    let settings = body["settings"].as_array().unwrap();
    assert_eq!(settings.len(), 1);
    assert_eq!(settings[0]["key"], "enable_direct_downloads");
    assert_eq!(settings[0]["value"], true);
}

#[tokio::test]
async fn test_get_sync_state_with_playlist() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a playlist
    let response = client
        .create_playlist("My Playlist", vec!["track-001", "track-002"])
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Get sync state
    let response = client.get_sync_state().await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Should have the playlist
    let playlists = body["playlists"].as_array().unwrap();
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0]["name"], "My Playlist");

    let tracks = playlists[0]["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 2);
    assert!(tracks.contains(&json!("track-001")));
    assert!(tracks.contains(&json!("track-002")));
}

#[tokio::test]
async fn test_get_sync_events_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.get_sync_events(0).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_sync_events_empty() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_sync_events(0).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Should have empty events and current_seq = 0
    assert_eq!(body["current_seq"], 0);
    assert!(body["events"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_get_sync_events_after_like() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Like a track
    let response = client.add_liked_content("track", "track-001").await;
    assert_eq!(response.status(), StatusCode::OK);

    // Get events since 0
    let response = client.get_sync_events(0).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Should have one event
    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);

    // Event should be content_liked (snake_case from serde)
    assert_eq!(events[0]["type"], "content_liked");
    assert_eq!(events[0]["payload"]["content_id"], "track-001");
    assert_eq!(events[0]["payload"]["content_type"], "track");

    // Current seq should match the event's seq
    assert_eq!(body["current_seq"], events[0]["seq"]);
}

#[tokio::test]
async fn test_get_sync_events_after_unlike() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Like then unlike
    client.add_liked_content("track", "track-001").await;
    client.remove_liked_content("track", "track-001").await;

    // Get events since 0
    let response = client.get_sync_events(0).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Should have two events
    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);

    // First event: content_liked
    assert_eq!(events[0]["type"], "content_liked");

    // Second event: content_unliked
    assert_eq!(events[1]["type"], "content_unliked");
}

#[tokio::test]
async fn test_get_sync_events_incremental() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Like first track
    client.add_liked_content("track", "track-001").await;

    // Get state to get current seq
    let response = client.get_sync_state().await;
    let body: serde_json::Value = response.json().await.unwrap();
    let seq_after_first = body["seq"].as_i64().unwrap();

    // Like second track
    client.add_liked_content("track", "track-002").await;

    // Get events since first seq
    let response = client.get_sync_events(seq_after_first).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Should only have the second event
    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["payload"]["content_id"], "track-002");
}

#[tokio::test]
async fn test_get_sync_events_setting_changed() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Change a setting
    let settings_body = json!({
        "settings": [
            { "key": "enable_direct_downloads", "value": true }
        ]
    });
    client.update_user_settings_json(settings_body).await;

    // Get events
    let response = client.get_sync_events(0).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["type"], "setting_changed");
}

#[tokio::test]
async fn test_get_sync_events_playlist_created() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a playlist
    client.create_playlist("Test Playlist", vec![]).await;

    // Get events
    let response = client.get_sync_events(0).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["type"], "playlist_created");
    assert_eq!(events[0]["payload"]["name"], "Test Playlist");
}

#[tokio::test]
async fn test_get_sync_events_playlist_deleted() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a playlist
    let response = client.create_playlist("Test Playlist", vec![]).await;
    let playlist_id: String = response.json().await.unwrap();

    // Delete it
    client.delete_playlist(&playlist_id).await;

    // Get events
    let response = client.get_sync_events(0).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["type"], "playlist_created");
    assert_eq!(events[1]["type"], "playlist_deleted");
}

#[tokio::test]
async fn test_get_sync_events_playlist_renamed() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create and rename a playlist
    let response = client.create_playlist("Old Name", vec![]).await;
    let playlist_id: String = response.json().await.unwrap();
    client.update_playlist(&playlist_id, Some("New Name"), None).await;

    // Get events
    let response = client.get_sync_events(0).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["type"], "playlist_renamed");
    assert_eq!(events[1]["payload"]["name"], "New Name");
}

#[tokio::test]
async fn test_get_sync_events_playlist_tracks_updated() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a playlist and add tracks (using valid test track IDs)
    let response = client.create_playlist("Test", vec![]).await;
    let playlist_id: String = response.json().await.unwrap();
    let response = client.add_tracks_to_playlist(&playlist_id, vec![TRACK_1_ID]).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Get events
    let response = client.get_sync_events(0).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["type"], "playlist_tracks_updated");
}
