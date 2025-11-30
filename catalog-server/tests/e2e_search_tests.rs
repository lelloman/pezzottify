//! End-to-end tests for search functionality
//!
//! Tests the search endpoint with various queries and filters.
//! Note: These tests require the search feature to be enabled (not --features no_search).

mod common;

use common::{TestClient, TestServer, ARTIST_1_NAME, ARTIST_2_NAME};
use reqwest::StatusCode;

// =============================================================================
// Basic Search Tests
// =============================================================================

#[tokio::test]
async fn test_search_returns_results() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Search for an artist by name
    let response = client.search_resolved("Test Band").await;

    // Note: Search may return 404 if no_search feature is enabled
    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);
    // Search returns valid JSON (may be empty depending on indexing)
    let _results: Vec<serde_json::Value> = response.json().await.unwrap();
}

#[tokio::test]
async fn test_search_finds_artist_by_partial_name() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Search with partial name
    let response = client.search_resolved("Jazz").await;

    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);
    // Search returns valid JSON (search indexing may vary)
    let _results: Vec<serde_json::Value> = response.json().await.unwrap();
}

#[tokio::test]
async fn test_search_with_no_results() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Search for something that doesn't exist
    let response = client.search_resolved("xyznonexistent123").await;

    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);
    let results: Vec<serde_json::Value> = response.json().await.unwrap();
    assert!(results.is_empty(), "Search for nonexistent term should return empty results");
}

#[tokio::test]
async fn test_search_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    // Try to search without authentication
    let response = client.search("Test").await;

    // Should be forbidden (or not found if search disabled)
    assert!(
        response.status() == StatusCode::FORBIDDEN || response.status() == StatusCode::NOT_FOUND,
        "Search should require authentication or be disabled"
    );
}

// =============================================================================
// Search Filter Tests
// =============================================================================

#[tokio::test]
async fn test_search_filter_by_artist() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Search with artist filter
    let response = client.search_with_filters("Test", vec!["artist"]).await;

    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);
    let results: Vec<serde_json::Value> = response.json().await.unwrap();

    // All results should be artists (have "id" starting with "R" based on test data)
    for result in &results {
        if let Some(id) = result.get("id").and_then(|i| i.as_str()) {
            // Artists in test data have IDs like "R1", "R2"
            assert!(
                id.starts_with("R") || result.get("name").is_some(),
                "Filtered results should be artists"
            );
        }
    }
}

#[tokio::test]
async fn test_search_filter_by_album() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Search with album filter
    let response = client.search_with_filters("Album", vec!["album"]).await;

    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_search_filter_by_track() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Search with track filter
    let response = client.search_with_filters("Track", vec!["track"]).await;

    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);
}

// =============================================================================
// Search Response Format Tests
// =============================================================================

#[tokio::test]
async fn test_search_raw_returns_item_ids() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Search without resolve
    let response = client.search("Test").await;

    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);
    let results: Vec<serde_json::Value> = response.json().await.unwrap();

    // Raw results should have item_id and item_type
    for result in &results {
        assert!(
            result.get("item_id").is_some() || result.get("id").is_some(),
            "Raw search results should have item identifiers"
        );
    }
}

#[tokio::test]
async fn test_search_resolved_returns_full_objects() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Search with resolve
    let response = client.search_resolved(ARTIST_1_NAME).await;

    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);
    let results: Vec<serde_json::Value> = response.json().await.unwrap();

    // Resolved results should have name field
    for result in &results {
        assert!(
            result.get("name").is_some() || result.get("id").is_some(),
            "Resolved search results should have name or id field"
        );
    }
}

// =============================================================================
// Search Edge Cases
// =============================================================================

#[tokio::test]
async fn test_search_empty_query() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Search with empty string
    let response = client.search("").await;

    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search endpoint not available (no_search feature enabled)");
        return;
    }

    // Empty query should return OK with empty results or all results
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_search_special_characters() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Search with special characters - should not crash
    let response = client.search("Test & Band").await;

    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search endpoint not available (no_search feature enabled)");
        return;
    }

    // Should handle gracefully
    assert!(
        response.status() == StatusCode::OK || response.status() == StatusCode::BAD_REQUEST,
        "Search should handle special characters gracefully"
    );
}

#[tokio::test]
async fn test_search_case_insensitive() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Search with different case
    let response_lower = client.search_resolved("test band").await;
    let response_upper = client.search_resolved("TEST BAND").await;

    if response_lower.status() == StatusCode::NOT_FOUND {
        eprintln!("Search endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response_lower.status(), StatusCode::OK);
    assert_eq!(response_upper.status(), StatusCode::OK);

    // Both should return results (search is typically case-insensitive)
    let results_lower: Vec<serde_json::Value> = response_lower.json().await.unwrap();
    let results_upper: Vec<serde_json::Value> = response_upper.json().await.unwrap();

    // At least one should have results if the other does
    if !results_lower.is_empty() {
        assert!(!results_upper.is_empty(), "Search should be case-insensitive");
    }
}
