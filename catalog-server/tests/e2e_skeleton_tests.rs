//! End-to-end tests for skeleton sync API endpoints
//!
//! Tests GET /v1/catalog/skeleton, GET /v1/catalog/skeleton/version,
//! and GET /v1/catalog/skeleton/delta endpoints.
//!
//! Note: TestServer::spawn() creates a catalog with fixtures:
//! 2 artists, 2 albums, 5 tracks. This means skeleton version will be 9
//! (one event per entity insertion) and the catalog is not empty.

mod common;

use common::{TestClient, TestServer};
use reqwest::StatusCode;

#[tokio::test]
async fn test_get_skeleton_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.get_skeleton().await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_skeleton_version_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.get_skeleton_version().await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_skeleton_delta_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.get_skeleton_delta(0).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_skeleton_version() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_skeleton_version().await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Check structure
    assert!(body.get("version").is_some());
    assert!(body.get("checksum").is_some());

    // Version should be 9 (2 artists + 2 albums + 5 tracks = 9 events)
    assert_eq!(body["version"], 9);
    // Checksum should be sha256 prefixed
    assert!(body["checksum"].as_str().unwrap().starts_with("sha256:"));
}

#[tokio::test]
async fn test_get_full_skeleton() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_skeleton().await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Check structure
    assert!(body.get("version").is_some());
    assert!(body.get("checksum").is_some());
    assert!(body.get("artists").is_some());
    assert!(body.get("albums").is_some());
    assert!(body.get("tracks").is_some());

    // Fixtures create 2 artists, 2 albums, 5 tracks
    let artists = body["artists"].as_array().unwrap();
    let albums = body["albums"].as_array().unwrap();
    let tracks = body["tracks"].as_array().unwrap();

    assert_eq!(artists.len(), 2, "Expected 2 artists in skeleton");
    assert_eq!(albums.len(), 2, "Expected 2 albums in skeleton");
    assert_eq!(tracks.len(), 5, "Expected 5 tracks in skeleton");

    // Albums should have artist_ids
    let first_album = &albums[0];
    assert!(first_album.get("id").is_some());
    assert!(first_album.get("artist_ids").is_some());

    // Tracks should have album_id
    let first_track = &tracks[0];
    assert!(first_track.get("id").is_some());
    assert!(first_track.get("album_id").is_some());
}

#[tokio::test]
async fn test_get_skeleton_delta_from_zero() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_skeleton_delta(0).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Check structure
    assert!(body.get("from_version").is_some());
    assert!(body.get("to_version").is_some());
    assert!(body.get("checksum").is_some());
    assert!(body.get("changes").is_some());

    // Delta from 0 should include all fixture events
    assert_eq!(body["from_version"], 0);
    assert_eq!(body["to_version"], 9);

    // Should have 9 changes (2 artists + 2 albums + 5 tracks)
    let changes = body["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 9);
}

#[tokio::test]
async fn test_get_skeleton_delta_from_current() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Request delta from current version (should be empty)
    let response = client.get_skeleton_delta(9).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    assert_eq!(body["from_version"], 9);
    assert_eq!(body["to_version"], 9);

    // No changes since we're already at current version
    let changes = body["changes"].as_array().unwrap();
    assert!(changes.is_empty());
}

#[tokio::test]
async fn test_skeleton_checksum_consistency() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Get version twice
    let response1 = client.get_skeleton_version().await;
    let body1: serde_json::Value = response1.json().await.unwrap();

    let response2 = client.get_skeleton_version().await;
    let body2: serde_json::Value = response2.json().await.unwrap();

    // Checksum should be consistent
    assert_eq!(body1["checksum"], body2["checksum"]);
    assert_eq!(body1["version"], body2["version"]);
}
