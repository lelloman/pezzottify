//! HTTP client for end-to-end tests
//!
//! This module provides a high-level HTTP client that wraps reqwest
//! and provides methods for all catalog-server endpoints.
//!
//! When API routes or request formats change, update only this file.

use super::constants::*;
use reqwest::Response;
use serde_json::json;
use std::time::Duration;

/// HTTP test client with cookie-based session management
pub struct TestClient {
    client: reqwest::Client,
    base_url: String,
}

impl TestClient {
    /// Creates a new unauthenticated client
    ///
    /// Use this for testing authentication flows.
    /// For most tests, use `authenticated()` or `authenticated_admin()` instead.
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .cookie_store(true) // Automatically handle session cookies
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("Failed to build reqwest client");

        Self { client, base_url }
    }

    /// Creates a client pre-authenticated as a regular user
    ///
    /// This is the most common way to create a test client.
    /// The client is ready to make authenticated requests.
    ///
    /// # Panics
    ///
    /// Panics if authentication fails (indicates test infrastructure problem).
    pub async fn authenticated(base_url: String) -> Self {
        let client = Self::new(base_url);

        let response = client.login(TEST_USER, TEST_PASS).await;
        assert_eq!(
            response.status(),
            reqwest::StatusCode::CREATED,
            "Test user authentication failed: {:?}",
            response.text().await
        );

        client
    }

    /// Creates a client pre-authenticated as an admin user
    ///
    /// Use this for testing admin-only endpoints.
    ///
    /// # Panics
    ///
    /// Panics if authentication fails (indicates test infrastructure problem).
    pub async fn authenticated_admin(base_url: String) -> Self {
        let client = Self::new(base_url);

        let response = client.login(ADMIN_USER, ADMIN_PASS).await;
        assert_eq!(
            response.status(),
            reqwest::StatusCode::CREATED,
            "Admin authentication failed: {:?}",
            response.text().await
        );

        client
    }

    // ========================================================================
    // Authentication Endpoints
    // ========================================================================

    /// POST /v1/auth/login
    pub async fn login(&self, handle: &str, password: &str) -> Response {
        self.client
            .post(format!("{}/v1/auth/login", self.base_url))
            .json(&json!({
                "user_handle": handle,
                "password": password
            }))
            .send()
            .await
            .expect("Login request failed")
    }

    /// GET /v1/auth/logout
    pub async fn logout(&self) -> Response {
        self.client
            .get(format!("{}/v1/auth/logout", self.base_url))
            .send()
            .await
            .expect("Logout request failed")
    }

    /// GET /v1/auth/session
    pub async fn get_session(&self) -> Response {
        self.client
            .get(format!("{}/v1/auth/session", self.base_url))
            .send()
            .await
            .expect("Get session request failed")
    }

    // ========================================================================
    // Catalog Endpoints
    // ========================================================================

    /// GET /v1/content/artist/{id}
    pub async fn get_artist(&self, id: &str) -> Response {
        self.client
            .get(format!("{}/v1/content/artist/{}", self.base_url, id))
            .send()
            .await
            .expect("Get artist request failed")
    }

    /// GET /v1/content/artist/{id}/discography
    pub async fn get_artist_discography(&self, id: &str) -> Response {
        self.client
            .get(format!(
                "{}/v1/content/artist/{}/discography",
                self.base_url, id
            ))
            .send()
            .await
            .expect("Get artist discography request failed")
    }

    /// GET /v1/content/album/{id}
    pub async fn get_album(&self, id: &str) -> Response {
        self.client
            .get(format!("{}/v1/content/album/{}", self.base_url, id))
            .send()
            .await
            .expect("Get album request failed")
    }

    /// GET /v1/content/track/{id}
    pub async fn get_track(&self, id: &str) -> Response {
        self.client
            .get(format!("{}/v1/content/track/{}", self.base_url, id))
            .send()
            .await
            .expect("Get track request failed")
    }

    /// GET /v1/content/track/{id}/resolved
    pub async fn get_resolved_track(&self, id: &str) -> Response {
        self.client
            .get(format!(
                "{}/v1/content/track/{}/resolved",
                self.base_url, id
            ))
            .send()
            .await
            .expect("Get resolved track request failed")
    }

    /// GET /v1/content/image/{id}
    pub async fn get_image(&self, id: &str) -> Response {
        self.client
            .get(format!("{}/v1/content/image/{}", self.base_url, id))
            .send()
            .await
            .expect("Get image request failed")
    }

    // ========================================================================
    // Streaming Endpoints
    // ========================================================================

    /// GET /v1/content/stream/{id}
    pub async fn stream_track(&self, id: &str) -> Response {
        self.client
            .get(format!("{}/v1/content/stream/{}", self.base_url, id))
            .send()
            .await
            .expect("Stream track request failed")
    }

    /// GET /v1/content/stream/{id} with Range header
    pub async fn stream_track_with_range(&self, id: &str, range: &str) -> Response {
        self.client
            .get(format!("{}/v1/content/stream/{}", self.base_url, id))
            .header("Range", range)
            .send()
            .await
            .expect("Stream track with range request failed")
    }

    // ========================================================================
    // User Content Endpoints
    // ========================================================================

    /// PUT /v1/user/liked/{content_id}
    pub async fn add_liked_content(&self, content_id: &str) -> Response {
        self.client
            .put(format!("{}/v1/user/liked/{}", self.base_url, content_id))
            .send()
            .await
            .expect("Add liked content request failed")
    }

    /// DELETE /v1/user/liked/{content_id}
    pub async fn remove_liked_content(&self, content_id: &str) -> Response {
        self.client
            .delete(format!("{}/v1/user/liked/{}", self.base_url, content_id))
            .send()
            .await
            .expect("Remove liked content request failed")
    }

    /// GET /v1/user/liked/{content_type}
    ///
    /// content_type: "artist", "album", or "track"
    pub async fn get_liked_content(&self, content_type: &str) -> Response {
        self.client
            .get(format!("{}/v1/user/liked/{}", self.base_url, content_type))
            .send()
            .await
            .expect("Get liked content request failed")
    }

    /// GET /v1/user/liked/{content_id}/status
    pub async fn get_liked_content_status(&self, content_id: &str) -> Response {
        self.client
            .get(format!(
                "{}/v1/user/liked/{}/status",
                self.base_url, content_id
            ))
            .send()
            .await
            .expect("Get liked content status request failed")
    }

    // ========================================================================
    // Search Endpoints
    // ========================================================================

    /// GET /v1/search?q={query}
    pub async fn search(&self, query: &str) -> Response {
        self.client
            .get(format!("{}/v1/search", self.base_url))
            .query(&[("q", query)])
            .send()
            .await
            .expect("Search request failed")
    }

    // ========================================================================
    // Health Check / System Endpoints
    // ========================================================================

    /// GET /
    pub async fn get_statics(&self) -> Response {
        self.client
            .get(format!("{}/", self.base_url))
            .send()
            .await
            .expect("Get statics request failed")
    }
}
