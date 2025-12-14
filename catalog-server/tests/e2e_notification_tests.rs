//! End-to-end tests for notification API endpoints
//!
//! Tests notification-related functionality including:
//! - Notifications in sync state
//! - Mark notification as read endpoint
//! - Notification sync events

mod common;

use common::{TestClient, TestServer};
use pezzottify_catalog_server::notifications::NotificationType;
use reqwest::StatusCode;

#[tokio::test]
async fn test_sync_state_includes_notifications_field() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_sync_state().await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Sync state should include notifications field
    assert!(body.get("notifications").is_some());
    assert!(body["notifications"].is_array());
    assert!(body["notifications"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_sync_state_includes_created_notification() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a notification directly via the store
    let notification = server
        .user_store
        .create_notification(
            1, // User ID 1 (the test user)
            NotificationType::DownloadCompleted,
            "Album Ready".to_string(),
            Some("by Artist".to_string()),
            serde_json::json!({
                "album_id": "test-album-001",
                "album_name": "Test Album",
                "artist_name": "Test Artist",
                "image_id": null,
                "request_id": "req-001",
            }),
        )
        .expect("Failed to create notification");

    // Get sync state
    let response = client.get_sync_state().await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();

    // Should include the notification
    let notifications = body["notifications"].as_array().unwrap();
    assert_eq!(notifications.len(), 1);

    let notif = &notifications[0];
    assert_eq!(notif["id"], notification.id);
    assert_eq!(notif["title"], "Album Ready");
    assert_eq!(notif["body"], "by Artist");
    assert_eq!(notif["notification_type"], "download_completed");
    assert!(notif["read_at"].is_null());
    assert!(notif["created_at"].as_i64().is_some());

    // Check data payload
    let data = &notif["data"];
    assert_eq!(data["album_id"], "test-album-001");
    assert_eq!(data["album_name"], "Test Album");
    assert_eq!(data["artist_name"], "Test Artist");
    assert_eq!(data["request_id"], "req-001");
}

#[tokio::test]
async fn test_mark_notification_as_read() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a notification
    let notification = server
        .user_store
        .create_notification(
            1,
            NotificationType::DownloadCompleted,
            "Album Ready".to_string(),
            None,
            serde_json::json!({"album_id": "test-album"}),
        )
        .expect("Failed to create notification");

    // Mark it as read
    let response = client.mark_notification_read(&notification.id).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["read_at"].as_i64().is_some());

    // Verify in sync state
    let response = client.get_sync_state().await;
    let state: serde_json::Value = response.json().await.unwrap();
    let notifications = state["notifications"].as_array().unwrap();
    assert_eq!(notifications.len(), 1);
    assert!(notifications[0]["read_at"].as_i64().is_some());
}

#[tokio::test]
async fn test_mark_notification_read_idempotent() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a notification
    let notification = server
        .user_store
        .create_notification(
            1,
            NotificationType::DownloadCompleted,
            "Album Ready".to_string(),
            None,
            serde_json::json!({}),
        )
        .expect("Failed to create notification");

    // Mark as read twice
    let response1 = client.mark_notification_read(&notification.id).await;
    assert_eq!(response1.status(), StatusCode::OK);
    let body1: serde_json::Value = response1.json().await.unwrap();
    let read_at1 = body1["read_at"].as_i64().unwrap();

    let response2 = client.mark_notification_read(&notification.id).await;
    assert_eq!(response2.status(), StatusCode::OK);
    let body2: serde_json::Value = response2.json().await.unwrap();
    let read_at2 = body2["read_at"].as_i64().unwrap();

    // Should return same timestamp (idempotent)
    assert_eq!(read_at1, read_at2);
}

#[tokio::test]
async fn test_mark_notification_read_not_found() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Try to mark non-existent notification
    let response = client.mark_notification_read("nonexistent-id").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// Note: Testing wrong user access would require setting up another user with password,
// which is complex. The mark_notification_read function already checks user_id ownership,
// returning NOT_FOUND if the notification doesn't belong to the user.
// The test_mark_notification_read_not_found test verifies this behavior for non-existent IDs.

