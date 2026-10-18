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

#[tokio::test]
async fn automation_permission_is_separate_and_control_changes_are_audited() {
    let server = TestServer::spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let base = &server.base_url;
    let integrations = format!("{base}/v1/admin/reports/integrations");
    let deliveries = format!("{base}/v1/admin/reports/deliveries");
    assert_eq!(
        reqwest::get(&integrations).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    for url in [&integrations, &deliveries] {
        assert_eq!(
            user.client.get(url).send().await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        user.client
            .post(&integrations)
            .json(&json!({"url":"https://example.invalid/"}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    for permission in ["TriageReports", "ViewReportDiagnostics"] {
        assert_eq!(
            admin
                .client
                .post(format!("{base}/v1/admin/users/testuser/permissions"))
                .json(&json!({"permission":permission}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::CREATED
        );
        assert_eq!(
            user.client
                .get(&integrations)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        admin
            .client
            .post(format!("{base}/v1/admin/users/testuser/permissions"))
            .json(&json!({"permission":"ManageReportIntegrations"}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        user.client
            .post(&integrations)
            .json(&json!({"url":"http://example.invalid/"}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let response = user
        .client
        .post(&integrations)
        .json(&json!({"url":"https://example.invalid/hook"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let created: Value = response.json().await.unwrap();
    assert_eq!(created["integration"]["enabled"], false);
    let id = created["integration"]["id"].as_str().unwrap();
    let secret = created["secret"].as_str().unwrap();
    assert!(!secret.is_empty());
    let listed = user
        .client
        .get(&integrations)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(!listed.contains("secret"));
    assert!(!listed.contains(secret));
    let url = format!("{integrations}/{id}");
    // Keep test destinations disabled: no external request is configured or dispatched.
    assert_eq!(
        user.client
            .patch(&url)
            .json(&json!({"expected_version":1,"enabled":false}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        user.client
            .patch(&url)
            .json(&json!({"expected_version":1,"enabled":true}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        user.client
            .get(format!("{deliveries}?limit=101"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let page: Value = user
        .client
        .get(&deliveries)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page["items"], json!([]));
    assert_eq!(
        user.client
            .post(format!("{deliveries}/unknown/retry"))
            .json(&json!({"expected_version":1}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let events: Value = user
        .client
        .get(format!("{base}/v1/admin/reports/automation/events"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let kinds: Vec<_> = events["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"integration_created"));
    assert!(kinds.contains(&"integration_disabled"));
    assert!(kinds.contains(&"deliveries_listed"));
    assert!(!events.to_string().contains(secret));
}

#[tokio::test]
async fn triage_notes_are_paginated_and_separate_from_sensitive_audit() {
    let server = TestServer::spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let base = &server.base_url;
    let receipt: Value = user.client.post(format!("{base}/v1/reports"))
        .json(&json!({"client_request_id":uuid::Uuid::new_v4().to_string(),"kind":"feature","category":"ui","description":"notes test","client_type":"web"}))
        .send().await.unwrap().json().await.unwrap();
    let id = receipt["id"].as_str().unwrap();
    let url = format!("{base}/v1/admin/reports/{id}");
    assert_eq!(
        user.client
            .get(format!("{url}/notes"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        admin
            .client
            .post(format!("{base}/v1/admin/users/testuser/permissions"))
            .json(&json!({"permission":"TriageReports"}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );
    for version in 1..=2 {
        assert_eq!(user.client.patch(&url).json(&json!({"expected_version":version,"note":format!("<script>note {version}</script>")}))
            .send().await.unwrap().status(), StatusCode::OK);
    }
    assert_eq!(
        user.client
            .get(format!("{url}/events"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let first = user
        .client
        .get(format!("{url}/notes?limit=1"))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(first.headers()["cache-control"], "no-store");
    let first: Value = first.json().await.unwrap();
    assert_eq!(first["items"][0]["kind"], "note");
    assert_eq!(first["items"][0]["data"]["text"], "<script>note 2</script>");
    let cursor = first["next_cursor"].as_i64().unwrap();
    let second: Value = user
        .client
        .get(format!("{url}/notes?limit=1&before={cursor}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        second["items"][0]["data"]["text"],
        "<script>note 1</script>"
    );
    assert!(second["next_cursor"].is_null());
    assert_eq!(
        user.client
            .get(format!("{url}/notes?limit=101"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        user.client
            .get(format!("{base}/v1/admin/reports/missing/notes"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let owner: Value = user
        .client
        .get(format!("{base}/v1/reports/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!owner.to_string().contains("<script>note"));
    assert_eq!(
        user.client
            .patch(&url)
            .json(&json!({"expected_version":1,"note":"stale"}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
}
