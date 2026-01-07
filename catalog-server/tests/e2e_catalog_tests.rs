//! End-to-end tests for catalog endpoints
//!
//! Tests artists, albums, tracks, images, and discography endpoints.

mod common;

use common::{
    TestClient, TestServer, ALBUM_1_ID, ALBUM_1_TITLE, ALBUM_2_ID, ALBUM_2_TITLE, ARTIST_1_ID,
    ARTIST_1_NAME, ARTIST_2_ID, ARTIST_2_NAME, TRACK_1_ID, TRACK_1_TITLE, TRACK_2_ID, TRACK_3_ID,
    TRACK_4_ID, TRACK_5_ID,
};
use reqwest::StatusCode;

// =============================================================================
// Artist Tests
// =============================================================================

#[tokio::test]
async fn test_get_artist_returns_correct_data() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_artist(ARTIST_1_ID).await;

    assert_eq!(response.status(), StatusCode::OK);

    // Note: Response is ResolvedArtist with nested artist field
    let resolved: serde_json::Value = response.json().await.unwrap();
    assert_eq!(resolved["artist"]["id"], ARTIST_1_ID);
    assert_eq!(resolved["artist"]["name"], ARTIST_1_NAME);
}

#[tokio::test]
async fn test_get_nonexistent_artist_returns_404() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_artist("nonexistent-artist").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_artist_discography() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_artist_discography(ARTIST_1_ID).await;

    assert_eq!(response.status(), StatusCode::OK);

    // Note: Response is ArtistDiscography with albums as full Album objects
    let discography: serde_json::Value = response.json().await.unwrap();
    let albums = discography["albums"].as_array().unwrap();
    let album_ids: Vec<String> = albums
        .iter()
        .map(|v| v["id"].as_str().unwrap().to_string())
        .collect();
    assert!(album_ids.contains(&ALBUM_1_ID.to_string()));
}

#[tokio::test]
async fn test_get_multiple_artists() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Get first artist
    let response = client.get_artist(ARTIST_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
    let artist1: serde_json::Value = response.json().await.unwrap();
    assert_eq!(artist1["artist"]["name"], ARTIST_1_NAME);

    // Get second artist
    let response = client.get_artist(ARTIST_2_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
    let artist2: serde_json::Value = response.json().await.unwrap();
    assert_eq!(artist2["artist"]["name"], ARTIST_2_NAME);
}

// =============================================================================
// Album Tests
// =============================================================================

#[tokio::test]
async fn test_get_album_returns_correct_data() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_album(ALBUM_1_ID).await;

    assert_eq!(response.status(), StatusCode::OK);

    // Note: New model returns Album directly (without artists/discs)
    let album: serde_json::Value = response.json().await.unwrap();
    assert_eq!(album["id"], ALBUM_1_ID);
    assert_eq!(album["name"], ALBUM_1_TITLE);
    // Spotify schema uses lowercase album_type
    assert_eq!(album["album_type"], "album");
}

#[tokio::test]
async fn test_get_nonexistent_album_returns_404() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_album("nonexistent-album").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_multiple_albums() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Get first album
    let response = client.get_album(ALBUM_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
    let album1: serde_json::Value = response.json().await.unwrap();
    assert_eq!(album1["name"], ALBUM_1_TITLE);

    // Get second album
    let response = client.get_album(ALBUM_2_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
    let album2: serde_json::Value = response.json().await.unwrap();
    assert_eq!(album2["name"], ALBUM_2_TITLE);
}

// =============================================================================
// Track Tests
// =============================================================================

#[tokio::test]
async fn test_get_track_returns_correct_data() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_track(TRACK_1_ID).await;

    assert_eq!(response.status(), StatusCode::OK);

    let track: serde_json::Value = response.json().await.unwrap();
    assert_eq!(track["id"], TRACK_1_ID);
    assert_eq!(track["name"], TRACK_1_TITLE);
}

#[tokio::test]
async fn test_get_resolved_track() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_resolved_track(TRACK_1_ID).await;

    assert_eq!(response.status(), StatusCode::OK);

    // ResolvedTrack has track, album, and artists (not tracks plural)
    let resolved: serde_json::Value = response.json().await.unwrap();
    assert!(resolved.get("track").is_some());
    assert!(resolved.get("album").is_some());
    assert!(resolved.get("artists").is_some());
    assert_eq!(resolved["track"]["id"], TRACK_1_ID);

    // Verify duration_ms is present and correct (test fixture sets 240000ms for TRACK_1)
    let duration = resolved["track"]["duration_ms"].as_i64();
    assert!(
        duration.is_some(),
        "duration_ms should be present in response"
    );
    assert_eq!(
        duration.unwrap(),
        240000,
        "duration_ms should be 240000ms (4 minutes)"
    );
}

#[tokio::test]
async fn test_get_nonexistent_track_returns_404() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_track("nonexistent-track").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_all_tracks() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Test catalog has 5 tracks
    for track_id in [TRACK_1_ID, TRACK_2_ID, TRACK_3_ID, TRACK_4_ID, TRACK_5_ID] {
        let response = client.get_track(track_id).await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}

// =============================================================================
// Image Tests
// =============================================================================

#[tokio::test]
async fn test_get_image_returns_image_data() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Image endpoint now takes item IDs (album or artist ID)
    let response = client.get_image(ALBUM_1_ID).await;

    assert_eq!(response.status(), StatusCode::OK);

    // Verify content-type is image
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.starts_with("image/"));

    // Verify we got image bytes
    let bytes = response.bytes().await.unwrap();
    assert!(bytes.len() > 0);
}

#[tokio::test]
async fn test_get_nonexistent_image_returns_404() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_image("nonexistent-image").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
