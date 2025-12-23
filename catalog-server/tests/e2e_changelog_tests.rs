//! End-to-end tests for changelog API endpoints
//!
//! Tests the admin changelog batch management and change query endpoints.

mod common;

use common::{TestClient, TestServer, ARTIST_1_ID};
use reqwest::StatusCode;
use serde_json::Value;

// =============================================================================
// Permission Tests
// =============================================================================

#[tokio::test]
async fn test_changelog_requires_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Regular user should not be able to access changelog endpoints
    let response = client.admin_list_changelog_batches(None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_changelog_unauthenticated_unauthorized() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.admin_list_changelog_batches(None).await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// =============================================================================
// Batch Management Tests
// =============================================================================

#[tokio::test]
async fn test_list_batches() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Should have the initial test data batch (open)
    let response = client.admin_list_changelog_batches(None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let batches: Vec<Value> = response.json().await.unwrap();
    assert!(
        !batches.is_empty(),
        "Should have at least one batch from test fixtures"
    );
}

#[tokio::test]
async fn test_list_batches_filter_open() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Filter for open batches only
    let response = client.admin_list_changelog_batches(Some(true)).await;
    assert_eq!(response.status(), StatusCode::OK);

    let batches: Vec<Value> = response.json().await.unwrap();
    for batch in &batches {
        assert_eq!(batch["is_open"], true, "All batches should be open");
    }
}

#[tokio::test]
async fn test_list_batches_filter_closed() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Close the existing batch first
    let list_response = client.admin_list_changelog_batches(Some(true)).await;
    let batches: Vec<Value> = list_response.json().await.unwrap();
    if let Some(batch) = batches.first() {
        let batch_id = batch["id"].as_str().unwrap();
        client.admin_close_changelog_batch(batch_id).await;
    }

    // Now filter for closed batches
    let response = client.admin_list_changelog_batches(Some(false)).await;
    assert_eq!(response.status(), StatusCode::OK);

    let closed_batches: Vec<Value> = response.json().await.unwrap();
    for batch in &closed_batches {
        assert_eq!(batch["is_open"], false, "All batches should be closed");
    }
}

#[tokio::test]
async fn test_create_batch() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // First close the existing batch from test fixtures
    let list_response = client.admin_list_changelog_batches(Some(true)).await;
    let batches: Vec<Value> = list_response.json().await.unwrap();
    if let Some(batch) = batches.first() {
        let batch_id = batch["id"].as_str().unwrap();
        client.admin_close_changelog_batch(batch_id).await;
    }

    // Now create a new batch
    let response = client
        .admin_create_changelog_batch("Test Batch", Some("Test description"))
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let batch: Value = response.json().await.unwrap();
    assert_eq!(batch["name"], "Test Batch");
    assert_eq!(batch["description"], "Test description");
    assert_eq!(batch["is_open"], true);
    assert!(batch["id"].as_str().is_some());
}

#[tokio::test]
async fn test_create_batch_without_description() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Close existing batch
    let list_response = client.admin_list_changelog_batches(Some(true)).await;
    let batches: Vec<Value> = list_response.json().await.unwrap();
    if let Some(batch) = batches.first() {
        let batch_id = batch["id"].as_str().unwrap();
        client.admin_close_changelog_batch(batch_id).await;
    }

    let response = client
        .admin_create_changelog_batch("Minimal Batch", None)
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let batch: Value = response.json().await.unwrap();
    assert_eq!(batch["name"], "Minimal Batch");
    assert!(batch["description"].is_null());
}

#[tokio::test]
async fn test_create_batch_conflict_when_active() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // There's already an active batch from test fixtures
    // Trying to create another should fail with 409 Conflict
    let response = client
        .admin_create_changelog_batch("Second Batch", None)
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_get_batch() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Get the existing batch
    let list_response = client.admin_list_changelog_batches(None).await;
    let batches: Vec<Value> = list_response.json().await.unwrap();
    let batch_id = batches[0]["id"].as_str().unwrap();

    let response = client.admin_get_changelog_batch(batch_id).await;
    assert_eq!(response.status(), StatusCode::OK);

    let batch: Value = response.json().await.unwrap();
    assert_eq!(batch["id"], batch_id);
}

#[tokio::test]
async fn test_get_batch_not_found() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client
        .admin_get_changelog_batch("nonexistent-batch-id")
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_close_batch() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Get the existing open batch
    let list_response = client.admin_list_changelog_batches(Some(true)).await;
    let batches: Vec<Value> = list_response.json().await.unwrap();
    let batch_id = batches[0]["id"].as_str().unwrap();

    // Close it
    let response = client.admin_close_changelog_batch(batch_id).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify it's closed
    let get_response = client.admin_get_changelog_batch(batch_id).await;
    let batch: Value = get_response.json().await.unwrap();
    assert_eq!(batch["is_open"], false);
    assert!(batch["closed_at"].as_i64().is_some());
}

