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
    /// The underlying reqwest client (public for custom requests in tests)
    pub client: reqwest::Client,
    /// The base URL of the test server
    pub base_url: String,
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
        self.login_with_device(handle, password, "test-device-uuid-12345")
            .await
    }

    /// POST /v1/auth/login with custom device UUID
    ///
    /// Useful for testing multi-device scenarios like WebSocket sync.
    pub async fn login_with_device(
        &self,
        handle: &str,
        password: &str,
        device_uuid: &str,
    ) -> Response {
        self.client
            .post(format!("{}/v1/auth/login", self.base_url))
            .json(&json!({
                "user_handle": handle,
                "password": password,
                "device_uuid": device_uuid,
                "device_type": "web",
                "device_name": format!("Test Client {}", device_uuid)
            }))
            .send()
            .await
            .expect("Login request failed")
    }

    /// Creates an authenticated client for a specific device
    ///
    /// Use this for testing multi-device scenarios.
    pub async fn authenticated_with_device(base_url: String, device_uuid: &str) -> Self {
        let client = Self::new(base_url);

        let response = client
            .login_with_device(TEST_USER, TEST_PASS, device_uuid)
            .await;
        assert_eq!(
            response.status(),
            reqwest::StatusCode::CREATED,
            "Test user authentication failed: {:?}",
            response.text().await
        );

        client
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

    /// POST /v1/content/batch - Batch fetch multiple artists, albums, and tracks
    pub async fn post_batch_content(&self, body: serde_json::Value) -> Response {
        self.client
            .post(format!("{}/v1/content/batch", self.base_url))
            .json(&body)
            .send()
            .await
            .expect("Post batch content request failed")
    }

    /// GET /v1/content/image/{id} - Get image for an item (album or artist ID)
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

    /// POST /v1/user/liked/{content_type}/{content_id}
    pub async fn add_liked_content(&self, content_type: &str, content_id: &str) -> Response {
        self.client
            .post(format!(
                "{}/v1/user/liked/{}/{}",
                self.base_url, content_type, content_id
            ))
            .send()
            .await
            .expect("Add liked content request failed")
    }

    /// DELETE /v1/user/liked/{content_type}/{content_id}
    pub async fn remove_liked_content(&self, content_type: &str, content_id: &str) -> Response {
        self.client
            .delete(format!(
                "{}/v1/user/liked/{}/{}",
                self.base_url, content_type, content_id
            ))
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
    // Playlist Endpoints
    // ========================================================================

    /// POST /v1/user/playlist
    pub async fn create_playlist(&self, name: &str, track_ids: Vec<&str>) -> Response {
        self.client
            .post(format!("{}/v1/user/playlist", self.base_url))
            .json(&serde_json::json!({
                "name": name,
                "track_ids": track_ids
            }))
            .send()
            .await
            .expect("Create playlist request failed")
    }

    /// GET /v1/user/playlists
    pub async fn get_playlists(&self) -> Response {
        self.client
            .get(format!("{}/v1/user/playlists", self.base_url))
            .send()
            .await
            .expect("Get playlists request failed")
    }

    /// GET /v1/user/playlist/{id}
    pub async fn get_playlist(&self, id: &str) -> Response {
        self.client
            .get(format!("{}/v1/user/playlist/{}", self.base_url, id))
            .send()
            .await
            .expect("Get playlist request failed")
    }

    /// PUT /v1/user/playlist/{id}
    pub async fn update_playlist(
        &self,
        id: &str,
        name: Option<&str>,
        track_ids: Option<Vec<&str>>,
    ) -> Response {
        let mut body = serde_json::Map::new();
        if let Some(n) = name {
            body.insert("name".to_string(), serde_json::json!(n));
        }
        if let Some(tracks) = track_ids {
            body.insert("track_ids".to_string(), serde_json::json!(tracks));
        }
        self.client
            .put(format!("{}/v1/user/playlist/{}", self.base_url, id))
            .json(&body)
            .send()
            .await
            .expect("Update playlist request failed")
    }

    /// DELETE /v1/user/playlist/{id}
    pub async fn delete_playlist(&self, id: &str) -> Response {
        self.client
            .delete(format!("{}/v1/user/playlist/{}", self.base_url, id))
            .send()
            .await
            .expect("Delete playlist request failed")
    }

    /// PUT /v1/user/playlist/{id}/add
    pub async fn add_tracks_to_playlist(&self, id: &str, track_ids: Vec<&str>) -> Response {
        self.client
            .put(format!("{}/v1/user/playlist/{}/add", self.base_url, id))
            .json(&serde_json::json!({
                "tracks_ids": track_ids
            }))
            .send()
            .await
            .expect("Add tracks to playlist request failed")
    }

    /// PUT /v1/user/playlist/{id}/remove
    pub async fn remove_tracks_from_playlist(&self, id: &str, positions: Vec<usize>) -> Response {
        self.client
            .put(format!("{}/v1/user/playlist/{}/remove", self.base_url, id))
            .json(&serde_json::json!({
                "tracks_positions": positions
            }))
            .send()
            .await
            .expect("Remove tracks from playlist request failed")
    }

    // ========================================================================
    // Search Endpoints
    // ========================================================================

    /// POST /v1/content/search
    pub async fn search(&self, query: &str) -> Response {
        self.client
            .post(format!("{}/v1/content/search", self.base_url))
            .json(&serde_json::json!({
                "query": query,
                "resolve": false
            }))
            .send()
            .await
            .expect("Search request failed")
    }

    /// POST /v1/content/search with resolve=true
    pub async fn search_resolved(&self, query: &str) -> Response {
        self.client
            .post(format!("{}/v1/content/search", self.base_url))
            .json(&serde_json::json!({
                "query": query,
                "resolve": true
            }))
            .send()
            .await
            .expect("Search resolved request failed")
    }

    /// POST /v1/content/search with filters
    pub async fn search_with_filters(&self, query: &str, filters: Vec<&str>) -> Response {
        self.client
            .post(format!("{}/v1/content/search", self.base_url))
            .json(&serde_json::json!({
                "query": query,
                "resolve": true,
                "filters": filters
            }))
            .send()
            .await
            .expect("Search with filters request failed")
    }

    /// GET /v1/content/search/stream?q={query}
    ///
    /// Returns an SSE stream of search sections. The response is streamed
    /// and should be processed as Server-Sent Events.
    pub async fn search_stream(&self, query: &str) -> Response {
        self.client
            .get(format!(
                "{}/v1/content/search/stream?q={}",
                self.base_url,
                urlencoding::encode(query)
            ))
            .send()
            .await
            .expect("Streaming search request failed")
    }

    // ========================================================================
    // Listening Stats Endpoints
    // ========================================================================

    /// POST /v1/user/listening
    pub async fn post_listening_event(
        &self,
        track_id: &str,
        duration_seconds: u32,
        track_duration_seconds: u32,
    ) -> Response {
        self.client
            .post(format!("{}/v1/user/listening", self.base_url))
            .json(&serde_json::json!({
                "track_id": track_id,
                "duration_seconds": duration_seconds,
                "track_duration_seconds": track_duration_seconds
            }))
            .send()
            .await
            .expect("Post listening event request failed")
    }

    /// POST /v1/user/listening with full payload
    pub async fn post_listening_event_full(
        &self,
        track_id: &str,
        session_id: Option<&str>,
        started_at: Option<u64>,
        ended_at: Option<u64>,
        duration_seconds: u32,
        track_duration_seconds: u32,
        seek_count: Option<u32>,
        pause_count: Option<u32>,
        playback_context: Option<&str>,
        client_type: Option<&str>,
    ) -> Response {
        let mut body = serde_json::json!({
            "track_id": track_id,
            "duration_seconds": duration_seconds,
            "track_duration_seconds": track_duration_seconds
        });
        if let Some(sid) = session_id {
            body["session_id"] = serde_json::json!(sid);
        }
        if let Some(sa) = started_at {
            body["started_at"] = serde_json::json!(sa);
        }
        if let Some(ea) = ended_at {
            body["ended_at"] = serde_json::json!(ea);
        }
        if let Some(sc) = seek_count {
            body["seek_count"] = serde_json::json!(sc);
        }
        if let Some(pc) = pause_count {
            body["pause_count"] = serde_json::json!(pc);
        }
        if let Some(ctx) = playback_context {
            body["playback_context"] = serde_json::json!(ctx);
        }
        if let Some(ct) = client_type {
            body["client_type"] = serde_json::json!(ct);
        }
        self.client
            .post(format!("{}/v1/user/listening", self.base_url))
            .json(&body)
            .send()
            .await
            .expect("Post listening event full request failed")
    }

    /// GET /v1/user/listening/summary
    pub async fn get_listening_summary(
        &self,
        start_date: Option<u32>,
        end_date: Option<u32>,
    ) -> Response {
        let mut url = format!("{}/v1/user/listening/summary", self.base_url);
        let mut params = vec![];
        if let Some(sd) = start_date {
            params.push(format!("start_date={}", sd));
        }
        if let Some(ed) = end_date {
            params.push(format!("end_date={}", ed));
        }
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }
        self.client
            .get(&url)
            .send()
            .await
            .expect("Get listening summary request failed")
    }

    /// GET /v1/user/listening/history
    pub async fn get_listening_history(&self, limit: Option<usize>) -> Response {
        let mut url = format!("{}/v1/user/listening/history", self.base_url);
        if let Some(l) = limit {
            url = format!("{}?limit={}", url, l);
        }
        self.client
            .get(&url)
            .send()
            .await
            .expect("Get listening history request failed")
    }

    /// GET /v1/user/listening/events
    pub async fn get_listening_events(
        &self,
        start_date: Option<u32>,
        end_date: Option<u32>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Response {
        let mut url = format!("{}/v1/user/listening/events", self.base_url);
        let mut params = vec![];
        if let Some(sd) = start_date {
            params.push(format!("start_date={}", sd));
        }
        if let Some(ed) = end_date {
            params.push(format!("end_date={}", ed));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(o) = offset {
            params.push(format!("offset={}", o));
        }
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }
        self.client
            .get(&url)
            .send()
            .await
            .expect("Get listening events request failed")
    }

    /// GET /v1/admin/listening/daily
    pub async fn admin_get_daily_listening_stats(
        &self,
        start_date: Option<u32>,
        end_date: Option<u32>,
    ) -> Response {
        let mut url = format!("{}/v1/admin/listening/daily", self.base_url);
        let mut params = vec![];
        if let Some(sd) = start_date {
            params.push(format!("start_date={}", sd));
        }
        if let Some(ed) = end_date {
            params.push(format!("end_date={}", ed));
        }
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }
        self.client
            .get(&url)
            .send()
            .await
            .expect("Admin get daily listening stats request failed")
    }

    /// GET /v1/admin/listening/top-tracks
    pub async fn admin_get_top_tracks(
        &self,
        start_date: Option<u32>,
        end_date: Option<u32>,
        limit: Option<usize>,
    ) -> Response {
        let mut url = format!("{}/v1/admin/listening/top-tracks", self.base_url);
        let mut params = vec![];
        if let Some(sd) = start_date {
            params.push(format!("start_date={}", sd));
        }
        if let Some(ed) = end_date {
            params.push(format!("end_date={}", ed));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }
        self.client
            .get(&url)
            .send()
            .await
            .expect("Admin get top tracks request failed")
    }

    /// GET /v1/admin/listening/track/{track_id}
    pub async fn admin_get_track_listening_stats(
        &self,
        track_id: &str,
        start_date: Option<u32>,
        end_date: Option<u32>,
    ) -> Response {
        let mut url = format!("{}/v1/admin/listening/track/{}", self.base_url, track_id);
        let mut params = vec![];
        if let Some(sd) = start_date {
            params.push(format!("start_date={}", sd));
        }
        if let Some(ed) = end_date {
            params.push(format!("end_date={}", ed));
        }
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }
        self.client
            .get(&url)
            .send()
            .await
            .expect("Admin get track listening stats request failed")
    }

    /// GET /v1/admin/listening/users/{handle}/summary
    pub async fn admin_get_user_listening_summary(
        &self,
        handle: &str,
        start_date: Option<u32>,
        end_date: Option<u32>,
    ) -> Response {
        let mut url = format!(
            "{}/v1/admin/listening/users/{}/summary",
            self.base_url, handle
        );
        let mut params = vec![];
        if let Some(sd) = start_date {
            params.push(format!("start_date={}", sd));
        }
        if let Some(ed) = end_date {
            params.push(format!("end_date={}", ed));
        }
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }
        self.client
            .get(&url)
            .send()
            .await
            .expect("Admin get user listening summary request failed")
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

    // ========================================================================
    // Admin Changelog Endpoints
    // ========================================================================

    /// POST /v1/admin/changelog/batch
    pub async fn admin_create_changelog_batch(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Response {
        let mut body = serde_json::json!({ "name": name });
        if let Some(desc) = description {
            body["description"] = serde_json::json!(desc);
        }
        self.client
            .post(format!("{}/v1/admin/changelog/batch", self.base_url))
            .json(&body)
            .send()
            .await
            .expect("Admin create changelog batch request failed")
    }

    /// GET /v1/admin/changelog/batches
    pub async fn admin_list_changelog_batches(&self, is_open: Option<bool>) -> Response {
        let mut url = format!("{}/v1/admin/changelog/batches", self.base_url);
        if let Some(open) = is_open {
            url = format!("{}?is_open={}", url, open);
        }
        self.client
            .get(&url)
            .send()
            .await
            .expect("Admin list changelog batches request failed")
    }

    /// GET /v1/admin/changelog/batch/{id}
    pub async fn admin_get_changelog_batch(&self, batch_id: &str) -> Response {
        self.client
            .get(format!(
                "{}/v1/admin/changelog/batch/{}",
                self.base_url, batch_id
            ))
            .send()
            .await
            .expect("Admin get changelog batch request failed")
    }

    /// POST /v1/admin/changelog/batch/{id}/close
    pub async fn admin_close_changelog_batch(&self, batch_id: &str) -> Response {
        self.client
            .post(format!(
                "{}/v1/admin/changelog/batch/{}/close",
                self.base_url, batch_id
            ))
            .send()
            .await
            .expect("Admin close changelog batch request failed")
    }

    /// DELETE /v1/admin/changelog/batch/{id}
    pub async fn admin_delete_changelog_batch(&self, batch_id: &str) -> Response {
        self.client
            .delete(format!(
                "{}/v1/admin/changelog/batch/{}",
                self.base_url, batch_id
            ))
            .send()
            .await
            .expect("Admin delete changelog batch request failed")
    }

    /// GET /v1/admin/changelog/batch/{id}/changes
    pub async fn admin_get_changelog_batch_changes(&self, batch_id: &str) -> Response {
        self.client
            .get(format!(
                "{}/v1/admin/changelog/batch/{}/changes",
                self.base_url, batch_id
            ))
            .send()
            .await
            .expect("Admin get changelog batch changes request failed")
    }

    /// GET /v1/admin/changelog/entity/{entity_type}/{entity_id}
    pub async fn admin_get_changelog_entity_history(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Response {
        self.client
            .get(format!(
                "{}/v1/admin/changelog/entity/{}/{}",
                self.base_url, entity_type, entity_id
            ))
            .send()
            .await
            .expect("Admin get changelog entity history request failed")
    }

    // =========================================================================
    // What's New (User) API
    // =========================================================================

    /// GET /v1/content/whatsnew
    pub async fn get_whats_new(&self, limit: Option<usize>) -> Response {
        let mut url = format!("{}/v1/content/whatsnew", self.base_url);
        if let Some(limit) = limit {
            url = format!("{}?limit={}", url, limit);
        }
        self.client
            .get(url)
            .send()
            .await
            .expect("Get what's new request failed")
    }

    // =========================================================================
    // User Settings API
    // =========================================================================

    /// GET /v1/user/settings
    pub async fn get_user_settings(&self) -> Response {
        self.client
            .get(format!("{}/v1/user/settings", self.base_url))
            .send()
            .await
            .expect("Get user settings request failed")
    }

    /// PUT /v1/user/settings (deprecated - use update_user_settings_json)
    #[allow(dead_code)]
    pub async fn update_user_settings(
        &self,
        settings: std::collections::HashMap<&str, &str>,
    ) -> Response {
        self.client
            .put(format!("{}/v1/user/settings", self.base_url))
            .json(&json!({ "settings": settings }))
            .send()
            .await
            .expect("Update user settings request failed")
    }

    /// PUT /v1/user/settings with JSON body
    pub async fn update_user_settings_json(&self, body: serde_json::Value) -> Response {
        self.client
            .put(format!("{}/v1/user/settings", self.base_url))
            .json(&body)
            .send()
            .await
            .expect("Update user settings request failed")
    }

    // =========================================================================
    // Sync API
    // =========================================================================

    /// GET /v1/sync/state
    pub async fn get_sync_state(&self) -> Response {
        self.client
            .get(format!("{}/v1/sync/state", self.base_url))
            .send()
            .await
            .expect("Get sync state request failed")
    }

    /// GET /v1/sync/events?since={since}
    pub async fn get_sync_events(&self, since: i64) -> Response {
        self.client
            .get(format!("{}/v1/sync/events?since={}", self.base_url, since))
            .send()
            .await
            .expect("Get sync events request failed")
    }

    /// POST /v1/user/notifications/{id}/read
    pub async fn mark_notification_read(&self, notification_id: &str) -> Response {
        self.client
            .post(format!(
                "{}/v1/user/notifications/{}/read",
                self.base_url, notification_id
            ))
            .send()
            .await
            .expect("Mark notification read request failed")
    }

    // ========================================================================
    // Admin Jobs Endpoints
    // ========================================================================

    /// GET /v1/admin/jobs
    pub async fn admin_list_jobs(&self) -> Response {
        self.client
            .get(format!("{}/v1/admin/jobs", self.base_url))
            .send()
            .await
            .expect("List jobs request failed")
    }

    /// GET /v1/admin/jobs/{job_id}
    pub async fn admin_get_job(&self, job_id: &str) -> Response {
        self.client
            .get(format!("{}/v1/admin/jobs/{}", self.base_url, job_id))
            .send()
            .await
            .expect("Get job request failed")
    }

    /// POST /v1/admin/jobs/{job_id}/trigger
    pub async fn admin_trigger_job(&self, job_id: &str) -> Response {
        self.client
            .post(format!(
                "{}/v1/admin/jobs/{}/trigger",
                self.base_url, job_id
            ))
            .send()
            .await
            .expect("Trigger job request failed")
    }

    /// GET /v1/admin/jobs/{job_id}/history?limit={limit}
    pub async fn admin_get_job_history(&self, job_id: &str, limit: usize) -> Response {
        self.client
            .get(format!(
                "{}/v1/admin/jobs/{}/history?limit={}",
                self.base_url, job_id, limit
            ))
            .send()
            .await
            .expect("Get job history request failed")
    }

    // ========================================================================
    // Search Admin Endpoints
    // ========================================================================

    /// GET /v1/admin/search/relevance-filter
    pub async fn admin_get_relevance_filter(&self) -> Response {
        self.client
            .get(format!(
                "{}/v1/admin/search/relevance-filter",
                self.base_url
            ))
            .send()
            .await
            .expect("Get relevance filter request failed")
    }

    /// PUT /v1/admin/search/relevance-filter
    pub async fn admin_set_relevance_filter(&self, config: serde_json::Value) -> Response {
        self.client
            .put(format!(
                "{}/v1/admin/search/relevance-filter",
                self.base_url
            ))
            .json(&config)
            .send()
            .await
            .expect("Set relevance filter request failed")
    }

    // ========================================================================
    // Download Manager Endpoints
    // ========================================================================

    /// GET /v1/download/limits
    pub async fn download_limits(&self) -> Response {
        self.client
            .get(format!("{}/v1/download/limits", self.base_url))
            .send()
            .await
            .expect("Download limits request failed")
    }

    /// GET /v1/download/my-requests
    pub async fn download_my_requests(&self) -> Response {
        self.client
            .get(format!("{}/v1/download/my-requests", self.base_url))
            .send()
            .await
            .expect("Download my requests failed")
    }

    /// POST /v1/download/request/album
    pub async fn download_request_album(
        &self,
        album_id: &str,
        album_name: &str,
        artist_name: &str,
    ) -> Response {
        self.client
            .post(format!("{}/v1/download/request/album", self.base_url))
            .json(&json!({
                "album_id": album_id,
                "album_name": album_name,
                "artist_name": artist_name
            }))
            .send()
            .await
            .expect("Download request album failed")
    }

    /// GET /v1/download/admin/stats
    pub async fn download_admin_stats(&self) -> Response {
        self.client
            .get(format!("{}/v1/download/admin/stats", self.base_url))
            .send()
            .await
            .expect("Download admin stats request failed")
    }

    /// GET /v1/download/admin/failed
    pub async fn download_admin_failed(&self) -> Response {
        self.client
            .get(format!("{}/v1/download/admin/failed", self.base_url))
            .send()
            .await
            .expect("Download admin failed request failed")
    }

    /// GET /v1/download/admin/activity
    pub async fn download_admin_activity(&self) -> Response {
        self.client
            .get(format!("{}/v1/download/admin/activity", self.base_url))
            .send()
            .await
            .expect("Download admin activity request failed")
    }

    /// GET /v1/download/admin/requests
    pub async fn download_admin_requests(&self) -> Response {
        self.client
            .get(format!("{}/v1/download/admin/requests", self.base_url))
            .send()
            .await
            .expect("Download admin requests failed")
    }

    /// POST /v1/download/admin/retry/{id}
    pub async fn download_admin_retry(&self, id: &str) -> Response {
        self.client
            .post(format!("{}/v1/download/admin/retry/{}", self.base_url, id))
            .send()
            .await
            .expect("Download admin retry request failed")
    }

    /// GET /v1/download/admin/audit
    pub async fn download_admin_audit(&self) -> Response {
        self.client
            .get(format!("{}/v1/download/admin/audit", self.base_url))
            .send()
            .await
            .expect("Download admin audit request failed")
    }

    /// GET /v1/download/admin/audit/item/{id}
    pub async fn download_admin_audit_item(&self, id: &str) -> Response {
        self.client
            .get(format!(
                "{}/v1/download/admin/audit/item/{}",
                self.base_url, id
            ))
            .send()
            .await
            .expect("Download admin audit item request failed")
    }

    /// GET /v1/download/admin/audit/user/{id}
    pub async fn download_admin_audit_user(&self, id: &str) -> Response {
        self.client
            .get(format!(
                "{}/v1/download/admin/audit/user/{}",
                self.base_url, id
            ))
            .send()
            .await
            .expect("Download admin audit user request failed")
    }

    // ========================================================================
    // Skeleton Sync Endpoints
    // ========================================================================

    /// GET /v1/catalog/skeleton
    pub async fn get_skeleton(&self) -> Response {
        self.client
            .get(format!("{}/v1/catalog/skeleton", self.base_url))
            .send()
            .await
            .expect("Get skeleton request failed")
    }

    /// GET /v1/catalog/skeleton/version
    pub async fn get_skeleton_version(&self) -> Response {
        self.client
            .get(format!("{}/v1/catalog/skeleton/version", self.base_url))
            .send()
            .await
            .expect("Get skeleton version request failed")
    }

    /// GET /v1/catalog/skeleton/delta?since={since}
    pub async fn get_skeleton_delta(&self, since: i64) -> Response {
        self.client
            .get(format!(
                "{}/v1/catalog/skeleton/delta?since={}",
                self.base_url, since
            ))
            .send()
            .await
            .expect("Get skeleton delta request failed")
    }
}
