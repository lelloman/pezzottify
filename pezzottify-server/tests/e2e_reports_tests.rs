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

/// The same UTF-8 envelope is consumed by Android recovery and the web recorder.
/// Exercise the deployed HTTP contract, rather than only its serde types.
#[tokio::test]
async fn shared_client_snapshot_contract_exact_bytes_and_manual_replay() {
    let server = TestServer::spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let url = format!("{}/v1/reports", server.base_url);
    let fixture = include_str!("../../docs/fixtures/assistant-diagnostics-v1.json").trim();
    for client in ["android", "web"] {
        let request = json!({
            "client_request_id": uuid::Uuid::new_v4().to_string(),
            "kind": "feature", "category": "assistant", "description": "Unicode 🌍",
            "client_type": client,
            "attachments": [{"kind": "assistant", "content": fixture, "consent": true}]
        });
        let first = user.client.post(&url).json(&request).send().await.unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);
        let receipt: Value = first.json().await.unwrap();
        // Independent in-flight manual retries must resolve to the same receipt.
        let (a, b) = tokio::join!(
            user.client.post(&url).json(&request).send(),
            user.client.post(&url).json(&request).send()
        );
        for response in [a.unwrap(), b.unwrap()] {
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.json::<Value>().await.unwrap()["id"], receipt["id"]);
        }
        let id = receipt["id"].as_str().unwrap();
        let detail: Value = user
            .client
            .get(format!("{url}/{id}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(detail["client_type"], client);
        let attachment = &detail["attachments"][0];
        let attachment_id = attachment["id"].as_str().unwrap();
        let response = user
            .client
            .get(format!("{url}/{id}/attachments/{attachment_id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.headers()["cache-control"], "no-store");
        let uploaded: Value = response.json().await.unwrap();
        assert_eq!(uploaded["content"], fixture); // byte-identical to the preview
        let mut changed = request.clone();
        changed["description"] = json!("edited after preview");
        assert_eq!(
            user.client
                .post(&url)
                .json(&changed)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::CONFLICT
        );
        let mut unconsented = request.clone();
        unconsented["client_request_id"] = json!(uuid::Uuid::new_v4().to_string());
        unconsented["attachments"][0]["consent"] = json!(false);
        assert_eq!(
            user.client
                .post(&url)
                .json(&unconsented)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    let list: Value = user
        .client
        .get(&url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list["items"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn diagnostic_utf8_boundary_and_invalid_schema_leave_submission_recoverable() {
    let server = TestServer::spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let url = format!("{}/v1/reports", server.base_url);
    let mut envelope: Value = serde_json::from_str(include_str!(
        "../../docs/fixtures/assistant-diagnostics-v1.json"
    ))
    .unwrap();
    envelope["turns"][0]["events"][0]["content"] = json!("");
    let overhead = serde_json::to_string(&envelope).unwrap().len();
    let remaining = 2_097_152 - overhead;
    envelope["turns"][0]["events"][0]["content"] =
        json!("🌍".repeat(remaining / 4) + &"x".repeat(remaining % 4));
    let exact = serde_json::to_string(&envelope).unwrap();
    assert_eq!(exact.len(), 2_097_152);
    let mut request = json!({
        "client_request_id": uuid::Uuid::new_v4().to_string(), "kind": "bug",
        "category": "assistant", "description": "boundary", "client_type": "web",
        "attachments": [{"kind": "assistant", "content": exact, "consent": true}]
    });
    // A rejected attempt must not reserve the idempotency key or charge quotas.
    request["attachments"][0]["content"] = json!(exact.clone() + " ");
    let oversized = user.client.post(&url).json(&request).send().await.unwrap();
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(oversized.headers()["cache-control"], "no-store");
    let mut unsupported = envelope.clone();
    unsupported["schema_version"] = json!(2);
    request["attachments"][0]["content"] = json!(serde_json::to_string(&unsupported).unwrap());
    assert_eq!(
        user.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    // The pinned Android serializer omits default-valued schema_version.
    // Keep the server fail-closed; qualify the client fix with FeedbackContractTest.
    let mut missing_version = envelope.clone();
    missing_version
        .as_object_mut()
        .unwrap()
        .remove("schema_version");
    request["attachments"][0]["content"] = json!(serde_json::to_string(&missing_version).unwrap());
    assert_eq!(
        user.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    request["attachments"][0]["content"] = json!(exact);
    let accepted = user.client.post(&url).json(&request).send().await.unwrap();
    assert_eq!(accepted.status(), StatusCode::CREATED);
    let receipt: Value = accepted.json().await.unwrap();
    let id = receipt["id"].as_str().unwrap();
    let detail: Value = user
        .client
        .get(format!("{url}/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["attachments"][0]["size_bytes"], 2_097_152);
    let attachment = detail["attachments"][0]["id"].as_str().unwrap();
    let uploaded: Value = user
        .client
        .get(format!("{url}/{id}/attachments/{attachment}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(uploaded["content"], request["attachments"][0]["content"]);
}
