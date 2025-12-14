//! Notification service for creating and broadcasting notifications

use std::sync::Arc;
use tracing::{debug, warn};

use crate::server::websocket::connection::ConnectionManager;
use crate::server::websocket::messages::msg_types::SYNC;
use crate::server::websocket::messages::sync::SyncEventMessage;
use crate::server::websocket::messages::ServerMessage;
use crate::user::sync_events::UserEvent;
use crate::user::FullUserStore;

use super::models::{Notification, NotificationType};

/// Service for creating notifications and broadcasting to connected clients
pub struct NotificationService {
    user_store: Arc<dyn FullUserStore>,
    connection_manager: Arc<ConnectionManager>,
}

impl NotificationService {
    pub fn new(
        user_store: Arc<dyn FullUserStore>,
        connection_manager: Arc<ConnectionManager>,
    ) -> Self {
        Self {
            user_store,
            connection_manager,
        }
    }

    /// Create a notification and broadcast to all user's devices.
    pub async fn create_notification(
        &self,
        user_id: usize,
        notification_type: NotificationType,
        title: String,
        body: Option<String>,
        data: serde_json::Value,
    ) -> anyhow::Result<Notification> {
        // 1. Create notification in database
        let notification =
            self.user_store
                .create_notification(user_id, notification_type, title, body, data)?;

        // 2. Log sync event
        let event = UserEvent::NotificationCreated {
            notification: notification.clone(),
        };

        let stored_event = match self.user_store.append_event(user_id, &event) {
            Ok(e) => e,
            Err(err) => {
                warn!("Failed to log notification_created event: {}", err);
                return Ok(notification);
            }
        };

        // 3. Broadcast to all user's devices
        let ws_msg = ServerMessage::new(SYNC, SyncEventMessage { event: stored_event });

        let failed = self
            .connection_manager
            .broadcast_to_user(user_id, ws_msg)
            .await;

        if !failed.is_empty() {
            debug!(
                "Failed to send notification to {} devices for user {}",
                failed.len(),
                user_id
            );
        }

        Ok(notification)
    }
}
