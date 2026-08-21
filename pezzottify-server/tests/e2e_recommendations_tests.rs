//! End-to-end route contracts for recommendation and radio reads.

mod common;

use common::{TestClient, TestServer, TRACK_1_ID};
use reqwest::{header, StatusCode};
use serde_json::Value;

#[tokio::test]
async fn recommendation_reads_require_authentication() {
    let server = TestServer::spawn().await;
    let client = TestClient::new(server.base_url.clone());

    let continuation = client
        .get_continuation_recommendations(vec![TRACK_1_ID], vec![], 1)
        .await;
    assert_eq!(continuation.status(), StatusCode::UNAUTHORIZED);

    let radio = client.get_radio("track", TRACK_1_ID, 10).await;
    assert_eq!(radio.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn empty_continuation_preserves_the_no_store_response_contract() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let response = client
        .get_continuation_recommendations(vec![], vec![], 5)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store, max-age=0"
    );
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        serde_json::json!({ "track_ids": [] })
    );
}

#[tokio::test]
async fn radio_reads_preserve_validation_and_seed_fallback_behavior() {
    let server = TestServer::spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;

    let invalid = client.get_radio("playlist", TRACK_1_ID, 10).await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_body = invalid.json::<Value>().await.unwrap();
    assert_eq!(invalid_body["code"], "unsupported_entity_type");

    let response = client.get_radio("track", TRACK_1_ID, 10).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store, max-age=0"
    );
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        serde_json::json!({ "track_ids": [TRACK_1_ID] })
    );
}
