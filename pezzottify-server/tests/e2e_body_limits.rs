#[allow(dead_code)]
mod common;
use common::{TestClient, TestServer};
use reqwest::StatusCode;

#[tokio::test]
async fn actual_http_preserves_small_json_and_large_multipart_route_limits() {
    let server = TestServer::builder().with_ingestion().spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    for (path, limit) in [
        ("/v1/user/bug-report", 2 * 1024 * 1024),
        ("/v1/reports", 20 * 1024 * 1024),
    ] {
        for length in [limit - 1, limit, limit + 1] {
            let mut payload = b"{}".to_vec();
            payload.resize(length, b' ');
            let response = user
                .client
                .post(format!("{}{path}", server.base_url))
                .header("content-type", "application/json")
                .body(payload)
                .send()
                .await
                .unwrap();
            let status = response.status();
            assert_eq!(response.headers()["cache-control"], "no-store");
            let body = response.text().await.unwrap();
            assert_eq!(
                status,
                if length > limit {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                },
                "{path} length={length}: {body}"
            );
        }
    }
    // The upload must consume >2 MiB successfully before applying filename
    // validation. An empty filename avoids creating ingestion jobs or files.
    let mut payload = b"--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"\"\r\nContent-Type: application/octet-stream\r\n\r\n".to_vec();
    payload.extend(vec![b'x'; 3 * 1024 * 1024]);
    payload.extend_from_slice(b"\r\n--boundary--\r\n");
    let response = admin
        .client
        .post(format!("{}/v1/ingestion/upload", server.base_url))
        .header("content-type", "multipart/form-data; boundary=boundary")
        .body(payload)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"error":"No filename provided"})
    );
}