#[tokio::test]
async fn test_close_batch_not_found() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client
        .admin_close_changelog_batch("nonexistent-batch-id")
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_close_batch_already_closed() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Get and close the batch
    let list_response = client.admin_list_changelog_batches(Some(true)).await;
    let batches: Vec<Value> = list_response.json().await.unwrap();
    let batch_id = batches[0]["id"].as_str().unwrap();
    client.admin_close_changelog_batch(batch_id).await;

    // Try to close again
    let response = client.admin_close_changelog_batch(batch_id).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_delete_batch_not_empty() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // The test fixtures batch has changes in it (the test data)
    let list_response = client.admin_list_changelog_batches(None).await;
    let batches: Vec<Value> = list_response.json().await.unwrap();
    let batch_id = batches[0]["id"].as_str().unwrap();

    // Try to delete - should fail because batch has changes
    let response = client.admin_delete_changelog_batch(batch_id).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_delete_empty_batch() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Close the existing batch
    let list_response = client.admin_list_changelog_batches(Some(true)).await;
    let batches: Vec<Value> = list_response.json().await.unwrap();
    if let Some(batch) = batches.first() {
        let batch_id = batch["id"].as_str().unwrap();
        client.admin_close_changelog_batch(batch_id).await;
    }

    // Create a new empty batch
    let create_response = client
        .admin_create_changelog_batch("Empty Batch", None)
        .await;
    let batch: Value = create_response.json().await.unwrap();
    let batch_id = batch["id"].as_str().unwrap();

    // Delete it - should succeed because it's empty
    let response = client.admin_delete_changelog_batch(batch_id).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify it's gone
    let get_response = client.admin_get_changelog_batch(batch_id).await;
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_batch_not_found() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client
        .admin_delete_changelog_batch("nonexistent-batch-id")
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// Change Query Tests
// =============================================================================

#[tokio::test]
async fn test_get_batch_changes() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Get the test fixtures batch
    let list_response = client.admin_list_changelog_batches(None).await;
    let batches: Vec<Value> = list_response.json().await.unwrap();
    let batch_id = batches[0]["id"].as_str().unwrap();

    // Get changes - should have the test data inserts
    let response = client.admin_get_changelog_batch_changes(batch_id).await;
    assert_eq!(response.status(), StatusCode::OK);

    let changes: Vec<Value> = response.json().await.unwrap();
    assert!(
        !changes.is_empty(),
        "Should have changes from test fixture data"
    );

    // Verify change structure
    let first_change = &changes[0];
    assert!(first_change["id"].as_i64().is_some());
    assert!(first_change["entity_type"].as_str().is_some());
    assert!(first_change["entity_id"].as_str().is_some());
    assert!(first_change["operation"].as_str().is_some());
}

#[tokio::test]
async fn test_get_batch_changes_not_found() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client
        .admin_get_changelog_batch_changes("nonexistent-batch-id")
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_entity_history() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Get history for an artist from test fixtures
    let response = client
        .admin_get_changelog_entity_history("artist", ARTIST_1_ID)
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let changes: Vec<Value> = response.json().await.unwrap();
    assert!(
        !changes.is_empty(),
        "Should have history for test fixture artist"
    );

    // Verify it's for the correct entity
    for change in &changes {
        assert_eq!(change["entity_type"], "Artist");
        assert_eq!(change["entity_id"], ARTIST_1_ID);
    }
}

#[tokio::test]
async fn test_get_entity_history_empty() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Get history for a non-existent entity
    let response = client
        .admin_get_changelog_entity_history("artist", "nonexistent-artist")
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let changes: Vec<Value> = response.json().await.unwrap();
    assert!(
        changes.is_empty(),
        "Should have no history for non-existent entity"
    );
}

#[tokio::test]
async fn test_get_entity_history_invalid_type() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client
        .admin_get_changelog_entity_history("invalid_type", "some-id")
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_entity_history_all_types() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Test all valid entity types
    for entity_type in &["artist", "album", "track", "image"] {
        let response = client
            .admin_get_changelog_entity_history(entity_type, "any-id")
            .await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Entity type '{}' should be valid",
            entity_type
        );
    }
}

// =============================================================================
// What's New User Endpoint Tests
// =============================================================================

#[tokio::test]
async fn test_whats_new_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let response = client.get_whats_new(None).await;
    // 401 Unauthorized - not authenticated
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_whats_new_regular_user_can_access() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_whats_new(None).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_whats_new_returns_empty_when_no_closed_batches() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client.get_whats_new(None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json().await.unwrap();
    let batches = body["batches"].as_array().unwrap();
    assert!(
        batches.is_empty(),
        "Should have no closed batches initially"
    );
}

