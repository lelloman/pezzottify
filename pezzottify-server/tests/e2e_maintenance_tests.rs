//! End-to-end coverage for server-wide maintenance mode.

mod common;

use common::{TestClient, TestServer};
use reqwest::StatusCode;
use serde_json::{json, Value};

const MAINTENANCE_STATE_KEY: &str = "server.maintenance.v1";

#[tokio::test]
async fn maintenance_mode_blocks_player_routes_and_can_be_disabled_by_admin() {
    let server = TestServer::spawn().await;
    let status_url = format!("{}/v1/status", server.base_url);
    let admin_url = format!("{}/v1/admin/maintenance", server.base_url);
    let whatsnew_url = format!("{}/v1/content/whatsnew", server.base_url);
    let anonymous = reqwest::Client::new();

    let status: Value = anonymous
        .get(&status_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["enabled"], false);
    assert_eq!(status["message"], Value::Null);

    let user = TestClient::authenticated(server.base_url.clone()).await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;

    assert_eq!(
        anonymous.get(&admin_url).send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        user.client.get(&admin_url).send().await.unwrap().status(),
        StatusCode::FORBIDDEN
    );

    let response = admin
        .client
        .put(&admin_url)
        .json(&json!({
            "enabled": true,
            "message": "  Back after the upgrade  ",
            "estimated_end": "2026-10-21T10:00:00+02:00"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let state: Value = response.json().await.unwrap();
    assert_eq!(state["enabled"], true);
    assert_eq!(state["message"], "Back after the upgrade");
    assert_eq!(state["estimated_end"], "2026-10-21T08:00:00Z");

    let status: Value = anonymous
        .get(&status_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["enabled"], true);
    assert_eq!(status["message"], "Back after the upgrade");

    let blocked = admin.client.get(&whatsnew_url).send().await.unwrap();
    assert_eq!(blocked.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(blocked.headers()["x-maintenance-mode"], "1");

    // Auth and admin routes stay reachable so admins can turn the flag off.
    assert_eq!(user.get_session().await.status(), StatusCode::OK);
    assert_eq!(
        admin.client.get(&admin_url).send().await.unwrap().status(),
        StatusCode::OK
    );

    let persisted = server
        .reopen_server_store()
        .get_state(MAINTENANCE_STATE_KEY)
        .unwrap()
        .expect("maintenance state should be persisted");
    assert!(persisted.contains("\"enabled\":true"));

    let response = admin
        .client
        .put(&admin_url)
        .json(&json!({"enabled": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    assert_eq!(
        user.client
            .get(&whatsnew_url)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn maintenance_rejects_invalid_eta_and_keeps_existing_state() {
    let server = TestServer::spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let admin_url = format!("{}/v1/admin/maintenance", server.base_url);

    let response = admin
        .client
        .put(&admin_url)
        .json(&json!({"enabled": true, "estimated_end": "soon"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let state: Value = admin
        .client
        .get(&admin_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(state["enabled"], false);
}
