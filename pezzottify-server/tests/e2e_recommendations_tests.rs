//! End-to-end route contracts for recommendation and radio reads.

mod common;

use common::{TestClient, TestServer, TRACK_1_ID, TRACK_2_ID, TRACK_3_ID, TRACK_4_ID, TRACK_5_ID};
use reqwest::{header, StatusCode};
use serde_json::{json, Value};

#[simple_server::test(host_runtime = true)]
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

#[simple_server::test(host_runtime = true)]
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
    let body = response.json::<Value>().await.unwrap();
    assert_eq!(body["track_ids"], json!([]));
}

#[simple_server::test(host_runtime = true)]
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

const NAMESPACE: &str = "musicfm.mean.v1";

/// Album 1 (artist 1): T1 [1,0], T2 [0.95,0.31], T3 [0.707,0.707].
/// Album 2 (artist 2): T4 [0,1], T5 [0.31,0.95].
async fn server_with_embeddings() -> (TestServer, TestClient) {
    let server = TestServer::builder().with_available_catalog().spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    for (track_id, vector) in [
        (TRACK_1_ID, [1.0, 0.0]),
        (TRACK_2_ID, [0.95, 0.31]),
        (TRACK_3_ID, [0.707, 0.707]),
        (TRACK_4_ID, [0.0, 1.0]),
        (TRACK_5_ID, [0.31, 0.95]),
    ] {
        let response = admin
            .client
            .put(format!(
                "{}/v1/content/embedding/track/{track_id}/{NAMESPACE}",
                server.base_url
            ))
            .json(&json!({ "vector": vector, "metadata": {}, "model": {} }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    let client = TestClient::authenticated(server.base_url.clone()).await;
    (server, client)
}

fn approx(value: &Value, expected: f64) -> bool {
    value
        .as_f64()
        .is_some_and(|actual| (actual - expected).abs() < 0.01)
}

#[simple_server::test(host_runtime = true)]
async fn continuation_anchors_on_source_tracks() {
    let (_server, client) = server_with_embeddings().await;
    let response = client
        .post_continuation(json!({
            "context_track_ids": [TRACK_1_ID, TRACK_4_ID],
            "source_track_ids": [TRACK_1_ID],
            "recent_track_ids": [TRACK_4_ID],
            "exclude_track_ids": [TRACK_1_ID, TRACK_4_ID],
            "recency_weight": 0,
            "randomness": 0,
        }))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store, max-age=0"
    );
    let body = response.json::<Value>().await.unwrap();
    assert_eq!(body["track_ids"], json!([TRACK_2_ID]));
    assert_eq!(body["namespaces"][0]["namespace"], NAMESPACE);
    assert_eq!(body["progress"], Value::Null);
}

#[simple_server::test(host_runtime = true)]
async fn continuation_destination_progress_reports_diagnostics() {
    let (_server, client) = server_with_embeddings().await;
    let at = |progress: f64| {
        client.post_continuation(json!({
            "source_track_ids": [TRACK_1_ID],
            "destination": {"entity_type": "track", "entity_id": TRACK_4_ID},
            "progress": progress,
            "recency_weight": 0,
            "randomness": 0,
        }))
    };
    let arrived = at(1.0).await.json::<Value>().await.unwrap();
    assert_eq!(arrived["track_ids"], json!([TRACK_4_ID]));
    assert!(approx(
        &arrived["namespaces"][0]["query_to_destination"],
        1.0
    ));

    let halfway = at(0.5).await.json::<Value>().await.unwrap();
    assert_eq!(halfway["track_ids"], json!([TRACK_3_ID]));
    assert!(approx(&halfway["progress"], 0.5));
    assert!(approx(&halfway["namespaces"][0]["query_to_source"], 0.707));
    assert!(approx(
        &halfway["namespaces"][0]["source_to_destination"],
        0.0
    ));
}

#[simple_server::test(host_runtime = true)]
async fn continuation_rejects_invalid_requests_with_json_errors() {
    let (_server, client) = server_with_embeddings().await;
    let reference = json!({"entity_type": "track", "entity_id": TRACK_1_ID});
    for body in [
        json!({"destination": {"entity_type": "playlist", "entity_id": "p"}}),
        json!({"source_references": vec![reference; 9]}),
    ] {
        let response = client.post_continuation(body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("application/json"));
        let error = response.json::<Value>().await.unwrap();
        assert_eq!(error["code"], "invalid_continuation_request");
    }
}

#[simple_server::test(host_runtime = true)]
async fn legacy_continuation_still_works() {
    let (_server, client) = server_with_embeddings().await;
    let response = client
        .get_continuation_recommendations(vec![TRACK_1_ID], vec![TRACK_1_ID], 1)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.json::<Value>().await.unwrap();
    let track_ids = body["track_ids"].as_array().unwrap();
    assert_eq!(track_ids.len(), 1);
    // Legacy requests keep the default jitter, so only assert the pick stays near the seed.
    assert!([json!(TRACK_2_ID), json!(TRACK_3_ID)].contains(&track_ids[0]));
    assert_eq!(body["namespaces"], json!([]));
}

#[simple_server::test(host_runtime = true)]
async fn concepts_require_authentication_and_list_as_json() {
    let server = TestServer::spawn().await;
    let anonymous = TestClient::new(server.base_url.clone());
    let response = anonymous
        .client
        .get(format!("{}/v1/content/concepts", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let client = TestClient::authenticated(server.base_url.clone()).await;
    let response = client
        .client
        .get(format!(
            "{}/v1/content/concepts?q=jazz&limit=5",
            server.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store, max-age=0"
    );
    // Concepts are materialized by the weekly job; a fresh catalog has none.
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({ "concepts": [] })
    );
}

#[simple_server::test(host_runtime = true)]
async fn continuation_accepts_destination_mixes_and_concept_references() {
    let (_server, client) = server_with_embeddings().await;
    let response = client
        .post_continuation(json!({
            "source_track_ids": [TRACK_1_ID],
            "destination": [
                {"entity_type": "track", "entity_id": TRACK_4_ID},
                {"entity_type": "concept", "entity_id": "audioset:Jazz", "weight": 0.5}
            ],
            "progress": 1.0,
            "recency_weight": 0,
            "randomness": 0,
        }))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.json::<Value>().await.unwrap();
    assert_eq!(body["track_ids"], json!([TRACK_4_ID]));
    let components = body["namespaces"][0]["destination_components"]
        .as_array()
        .unwrap();
    assert_eq!(components.len(), 2);
    assert!(approx(&components[0]["similarity"], 1.0));
    // The concept has no vector in this catalog, so it is reported without a similarity.
    assert_eq!(components[1]["similarity"], Value::Null);
}
