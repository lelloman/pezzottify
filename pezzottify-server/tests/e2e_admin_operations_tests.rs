//! Characterization tests for server-store, shows, and backup admin operations.

mod common;

use common::{TestClient, TestServer};
use reqwest::StatusCode;
use serde_json::{json, Value};

#[tokio::test]
async fn show_draft_is_admin_visible_publicly_hidden_and_deletable() {
    let server = TestServer::builder().with_available_catalog().spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = admin
        .client
        .post(format!("{}/v1/content/admin/shows/drafts", server.base_url))
        .json(&json!({
            "brief": "A compact tour through the test catalog",
            "target_duration_minutes": 30,
            "language": "en"
        }))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let response_body = response.text().await.unwrap();
    assert_eq!(status, StatusCode::CREATED, "{response_body}");
    let show: Value = serde_json::from_str(&response_body).unwrap();
    let show_id = show["id"].as_str().unwrap();
    assert_eq!(show["status"], "script_ready");

    let response = admin
        .client
        .get(format!("{}/v1/content/admin/shows", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let shows: Vec<Value> = response.json().await.unwrap();
    assert!(shows.iter().any(|show| show["id"] == show_id));

    let response = admin
        .client
        .get(format!("{}/v1/content/show/{}", server.base_url, show_id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = admin
        .client
        .delete(format!(
            "{}/v1/content/admin/shows/{}",
            server.base_url, show_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn bug_report_round_trip_preserves_user_and_admin_contracts() {
    let server = TestServer::spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = user
        .client
        .post(format!("{}/v1/user/bug-report", server.base_url))
        .json(&json!({
            "title": "Test report",
            "description": "Something reproducible happened",
            "client_type": "rust-integration"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    let report_id = body["id"].as_str().unwrap();

    let response = admin
        .client
        .get(format!(
            "{}/v1/admin/bug-report/{}",
            server.base_url, report_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let report: Value = response.json().await.unwrap();
    assert_eq!(report["title"], "Test report");
    assert_eq!(report["user_handle"], "testuser");

    let response = admin
        .client
        .delete(format!(
            "{}/v1/admin/bug-report/{}",
            server.base_url, report_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn catalog_sync_and_backup_prepare_return_complete_response_shapes() {
    let server = TestServer::spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;

    let response = user
        .client
        .get(format!("{}/v1/sync/catalog?since=0", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let sync: Value = response.json().await.unwrap();
    assert!(sync["events"].is_array());
    assert!(sync["current_seq"].is_number());

    let response = admin
        .client
        .post(format!("{}/v1/admin/backup/prepare", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let backup: Value = response.json().await.unwrap();
    assert_eq!(backup["all_succeeded"], true);
    assert!(backup["databases"].as_array().unwrap().len() >= 4);
    assert!(backup["databases"]
        .as_array()
        .unwrap()
        .iter()
        .all(|database| database["success"] == true));
}
