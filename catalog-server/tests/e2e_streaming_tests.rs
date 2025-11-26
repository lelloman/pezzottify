//! End-to-end tests for audio streaming endpoints
//!
//! Tests track streaming, range requests, and HTTP range header support.

mod common;

use common::{TestClient, TestServer, TEST_AUDIO_SIZE_BYTES, TRACK_1_ID, TRACK_2_ID};
use reqwest::StatusCode;

#[tokio::test]
async fn test_stream_track_returns_audio_data() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.stream_track(TRACK_1_ID).await;

    assert_eq!(response.status(), StatusCode::OK);

    // Verify content-type is audio
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        content_type.starts_with("audio/"),
        "Expected audio content-type, got: {}",
        content_type
    );

    // Verify we got audio bytes
    let bytes = response.bytes().await.unwrap();
    assert!(bytes.len() > 0);
    assert!(
        bytes.len() <= TEST_AUDIO_SIZE_BYTES + 1000,
        "Expected ~{} bytes, got {}",
        TEST_AUDIO_SIZE_BYTES,
        bytes.len()
    );
}

#[tokio::test]
async fn test_stream_nonexistent_track_returns_404() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.stream_track("nonexistent-track").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_stream_track_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.stream_track(TRACK_1_ID).await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_stream_multiple_tracks() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Stream first track
    let response = client.stream_track(TRACK_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes1 = response.bytes().await.unwrap();

    // Stream second track
    let response = client.stream_track(TRACK_2_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes2 = response.bytes().await.unwrap();

    // Both should return data
    assert!(bytes1.len() > 0);
    assert!(bytes2.len() > 0);
}

// =============================================================================
// Range Request Tests
// =============================================================================

#[tokio::test]
async fn test_stream_track_with_range_request() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client
        .stream_track_with_range(TRACK_1_ID, "bytes=0-1023")
        .await;

    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);

    // Verify content-range header is present
    let content_range = response.headers().get("content-range");
    assert!(
        content_range.is_some(),
        "Expected Content-Range header for partial content"
    );

    // Verify we got exactly 1024 bytes (0-1023 inclusive)
    let bytes = response.bytes().await.unwrap();
    assert_eq!(
        bytes.len(),
        1024,
        "Expected 1024 bytes for range 0-1023, got {}",
        bytes.len()
    );
}

#[tokio::test]
async fn test_stream_track_with_open_ended_range() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Request from byte 100 to end
    let response = client
        .stream_track_with_range(TRACK_1_ID, "bytes=100-")
        .await;

    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);

    let bytes = response.bytes().await.unwrap();
    // Should get file size minus 100 bytes
    assert!(
        bytes.len() > 0,
        "Expected some bytes for open-ended range"
    );
}

#[tokio::test]
async fn test_stream_track_with_suffix_range() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Request last 500 bytes
    let response = client
        .stream_track_with_range(TRACK_1_ID, "bytes=-500")
        .await;

    // Should return partial content or full content if file < 500 bytes
    assert!(
        response.status() == StatusCode::PARTIAL_CONTENT
            || response.status() == StatusCode::OK
    );

    let bytes = response.bytes().await.unwrap();
    assert!(bytes.len() > 0);
    assert!(bytes.len() <= 500);
}

#[tokio::test]
async fn test_stream_track_full_then_partial() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // First get full track
    let response = client.stream_track(TRACK_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
    let full_bytes = response.bytes().await.unwrap();
    let file_size = full_bytes.len();

    // Then get just first 100 bytes with range request
    let response = client
        .stream_track_with_range(TRACK_1_ID, "bytes=0-99")
        .await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    let partial_bytes = response.bytes().await.unwrap();
    assert_eq!(partial_bytes.len(), 100);

    // Verify the partial content matches the beginning of full content
    assert_eq!(&full_bytes[0..100], &partial_bytes[..]);
}

#[tokio::test]
async fn test_concurrent_streaming() {
    let server = TestServer::spawn().await;

    // Spawn 5 concurrent streaming requests
    let handles: Vec<_> = (0..5)
        .map(|_| {
            let base_url = server.base_url.clone();
            tokio::spawn(async move {
                let client = TestClient::authenticated(base_url).await;
                let response = client.stream_track(TRACK_1_ID).await;
                response.status()
            })
        })
        .collect();

    // All should succeed
    for handle in handles {
        let status = handle.await.unwrap();
        assert_eq!(status, StatusCode::OK);
    }
}
