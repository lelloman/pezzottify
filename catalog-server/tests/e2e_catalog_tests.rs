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

// =============================================================================
// Batch Content Tests
// =============================================================================

#[tokio::test]
async fn test_batch_content_returns_multiple_items() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client
        .post_batch_content(serde_json::json!({
            "artists": [{"id": ARTIST_1_ID, "resolved": true}],
            "albums": [{"id": ALBUM_1_ID, "resolved": true}],
            "tracks": [{"id": TRACK_1_ID, "resolved": true}]
        }))
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Verify artist result
    let artist_result = &body["artists"][ARTIST_1_ID];
    assert!(artist_result["ok"].is_object(), "Expected ok wrapper for artist");
    assert_eq!(artist_result["ok"]["artist"]["name"], ARTIST_1_NAME);

    // Verify album result
    let album_result = &body["albums"][ALBUM_1_ID];
    assert!(album_result["ok"].is_object(), "Expected ok wrapper for album");
    assert_eq!(album_result["ok"]["album"]["name"], ALBUM_1_TITLE);

    // Verify track result
    let track_result = &body["tracks"][TRACK_1_ID];
    assert!(track_result["ok"].is_object(), "Expected ok wrapper for track");
    assert_eq!(track_result["ok"]["track"]["name"], TRACK_1_TITLE);
}

#[tokio::test]
async fn test_batch_content_handles_not_found() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client
        .post_batch_content(serde_json::json!({
            "artists": [{"id": "nonexistent-artist", "resolved": true}],
            "albums": [],
            "tracks": []
        }))
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Verify error result for nonexistent artist
    let artist_result = &body["artists"]["nonexistent-artist"];
    assert_eq!(artist_result["error"], "not_found");
}

#[tokio::test]
async fn test_batch_content_mixed_success_and_failure() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client
        .post_batch_content(serde_json::json!({
            "artists": [
                {"id": ARTIST_1_ID, "resolved": true},
                {"id": "nonexistent-artist", "resolved": true}
            ],
            "albums": [],
            "tracks": []
        }))
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Verify existing artist succeeds
    assert!(body["artists"][ARTIST_1_ID]["ok"].is_object());

    // Verify nonexistent artist fails
    assert_eq!(body["artists"]["nonexistent-artist"]["error"], "not_found");
}

#[tokio::test]
async fn test_batch_content_empty_request() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client
        .post_batch_content(serde_json::json!({
            "artists": [],
            "albums": [],
            "tracks": []
        }))
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Verify empty maps returned
    assert!(body["artists"].as_object().unwrap().is_empty());
    assert!(body["albums"].as_object().unwrap().is_empty());
    assert!(body["tracks"].as_object().unwrap().is_empty());
}

#[tokio::test]
async fn test_batch_content_exceeds_limit() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a request with 101 items (exceeds 100 limit)
    let artists: Vec<serde_json::Value> = (0..101)
        .map(|i| serde_json::json!({"id": format!("artist-{}", i), "resolved": false}))
        .collect();

    let response = client
        .post_batch_content(serde_json::json!({
            "artists": artists,
            "albums": [],
            "tracks": []
        }))
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_batch_content_non_resolved() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client
        .post_batch_content(serde_json::json!({
            "artists": [],
            "albums": [{"id": ALBUM_1_ID, "resolved": false}],
            "tracks": []
        }))
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Non-resolved album should return basic Album (not ResolvedAlbum with discs)
    let album_result = &body["albums"][ALBUM_1_ID]["ok"];
    assert!(album_result.is_object());
    assert_eq!(album_result["name"], ALBUM_1_TITLE);
    // Non-resolved should NOT have discs field
    assert!(album_result.get("discs").is_none());
}

#[tokio::test]
async fn test_batch_content_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client
        .post_batch_content(serde_json::json!({
            "artists": [{"id": ARTIST_1_ID, "resolved": true}],
            "albums": [],
            "tracks": []
        }))
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
