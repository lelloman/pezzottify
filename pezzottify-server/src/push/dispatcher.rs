//! Per-user coalescing of wake-ups and delivery to every registration.

use super::sender::{send_wakeup, DeliveryOutcome, PushTarget, WakeupOptions};
use super::{VapidKeys, WakeKind};
use crate::server::websocket::connection::ConnectionManager;
use crate::user::{FullUserStore, PushDeliveryRecord};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tracing::{debug, info, warn};

/// Notification wake-ups for one user within this window are sent once.
pub const NOTIFICATION_WINDOW: Duration = Duration::from_secs(2);
/// Ordinary changes wake other devices at most once per window: the first change
/// opens it and later ones fold in, so continuous editing still syncs every minute.
pub const SYNC_WINDOW: Duration = Duration::from_secs(60);

fn window(kind: WakeKind) -> Duration {
    match kind {
        WakeKind::Notification => NOTIFICATION_WINDOW,
        WakeKind::Sync => SYNC_WINDOW,
    }
}
/// A registration failing continuously for this long is dropped.
pub const MAX_FAILURE_SECS: i64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pending {
    due: Instant,
    seq: i64,
    kind: WakeKind,
}

/// Collects wake-up requests and releases each user once per window.
#[derive(Debug, Default)]
pub(crate) struct Coalescer {
    pending: HashMap<usize, Pending>,
}

impl Coalescer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Record a wake-up for `user_id`. The first request opens a window sized by its
    /// kind; later ones raise the sequence number, and a notification arriving in an
    /// open sync window shortens it and upgrades the wake-up.
    pub(crate) fn push(&mut self, user_id: usize, seq: i64, kind: WakeKind, now: Instant) {
        let due = now + window(kind);
        self.pending
            .entry(user_id)
            .and_modify(|pending| {
                pending.seq = pending.seq.max(seq);
                pending.due = pending.due.min(due);
                pending.kind = pending.kind.max(kind);
            })
            .or_insert(Pending { due, seq, kind });
    }

    pub(crate) fn next_due(&self) -> Option<Instant> {
        self.pending.values().map(|pending| pending.due).min()
    }

    /// Remove and return every user whose window has elapsed.
    pub(crate) fn take_due(&mut self, now: Instant) -> Vec<(usize, i64, WakeKind)> {
        let due = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.due <= now)
            .map(|(user, pending)| (*user, pending.seq, pending.kind))
            .collect::<Vec<_>>();
        for (user, _, _) in &due {
            self.pending.remove(user);
        }
        due
    }
}

/// Everything needed to deliver wake-ups.
pub(crate) struct Delivery {
    pub(crate) store: Arc<dyn FullUserStore>,
    pub(crate) client: reqwest::Client,
    pub(crate) vapid: VapidKeys,
    pub(crate) subject: String,
    /// Live WebSocket connections; those devices already receive events.
    pub(crate) connections: Option<Arc<ConnectionManager>>,
}

/// Run until the request channel closes.
pub(crate) async fn run(
    mut requests: mpsc::UnboundedReceiver<(usize, i64, WakeKind)>,
    delivery: Arc<Delivery>,
) {
    let mut coalescer = Coalescer::new();
    loop {
        let due = coalescer.next_due();
        tokio::select! {
            request = requests.recv() => match request {
                Some((user_id, seq, kind)) => coalescer.push(user_id, seq, kind, Instant::now()),
                None => break,
            },
            _ = async { tokio::time::sleep_until(due.unwrap()).await }, if due.is_some() => {
                for (user_id, seq, kind) in coalescer.take_due(Instant::now()) {
                    let delivery = delivery.clone();
                    tokio::spawn(async move { deliver_to_user(&delivery, user_id, seq, kind).await });
                }
            }
        }
    }
}

