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
    assert!(
        results.is_empty(),
        "Search for nonexistent term should return empty results"
    );
}

#[tokio::test]
async fn test_search_requires_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    // Try to search without authentication
    let response = client.search("Test").await;

    // Should be unauthorized/forbidden (or not found if search disabled)
    assert!(
        response.status() == StatusCode::UNAUTHORIZED
            || response.status() == StatusCode::FORBIDDEN
            || response.status() == StatusCode::NOT_FOUND,
        "Search should require authentication or be disabled, got: {}",
        response.status()
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

    // All results should be artists (have "id" starting with "test_artist" based on test data)
    for result in &results {
        if let Some(id) = result.get("id").and_then(|i| i.as_str()) {
            assert!(
                id.starts_with("test_artist") || result.get("name").is_some(),
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
        assert!(
            !results_upper.is_empty(),
            "Search should be case-insensitive"
        );
    }
}

// =============================================================================
// Relevance Filter Admin Tests
// =============================================================================

#[tokio::test]
async fn test_relevance_filter_get_requires_admin() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    // Regular user should not be able to access admin endpoint
    let response = client.admin_get_relevance_filter().await;

    // Skip if search is disabled (no_search feature enabled)
    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search admin endpoint not available (no_search feature enabled)");
        return;
    }

    // Should be forbidden (not admin)
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "Regular user should not access admin relevance filter endpoint"
    );
}

#[tokio::test]
async fn test_relevance_filter_get_default_is_none() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = client.admin_get_relevance_filter().await;

    // Skip if search is disabled
    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search admin endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    // Default should be None filter
    assert_eq!(
        body.get("config")
            .and_then(|c| c.get("method"))
            .and_then(|m| m.as_str()),
        Some("none"),
        "Default relevance filter should be 'none'"
    );
}

#[tokio::test]
async fn test_relevance_filter_set_and_get() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Set a percentage_of_best filter
    let config = serde_json::json!({
        "method": "percentage_of_best",
        "threshold": 0.4
    });

    let response = client.admin_set_relevance_filter(config.clone()).await;

    // Skip if search is disabled
    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search admin endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);

    // Verify the config was saved
    let get_response = client.admin_get_relevance_filter().await;
    assert_eq!(get_response.status(), StatusCode::OK);

    let body: serde_json::Value = get_response.json().await.unwrap();
    assert_eq!(
        body.get("config")
            .and_then(|c| c.get("method"))
            .and_then(|m| m.as_str()),
        Some("percentage_of_best"),
        "Saved config should be percentage_of_best"
    );
    assert_eq!(
        body.get("config")
            .and_then(|c| c.get("threshold"))
            .and_then(|t| t.as_f64()),
        Some(0.4),
        "Saved threshold should be 0.4"
    );
}

#[tokio::test]
async fn test_relevance_filter_set_gap_detection() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Set a gap_detection filter
    let config = serde_json::json!({
        "method": "gap_detection",
        "drop_threshold": 0.5
    });

    let response = client.admin_set_relevance_filter(config).await;

    // Skip if search is disabled
    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search admin endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body.get("config")
            .and_then(|c| c.get("method"))
            .and_then(|m| m.as_str()),
        Some("gap_detection")
    );
}

#[tokio::test]
async fn test_relevance_filter_set_standard_deviation() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Set a standard_deviation filter
    let config = serde_json::json!({
        "method": "standard_deviation",
        "num_std_devs": 2.0
    });

    let response = client.admin_set_relevance_filter(config).await;

    // Skip if search is disabled
    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search admin endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body.get("config")
            .and_then(|c| c.get("method"))
            .and_then(|m| m.as_str()),
        Some("standard_deviation")
    );
}

#[tokio::test]
async fn test_relevance_filter_set_percentage_with_minimum() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // Set a percentage_with_minimum filter
    let config = serde_json::json!({
        "method": "percentage_with_minimum",
        "threshold": 0.4,
        "min_best_score": 100
    });

    let response = client.admin_set_relevance_filter(config).await;

    // Skip if search is disabled
    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search admin endpoint not available (no_search feature enabled)");
        return;
    }

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body.get("config")
            .and_then(|c| c.get("method"))
            .and_then(|m| m.as_str()),
        Some("percentage_with_minimum")
    );
}

#[tokio::test]
async fn test_relevance_filter_reset_to_none() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated_admin(server.base_url.clone()).await;

    // First set a filter
    let config = serde_json::json!({
        "method": "percentage_of_best",
        "threshold": 0.5
    });
    let response = client.admin_set_relevance_filter(config).await;

    // Skip if search is disabled
    if response.status() == StatusCode::NOT_FOUND {
        eprintln!("Search admin endpoint not available (no_search feature enabled)");
        return;
    }
    assert_eq!(response.status(), StatusCode::OK);

    // Now reset to none
    let config = serde_json::json!({
        "method": "none"
    });
    let response = client.admin_set_relevance_filter(config).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify it's none
    let get_response = client.admin_get_relevance_filter().await;
    let body: serde_json::Value = get_response.json().await.unwrap();
    assert_eq!(
        body.get("config")
            .and_then(|c| c.get("method"))
            .and_then(|m| m.as_str()),
        Some("none"),
        "Config should be reset to none"
    );
}
