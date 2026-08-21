//! Characterization tests for server-store, shows, and backup admin operations.

mod common;

use common::{TestClient, TestServer};
use pezzottify_server::server_store::{CatalogContentType, CatalogEventType};
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
    assert_eq!(sync["has_more"], false);
    assert_eq!(sync["next_since"], sync["current_seq"]);

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

#[tokio::test]
async fn catalog_sync_returns_every_high_volume_event_in_sequence() {
    const EVENT_COUNT: i64 = 1_205;
    let server = TestServer::spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let initial_seq = server
        .server_store
        .get_catalog_events_current_seq()
        .unwrap();

    for index in 1..=EVENT_COUNT {
        server
            .server_store
            .append_catalog_event(
                CatalogEventType::TrackUpdated,
                CatalogContentType::Track,
                &format!("high-volume-track-{index}"),
                Some("high_volume_test"),
            )
            .unwrap();
    }

    let expected_final_seq = initial_seq + EVENT_COUNT;
    let mut since = initial_seq;
    let mut received_sequences = Vec::new();
    let mut page_count = 0;

    loop {
        let response = user
            .client
            .get(format!("{}/v1/sync/catalog?since={since}", server.base_url))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let page: Value = response.json().await.unwrap();
        page_count += 1;

        let page_sequences: Vec<i64> = page["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["seq"].as_i64().unwrap())
            .collect();
        assert!(page_sequences.len() <= 500);
        assert!(page_sequences.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(page_sequences.iter().all(|seq| *seq > since));
        received_sequences.extend(page_sequences);

        let current_seq = page["current_seq"].as_i64().unwrap();
        assert_eq!(current_seq, expected_final_seq);
        let next_since = page["next_since"].as_i64().unwrap();
        if page["has_more"] == true {
            assert!(next_since > since);
            since = next_since;
        } else {
            assert_eq!(next_since, current_seq);
            break;
        }
    }

    assert_eq!(page_count, 3);
    assert_eq!(received_sequences.len(), EVENT_COUNT as usize);
    assert_eq!(received_sequences.first(), Some(&(initial_seq + 1)));
    assert_eq!(received_sequences.last(), Some(&expected_final_seq));
    assert!(received_sequences
        .windows(2)
        .all(|pair| pair[1] == pair[0] + 1));
}

#[tokio::test]
async fn catalog_sync_honors_a_smaller_requested_page_limit() {
    let server = TestServer::spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;

    for index in 1..=40 {
        server
            .server_store
            .append_catalog_event(
                CatalogEventType::ArtistUpdated,
                CatalogContentType::Artist,
                &format!("limited-artist-{index}"),
                Some("page_limit_test"),
            )
            .unwrap();
    }

    let response = user
        .client
        .get(format!(
            "{}/v1/sync/catalog?since=0&limit=17",
            server.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let page: Value = response.json().await.unwrap();

    assert_eq!(page["events"].as_array().unwrap().len(), 17);
    assert_eq!(page["current_seq"], 40);
    assert_eq!(page["has_more"], true);
    assert_eq!(page["next_since"], 17);
}