#[tokio::test]
async fn test_whats_new_returns_closed_batches() {
    let server = TestServer::spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let user = TestClient::authenticated(server.base_url.clone()).await;

    // Close any existing active batch from fixtures
    let response = admin.admin_list_changelog_batches(Some(true)).await;
    let open_batches: Vec<Value> = response.json().await.unwrap();
    for batch in open_batches {
        let id = batch["id"].as_str().unwrap();
        admin.admin_close_changelog_batch(id).await;
    }

    // Create a batch
    let response = admin
        .admin_create_changelog_batch("Test Release", Some("New music release"))
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let batch: Value = response.json().await.unwrap();
    let batch_id = batch["id"].as_str().unwrap();

    // Close the batch
    let response = admin.admin_close_changelog_batch(batch_id).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Regular user should see the closed batch (only the one we created, not fixture batch)
    let response = user.get_whats_new(None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json().await.unwrap();
    let batches = body["batches"].as_array().unwrap();
    // At least our test batch should be there
    let test_batch = batches.iter().find(|b| b["name"] == "Test Release");
    assert!(test_batch.is_some(), "Should find our test batch");
    let test_batch = test_batch.unwrap();
    assert_eq!(test_batch["description"], "New music release");
    assert!(
        test_batch["closed_at"].is_number(),
        "Should have closed_at timestamp"
    );
}

#[tokio::test]
async fn test_whats_new_respects_limit_parameter() {
    let server = TestServer::spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let user = TestClient::authenticated(server.base_url.clone()).await;

    // Close any existing active batch from fixtures
    let response = admin.admin_list_changelog_batches(Some(true)).await;
    let open_batches: Vec<Value> = response.json().await.unwrap();
    for batch in open_batches {
        let id = batch["id"].as_str().unwrap();
        admin.admin_close_changelog_batch(id).await;
    }

    // Create and close 3 batches
    for i in 1..=3 {
        let response = admin
            .admin_create_changelog_batch(&format!("Release {}", i), None)
            .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let batch: Value = response.json().await.unwrap();
        let batch_id = batch["id"].as_str().unwrap();

        let response = admin.admin_close_changelog_batch(batch_id).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Request with limit=2
    let response = user.get_whats_new(Some(2)).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json().await.unwrap();
    let batches = body["batches"].as_array().unwrap();
    assert_eq!(batches.len(), 2, "Should return only 2 batches");
}

#[tokio::test]
async fn test_whats_new_default_limit() {
    let server = TestServer::spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let user = TestClient::authenticated(server.base_url.clone()).await;

    // Close any existing active batch from fixtures
    let response = admin.admin_list_changelog_batches(Some(true)).await;
    let open_batches: Vec<Value> = response.json().await.unwrap();
    for batch in open_batches {
        let id = batch["id"].as_str().unwrap();
        admin.admin_close_changelog_batch(id).await;
    }

    // Create and close 12 batches
    for i in 1..=12 {
        let response = admin
            .admin_create_changelog_batch(&format!("Release {}", i), None)
            .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let batch: Value = response.json().await.unwrap();
        let batch_id = batch["id"].as_str().unwrap();

        let response = admin.admin_close_changelog_batch(batch_id).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Request without limit should use default (10)
    let response = user.get_whats_new(None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json().await.unwrap();
    let batches = body["batches"].as_array().unwrap();
    assert_eq!(batches.len(), 10, "Should return default 10 batches");
}

#[tokio::test]
async fn test_whats_new_orders_by_closed_at_desc() {
    let server = TestServer::spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let user = TestClient::authenticated(server.base_url.clone()).await;

    // Close any existing active batch from fixtures
    let response = admin.admin_list_changelog_batches(Some(true)).await;
    let open_batches: Vec<Value> = response.json().await.unwrap();
    for batch in open_batches {
        let id = batch["id"].as_str().unwrap();
        admin.admin_close_changelog_batch(id).await;
    }

    // Create and close batches
    for i in 1..=3 {
        let response = admin
            .admin_create_changelog_batch(&format!("Release {}", i), None)
            .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let batch: Value = response.json().await.unwrap();
        let batch_id = batch["id"].as_str().unwrap();

        let response = admin.admin_close_changelog_batch(batch_id).await;
        assert_eq!(response.status(), StatusCode::OK);

        // 1 second delay to ensure different closed_at timestamps (stored in seconds)
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    let response = user.get_whats_new(None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json().await.unwrap();
    let batches = body["batches"].as_array().unwrap();

    // Find the batches we created (they should be at the top since they're most recent)
    let release_names: Vec<&str> = batches
        .iter()
        .filter_map(|b| b["name"].as_str())
        .filter(|n| n.starts_with("Release "))
        .collect();

    // Most recent should be first (Release 3)
    assert!(
        release_names.len() >= 3,
        "Should have at least 3 release batches"
    );
    assert_eq!(release_names[0], "Release 3");
    assert_eq!(release_names[1], "Release 2");
    assert_eq!(release_names[2], "Release 1");
}
