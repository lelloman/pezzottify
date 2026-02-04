//! Sync event notifier for download status updates.
//!
//! Emits sync events to notify connected clients about download status changes.
//! Events are stored in the user's event log and broadcast via WebSocket.

use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::server::websocket::connection::ConnectionManager;
use crate::server::websocket::messages::catalog::{
    CatalogInvalidationMessage, CatalogUpdatedMessage,
};
use crate::server::websocket::messages::msg_types::{CATALOG_INVALIDATION, CATALOG_UPDATED, SYNC};
use crate::server::websocket::messages::sync::SyncEventMessage;
use crate::server::websocket::messages::ServerMessage;
use crate::server_store::{CatalogContentType, CatalogEvent, CatalogEventType, ServerStore};
use crate::user::sync_events::{
    StoredEvent, SyncDownloadContentType, SyncDownloadProgress, SyncQueueStatus, UserEvent,
};
use crate::user::FullUserStore;

use super::models::{DownloadProgress, QueueItem, QueueStatus};

/// Handles sync event emission for download status updates.
///
/// This component bridges the download manager with the sync event system,
/// allowing download status updates to be pushed to connected clients in real-time.
pub struct DownloadSyncNotifier {
    user_store: Arc<dyn FullUserStore>,
    connection_manager: Arc<ConnectionManager>,
    server_store: Arc<dyn ServerStore>,
}

impl DownloadSyncNotifier {
    /// Create a new sync notifier.
    pub fn new(
        user_store: Arc<dyn FullUserStore>,
        connection_manager: Arc<ConnectionManager>,
        server_store: Arc<dyn ServerStore>,
    ) -> Self {
        Self {
            user_store,
            connection_manager,
            server_store,
        }
    }

    /// Notify that a download request was created.
    ///
    /// Called when a user requests an album download.
    pub async fn notify_request_created(&self, queue_item: &QueueItem, queue_position: i32) {
        let Some(user_id) = self.parse_user_id(&queue_item.requested_by_user_id) else {
            return;
        };

        let event = UserEvent::DownloadRequestCreated {
            request_id: queue_item.id.clone(),
            content_id: queue_item.content_id.clone(),
            content_type: SyncDownloadContentType::Album,
            content_name: queue_item.content_name.clone().unwrap_or_default(),
            artist_name: queue_item.artist_name.clone(),
            queue_position,
        };

        self.emit_event(user_id, event).await;
    }

    /// Notify that a download status changed.
    ///
    /// Called when an item transitions to a new status (InProgress, RetryWaiting, Failed).
    pub async fn notify_status_changed(
        &self,
        queue_item: &QueueItem,
        new_status: QueueStatus,
        queue_position: Option<i32>,
        error_message: Option<String>,
    ) {
        let Some(user_id) = self.parse_user_id(&queue_item.requested_by_user_id) else {
            return;
        };

        let event = UserEvent::DownloadStatusChanged {
            request_id: queue_item.id.clone(),
            content_id: queue_item.content_id.clone(),
            status: queue_status_to_sync(new_status),
            queue_position,
            error_message,
        };

        self.emit_event(user_id, event).await;
    }

    /// Notify that download progress was updated.
    ///
    /// Called when children of a parent item complete/fail, updating overall progress.
    pub async fn notify_progress_updated(
        &self,
        parent_item: &QueueItem,
        progress: &DownloadProgress,
    ) {
        let Some(user_id) = self.parse_user_id(&parent_item.requested_by_user_id) else {
            return;
        };

        let event = UserEvent::DownloadProgressUpdated {
            request_id: parent_item.id.clone(),
            content_id: parent_item.content_id.clone(),
            progress: SyncDownloadProgress {
                total_children: progress.total_children as i32,
                completed: progress.completed as i32,
                failed: progress.failed as i32,
                pending: progress.pending as i32,
                in_progress: progress.in_progress as i32,
            },
        };

        self.emit_event(user_id, event).await;
    }

    /// Notify that a download completed successfully.
    ///
    /// Called when a parent item (album) completes with all children done.
    pub async fn notify_completed(&self, queue_item: &QueueItem) {
        let Some(user_id) = self.parse_user_id(&queue_item.requested_by_user_id) else {
            return;
        };

        let event = UserEvent::DownloadCompleted {
            request_id: queue_item.id.clone(),
            content_id: queue_item.content_id.clone(),
        };

        self.emit_event(user_id, event).await;
    }

