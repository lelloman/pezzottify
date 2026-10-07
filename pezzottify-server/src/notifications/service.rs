//! Notification service for creating and broadcasting notifications

use std::sync::Arc;
use tracing::{debug, warn};

use crate::db_executor::{DbHandle, DbPriority};
use crate::server::websocket::connection::ConnectionManager;
use crate::server::websocket::messages::msg_types::SYNC;
use crate::server::websocket::messages::sync::SyncEventMessage;
use crate::server::websocket::messages::ServerMessage;
use crate::user::sync_events::UserEvent;
use crate::user::FullUserStore;

use super::models::{Notification, NotificationType};

/// Service for creating notifications and broadcasting to connected clients
pub struct NotificationService {
    user_store: DbHandle<dyn FullUserStore>,
    connection_manager: Arc<ConnectionManager>,
}

impl NotificationService {
    pub fn new(
        user_store: DbHandle<dyn FullUserStore>,
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
        let (notification, stored_event) = self
            .user_store
            .run(DbPriority::Background, move |store| {
                let notification =
                    store.create_notification(user_id, notification_type, title, body, data)?;
                let event = UserEvent::NotificationCreated {
                    notification: notification.clone(),
                };
                match store.append_event(user_id, &event) {
                    Ok(event) => Ok((notification, Some(event))),
                    Err(error) => {
                        warn!("Failed to log notification_created event: {}", error);
                        Ok((notification, None))
                    }
                }
            })
            .await?;

        let Some(stored_event) = stored_event else {
            return Ok(notification);
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
        let handle = DbHandle::new(
            store.clone(),
            crate::db_executor::DbExecutor::new(Default::default()),
            crate::db_executor::DbLane::User,
        );
        let service = NotificationService::new(handle, Arc::new(ConnectionManager::new()));

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
