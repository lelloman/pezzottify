//! End-to-end tests for listening stats endpoints
//!
//! Tests user listening event recording, summaries, history,
//! and admin analytics endpoints.

mod common;

use common::{TestClient, TestServer, TRACK_1_ID, TRACK_2_ID, TRACK_3_ID, TEST_USER};
use reqwest::StatusCode;

// =============================================================================
// User Endpoint Tests - POST /v1/user/listening
// =============================================================================

#[tokio::test]
async fn test_record_listening_event_minimal() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Record a listening event with minimal fields
    let response = client.post_listening_event(TRACK_1_ID, 180, 200).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["id"].as_u64().unwrap() > 0);
    assert!(body["created"].as_bool().unwrap());
}

#[tokio::test]
async fn test_record_listening_event_full() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Record a listening event with all fields
    let response = client
        .post_listening_event_full(
            TRACK_1_ID,
            Some("test-session-uuid"),
            Some(1732982400),
            Some(1732982580),
            180,
            200,
            Some(2),
            Some(1),
            Some("album"),
            Some("android"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["id"].as_u64().unwrap() > 0);
    assert!(body["created"].as_bool().unwrap());
}

#[tokio::test]
async fn test_record_listening_event_deduplication() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let session_id = "unique-dedup-test-session";

    // First request should create
    let response = client
        .post_listening_event_full(
            TRACK_1_ID,
            Some(session_id),
            None,
            None,
            180,
            200,
            None,
            None,
            None,
            None,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body1: serde_json::Value = response.json().await.unwrap();
    assert!(body1["created"].as_bool().unwrap());
    let id1 = body1["id"].as_u64().unwrap();

    // Second request with same session_id should be deduplicated
    let response = client
        .post_listening_event_full(
            TRACK_1_ID,
            Some(session_id),
            None,
            None,
            180,
            200,
            None,
            None,
            None,
            None,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body2: serde_json::Value = response.json().await.unwrap();
    assert!(!body2["created"].as_bool().unwrap());
    assert_eq!(body2["id"].as_u64().unwrap(), id1);
}

#[tokio::test]
async fn test_record_listening_event_requires_auth() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.post_listening_event(TRACK_1_ID, 180, 200).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =============================================================================
// User Endpoint Tests - GET /v1/user/listening/summary
// =============================================================================

#[tokio::test]
async fn test_get_listening_summary_empty() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_listening_summary(None, None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["total_plays"].as_u64().unwrap(), 0);
    assert_eq!(body["total_duration_seconds"].as_u64().unwrap(), 0);
    assert_eq!(body["completed_plays"].as_u64().unwrap(), 0);
    assert_eq!(body["unique_tracks"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn test_get_listening_summary_with_events() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Record some listening events
    // Complete listen (190/200 = 95% > 90%)
    client.post_listening_event(TRACK_1_ID, 190, 200).await;
    // Incomplete listen (50/200 = 25% < 90%)
    client.post_listening_event(TRACK_2_ID, 50, 200).await;
    // Another complete listen of same track
    client.post_listening_event(TRACK_1_ID, 185, 200).await;

    let response = client.get_listening_summary(None, None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["total_plays"].as_u64().unwrap(), 3);
    assert_eq!(body["total_duration_seconds"].as_u64().unwrap(), 425); // 190 + 50 + 185
    assert_eq!(body["completed_plays"].as_u64().unwrap(), 2);
    assert_eq!(body["unique_tracks"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn test_get_listening_summary_requires_auth() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.get_listening_summary(None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =============================================================================
// User Endpoint Tests - GET /v1/user/listening/history
// =============================================================================

#[tokio::test]
async fn test_get_listening_history_empty() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_listening_history(None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    assert!(body.is_empty());
}

#[tokio::test]
async fn test_get_listening_history_with_events() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Record events for multiple tracks
    client.post_listening_event(TRACK_1_ID, 180, 200).await;
    client.post_listening_event(TRACK_2_ID, 150, 200).await;
    client.post_listening_event(TRACK_1_ID, 190, 200).await; // Play TRACK_1 again

    let response = client.get_listening_history(None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(body.len(), 2); // 2 unique tracks

    // TRACK_1 should have 2 plays and higher total duration
    let track1_entry = body.iter().find(|e| e["track_id"] == TRACK_1_ID).unwrap();
    assert_eq!(track1_entry["play_count"].as_u64().unwrap(), 2);
    assert_eq!(track1_entry["total_duration_seconds"].as_u64().unwrap(), 370); // 180 + 190
}

#[tokio::test]
async fn test_get_listening_history_with_limit() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Record events for 3 different tracks
    client.post_listening_event(TRACK_1_ID, 180, 200).await;
    client.post_listening_event(TRACK_2_ID, 150, 200).await;
    client.post_listening_event(TRACK_3_ID, 160, 200).await;

    // Request only 2
    let response = client.get_listening_history(Some(2)).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(body.len(), 2);
}

// =============================================================================
// User Endpoint Tests - GET /v1/user/listening/events
// =============================================================================

#[tokio::test]
async fn test_get_listening_events_empty() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_listening_events(None, None, None, None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    assert!(body.is_empty());
}

#[tokio::test]
async fn test_get_listening_events_with_data() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Record some events
    client.post_listening_event(TRACK_1_ID, 180, 200).await;
    client.post_listening_event(TRACK_2_ID, 150, 200).await;

    let response = client.get_listening_events(None, None, None, None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(body.len(), 2);
}

#[tokio::test]
async fn test_get_listening_events_pagination() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Record 5 events
    for _ in 0..5 {
        client.post_listening_event(TRACK_1_ID, 180, 200).await;
    }

    // Get first 2
    let response = client
        .get_listening_events(None, None, Some(2), None)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(body.len(), 2);

    // Get next 2 (offset 2)
    let response = client
        .get_listening_events(None, None, Some(2), Some(2))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(body.len(), 2);

    // Get remaining (offset 4)
    let response = client
        .get_listening_events(None, None, Some(2), Some(4))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(body.len(), 1);
}

// =============================================================================
// Admin Endpoint Tests - GET /v1/admin/listening/daily
// =============================================================================

#[tokio::test]
async fn test_admin_daily_stats_requires_admin() {
    let server = TestServer::spawn().await;

    // Regular user should be forbidden
    let client = TestClient::authenticated(server.base_url.clone()).await;
    let response = client.admin_get_daily_listening_stats(None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Unauthenticated should be forbidden
    let client = TestClient::new(server.base_url.clone());
    let response = client.admin_get_daily_listening_stats(None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_admin_daily_stats_with_data() {
    let server = TestServer::spawn().await;

    // First, create some listening data as regular user
    let user_client = TestClient::authenticated(server.base_url.clone()).await;
    user_client.post_listening_event(TRACK_1_ID, 180, 200).await;
    user_client.post_listening_event(TRACK_2_ID, 150, 200).await;

    // Then query as admin
    let admin_client = TestClient::authenticated_admin(server.base_url.clone()).await;
    let response = admin_client.admin_get_daily_listening_stats(None, None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    // Should have at least one day of stats
    assert!(!body.is_empty());

    let today_stats = &body[0];
    assert!(today_stats["total_plays"].as_u64().unwrap() >= 2);
}

// =============================================================================
// Admin Endpoint Tests - GET /v1/admin/listening/top-tracks
// =============================================================================

#[tokio::test]
async fn test_admin_top_tracks_requires_admin() {
    let server = TestServer::spawn().await;

    let client = TestClient::authenticated(server.base_url.clone()).await;
    let response = client.admin_get_top_tracks(None, None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_admin_top_tracks_with_data() {
    let server = TestServer::spawn().await;

    // Create listening data as regular user
    let user_client = TestClient::authenticated(server.base_url.clone()).await;
    // TRACK_1 played 3 times
    user_client.post_listening_event(TRACK_1_ID, 180, 200).await;
    user_client.post_listening_event(TRACK_1_ID, 185, 200).await;
    user_client.post_listening_event(TRACK_1_ID, 190, 200).await;
    // TRACK_2 played 1 time
    user_client.post_listening_event(TRACK_2_ID, 150, 200).await;

    // Query as admin
    let admin_client = TestClient::authenticated_admin(server.base_url.clone()).await;
    let response = admin_client.admin_get_top_tracks(None, None, None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    assert!(!body.is_empty());

    // TRACK_1 should be first (most plays)
    assert_eq!(body[0]["track_id"].as_str().unwrap(), TRACK_1_ID);
    assert_eq!(body[0]["play_count"].as_u64().unwrap(), 3);
}

#[tokio::test]
async fn test_admin_top_tracks_with_limit() {
    let server = TestServer::spawn().await;

    // Create listening data
    let user_client = TestClient::authenticated(server.base_url.clone()).await;
    user_client.post_listening_event(TRACK_1_ID, 180, 200).await;
    user_client.post_listening_event(TRACK_2_ID, 150, 200).await;
    user_client.post_listening_event(TRACK_3_ID, 160, 200).await;

    // Query with limit
    let admin_client = TestClient::authenticated_admin(server.base_url.clone()).await;
    let response = admin_client.admin_get_top_tracks(None, None, Some(2)).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    assert_eq!(body.len(), 2);
}

// =============================================================================
// Admin Endpoint Tests - GET /v1/admin/listening/track/{track_id}
// =============================================================================

#[tokio::test]
async fn test_admin_track_stats_requires_admin() {
    let server = TestServer::spawn().await;

    let client = TestClient::authenticated(server.base_url.clone()).await;
    let response = client
        .admin_get_track_listening_stats(TRACK_1_ID, None, None)
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_admin_track_stats_with_data() {
    let server = TestServer::spawn().await;

    // Create listening data
    let user_client = TestClient::authenticated(server.base_url.clone()).await;
    user_client.post_listening_event(TRACK_1_ID, 190, 200).await; // Complete
    user_client.post_listening_event(TRACK_1_ID, 50, 200).await; // Incomplete

    // Query as admin
    let admin_client = TestClient::authenticated_admin(server.base_url.clone()).await;
    let response = admin_client
        .admin_get_track_listening_stats(TRACK_1_ID, None, None)
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["track_id"].as_str().unwrap(), TRACK_1_ID);
    assert_eq!(body["play_count"].as_u64().unwrap(), 2);
    assert_eq!(body["total_duration_seconds"].as_u64().unwrap(), 240); // 190 + 50
    assert_eq!(body["completed_count"].as_u64().unwrap(), 1);
    assert_eq!(body["unique_listeners"].as_u64().unwrap(), 1);
}

#[tokio::test]
async fn test_admin_track_stats_nonexistent() {
    let server = TestServer::spawn().await;

    let admin_client = TestClient::authenticated_admin(server.base_url.clone()).await;
    let response = admin_client
        .admin_get_track_listening_stats("nonexistent-track", None, None)
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Should return zeros for non-existent track
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["play_count"].as_u64().unwrap(), 0);
}

// =============================================================================
// Admin Endpoint Tests - GET /v1/admin/listening/users/{handle}/summary
// =============================================================================

#[tokio::test]
async fn test_admin_user_summary_requires_admin() {
    let server = TestServer::spawn().await;

    let client = TestClient::authenticated(server.base_url.clone()).await;
    let response = client
        .admin_get_user_listening_summary(TEST_USER, None, None)
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_admin_user_summary_with_data() {
    let server = TestServer::spawn().await;

    // Create listening data as test user
    let user_client = TestClient::authenticated(server.base_url.clone()).await;
    user_client.post_listening_event(TRACK_1_ID, 190, 200).await;
    user_client.post_listening_event(TRACK_2_ID, 150, 200).await;

    // Query as admin
    let admin_client = TestClient::authenticated_admin(server.base_url.clone()).await;
    let response = admin_client
        .admin_get_user_listening_summary(TEST_USER, None, None)
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["total_plays"].as_u64().unwrap(), 2);
    assert_eq!(body["total_duration_seconds"].as_u64().unwrap(), 340); // 190 + 150
}

#[tokio::test]
async fn test_admin_user_summary_nonexistent_user() {
    let server = TestServer::spawn().await;

    let admin_client = TestClient::authenticated_admin(server.base_url.clone()).await;
    let response = admin_client
        .admin_get_user_listening_summary("nonexistent-user", None, None)
        .await;

    // Should return NOT_FOUND for non-existent user
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// User Isolation Tests
// =============================================================================

#[tokio::test]
async fn test_listening_events_user_isolation() {
    let server = TestServer::spawn().await;

    // User 1 creates events
    let user1_client = TestClient::authenticated(server.base_url.clone()).await;
    user1_client
        .post_listening_event(TRACK_1_ID, 180, 200)
        .await;
    user1_client
        .post_listening_event(TRACK_2_ID, 150, 200)
        .await;

    // Logout and login as admin (different user)
    let admin_client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Admin's personal listening history should be empty
    // (admin didn't listen to anything)
    let response = admin_client.get_listening_history(None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Vec<serde_json::Value> = response.json().await.unwrap();
    assert!(body.is_empty());

    // But admin can see user1's stats via admin endpoint
    let response = admin_client
        .admin_get_user_listening_summary(TEST_USER, None, None)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["total_plays"].as_u64().unwrap(), 2);
}