    /// Notify ALL connected clients that the catalog has been updated.
    ///
    /// Called when new content is added to the catalog (after download completes).
    /// This is a broadcast to all users, not just the one who requested the download.
    pub async fn notify_catalog_updated(&self, skeleton_version: i64) {
        let ws_msg =
            ServerMessage::new(CATALOG_UPDATED, CatalogUpdatedMessage { skeleton_version });

        let failed_count = self.connection_manager.broadcast_to_all(ws_msg).await;

        if failed_count > 0 {
            debug!(
                "Failed to send catalog_updated to {} connections",
                failed_count
            );
        }

        info!(
            "Broadcast catalog_updated (version={}) to all connected clients",
            skeleton_version
        );
    }

    /// Emit a catalog invalidation event and broadcast to ALL connected clients.
    ///
    /// This is used for surgical cache invalidation - clients can invalidate
    /// specific content entries rather than their entire cache.
    pub async fn emit_catalog_event(
        &self,
        event_type: CatalogEventType,
        content_type: CatalogContentType,
        content_id: &str,
        triggered_by: &str,
    ) {
        // Store the event in the server database
        let event = match self.server_store.append_catalog_event(
            event_type.clone(),
            content_type.clone(),
            content_id,
            Some(triggered_by),
        ) {
            Ok(seq) => CatalogEvent {
                seq,
                event_type: event_type.clone(),
                content_type: content_type.clone(),
                content_id: content_id.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
                triggered_by: Some(triggered_by.to_string()),
            },
            Err(e) => {
                warn!("Failed to store catalog event: {}", e);
                // Create a minimal event for broadcast even if storage failed
                CatalogEvent {
                    seq: 0,
                    event_type: event_type.clone(),
                    content_type: content_type.clone(),
                    content_id: content_id.to_string(),
                    timestamp: chrono::Utc::now().timestamp(),
                    triggered_by: Some(triggered_by.to_string()),
                }
            }
        };

        // Broadcast to all connected clients
        let ws_msg = ServerMessage::new(
            CATALOG_INVALIDATION,
            CatalogInvalidationMessage {
                seq: event.seq,
                event_type: event.event_type.as_str().to_string(),
                content_type: event.content_type.as_str().to_string(),
                content_id: event.content_id.clone(),
                timestamp: event.timestamp,
            },
        );

        let failed_count = self.connection_manager.broadcast_to_all(ws_msg).await;

        if failed_count > 0 {
            debug!(
                "Failed to send catalog_invalidation to {} connections",
                failed_count
            );
        }

        info!(
            "Broadcast catalog_invalidation: {} {} {} (seq={})",
            event.event_type.as_str(),
            event.content_type.as_str(),
            event.content_id,
            event.seq
        );
    }

    /// Parse user ID from the optional string stored in queue items.
    fn parse_user_id(&self, user_id_str: &Option<String>) -> Option<usize> {
        user_id_str.as_ref().and_then(|s| {
            s.parse::<usize>().ok().or_else(|| {
                warn!("Failed to parse user_id '{}' as usize", s);
                None
            })
        })
    }

    /// Store event and broadcast to user's connected devices.
    async fn emit_event(&self, user_id: usize, event: UserEvent) {
        // Store the event in the user's event log
        let stored_event = match self.user_store.append_event(user_id, &event) {
            Ok(stored) => stored,
            Err(e) => {
                warn!("Failed to store sync event for user {}: {}", user_id, e);
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
                "Failed to send sync event to {} devices for user {}",
                failed.len(),
                user_id
            );
        }
    }
}

/// Convert internal QueueStatus to sync event SyncQueueStatus.
fn queue_status_to_sync(status: QueueStatus) -> SyncQueueStatus {
    match status {
        QueueStatus::Pending => SyncQueueStatus::Pending,
        QueueStatus::InProgress => SyncQueueStatus::InProgress,
        QueueStatus::Completed => SyncQueueStatus::Completed,
        QueueStatus::Failed => SyncQueueStatus::Failed,
        QueueStatus::RetryWaiting => SyncQueueStatus::RetryWaiting,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_status_to_sync() {
        assert_eq!(
            queue_status_to_sync(QueueStatus::Pending),
            SyncQueueStatus::Pending
        );
        assert_eq!(
            queue_status_to_sync(QueueStatus::InProgress),
            SyncQueueStatus::InProgress
        );
        assert_eq!(
            queue_status_to_sync(QueueStatus::Completed),
            SyncQueueStatus::Completed
        );
        assert_eq!(
            queue_status_to_sync(QueueStatus::Failed),
            SyncQueueStatus::Failed
        );
        assert_eq!(
            queue_status_to_sync(QueueStatus::RetryWaiting),
            SyncQueueStatus::RetryWaiting
        );
    }
}