#[tokio::test]
async fn test_mark_notification_read_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.mark_notification_read("some-id").await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_notification_created_event_in_sync_events() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Get initial sync state
    let response = client.get_sync_state().await;
    let state: serde_json::Value = response.json().await.unwrap();
    let initial_seq = state["seq"].as_i64().unwrap();

    // Create a notification and log the event
    use pezzottify_catalog_server::user::sync_events::UserEvent;
    use pezzottify_catalog_server::user::UserEventStore;

    let notification = server
        .user_store
        .create_notification(
            1,
            NotificationType::DownloadCompleted,
            "Album Ready".to_string(),
            Some("by Artist".to_string()),
            serde_json::json!({
                "album_id": "test-album",
                "album_name": "Test Album",
                "artist_name": "Test Artist",
            }),
        )
        .expect("Failed to create notification");

    // Log the notification_created event (simulating what NotificationService does)
    let event = UserEvent::NotificationCreated {
        notification: notification.clone(),
    };
    server.user_store.append_event(1, &event).unwrap();

    // Get sync events since initial seq
    let response = client.get_sync_events(initial_seq).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    let events = body["events"].as_array().unwrap();

    // Should have the notification_created event
    // Note: UserEvent uses adjacently tagged format: {"type": "...", "payload": {...}}
    // StoredEvent flattens this, so we get: {"seq": ..., "type": ..., "payload": {...}, "server_timestamp": ...}
    let notification_event = events.iter().find(|e| e["type"] == "notification_created");
    assert!(
        notification_event.is_some(),
        "Should have notification_created event. Events: {:?}",
        events
    );

    let notif_data = &notification_event.unwrap()["payload"]["notification"];
    assert_eq!(notif_data["id"], notification.id);
    assert_eq!(notif_data["title"], "Album Ready");
}

#[tokio::test]
async fn test_notification_read_event_in_sync_events() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create a notification
    let notification = server
        .user_store
        .create_notification(
            1,
            NotificationType::DownloadCompleted,
            "Album Ready".to_string(),
            None,
            serde_json::json!({}),
        )
        .expect("Failed to create notification");

    // Get current seq
    let response = client.get_sync_state().await;
    let state: serde_json::Value = response.json().await.unwrap();
    let seq_before = state["seq"].as_i64().unwrap();

    // Mark as read (this logs an event)
    let response = client.mark_notification_read(&notification.id).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Get sync events since before
    let response = client.get_sync_events(seq_before).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    let events = body["events"].as_array().unwrap();

    // Should have the notification_read event
    // Note: UserEvent uses adjacently tagged format: {"type": "...", "payload": {...}}
    let read_event = events.iter().find(|e| e["type"] == "notification_read");
    assert!(read_event.is_some(), "Should have notification_read event");

    let payload = &read_event.unwrap()["payload"];
    assert_eq!(payload["notification_id"], notification.id);
    assert!(payload["read_at"].as_i64().is_some());
}

#[tokio::test]
async fn test_notifications_ordered_by_creation_time_desc() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Create multiple notifications with explicit delays to ensure distinct timestamps
    for i in 1..=3 {
        // Longer delay to ensure different timestamps (SQLite stores seconds)
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        server
            .user_store
            .create_notification(
                1,
                NotificationType::DownloadCompleted,
                format!("Notification {}", i),
                None,
                serde_json::json!({"order": i}),
            )
            .expect("Failed to create notification");
    }

    // Get sync state
    let response = client.get_sync_state().await;
    let state: serde_json::Value = response.json().await.unwrap();
    let notifications = state["notifications"].as_array().unwrap();

    assert_eq!(notifications.len(), 3);

    // Should be ordered newest first (descending by created_at)
    // The last created notification (Notification 3) should be first
    assert_eq!(notifications[0]["title"], "Notification 3");
    assert_eq!(notifications[1]["title"], "Notification 2");
    assert_eq!(notifications[2]["title"], "Notification 1");
}
