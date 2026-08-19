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
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers().get("accept-ranges").unwrap(), "bytes");
    assert!(
        response.headers().get("content-range").is_none(),
        "Full responses must not include Content-Range"
    );
    let content_length: usize = response
        .headers()
        .get("content-length")
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();

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
    assert!(!bytes.is_empty());
    assert_eq!(bytes.len(), content_length);
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
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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
    assert!(!bytes1.is_empty());
    assert!(!bytes2.is_empty());
}

// =============================================================================
// Range Request Tests
// =============================================================================

#[tokio::test]
async fn test_stream_track_with_range_request() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let full_bytes = client.stream_track(TRACK_1_ID).await.bytes().await.unwrap();
    let response = client
        .stream_track_with_range(TRACK_1_ID, "bytes=0-1023")
        .await;

    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()["cache-control"], "no-store");

    // Verify content-range header is present
    assert_eq!(
        response.headers().get("content-range").unwrap(),
        format!("bytes 0-1023/{}", full_bytes.len()).as_str()
    );
    assert_eq!(response.headers().get("content-length").unwrap(), "1024");
    assert_eq!(response.headers().get("accept-ranges").unwrap(), "bytes");

    // Verify we got exactly 1024 bytes (0-1023 inclusive)
    let bytes = response.bytes().await.unwrap();
    assert_eq!(
        bytes.len(),
        1024,
        "Expected 1024 bytes for range 0-1023, got {}",
        bytes.len()
    );
    assert_eq!(&bytes[..], &full_bytes[..1024]);
}

#[tokio::test]
async fn test_stream_track_with_open_ended_range() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let full_bytes = client.stream_track(TRACK_1_ID).await.bytes().await.unwrap();

    // Request from byte 100 to end
    let response = client
        .stream_track_with_range(TRACK_1_ID, "bytes=100-")
        .await;

    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response.headers().get("content-range").unwrap(),
        format!("bytes 100-{}/{}", full_bytes.len() - 1, full_bytes.len()).as_str()
    );
    assert_eq!(
        response.headers().get("content-length").unwrap(),
        (full_bytes.len() - 100).to_string().as_str()
    );

    let bytes = response.bytes().await.unwrap();
    assert_eq!(&bytes[..], &full_bytes[100..]);
}

#[tokio::test]
async fn test_stream_track_with_suffix_range() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let full_bytes = client.stream_track(TRACK_1_ID).await.bytes().await.unwrap();

    // Request last 500 bytes
    let response = client
        .stream_track_with_range(TRACK_1_ID, "bytes=-500")
        .await;

    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    let expected_start = full_bytes.len().saturating_sub(500);
    let expected_length = full_bytes.len() - expected_start;
    assert_eq!(
        response.headers().get("content-range").unwrap(),
        format!(
            "bytes {}-{}/{}",
            expected_start,
            full_bytes.len() - 1,
            full_bytes.len()
        )
        .as_str()
    );
    assert_eq!(
        response.headers().get("content-length").unwrap(),
        expected_length.to_string().as_str()
    );

    let bytes = response.bytes().await.unwrap();
    assert_eq!(bytes.len(), expected_length);
    assert_eq!(&bytes[..], &full_bytes[expected_start..]);
}

#[tokio::test]
async fn test_stream_track_clamps_range_end_to_file_length() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;
    let full_bytes = client.stream_track(TRACK_1_ID).await.bytes().await.unwrap();

    let response = client
        .stream_track_with_range(TRACK_1_ID, "bytes=100-18446744073709551615")
        .await;

    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response.headers().get("content-range").unwrap(),
        format!("bytes 100-{}/{}", full_bytes.len() - 1, full_bytes.len()).as_str()
    );
    let bytes = response.bytes().await.unwrap();
    assert_eq!(&bytes[..], &full_bytes[100..]);
}

#[tokio::test]
async fn test_stream_track_rejects_invalid_and_unsatisfiable_ranges() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;
    let file_length = client
        .stream_track(TRACK_1_ID)
        .await
        .bytes()
        .await
        .unwrap()
        .len();
    let cases = [
        "items=0-1",
        "bytes=",
        "bytes=-",
        "bytes=-0",
        "bytes=100-99",
        "bytes=999999-",
        "bytes=0-1,3-4",
        "bytes=18446744073709551616-",
    ];

    for range in cases {
        let response = client.stream_track_with_range(TRACK_1_ID, range).await;

        assert_eq!(
            response.status(),
            StatusCode::RANGE_NOT_SATISFIABLE,
            "range: {range}"
        );
        assert_eq!(
            response.headers().get("content-range").unwrap(),
            format!("bytes */{file_length}").as_str(),
            "range: {range}"
        );
        assert_eq!(
            response.headers().get("content-length").unwrap(),
            "0",
            "range: {range}"
        );
        assert!(response.bytes().await.unwrap().is_empty(), "range: {range}");
    }
}

#[tokio::test]
async fn test_stream_track_full_then_partial() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // First get full track
    let response = client.stream_track(TRACK_1_ID).await;
    assert_eq!(response.status(), StatusCode::OK);
    let full_bytes = response.bytes().await.unwrap();
    let _file_size = full_bytes.len();

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
