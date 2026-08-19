//! End-to-end contracts for administrative user and permission mutations.

mod common;

use common::{TestClient, TestServer};
use reqwest::StatusCode;

#[tokio::test]
async fn admin_user_role_permission_and_password_lifecycle() {
    let server = TestServer::spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let handle = "managed-user";

    let created = admin
        .client
        .post(format!("{}/v1/admin/users", server.base_url))
        .json(&serde_json::json!({ "user_handle": handle }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created: serde_json::Value = created.json().await.unwrap();
    assert_eq!(created["user_handle"], handle);
    assert!(created["user_id"].as_u64().is_some());

    let role = admin
        .client
        .post(format!("{}/v1/admin/users/{handle}/roles", server.base_url))
        .json(&serde_json::json!({ "role": "regular" }))
        .send()
        .await
        .unwrap();
    assert_eq!(role.status(), StatusCode::CREATED);

    let roles = admin
        .client
        .get(format!("{}/v1/admin/users/{handle}/roles", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(roles.status(), StatusCode::OK);
    let roles: serde_json::Value = roles.json().await.unwrap();
    assert_eq!(roles["user_handle"], handle);
    assert_eq!(roles["roles"], serde_json::json!(["Regular"]));

    let permission = admin
        .client
        .post(format!(
            "{}/v1/admin/users/{handle}/permissions",
            server.base_url
        ))
        .json(&serde_json::json!({
            "permission": "ServerAdmin",
            "countdown": 3
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(permission.status(), StatusCode::CREATED);
    let permission: serde_json::Value = permission.json().await.unwrap();
    let permission_id = permission["permission_id"].as_u64().unwrap();

    let permissions = admin
        .client
        .get(format!(
            "{}/v1/admin/users/{handle}/permissions",
            server.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(permissions.status(), StatusCode::OK);
    let permissions: serde_json::Value = permissions.json().await.unwrap();
    assert!(permissions["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|permission| permission == "ServerAdmin"));

    let password = admin
        .client
        .put(format!(
            "{}/v1/admin/users/{handle}/password",
            server.base_url
        ))
        .json(&serde_json::json!({ "password": "managed-password" }))
        .send()
        .await
        .unwrap();
    assert_eq!(password.status(), StatusCode::NO_CONTENT);

    let managed = TestClient::new(server.base_url.clone());
    assert_eq!(
        managed.login(handle, "managed-password").await.status(),
        StatusCode::CREATED
    );

    let revoked = admin
        .client
        .delete(format!(
            "{}/v1/admin/permissions/{permission_id}",
            server.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(revoked.status(), StatusCode::OK);

    let deleted = admin
        .client
        .delete(format!("{}/v1/admin/users/{handle}", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let missing = admin
        .client
        .get(format!("{}/v1/admin/users/{handle}/roles", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_user_mutations_preserve_validation_and_ownership_guards() {
    let server = TestServer::spawn().await;
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;

    let empty_handle = admin
        .client
        .post(format!("{}/v1/admin/users", server.base_url))
        .json(&serde_json::json!({ "user_handle": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(empty_handle.status(), StatusCode::BAD_REQUEST);

    let duplicate = admin
        .client
        .post(format!("{}/v1/admin/users", server.base_url))
        .json(&serde_json::json!({ "user_handle": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let self_delete = admin
        .client
        .delete(format!("{}/v1/admin/users/admin", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(self_delete.status(), StatusCode::BAD_REQUEST);
}
