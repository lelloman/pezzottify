//! Contracts at the remaining shared-router protocol boundaries.
mod common;
use common::{TestClient, TestServer};
use reqwest::StatusCode;

#[tokio::test]
async fn upload_preserves_auth_extraction_and_field_errors() {
    let server = TestServer::builder().with_ingestion().spawn().await;
    let url = format!("{}/v1/ingestion/upload", server.base_url);
    let anonymous = reqwest::Client::new().post(&url).send().await.unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let missing_type = admin
        .client
        .post(&url)
        .body("not multipart")
        .send()
        .await
        .unwrap();
    assert_eq!(missing_type.status(), StatusCode::BAD_REQUEST);
    for (body,error) in [
        ("--boundary--\r\n","No filename provided"),
        ("--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"empty.mp3\"\r\nContent-Type: audio/mpeg\r\n\r\n\r\n--boundary--\r\n","No file data provided"),
    ] {
        let response=admin.client.post(&url).header("content-type","multipart/form-data; boundary=boundary").body(body).send().await.unwrap();
        assert_eq!(response.status(),StatusCode::BAD_REQUEST);
        assert_eq!(response.json::<serde_json::Value>().await.unwrap()["error"],error);
    }
}

#[tokio::test]
async fn unsupported_methods_and_head_preserve_router_contracts() {
    let server = TestServer::spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let url = format!("{}/v1/content/genres", server.base_url);
    let get = admin.client.get(&url).send().await.unwrap();
    assert_eq!(get.status(), StatusCode::OK);
    let content_type = get.headers()["content-type"].clone();
    let head = admin.client.head(&url).send().await.unwrap();
    assert_eq!(head.status(), StatusCode::OK);
    assert_eq!(head.headers()["content-type"], content_type);
    assert!(head.bytes().await.unwrap().is_empty());
    let unsupported = admin.client.post(&url).send().await.unwrap();
    assert_eq!(unsupported.status(), StatusCode::METHOD_NOT_ALLOWED);
    let allow = unsupported.headers()["allow"].to_str().unwrap();
    assert!(allow.contains("GET") && allow.contains("HEAD"));
    let missing = admin
        .client
        .get(format!("{}/v1/no-such-route", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}
