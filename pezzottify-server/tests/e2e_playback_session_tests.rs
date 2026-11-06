//! End-to-end tests for the multi-device playback protocol (`playback.*`
//! WebSocket messages). The contract is `docs/multi-device-playback.md`.

mod common;

use common::{TestClient, TestServer, TEST_PASS, TEST_USER};
use futures::{SinkExt, StreamExt};
use http::header;
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

const RECV_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE: Duration = Duration::from_millis(500);

async fn extract_session_token(response: reqwest::Response) -> String {
    response
        .cookies()
        .find(|c| c.name() == "session_token")
        .expect("Session token cookie not found")
        .value()
        .to_string()
}

async fn connect_ws(base_url: &str, session_token: &str) -> Ws {
    let ws_url = base_url.replace("http://", "ws://") + "/v1/ws";
    let request = http::Request::builder()
        .uri(&ws_url)
        .header(header::COOKIE, format!("session_token={}", session_token))
        .header(header::HOST, "localhost")
        .header(header::CONNECTION, "Upgrade")
        .header(header::UPGRADE, "websocket")
        .header(header::SEC_WEBSOCKET_VERSION, "13")
        .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .body(())
        .expect("Failed to build WebSocket request");
    let (ws_stream, _) = connect_async(request)
        .await
        .expect("Failed to connect to WebSocket");
    ws_stream
}

