//! Per-user coalescing of wake-ups and delivery to every registration.

use super::sender::{send_wakeup, DeliveryOutcome};
use super::VapidKeys;
use crate::user::{FullUserStore, PushDeliveryRecord};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tracing::{debug, info, warn};

/// Wake-ups for one user within this window are sent once, carrying the latest seq.
pub const COALESCE_WINDOW: Duration = Duration::from_secs(2);
/// A registration failing continuously for this long is dropped.
pub const MAX_FAILURE_SECS: i64 = 7 * 24 * 60 * 60;

/// Collects wake-up requests and releases each user once per window.
#[derive(Debug, Default)]
pub(crate) struct Coalescer {
    window: Duration,
    pending: HashMap<usize, (Instant, i64)>,
}

impl Coalescer {
    pub(crate) fn new(window: Duration) -> Self {
        Self {
            window,
            pending: HashMap::new(),
        }
    }

    /// Record a wake-up for `user_id`. The first request opens the window; later
    /// ones in the same window only raise the sequence number.
    pub(crate) fn push(&mut self, user_id: usize, seq: i64, now: Instant) {
        self.pending
            .entry(user_id)
            .and_modify(|(_, latest)| *latest = (*latest).max(seq))
            .or_insert((now + self.window, seq));
    }

    pub(crate) fn next_due(&self) -> Option<Instant> {
        self.pending.values().map(|(due, _)| *due).min()
    }

    /// Remove and return every user whose window has elapsed.
    pub(crate) fn take_due(&mut self, now: Instant) -> Vec<(usize, i64)> {
        let due = self
            .pending
            .iter()
            .filter(|(_, (deadline, _))| *deadline <= now)
            .map(|(user, (_, seq))| (*user, *seq))
            .collect::<Vec<_>>();
        for (user, _) in &due {
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
}

/// Run until the request channel closes.
pub(crate) async fn run(mut requests: mpsc::UnboundedReceiver<(usize, i64)>, delivery: Arc<Delivery>) {
    let mut coalescer = Coalescer::new(COALESCE_WINDOW);
    loop {
        let due = coalescer.next_due();
        tokio::select! {
            request = requests.recv() => match request {
                Some((user_id, seq)) => coalescer.push(user_id, seq, Instant::now()),
                None => break,
            },
            _ = async { tokio::time::sleep_until(due.unwrap()).await }, if due.is_some() => {
                for (user_id, seq) in coalescer.take_due(Instant::now()) {
                    let delivery = delivery.clone();
                    tokio::spawn(async move { deliver_to_user(&delivery, user_id, seq).await });
                }
            }
        }
    }
}

/// Send one wake-up to each of the user's registrations and record the outcomes.
pub(crate) async fn deliver_to_user(delivery: &Delivery, user_id: usize, seq: i64) {
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
    let payload = serde_json::json!({ "type": "sync", "seq": seq }).to_string();
    for registration in registrations {
        let outcome = send_wakeup(
            &delivery.client,
            &delivery.vapid,
            &delivery.subject,
            &registration.endpoint,
            &registration.p256dh,
            &registration.auth,
            payload.as_bytes(),
        )
        .await;
        let store = delivery.store.clone();
        let endpoint = registration.endpoint.clone();
        let log_outcome = outcome.clone();
        let result = tokio::task::spawn_blocking(move || match outcome {
            DeliveryOutcome::Delivered => store
                .record_push_delivery(&endpoint, true, chrono::Utc::now().timestamp(), MAX_FAILURE_SECS)
                .map(|_| false),
            DeliveryOutcome::Gone => store.remove_push_endpoint(&endpoint),
            DeliveryOutcome::Failed(_) => store
                .record_push_delivery(&endpoint, false, chrono::Utc::now().timestamp(), MAX_FAILURE_SECS)
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
        let mut coalescer = Coalescer::new(Duration::from_secs(2));
        coalescer.push(1, 10, start);
        coalescer.push(1, 12, start + Duration::from_millis(500));
        coalescer.push(1, 11, start + Duration::from_millis(900));
        coalescer.push(2, 5, start + Duration::from_secs(1));
        assert_eq!(coalescer.next_due(), Some(start + Duration::from_secs(2)));

        assert!(coalescer.take_due(start + Duration::from_millis(1999)).is_empty());
        assert_eq!(coalescer.take_due(start + Duration::from_secs(2)), vec![(1, 12)]);
        assert_eq!(coalescer.next_due(), Some(start + Duration::from_secs(3)));
        assert_eq!(coalescer.take_due(start + Duration::from_secs(3)), vec![(2, 5)]);
        assert_eq!(coalescer.next_due(), None);

        // A new event after release opens a fresh window.
        coalescer.push(1, 13, start + Duration::from_secs(4));
        assert_eq!(coalescer.next_due(), Some(start + Duration::from_secs(6)));
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
                let response = format!("HTTP/1.1 {status} X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
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
            },
            vapid,
            dir,
        )
    }

    fn user_store() -> (Arc<SqliteUserStore>, tempfile::TempDir, usize) {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteUserStore::new(dir.path().join("user.db"), &crate::backup::DbRegistry::new())
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

        deliver_to_user(&delivery, user, 42).await;

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
            store.upsert_push_registration(user, &url, &p256dh, &auth, None).unwrap();
            deliver_to_user(&delivery, user, 1).await;
            assert!(store.list_push_registrations(user).unwrap().is_empty(), "HTTP {status}");
        }

        let (url, _) = push_service(vec![500]).await;
        store.upsert_push_registration(user, &url, &p256dh, &auth, None).unwrap();
        deliver_to_user(&delivery, user, 1).await;
        let registration = &store.list_push_registrations(user).unwrap()[0];
        assert!(registration.first_failure_at.is_some(), "a 500 keeps the registration");
    }

    #[tokio::test]
    async fn listener_wakes_only_for_notification_worthy_events() {
        use crate::user::{UserEvent, UserEventStore};
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
        let service = super::super::PushService::start(settings, store.clone()).unwrap();
        assert!(service.install_listener(store.as_ref()));

        // Not worthy: no push.
        store
            .append_event(user, &UserEvent::NotificationRead { notification_id: "n".into(), read_at: 1 })
            .unwrap();
        // Two worthy events in one window coalesce into one push with the later seq.
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
            .append_event(user, &UserEvent::NotificationCreated { notification: notification.clone() })
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
        let plain = ece::decrypt(&keypair.raw_components().unwrap(), &auth, &requests[0].body).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&plain).unwrap()["seq"],
            serde_json::json!(last.seq)
        );
    }
}
