//! UnifiedPush registration routes and end-to-end wake-ups (docs/unifiedpush.md).

mod common;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use common::{TestClient, TestServer, TEST_USER};
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn receiver_keys() -> (Box<dyn ece::LocalKeyPair>, [u8; 16], String, String) {
    let (keypair, auth) = ece::generate_keypair_and_auth_secret().unwrap();
    let p256dh = URL_SAFE_NO_PAD.encode(keypair.pub_as_raw().unwrap());
    let auth_b64 = URL_SAFE_NO_PAD.encode(auth);
    (keypair, auth, p256dh, auth_b64)
}

/// Accepts pushes forever, answering 201, and records the request bodies.
async fn push_service() -> (String, Arc<Mutex<Vec<Vec<u8>>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/up/e2e", listener.local_addr().unwrap());
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let log = bodies.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            let log = log.clone();
            tokio::spawn(async move {
                let mut buffer = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let read = socket.read(&mut chunk).await.unwrap_or(0);
                    if read == 0 {
                        return;
                    }
                    buffer.extend_from_slice(&chunk[..read]);
                    if let Some(split) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buffer[..split]).to_ascii_lowercase();
                        let length = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                            .unwrap_or(0);
                        while buffer.len() < split + 4 + length {
                            let read = socket.read(&mut chunk).await.unwrap_or(0);
                            if read == 0 {
                                return;
                            }
                            buffer.extend_from_slice(&chunk[..read]);
                        }
                        log.lock().unwrap().push(buffer[split + 4..split + 4 + length].to_vec());
                        let _ = socket
                            .write_all(b"HTTP/1.1 201 Created\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
                            .await;
                        return;
                    }
                }
            });
        }
    });
    (url, bodies)
}

#[tokio::test]
async fn push_routes_require_configuration_and_authentication() {
    let server = TestServer::spawn().await;
    let anonymous = TestClient::new(server.base_url.clone());
    let response = anonymous
        .client
        .get(format!("{}/v1/push/vapid", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let client = TestClient::authenticated(server.base_url.clone()).await;
    let response = client
        .client
        .get(format!("{}/v1/push/vapid", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.json::<Value>().await.unwrap()["code"], "push_disabled");
}

#[tokio::test]
async fn registrations_can_be_created_validated_and_removed() {
    let server = TestServer::builder().with_push().spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;
    let url = |path: &str| format!("{}/v1/push{}", server.base_url, path);

    let vapid: Value = client.client.get(url("/vapid")).send().await.unwrap().json().await.unwrap();
    let public_key = vapid["public_key"].as_str().unwrap();
    assert_eq!(public_key.len(), 87);
    assert_eq!(URL_SAFE_NO_PAD.decode(public_key).unwrap()[0], 0x04);

    let (_keypair, _auth, p256dh, auth) = receiver_keys();
    let endpoint = "http://127.0.0.1:9/up/registration";
    let put = |body: Value| client.client.put(url("/registrations")).json(&body).send();
    let response = put(json!({"endpoint": endpoint, "p256dh": p256dh, "auth": auth, "device_id": "phone"}))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let user_id = server.user_store.get_user_id(TEST_USER).unwrap().unwrap();
    let registrations = server.user_store.list_push_registrations(user_id).unwrap();
    assert_eq!(registrations.len(), 1);
    assert_eq!(registrations[0].device_id.as_deref(), Some("phone"));

    for (body, why) in [
        (json!({"endpoint": "ftp://push.example/x", "p256dh": p256dh, "auth": auth}), "scheme"),
        (json!({"endpoint": endpoint, "p256dh": "short", "auth": auth}), "p256dh"),
        (json!({"endpoint": endpoint, "p256dh": p256dh, "auth": "c2hvcnQ"}), "auth"),
    ] {
        let response = put(body).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{why}");
        assert_eq!(response.json::<Value>().await.unwrap()["code"], "invalid_push_registration");
    }

    let response = client
        .client
        .delete(url("/registrations"))
        .json(&json!({"endpoint": endpoint}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(server.user_store.list_push_registrations(user_id).unwrap().is_empty());
}

#[tokio::test]
async fn notification_events_wake_registered_devices() {
    let server = TestServer::builder().with_push().spawn().await;
    let client = TestClient::authenticated(server.base_url.clone()).await;
    let (endpoint, bodies) = push_service().await;
    let (keypair, auth, p256dh, auth_b64) = receiver_keys();
    let response = client
        .client
        .put(format!("{}/v1/push/registrations", server.base_url))
        .json(&json!({"endpoint": endpoint, "p256dh": p256dh, "auth": auth_b64}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let user_id = server.user_store.get_user_id(TEST_USER).unwrap().unwrap();
    let notification = server
        .user_store
        .create_notification(
            user_id,
            pezzottify_server::notifications::NotificationType::DownloadCompleted,
            "Ready".into(),
            None,
            json!({}),
        )
        .unwrap();
    let stored = server
        .user_store
        .append_event(user_id, &pezzottify_server::user::UserEvent::NotificationCreated { notification })
        .unwrap();

    for _ in 0..100 {
        if !bodies.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let received = bodies.lock().unwrap().clone();
    assert_eq!(received.len(), 1, "one coalesced wake-up");
    let plain = ece::decrypt(&keypair.raw_components().unwrap(), &auth, &received[0]).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&plain).unwrap(),
        json!({"type": "sync", "seq": stored.seq})
    );
}
