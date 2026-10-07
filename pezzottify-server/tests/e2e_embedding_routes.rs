//! Before/after contracts for the shared HTTP route canary.
mod common;

use common::{TestClient, TestServer, TRACK_1_ID};
use reqwest::StatusCode;
use serde_json::{json, Value};

#[tokio::test]
async fn embedding_crud_search_and_query_contracts() {
    let server = TestServer::spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let list = format!(
        "{}/v1/content/embedding/track/{TRACK_1_ID}",
        server.base_url
    );
    let item = format!("{list}/route%20canary");
    let response = admin
        .client
        .put(&item)
        .json(&json!({
            "vector": [1.0, 2.0, 3.0], "metadata": {"label": "canary"}, "model": {"version": 1}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/json");
    let stored: Value = response.json().await.unwrap();
    assert_eq!(stored["namespace"], "route canary");
    assert_eq!(stored["dim"], 3);

    let response = user
        .client
        .get(format!("{item}?include_vector=true"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let full: Value = response.json().await.unwrap();
    assert_eq!(full["vector"], json!([1.0, 2.0, 3.0]));
    assert_eq!(full["metadata"]["label"], "canary");
    let listed: Value = user
        .client
        .get(&list)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert!(listed[0].get("vector").is_none());

    let response = user.client.post(format!("{}/v1/content/embedding/search", server.base_url))
        .json(&json!({"namespace": "route canary", "vector": [1.0,2.0,3.0], "entity_type":"track", "limit":1}))
        .send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let results: Value = response.json().await.unwrap();
    assert_eq!(results["namespace"], "route canary");
    assert_eq!(results["results"][0]["entity_id"], TRACK_1_ID);

    let response = admin.client.delete(&item).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(response.bytes().await.unwrap().is_empty());
    let response = user.client.get(&item).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let request_id = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    let error: Value = response.json().await.unwrap();
    assert_eq!(error["code"], "embedding_not_found");
    assert_eq!(error["request_id"], request_id);
}

#[tokio::test]
async fn embedding_routes_preserve_auth_permissions_and_csrf() {
    let server = TestServer::spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let item = format!(
        "{}/v1/content/embedding/track/{TRACK_1_ID}/test",
        server.base_url
    );
    let response = reqwest::Client::new().get(&item).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = user
        .client
        .put(&item)
        .json(&json!({"vector":[1.0]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = admin
        .client
        .post_without_csrf(format!("{}/v1/content/embedding/search", server.base_url))
        .json(&json!({"namespace":"test","vector":[1.0]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn embedding_routes_preserve_extractor_and_application_rejections() {
    let server = TestServer::spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let item = format!(
        "{}/v1/content/embedding/track/{TRACK_1_ID}/test",
        server.base_url
    );
    for (body, content_type, expected) in [
        (
            "{}".to_owned(),
            "text/plain",
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        ("{".to_owned(), "application/json", StatusCode::BAD_REQUEST),
        (
            "{\"vector\":\"bad\"}".to_owned(),
            "application/json",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            format!(
                "{{\"vector\":[1],\"padding\":\"{}\"}}",
                "x".repeat(2 * 1024 * 1024)
            ),
            "application/json",
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ] {
        let response = admin
            .client
            .put(&item)
            .header("content-type", content_type)
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        assert_eq!(
            response.headers()["content-type"],
            "text/plain; charset=utf-8"
        );
        assert!(!response.text().await.unwrap().is_empty());
    }
    let response = admin
        .client
        .get(format!("{item}?include_vector=bad"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    for (body, code) in [
        (json!({"vector":[]}), "invalid_embedding"),
        (
            json!({"vector":[1.0],"dtype":"int8"}),
            "invalid_embedding_dtype",
        ),
    ] {
        let response = admin.client.put(&item).json(&body).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.json::<Value>().await.unwrap()["code"], code);
    }
}
