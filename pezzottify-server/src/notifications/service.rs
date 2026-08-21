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
                "Failed to send notification to {} devices for user {}",
                failed.len(),
                user_id
            );
        }

        Ok(notification)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user::{FullUserStore, SqliteUserStore};

    #[tokio::test]
    async fn notification_is_persisted_with_its_sync_event_without_connections() {
        let temp = tempfile::tempdir().unwrap();
        let store: Arc<dyn FullUserStore> = Arc::new(
            SqliteUserStore::new(
                temp.path().join("users.db"),
                &crate::backup::DbRegistry::new(),
            )
            .unwrap(),
        );
        let user_id = store.create_user("notification-user").unwrap();
        let service = NotificationService::new(store.clone(), Arc::new(ConnectionManager::new()));

        let notification = service
            .create_notification(
                user_id,
                NotificationType::DownloadCompleted,
                "Ready".to_string(),
                Some("Your album is ready".to_string()),
                serde_json::json!({"album_id": "album-1"}),
            )
            .await
            .unwrap();

        let stored = store.get_user_notifications(user_id).unwrap();
        assert_eq!(stored, vec![notification.clone()]);
        let events = store.get_events_since(user_id, 0).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0].event,
            UserEvent::NotificationCreated { notification: event_notification }
                if event_notification == &notification
        ));
    }
}
