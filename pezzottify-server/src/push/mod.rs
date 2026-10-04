//! UnifiedPush / Web Push wake-ups. See docs/unifiedpush.md.
//!
//! A push carries no user content: it tells the app that new sync events exist,
//! and the app runs its normal authenticated catch-up.

mod dispatcher;
mod sender;
mod vapid;

pub use sender::{validate_endpoint, validate_keys, DeliveryOutcome, RegistrationError, WakeupOptions};
pub use vapid::VapidKeys;

use crate::config::PushSettings;
use crate::server::websocket::connection::ConnectionManager;
use crate::user::{FullUserStore, UserEvent};
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Why a user's devices are woken. Ordered: a notification outranks a sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WakeKind {
    /// Other devices should catch up with ordinary changes (likes, playlists,
    /// settings, ...), at most once per [`dispatcher::SYNC_WINDOW`].
    Sync,
    /// Something the user should be notified about; sent within seconds.
    Notification,
}

/// How an appended event should wake the user's devices, if at all.
pub fn wake_kind(event: &UserEvent) -> Option<WakeKind> {
    match event {
        UserEvent::NotificationCreated { .. } | UserEvent::WhatsNewBatchClosed { .. } => {
            Some(WakeKind::Notification)
        }
        // Repeats throughout a download; completion has its own notification.
        UserEvent::DownloadProgressUpdated { .. } => None,
        _ => Some(WakeKind::Sync),
    }
}

/// Whether an appended event should wake the user's devices.
pub fn wakes_devices(event: &UserEvent) -> bool {
    wake_kind(event).is_some()
}

/// Sends wake-ups for notification-worthy events. Created once per server.
pub struct PushService {
    settings: PushSettings,
    vapid: VapidKeys,
    requests: mpsc::UnboundedSender<(usize, i64, WakeKind)>,
}

impl PushService {
    /// Load or create the VAPID key and start the delivery task. Must be called
    /// from within a Tokio runtime.
    /// Devices with a live WebSocket in `connections` already receive events and are
    /// not woken.
    pub fn start(
        settings: PushSettings,
        store: Arc<dyn FullUserStore>,
        connections: Option<Arc<ConnectionManager>>,
    ) -> Result<Arc<Self>> {
        let vapid = VapidKeys::load_or_generate(&settings.vapid_private_key_file)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let (requests, receiver) = mpsc::unbounded_channel();
        let delivery = Arc::new(dispatcher::Delivery {
            store,
            client,
            vapid: vapid.clone(),
            subject: settings.vapid_subject.clone(),
            connections,
        });
        tokio::spawn(dispatcher::run(receiver, delivery));
        Ok(Arc::new(Self {
            settings,
            vapid,
            requests,
        }))
    }

    /// Schedule a (coalesced) wake-up for `user_id`. Never blocks.
    pub fn wake(&self, user_id: usize, seq: i64, kind: WakeKind) {
        let _ = self.requests.send((user_id, seq, kind));
    }

    /// Have `store` call [`Self::wake`] after each notification-worthy append.
    /// Returns false if the store does not support event listeners.
    pub fn install_listener(self: &Arc<Self>, store: &dyn FullUserStore) -> bool {
        let requests = self.requests.clone();
        store.set_event_listener(Arc::new(move |user_id, stored| {
            if let Some(kind) = wake_kind(&stored.event) {
                let _ = requests.send((user_id, stored.seq, kind));
            }
        }))
    }

    pub fn public_key(&self) -> &str {
        self.vapid.public_key()
    }

    pub fn settings(&self) -> &PushSettings {
        &self.settings
    }
}
