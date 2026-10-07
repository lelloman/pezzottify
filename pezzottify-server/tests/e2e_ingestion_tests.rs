mod common;

use common::{TestClient, TestServer};
use reqwest::StatusCode;

#[tokio::test]
async fn enabled_ingestion_lists_are_empty_and_preserve_response_shapes() {
    let server = TestServer::builder().with_ingestion().spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;

    let my_jobs = admin
        .client
        .get(format!("{}/v1/ingestion/my-jobs?limit=1", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(my_jobs.status(), StatusCode::OK);
    assert_eq!(
        my_jobs.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!([])
    );

    let all_jobs = admin
        .client
        .get(format!(
            "{}/v1/ingestion/admin/jobs?limit=1",
            server.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(all_jobs.status(), StatusCode::OK);
    assert_eq!(
        all_jobs.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!([])
    );

    let reviews = admin
        .client
        .get(format!("{}/v1/ingestion/reviews?limit=1", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(reviews.status(), StatusCode::OK);
    assert_eq!(
        reviews.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"items": []})
    );
}

#[tokio::test]
async fn enabled_ingestion_preserves_permission_and_missing_job_contracts() {
    let server = TestServer::builder().with_ingestion().spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;

    for path in ["/v1/ingestion/my-jobs", "/v1/ingestion/reviews"] {
        let response = user
            .client
            .get(format!("{}{}", server.base_url, path))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
    }
    let admin_jobs = user
        .client
        .get(format!("{}/v1/ingestion/admin/jobs", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(admin_jobs.status(), StatusCode::FORBIDDEN);

    for suffix in ["", "/details"] {
        let response = admin
            .client
            .get(format!(
                "{}/v1/ingestion/job/missing{}",
                server.base_url, suffix
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{suffix}");
    }
}

#[tokio::test]
async fn disabled_ingestion_returns_service_unavailable_after_authorization() {
    let server = TestServer::spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;

    let forbidden = user
        .client
        .get(format!("{}/v1/ingestion/my-jobs", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let unavailable = admin
        .client
        .get(format!("{}/v1/ingestion/my-jobs", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        unavailable.text().await.unwrap(),
        "Ingestion manager not enabled"
    );
}
