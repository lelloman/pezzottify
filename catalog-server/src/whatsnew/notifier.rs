//! WhatsNew event notifier.
//!
//! Emits sync events to notify users who have opted in when a changelog batch is closed.
//! Events are stored in the user's event log and broadcast via WebSocket.

use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::catalog_store::{BatchChangeSummary, CatalogBatch};
use crate::server::websocket::connection::ConnectionManager;
use crate::server::websocket::messages::msg_types::SYNC;
use crate::server::websocket::messages::sync::SyncEventMessage;
use crate::server::websocket::messages::ServerMessage;
use crate::user::settings::UserSetting;
use crate::user::sync_events::{StoredEvent, UserEvent};
use crate::user::FullUserStore;

/// Handles WhatsNew event emission for catalog batch closures.
///
/// This component notifies users who have opted in to WhatsNew notifications
/// when a changelog batch is closed, meaning new content is available.
pub struct WhatsNewNotifier {
    user_store: Arc<dyn FullUserStore>,
    connection_manager: Arc<ConnectionManager>,
}

impl WhatsNewNotifier {
    /// Create a new WhatsNew notifier.
    pub fn new(
        user_store: Arc<dyn FullUserStore>,
        connection_manager: Arc<ConnectionManager>,
    ) -> Self {
        Self {
            user_store,
            connection_manager,
        }
    }

    /// Notify all opted-in users that a changelog batch has been closed.
    ///
    /// This will:
    /// 1. Query all users who have notify_whatsnew = true
    /// 2. For each user, store the event and broadcast via WebSocket
    pub async fn notify_batch_closed(&self, batch: &CatalogBatch, summary: &BatchChangeSummary) {
        // Get the setting key and value for notify_whatsnew = true
        let setting = UserSetting::NotifyWhatsNew(true);
        let key = setting.key();
        let value = setting.value_to_string();

        // Get all users with this setting enabled
        let user_ids = match self.user_store.get_user_ids_with_setting(key, &value) {
            Ok(ids) => ids,
            Err(e) => {
                warn!("Failed to get users with notify_whatsnew enabled: {}", e);
                return;
            }
        };

        if user_ids.is_empty() {
            debug!("No users opted in for WhatsNew notifications");
            return;
        }

        info!(
            "Notifying {} users about batch closure: {} ({})",
            user_ids.len(),
            batch.id,
            batch.name
        );

        // Create the event with counts from the summary
        let event = UserEvent::WhatsNewBatchClosed {
            batch_id: batch.id.clone(),
            batch_name: batch.name.clone(),
            description: batch.description.clone(),
            albums_added: summary.albums.added.len() as i32,
            artists_added: summary.artists.added.len() as i32,
            tracks_added: summary.tracks.added_count as i32,
        };

        // Notify each opted-in user
        for user_id in user_ids {
            self.emit_event(user_id, event.clone()).await;
        }
    }

    /// Store event and broadcast to user's connected devices.
    async fn emit_event(&self, user_id: usize, event: UserEvent) {
        // Store the event in the user's event log
        let stored_event = match self.user_store.append_event(user_id, &event) {
            Ok(stored) => stored,
            Err(e) => {
                warn!("Failed to store WhatsNew event for user {}: {}", user_id, e);
                // Create a minimal stored event for broadcast even if storage failed
                StoredEvent {
                    seq: 0,
                    event,
                    server_timestamp: chrono::Utc::now().timestamp(),
                }
            }
        };

        // Broadcast to all connected devices
        let ws_msg = ServerMessage::new(
            SYNC,
            SyncEventMessage {
                event: stored_event,
            },
        );

        let failed = self
            .connection_manager
            .broadcast_to_user(user_id, ws_msg)
            .await;

        if !failed.is_empty() {
            debug!(
                "Failed to send WhatsNew event to {} devices for user {}",
                failed.len(),
                user_id
            );
        }
    }
}
