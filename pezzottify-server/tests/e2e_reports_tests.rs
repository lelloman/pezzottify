mod common;
use common::{TestClient, TestServer};
use reqwest::StatusCode;
use serde_json::{json, Value};

#[tokio::test]
async fn reports_owner_api_and_legacy_submission() {
    let server = TestServer::spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let request = json!({
        "client_request_id":uuid::Uuid::new_v4().to_string(),
        "kind":"feature","category":"assistant","title":"Better reports",
        "description":"Please improve this","client_type":"web",
        "attachments":[{"kind":"technical_logs","content":"test diagnostic","consent":true}]
    });
    let response = user
        .client
        .post(format!("{}/v1/reports", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "{}",
        response.text().await.unwrap()
    );
    let replay = user
        .client
        .post(format!("{}/v1/reports", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    let receipt: Value = replay.json().await.unwrap();
    let id = receipt["id"].as_str().unwrap();
    let detail: Value = user
        .client
        .get(format!("{}/v1/reports/{id}", server.base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["kind"], "feature");
    let attachment = detail["attachments"][0]["id"].as_str().unwrap();
    let content: Value = user
        .client
        .get(format!(
            "{}/v1/reports/{id}/attachments/{attachment}",
            server.base_url
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(content["content"], "test diagnostic");
    assert_eq!(
        reqwest::get(format!("{}/v1/reports/{id}", server.base_url))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        user.client
            .get(format!("{}/v1/admin/reports", server.base_url))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let response=user.client.post(format!("{}/v1/user/bug-report",server.base_url))
        .json(&json!({"title":"Legacy","description":"Old client","client_type":"android","logs":"legacy"})).send().await.unwrap();
    assert!(
        response.status().is_success(),
        "{}",
        response.text().await.unwrap()
    );
    let list: Value = user
        .client
        .get(format!("{}/v1/reports", server.base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list["items"].as_array().unwrap().len(), 2);
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let filtered: Value = admin
        .client
        .get(format!(
            "{}/v1/admin/reports?kind=feature&category=assistant&has_diagnostics=true",
            server.base_url
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(filtered["items"].as_array().unwrap().len(), 1);
    assert_eq!(filtered["items"][0]["id"], id);
    let edit = json!({"expected_version":1,"status":"investigating","note":"Reproduced"});
    let url = format!("{}/v1/admin/reports/{id}", server.base_url);
    assert_eq!(
        admin
            .client
            .patch(&url)
            .json(&edit)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        admin
            .client
            .patch(&url)
            .json(&edit)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
}
