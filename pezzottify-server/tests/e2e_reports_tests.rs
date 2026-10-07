mod common;
use common::{TestClient, TestServer};
use reqwest::StatusCode;
use serde_json::{json, Value};

#[tokio::test]
async fn report_limits_legacy_parity_and_explicit_permissions() {
    let server = TestServer::spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let base = &server.base_url;
    let settings_url = format!("{base}/v1/admin/reports/settings");
    let mut settings: Value = admin
        .client
        .get(&settings_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    settings["hourly_reports"] = json!(1);
    assert_eq!(
        admin
            .client
            .put(&settings_url)
            .json(&settings)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        admin
            .client
            .put(&settings_url)
            .json(&settings)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    let request = json!({"client_request_id":uuid::Uuid::new_v4().to_string(),"kind":"bug","category":"other","description":"hello","client_type":"web"});
    let url = format!("{base}/v1/reports");
    let response = user.client.post(&url).json(&request).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let receipt: Value = response.json().await.unwrap();
    assert_eq!(
        user.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let legacy = user
        .client
        .post(format!("{base}/v1/user/bug-report"))
        .json(&json!({"description":"legacy","client_type":"android"}))
        .send()
        .await
        .unwrap();
    assert_eq!(legacy.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(legacy.headers().contains_key("retry-after"));
    assert_eq!(legacy.headers()["cache-control"], "no-store");
    assert_eq!(
        legacy.json::<Value>().await.unwrap()["error"]["code"],
        "quota_exceeded"
    );
    assert_eq!(
        user.client
            .post(&url)
            .header("Content-Encoding", "gzip")
            .body("not gzip")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    assert_eq!(
        user.client
            .post(&url)
            .header("Content-Type", "application/json")
            .body("{")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    for permission in [
        "TriageReports",
        "ViewReportDiagnostics",
        "ManageReportIntegrations",
    ] {
        let granted = admin
            .client
            .post(format!("{base}/v1/admin/users/testuser/permissions"))
            .json(&json!({"permission":permission}))
            .send()
            .await
            .unwrap();
        assert_eq!(
            granted.status(),
            StatusCode::CREATED,
            "{}",
            granted.text().await.unwrap()
        );
        if permission == "TriageReports" {
            assert_eq!(
                user.client
                    .get(format!("{base}/v1/admin/bug-reports"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                StatusCode::OK
            );
            assert_eq!(
                user.client
                    .get(format!(
                        "{base}/v1/admin/bug-report/{}",
                        receipt["id"].as_str().unwrap()
                    ))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                StatusCode::FORBIDDEN
            );
            assert_eq!(
                user.client
                    .get(&settings_url)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                StatusCode::FORBIDDEN
            );
        }
    }
    assert_eq!(
        user.client
            .get(&settings_url)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        user.client
            .get(format!(
                "{base}/v1/admin/bug-report/{}",
                receipt["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let stats: Value = user
        .client
        .get(format!("{base}/v1/admin/reports/stats"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stats["storage"]["reports"], 1);
    assert!(
        stats["admission"]["rejections_since_start"][6]
            .as_u64()
            .unwrap()
            > 0
    );
}

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
    let escaped_note = json!({"expected_version":2,"note":"\u{0001}".repeat(16*1024)});
    assert_eq!(
        admin
            .client
            .patch(&url)
            .json(&escaped_note)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}