/// UUIDs of the user's devices that currently have a WebSocket open.
async fn connected_device_uuids(
    delivery: &Delivery,
    user_id: usize,
) -> std::collections::HashSet<String> {
    let Some(connections) = &delivery.connections else {
        return Default::default();
    };
    let connected = connections.get_connected_devices(user_id).await;
    if connected.is_empty() {
        return Default::default();
    }
    let store = delivery.store.clone();
    tokio::task::spawn_blocking(move || {
        connected
            .into_iter()
            .filter_map(|device_id| store.get_device(device_id).ok().flatten())
            .map(|device| device.device_uuid)
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// Send one wake-up to each of the user's registrations and record the outcomes.
/// Devices with a live WebSocket are skipped: they already received the events.
pub(crate) async fn deliver_to_user(delivery: &Delivery, user_id: usize, seq: i64, kind: WakeKind) {
    let store = delivery.store.clone();
    let registrations =
        match tokio::task::spawn_blocking(move || store.list_push_registrations(user_id)).await {
            Ok(Ok(registrations)) => registrations,
            Ok(Err(error)) => {
                warn!("Push: cannot list registrations for user {user_id}: {error:#}");
                return;
            }
            Err(error) => {
                warn!("Push: registration lookup panicked: {error}");
                return;
            }
        };
    if registrations.is_empty() {
        return;
    }
    let connected = connected_device_uuids(delivery, user_id).await;
    let registrations = registrations
        .into_iter()
        .filter(|registration| {
            !registration
                .device_id
                .as_ref()
                .is_some_and(|device| connected.contains(device))
        })
        .collect::<Vec<_>>();
    let options = match kind {
        WakeKind::Notification => WakeupOptions::NOTIFICATION,
        WakeKind::Sync => WakeupOptions::SYNC,
    };
    let payload = serde_json::json!({ "type": "sync", "seq": seq }).to_string();
    for registration in registrations {
        let outcome = send_wakeup(
            &delivery.client,
            &delivery.vapid,
            &delivery.subject,
            PushTarget {
                endpoint: &registration.endpoint,
                p256dh: &registration.p256dh,
                auth: &registration.auth,
            },
            payload.as_bytes(),
            options,
        )
        .await;
        let store = delivery.store.clone();
        let endpoint = registration.endpoint.clone();
        let log_outcome = outcome.clone();
        let result = tokio::task::spawn_blocking(move || match outcome {
            DeliveryOutcome::Delivered => store
                .record_push_delivery(
                    &endpoint,
                    true,
                    chrono::Utc::now().timestamp(),
                    MAX_FAILURE_SECS,
                )
                .map(|_| false),
            DeliveryOutcome::Gone => store.remove_push_endpoint(&endpoint),
            DeliveryOutcome::Failed(_) => store
                .record_push_delivery(
                    &endpoint,
                    false,
                    chrono::Utc::now().timestamp(),
                    MAX_FAILURE_SECS,
                )
                .map(|record| record == PushDeliveryRecord::Removed),
        })
        .await;
        let host = reqwest::Url::parse(&registration.endpoint)
            .ok()
            .and_then(|url| url.host_str().map(str::to_string))
            .unwrap_or_default();
        match (&log_outcome, result) {
            (DeliveryOutcome::Delivered, _) => {
                debug!("Push: woke user {user_id} via {host} (seq {seq})")
            }
            (DeliveryOutcome::Gone, Ok(Ok(_))) => {
                info!("Push: endpoint on {host} is gone; registration removed (user {user_id})")
            }
            (DeliveryOutcome::Failed(reason), Ok(Ok(true))) => warn!(
                "Push: endpoint on {host} failed for 7 days ({reason}); registration removed (user {user_id})"
            ),
            (DeliveryOutcome::Failed(reason), _) => {
                warn!("Push: delivery to {host} failed for user {user_id}: {reason}")
            }
            (_, Ok(Err(error))) => warn!("Push: cannot record delivery: {error:#}"),
            (_, Err(error)) => warn!("Push: delivery bookkeeping panicked: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_bursts_per_user_and_keeps_the_latest_seq() {
        let start = Instant::now();
        let mut coalescer = Coalescer::new();
        coalescer.push(1, 10, WakeKind::Notification, start);
        coalescer.push(
            1,
            12,
            WakeKind::Notification,
            start + Duration::from_millis(500),
        );
        coalescer.push(
            1,
            11,
            WakeKind::Notification,
            start + Duration::from_millis(900),
        );
        coalescer.push(2, 5, WakeKind::Notification, start + Duration::from_secs(1));
        assert_eq!(coalescer.next_due(), Some(start + Duration::from_secs(2)));

        assert!(coalescer
            .take_due(start + Duration::from_millis(1999))
            .is_empty());
        assert_eq!(
            coalescer.take_due(start + Duration::from_secs(2)),
            vec![(1, 12, WakeKind::Notification)]
        );
        assert_eq!(coalescer.next_due(), Some(start + Duration::from_secs(3)));
        assert_eq!(
            coalescer.take_due(start + Duration::from_secs(3)),
            vec![(2, 5, WakeKind::Notification)]
        );
        assert_eq!(coalescer.next_due(), None);

        // A new event after release opens a fresh window.
        coalescer.push(
            1,
            13,
            WakeKind::Notification,
            start + Duration::from_secs(4),
        );
        assert_eq!(coalescer.next_due(), Some(start + Duration::from_secs(6)));
    }

    #[test]
    fn sync_changes_wake_at_most_once_per_minute() {
        let start = Instant::now();
        let mut coalescer = Coalescer::new();
        // Continuous editing: one change every 5 s keeps folding into the same window.
        for i in 0..12 {
            coalescer.push(
                1,
                i,
                WakeKind::Sync,
                start + Duration::from_secs(5 * i as u64),
            );
        }
        assert_eq!(coalescer.next_due(), Some(start + SYNC_WINDOW));
        assert!(coalescer
            .take_due(start + Duration::from_secs(59))
            .is_empty());
        assert_eq!(
            coalescer.take_due(start + SYNC_WINDOW),
            vec![(1, 11, WakeKind::Sync)]
        );
        // Editing continues: the next change opens the next one-minute window.
        coalescer.push(1, 12, WakeKind::Sync, start + Duration::from_secs(61));
        assert_eq!(coalescer.next_due(), Some(start + Duration::from_secs(121)));
    }

    #[test]
    fn a_notification_shortens_and_upgrades_an_open_sync_window() {
        let start = Instant::now();
        let mut coalescer = Coalescer::new();
        coalescer.push(1, 1, WakeKind::Sync, start);
        coalescer.push(
            1,
            2,
            WakeKind::Notification,
            start + Duration::from_secs(10),
        );
        assert_eq!(coalescer.next_due(), Some(start + Duration::from_secs(12)));
        // A later sync change folds in without delaying or downgrading it.
        coalescer.push(1, 3, WakeKind::Sync, start + Duration::from_secs(11));
        assert_eq!(
            coalescer.take_due(start + Duration::from_secs(12)),
            vec![(1, 3, WakeKind::Notification)]
        );
    }

    #[test]
    fn events_map_to_wake_kinds() {
        use crate::user::UserEvent;
        assert_eq!(
            super::super::wake_kind(&UserEvent::NotificationRead {
                notification_id: "n".into(),
                read_at: 1
            }),
            Some(WakeKind::Sync)
        );
        let progress = UserEvent::DownloadProgressUpdated {
            request_id: "r".into(),
            content_id: "c".into(),
            progress: crate::user::sync_events::SyncDownloadProgress {
                total_children: 2,
                completed: 1,
                failed: 0,
                pending: 1,
                in_progress: 0,
            },
        };
        assert_eq!(super::super::wake_kind(&progress), None);
    }

    // ---- delivery against a local push service ----

    use crate::user::{PushRegistrationStore, SqliteUserStore, UserStore};
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[derive(Debug, Clone)]
    struct Received {
        head: String,
        body: Vec<u8>,
    }

    /// A one-route HTTP server answering every request with the next status.
    async fn push_service(statuses: Vec<u16>) -> (String, Arc<Mutex<Vec<Received>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/up/endpoint", listener.local_addr().unwrap());
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = received.clone();
        tokio::spawn(async move {
            for status in statuses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buffer = Vec::new();
                let mut chunk = [0u8; 4096];
                let (head, body) = loop {
                    let read = socket.read(&mut chunk).await.unwrap();
                    buffer.extend_from_slice(&chunk[..read]);
                    if let Some(split) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buffer[..split]).to_string();
                        let length = head
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        while buffer.len() < split + 4 + length {
                            let read = socket.read(&mut chunk).await.unwrap();
                            buffer.extend_from_slice(&chunk[..read]);
                        }
                        break (head, buffer[split + 4..split + 4 + length].to_vec());
                    }
                };
                log.lock().unwrap().push(Received { head, body });
                let response = format!(
                    "HTTP/1.1 {status} X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        (url, received)
    }

    fn delivery(store: Arc<SqliteUserStore>) -> (Delivery, VapidKeys, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let vapid = VapidKeys::load_or_generate(&dir.path().join("vapid.pem")).unwrap();
        (
            Delivery {
                store,
                client: reqwest::Client::new(),
                vapid: vapid.clone(),
                subject: "mailto:test@example.org".into(),
                connections: None,
            },
            vapid,
            dir,
        )
    }

    fn user_store() -> (Arc<SqliteUserStore>, tempfile::TempDir, usize) {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteUserStore::new(
                dir.path().join("user.db"),
                &crate::backup::DbRegistry::new(),
            )
            .unwrap(),
        );
        let user = store.create_user("listener").unwrap();
        (store, dir, user)
    }

    #[tokio::test]
    async fn delivers_an_encrypted_vapid_signed_wakeup() {
        let (store, _db, user) = user_store();
        let (url, received) = push_service(vec![201]).await;
        let (keypair, auth) = ece::generate_keypair_and_auth_secret().unwrap();
        let p256dh = URL_SAFE_NO_PAD.encode(keypair.pub_as_raw().unwrap());
        store
            .upsert_push_registration(user, &url, &p256dh, &URL_SAFE_NO_PAD.encode(auth), None)
            .unwrap();
        let (delivery, vapid, _keys) = delivery(store.clone());

        deliver_to_user(&delivery, user, 42, WakeKind::Notification).await;

        let request = received.lock().unwrap()[0].clone();
        let head = request.head.to_ascii_lowercase();
        assert!(head.starts_with("post /up/endpoint "));
        assert!(head.contains("ttl: 86400"));
        assert!(head.contains("urgency: normal"));
        assert!(head.contains("content-encoding: aes128gcm"));
        assert!(head.contains(&format!("k={}", vapid.public_key().to_ascii_lowercase())));
        let plain = ece::decrypt(&keypair.raw_components().unwrap(), &auth, &request.body).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&plain).unwrap(),
            serde_json::json!({"type": "sync", "seq": 42})
        );
        let registration = &store.list_push_registrations(user).unwrap()[0];
        assert!(registration.last_success_at.is_some());
        assert_eq!(registration.first_failure_at, None);
    }

    #[tokio::test]
    async fn gone_endpoints_are_removed_and_failures_are_recorded() {
        let (store, _db, user) = user_store();
        let (keypair, auth) = ece::generate_keypair_and_auth_secret().unwrap();
        let p256dh = URL_SAFE_NO_PAD.encode(keypair.pub_as_raw().unwrap());
        let auth = URL_SAFE_NO_PAD.encode(auth);
        let (delivery, _vapid, _keys) = delivery(store.clone());

        for status in [404, 410] {
            let (url, _) = push_service(vec![status]).await;
            store
                .upsert_push_registration(user, &url, &p256dh, &auth, None)
                .unwrap();
            deliver_to_user(&delivery, user, 1, WakeKind::Notification).await;
            assert!(
                store.list_push_registrations(user).unwrap().is_empty(),
                "HTTP {status}"
            );
        }

        let (url, _) = push_service(vec![500]).await;
        store
            .upsert_push_registration(user, &url, &p256dh, &auth, None)
            .unwrap();
        deliver_to_user(&delivery, user, 1, WakeKind::Notification).await;
        let registration = &store.list_push_registrations(user).unwrap()[0];
        assert!(
            registration.first_failure_at.is_some(),
            "a 500 keeps the registration"
        );
    }

    #[tokio::test]
    async fn listener_wakes_only_for_notification_worthy_events() {
        use crate::user::UserEvent;
        let (store, _db, user) = user_store();
        let (url, received) = push_service(vec![201]).await;
        let (keypair, auth) = ece::generate_keypair_and_auth_secret().unwrap();
        store
            .upsert_push_registration(
                user,
                &url,
                &URL_SAFE_NO_PAD.encode(keypair.pub_as_raw().unwrap()),
                &URL_SAFE_NO_PAD.encode(auth),
                None,
            )
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut settings = crate::config::PushSettings::with_defaults(dir.path());
        settings.allow_insecure_endpoints = true;
        let service = super::super::PushService::start(settings, store.clone(), None).unwrap();
        assert!(service.install_listener(store.as_ref()));

        // An ordinary change opens a one-minute sync window...
        store
            .append_event(
                user,
                &UserEvent::NotificationRead {
                    notification_id: "n".into(),
                    read_at: 1,
                },
            )
            .unwrap();
        // ...which the notifications shorten: one push, carrying the latest seq.
        let notification = crate::notifications::Notification {
            id: "n1".into(),
            notification_type: crate::notifications::NotificationType::DownloadCompleted,
            title: "Ready".into(),
            body: None,
            data: serde_json::json!({}),
            read_at: None,
            created_at: 1,
        };
        store
            .append_event(
                user,
                &UserEvent::NotificationCreated {
                    notification: notification.clone(),
                },
            )
            .unwrap();
        let last = store
            .append_event(user, &UserEvent::NotificationCreated { notification })
            .unwrap();
        for _ in 0..400 {
            if !received.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let requests = received.lock().unwrap().clone();
        assert_eq!(requests.len(), 1);
        let plain =
            ece::decrypt(&keypair.raw_components().unwrap(), &auth, &requests[0].body).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&plain).unwrap()["seq"],
            serde_json::json!(last.seq)
        );
    }

    #[tokio::test]
    async fn sync_wakeups_are_low_urgency_and_skip_connected_devices() {
        use crate::user::device::{DeviceRegistration, DeviceType};
        use crate::user::DeviceStore;
        let (store, _db, user) = user_store();
        let (keypair, auth) = ece::generate_keypair_and_auth_secret().unwrap();
        let p256dh = URL_SAFE_NO_PAD.encode(keypair.pub_as_raw().unwrap());
        let auth = URL_SAFE_NO_PAD.encode(auth);
        let device = |uuid: &str| DeviceRegistration {
            device_uuid: uuid.into(),
            device_type: DeviceType::Android,
            device_name: None,
            os_info: None,
        };
        let online = store
            .register_or_update_device(&device("phone-online"))
            .unwrap();
        store
            .register_or_update_device(&device("phone-asleep"))
            .unwrap();
        let (online_url, online_received) = push_service(vec![201]).await;
        let (asleep_url, asleep_received) = push_service(vec![201]).await;
        store
            .upsert_push_registration(user, &online_url, &p256dh, &auth, Some("phone-online"))
            .unwrap();
        store
            .upsert_push_registration(user, &asleep_url, &p256dh, &auth, Some("phone-asleep"))
            .unwrap();
        let connections = Arc::new(ConnectionManager::new());
        let _socket = connections.register(user, online, "android".into()).await;
        let (mut delivery, _vapid, _keys) = delivery(store.clone());
        delivery.connections = Some(connections);

        deliver_to_user(&delivery, user, 7, WakeKind::Sync).await;

        assert!(
            online_received.lock().unwrap().is_empty(),
            "live WebSocket: no push"
        );
        let request = asleep_received.lock().unwrap()[0].clone();
        let head = request.head.to_ascii_lowercase();
        assert!(head.contains("urgency: low"));
        assert!(head.contains("ttl: 3600"));
        assert!(head.contains("topic: sync"));
    }
}
