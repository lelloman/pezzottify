//! Coarse-grained contracts for top-level route composition.
//!
//! Domain suites assert successful payloads. These tests deliberately assert the
//! boundaries most likely to regress while routers are moved between modules:
//! nesting, authentication, disabled-feature behavior, and optional subsystem wiring.

mod common;

use common::{TestClient, TestServer};
use reqwest::{Method, StatusCode};

#[tokio::test]
async fn protected_route_groups_remain_mounted_and_authenticated() {
    let server = TestServer::spawn().await;
    let client = reqwest::Client::new();
    let routes = [
        (
            Method::GET,
            "/v1/content/artist/unknown",
            StatusCode::UNAUTHORIZED,
        ),
        (Method::GET, "/v1/user/playlists", StatusCode::UNAUTHORIZED),
        (Method::GET, "/v1/sync/state", StatusCode::UNAUTHORIZED),
        (Method::GET, "/v1/admin/users", StatusCode::UNAUTHORIZED),
        (Method::GET, "/v1/download/limits", StatusCode::UNAUTHORIZED),
        (
            Method::GET,
            "/v1/ingestion/my-jobs",
            StatusCode::UNAUTHORIZED,
        ),
        // Without WebSocket upgrade headers Axum rejects the request before the
        // Session extractor runs. The authenticated WebSocket suites cover the
        // upgraded MCP/playback paths separately.
        (Method::GET, "/v1/mcp", StatusCode::BAD_REQUEST),
    ];

    for (method, path, expected) in routes {
        let response = client
            .request(method.clone(), format!("{}{}", server.base_url, path))
            .send()
            .await
            .unwrap_or_else(|error| panic!("request to {path} failed: {error}"));
        assert_eq!(
            response.status(),
            expected,
            "unexpected unauthenticated contract for {method} {path}"
        );
    }
}

#[tokio::test]
async fn intentionally_disabled_changelog_and_image_mutations_return_501() {
    let server = TestServer::spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;

    let changelog = admin.admin_list_changelog_batches(None).await;
    assert_eq!(changelog.status(), StatusCode::NOT_IMPLEMENTED);
    assert_eq!(
        changelog.text().await.unwrap(),
        "Changelog not available for Spotify catalog"
    );

    let image = admin
        .client
        .post(format!("{}/v1/content/image", server.base_url))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(image.status(), StatusCode::NOT_IMPLEMENTED);
    assert_eq!(
        image.text().await.unwrap(),
        "Image CRUD not yet implemented"
    );
}

#[tokio::test]
async fn test_builder_can_toggle_password_auth_and_optional_managers() {
    let password_disabled = TestServer::builder()
        .with_password_auth_disabled()
        .spawn()
        .await;
    let client = TestClient::new(password_disabled.base_url.clone());
    let login = client.login("testuser", "testpass123").await;
    assert_eq!(login.status(), StatusCode::FORBIDDEN);

    let download_enabled = TestServer::builder().with_download_manager().spawn().await;
    let client = TestClient::authenticated_admin(download_enabled.base_url.clone()).await;
    let limits = client.download_limits().await;
    assert_eq!(limits.status(), StatusCode::OK);

    let ingestion_enabled = TestServer::builder().with_ingestion().spawn().await;
    let admin = TestClient::authenticated_admin(ingestion_enabled.base_url.clone()).await;
    let jobs = admin
        .client
        .get(format!(
            "{}/v1/ingestion/my-jobs",
            ingestion_enabled.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(jobs.status(), StatusCode::OK);
}