/// Wait for the next message of `expected_type`, discarding others.
async fn wait_for_message(
    ws: &mut Ws,
    expected_type: &str,
    timeout_duration: Duration,
) -> Option<Value> {
    timeout(timeout_duration, async {
        while let Some(Ok(msg)) = ws.next().await {
            if let Message::Text(text) = msg {
                if let Ok(json) = serde_json::from_str::<Value>(&text) {
                    if json.get("type").and_then(|t| t.as_str()) == Some(expected_type) {
                        return Some(json);
                    }
                }
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

/// Wait for the next `playback.device_list_changed` whose change type matches.
async fn wait_for_list_change(ws: &mut Ws, change_type: &str) -> Option<Value> {
    timeout(RECV_TIMEOUT, async {
        loop {
            let msg = wait_for_message(ws, "playback.device_list_changed", RECV_TIMEOUT).await?;
            if msg["payload"]["change"]["type"] == change_type {
                return Some(msg);
            }
        }
    })
    .await
    .ok()
    .flatten()
}

async fn send(ws: &mut Ws, msg_type: &str, payload: Value) {
    let text = json!({ "type": msg_type, "payload": payload }).to_string();
    ws.send(Message::Text(text.into()))
        .await
        .expect("send failed");
}

/// Log in a device of the given user and open its WebSocket. Returns the
/// HTTP client, the socket and the server-side device id.
async fn open_device(
    server: &TestServer,
    handle: &str,
    password: &str,
    uuid: &str,
) -> (TestClient, Ws, u64) {
    let client = TestClient::new(server.base_url.clone());
    let response = client.login_with_device(handle, password, uuid).await;
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "login failed for {handle}/{uuid}"
    );
    let token = extract_session_token(response).await;
    let mut ws = connect_ws(&server.base_url, &token).await;
    let connected = wait_for_message(&mut ws, "connected", RECV_TIMEOUT)
        .await
        .expect("no connected message");
    let device_id = connected["payload"]["device_id"]
        .as_u64()
        .expect("device_id");
    (client, ws, device_id)
}

/// Send hello and return the welcome payload.
async fn hello(ws: &mut Ws, name: &str) -> Value {
    send(
        ws,
        "playback.hello",
        json!({ "device_name": name, "device_type": "web" }),
    )
    .await;
    wait_for_message(ws, "playback.welcome", RECV_TIMEOUT)
        .await
        .expect("no welcome")["payload"]
        .take()
}

fn playing_state(title: &str, is_playing: bool, position: f64) -> Value {
    json!({
        "current_track": {
            "id": "track-1",
            "title": title,
            "artist_id": "artist-1",
            "artist_name": "Artist",
            "artists_ids": ["artist-1"],
            "album_id": "album-1",
            "album_title": "Album",
            "duration": 183.5,
            "track_number": 1,
            "image_id": "image-1"
        },
        "queue_position": 0,
        "queue_version": 1,
        "position": position,
        "is_playing": is_playing,
        "volume": 0.8,
        "muted": false,
        "shuffle": false,
        "repeat": "off",
        "timestamp": 1_730_000_000_000u64
    })
}

fn device_ids(list: &Value) -> Vec<u64> {
    list.as_array()
        .expect("devices array")
        .iter()
        .filter_map(|d| d["id"].as_u64())
        .collect()
}

/// Create a second user with a password and the regular role.
async fn create_second_user(server: &TestServer, handle: &str, password: &str) {
    let admin = TestClient::authenticated_admin(server.base_url.clone()).await;
    let created = admin
        .client
        .post(format!("{}/v1/admin/users", server.base_url))
        .json(&json!({ "user_handle": handle }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let role = admin
        .client
        .post(format!("{}/v1/admin/users/{handle}/roles", server.base_url))
        .json(&json!({ "role": "regular" }))
        .send()
        .await
        .unwrap();
    assert_eq!(role.status(), StatusCode::CREATED);
    let password_set = admin
        .client
        .put(format!(
            "{}/v1/admin/users/{handle}/password",
            server.base_url
        ))
        .json(&json!({ "password": password }))
        .send()
        .await
        .unwrap();
    assert_eq!(password_set.status(), StatusCode::NO_CONTENT);
}

async fn put_share_policy(client: &TestClient, base_url: &str, device_id: u64, mode: &str) {
    let response = client
        .client
        .put(format!(
            "{base_url}/v1/user/devices/{device_id}/share_policy"
        ))
        .json(&json!({
            "mode": mode,
            "allow_users": [],
            "deny_users": [],
            "allow_roles": []
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "share policy update failed"
    );
}

#[tokio::test]
async fn hello_returns_welcome_with_own_device() {
    let server = TestServer::spawn().await;
    let (_client, mut ws, device_id) =
        open_device(&server, TEST_USER, TEST_PASS, "pb-hello-device-a").await;

    let welcome = hello(&mut ws, "Device A").await;

    assert_eq!(welcome["device_id"].as_u64(), Some(device_id));
    assert_eq!(
        welcome["session"]["active_devices"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let devices = welcome["devices"].as_array().unwrap();
    let me = devices
        .iter()
        .find(|d| d["id"].as_u64() == Some(device_id))
        .expect("own device listed");
    assert_eq!(me["name"], "Device A");
    assert_eq!(me["device_type"], "web");
    assert_eq!(me["has_session"], false);
    assert_eq!(me["is_playing"], false);
    assert_eq!(me["is_shared"], false);
    assert_eq!(me["owner_handle"], TEST_USER);

    ws.close(None).await.ok();
}

#[tokio::test]
async fn state_is_relayed_to_peers_and_not_to_sender() {
    let server = TestServer::spawn().await;
    let (_ca, mut ws_a, id_a) =
        open_device(&server, TEST_USER, TEST_PASS, "pb-state-device-a").await;
    let (_cb, mut ws_b, id_b) =
        open_device(&server, TEST_USER, TEST_PASS, "pb-state-device-b").await;
    assert_ne!(id_a, id_b);
    hello(&mut ws_a, "Device A").await;
    hello(&mut ws_b, "Device B").await;

    send(
        &mut ws_a,
        "playback.state",
        playing_state("Song", true, 12.0),
    )
    .await;

    let relayed = wait_for_message(&mut ws_b, "playback.device_state", RECV_TIMEOUT)
        .await
        .expect("B must receive device_state");
    assert_eq!(relayed["payload"]["device_id"].as_u64(), Some(id_a));
    assert_eq!(relayed["payload"]["device_name"], "Device A");
    assert_eq!(relayed["payload"]["state"]["is_playing"], true);
    assert_eq!(
        relayed["payload"]["state"]["current_track"]["title"],
        "Song"
    );
    assert_eq!(relayed["payload"]["state"]["position"], 12.0);

    let started = wait_for_list_change(&mut ws_b, "started_playing")
        .await
        .expect("B must receive started_playing list");
    let a = started["payload"]["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"].as_u64() == Some(id_a))
        .unwrap();
    assert_eq!(a["has_session"], true);
    assert_eq!(a["is_playing"], true);

    assert!(
        wait_for_message(&mut ws_a, "playback.device_state", SILENCE)
            .await
            .is_none(),
        "sender must not receive its own state"
    );

    // Pause flips is_playing and refreshes the list.
    send(
        &mut ws_a,
        "playback.state",
        playing_state("Song", false, 14.0),
    )
    .await;
    let changed = wait_for_list_change(&mut ws_b, "playback_changed")
        .await
        .expect("B must receive playback_changed list");
    let a = changed["payload"]["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"].as_u64() == Some(id_a))
        .unwrap();
    assert_eq!(a["has_session"], true);
    assert_eq!(a["is_playing"], false);

    // Stop marker removes the session entry.
    send(
        &mut ws_a,
        "playback.state",
        json!({
            "current_track": null,
            "queue_position": 0,
            "queue_version": 1,
            "position": 0.0,
            "is_playing": false,
            "volume": 0.8,
            "muted": false,
            "shuffle": false,
            "repeat": "off",
            "timestamp": 1_730_000_000_000u64
        }),
    )
    .await;
    let stopped = wait_for_message(&mut ws_b, "playback.device_stopped", RECV_TIMEOUT)
        .await
        .expect("B must receive device_stopped");
    assert_eq!(stopped["payload"]["device_id"].as_u64(), Some(id_a));
    assert_eq!(stopped["payload"]["reason"], "stopped");
    let list = wait_for_list_change(&mut ws_b, "stopped_playing")
        .await
        .expect("B must receive stopped_playing list");
    let a = list["payload"]["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"].as_u64() == Some(id_a))
        .unwrap();
    assert_eq!(a["has_session"], false);

    ws_a.close(None).await.ok();
    ws_b.close(None).await.ok();
}

#[tokio::test]
async fn queue_update_is_relayed_and_served_by_request_queue() {
    let server = TestServer::spawn().await;
    let (_ca, mut ws_a, id_a) =
        open_device(&server, TEST_USER, TEST_PASS, "pb-queue-device-a").await;
    let (_cb, mut ws_b, _id_b) =
        open_device(&server, TEST_USER, TEST_PASS, "pb-queue-device-b").await;
    hello(&mut ws_a, "Device A").await;
    hello(&mut ws_b, "Device B").await;

    let queue = json!([
        { "id": "track-1", "added_at": 1_730_000_000_000u64 },
        { "id": "track-2", "added_at": 1_730_000_000_001u64 }
    ]);
    let context = json!({ "type": "album", "id": "album-1" });
    send(
        &mut ws_a,
        "playback.queue_update",
        json!({ "queue": queue, "queue_version": 7, "context": context }),
    )
    .await;

    let relayed = wait_for_message(&mut ws_b, "playback.device_queue", RECV_TIMEOUT)
        .await
        .expect("B must receive device_queue");
    assert_eq!(relayed["payload"]["device_id"].as_u64(), Some(id_a));
    assert_eq!(relayed["payload"]["queue_version"], 7);
    assert_eq!(relayed["payload"]["queue"], queue);
    assert_eq!(relayed["payload"]["context"], context);

    send(
        &mut ws_b,
        "playback.request_queue",
        json!({ "target_device_id": id_a }),
    )
    .await;
    let sync = wait_for_message(&mut ws_b, "playback.queue_sync", RECV_TIMEOUT)
        .await
        .expect("B must receive queue_sync");
    assert_eq!(sync["payload"]["device_id"].as_u64(), Some(id_a));
    assert_eq!(sync["payload"]["queue_version"], 7);
    assert_eq!(sync["payload"]["queue"], queue);
    assert_eq!(sync["payload"]["context"], context);

    // Welcome for a late device carries the queue too.
    let (_cc, mut ws_c, _id_c) =
        open_device(&server, TEST_USER, TEST_PASS, "pb-queue-device-c").await;
    let welcome = hello(&mut ws_c, "Device C").await;
    let active = welcome["session"]["active_devices"].as_array().unwrap();
    let a = active
        .iter()
        .find(|d| d["device_id"].as_u64() == Some(id_a))
        .expect("A active in welcome");
    assert_eq!(a["queue"], queue);
    assert_eq!(a["queue_version"], 7);
    assert_eq!(a["context"], context);

    ws_a.close(None).await.ok();
    ws_b.close(None).await.ok();
    ws_c.close(None).await.ok();
}

#[tokio::test]
async fn commands_are_forwarded_and_validated() {
    let server = TestServer::spawn().await;
    let (_ca, mut ws_a, id_a) = open_device(&server, TEST_USER, TEST_PASS, "pb-cmd-device-a").await;
    let (_cb, mut ws_b, _id_b) =
        open_device(&server, TEST_USER, TEST_PASS, "pb-cmd-device-b").await;
    hello(&mut ws_a, "Device A").await;
    hello(&mut ws_b, "Device B").await;

    send(
        &mut ws_b,
        "playback.command",
        json!({ "command": "seek", "payload": { "position": 42.5 }, "target_device_id": id_a }),
    )
    .await;
    let forwarded = wait_for_message(&mut ws_a, "playback.command", RECV_TIMEOUT)
        .await
        .expect("A must receive the command");
    assert_eq!(forwarded["payload"]["command"], "seek");
    assert_eq!(forwarded["payload"]["payload"]["position"], 42.5);
    assert!(forwarded["payload"].get("target_device_id").is_none());

    // Unknown command name is rejected to the sender and not forwarded.
    send(
        &mut ws_b,
        "playback.command",
        json!({ "command": "selfDestruct", "payload": {}, "target_device_id": id_a }),
    )
    .await;
    let error = wait_for_message(&mut ws_b, "playback.error", RECV_TIMEOUT)
        .await
        .expect("B must receive playback.error");
    assert_eq!(error["payload"]["code"], "unknown_command");
    assert_eq!(error["payload"]["context"]["command"], "selfDestruct");
    assert!(
        wait_for_message(&mut ws_a, "playback.command", SILENCE)
            .await
            .is_none(),
        "unknown command must not be forwarded"
    );

    // Missing target is invalid.
    send(&mut ws_b, "playback.command", json!({ "command": "play" })).await;
    let error = wait_for_message(&mut ws_b, "playback.error", RECV_TIMEOUT)
        .await
        .expect("B must receive playback.error");
    assert_eq!(error["payload"]["code"], "invalid_message");

    // Unknown target device.
    send(
        &mut ws_b,
        "playback.command",
        json!({ "command": "play", "target_device_id": 999_999 }),
    )
    .await;
    let error = wait_for_message(&mut ws_b, "playback.error", RECV_TIMEOUT)
        .await
        .expect("B must receive playback.error");
    assert_eq!(error["payload"]["code"], "device_not_found");

    ws_a.close(None).await.ok();
    ws_b.close(None).await.ok();
}

#[tokio::test]
async fn share_policy_controls_visibility_for_other_users() {
    let server = TestServer::spawn().await;
    let other_handle = "pb-other-user";
    let other_pass = "pb-other-pass-123";
    create_second_user(&server, other_handle, other_pass).await;

    let (client_a, mut ws_a, id_a) =
        open_device(&server, TEST_USER, TEST_PASS, "pb-share-device-a").await;
    hello(&mut ws_a, "Device A").await;
    send(
        &mut ws_a,
        "playback.state",
        playing_state("Song", true, 1.0),
    )
    .await;

    let (_client_o, mut ws_o, id_o) =
        open_device(&server, other_handle, other_pass, "pb-share-device-o").await;
    let welcome = hello(&mut ws_o, "Other Device").await;
    assert!(
        !device_ids(&welcome["devices"]).contains(&id_a),
        "default policy must hide the device from other users"
    );
    assert!(
        welcome["session"]["active_devices"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["device_id"].as_u64() != Some(id_a)),
        "default policy must hide the device state from other users"
    );

    // Owner shares the device with everyone.
    put_share_policy(&client_a, &server.base_url, id_a, "allow_everyone").await;
    let changed = wait_for_list_change(&mut ws_o, "policy_changed")
        .await
        .expect("other user must receive policy_changed");
    assert_eq!(
        changed["payload"]["change"]["device_id"].as_u64(),
        Some(id_a)
    );
    let shared = changed["payload"]["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"].as_u64() == Some(id_a))
        .expect("shared device now visible");
    assert_eq!(shared["is_shared"], true);
    assert_eq!(shared["owner_handle"], TEST_USER);
    assert_eq!(shared["has_session"], true);
    assert!(device_ids(&changed["payload"]["devices"]).contains(&id_o));

    // The owner gets a refreshed list too.
    assert!(wait_for_list_change(&mut ws_a, "policy_changed")
        .await
        .is_some());

    // State relays now reach the other user, and they may send commands.
    send(
        &mut ws_a,
        "playback.state",
        playing_state("Song", true, 2.0),
    )
    .await;
    let relayed = wait_for_message(&mut ws_o, "playback.device_state", RECV_TIMEOUT)
        .await
        .expect("shared state must reach other user");
    assert_eq!(relayed["payload"]["device_id"].as_u64(), Some(id_a));

    send(
        &mut ws_o,
        "playback.command",
        json!({ "command": "pause", "target_device_id": id_a }),
    )
    .await;
    let forwarded = wait_for_message(&mut ws_a, "playback.command", RECV_TIMEOUT)
        .await
        .expect("owner device must receive command from allowed user");
    assert_eq!(forwarded["payload"]["command"], "pause");

    // Revoking access removes the device from the other user's list.
    put_share_policy(&client_a, &server.base_url, id_a, "deny_everyone").await;
    let revoked = wait_for_list_change(&mut ws_o, "policy_changed")
        .await
        .expect("other user must be told about the revoked policy");
    assert!(
        !device_ids(&revoked["payload"]["devices"]).contains(&id_a),
        "device must disappear after deny_everyone"
    );

    send(
        &mut ws_o,
        "playback.command",
        json!({ "command": "play", "target_device_id": id_a }),
    )
    .await;
    let error = wait_for_message(&mut ws_o, "playback.error", RECV_TIMEOUT)
        .await
        .expect("command after revoke must fail");
    assert_eq!(error["payload"]["code"], "forbidden");

    ws_a.close(None).await.ok();
    ws_o.close(None).await.ok();
}

#[tokio::test]
async fn reconnecting_device_survives_old_socket_cleanup() {
    let server = TestServer::spawn().await;
    let (_cb, mut ws_b, _id_b) =
        open_device(&server, TEST_USER, TEST_PASS, "pb-reconnect-device-b").await;
    hello(&mut ws_b, "Device B").await;

    // First connection of device A.
    let (_ca1, mut ws_a1, id_a) =
        open_device(&server, TEST_USER, TEST_PASS, "pb-reconnect-device-a").await;
    hello(&mut ws_a1, "Device A").await;
    send(
        &mut ws_a1,
        "playback.state",
        playing_state("Song", true, 5.0),
    )
    .await;
    wait_for_message(&mut ws_b, "playback.device_state", RECV_TIMEOUT)
        .await
        .expect("B sees A's first state");

    // Reconnect the same device while the old socket is still open.
    let (_ca2, mut ws_a2, id_a_again) =
        open_device(&server, TEST_USER, TEST_PASS, "pb-reconnect-device-a").await;
    assert_eq!(
        id_a, id_a_again,
        "same device uuid must map to the same device id"
    );
    let welcome = hello(&mut ws_a2, "Device A").await;
    assert_eq!(welcome["device_id"].as_u64(), Some(id_a));
    send(
        &mut ws_a2,
        "playback.state",
        playing_state("Song", true, 6.0),
    )
    .await;
    wait_for_message(&mut ws_b, "playback.device_state", RECV_TIMEOUT)
        .await
        .expect("B sees A's state from the new socket");

    // Now the old socket goes away. Its cleanup must not tear down the new one.
    ws_a1.close(None).await.ok();
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert!(
        wait_for_message(&mut ws_b, "playback.device_stopped", SILENCE)
            .await
            .is_none(),
        "closing the superseded socket must not stop the device"
    );

    // The new socket still receives relays from B.
    send(
        &mut ws_b,
        "playback.state",
        playing_state("Other Song", true, 1.0),
    )
    .await;
    let relayed = wait_for_message(&mut ws_a2, "playback.device_state", RECV_TIMEOUT)
        .await
        .expect("new socket of A must still receive relays");
    assert_eq!(
        relayed["payload"]["state"]["current_track"]["title"],
        "Other Song"
    );

    // And commands still reach it.
    send(
        &mut ws_b,
        "playback.command",
        json!({ "command": "pause", "target_device_id": id_a }),
    )
    .await;
    let forwarded = wait_for_message(&mut ws_a2, "playback.command", RECV_TIMEOUT)
        .await
        .expect("new socket of A must still receive commands");
    assert_eq!(forwarded["payload"]["command"], "pause");

    // A is still listed with its session for a late joiner.
    let (_cc, mut ws_c, _id_c) =
        open_device(&server, TEST_USER, TEST_PASS, "pb-reconnect-device-c").await;
    let welcome = hello(&mut ws_c, "Device C").await;
    let a = welcome["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"].as_u64() == Some(id_a))
        .expect("A still listed after old socket closed");
    assert_eq!(a["has_session"], true);

    ws_a2.close(None).await.ok();
    ws_b.close(None).await.ok();
    ws_c.close(None).await.ok();
}
