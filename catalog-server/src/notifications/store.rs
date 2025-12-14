//! Notification storage trait

use anyhow::Result;

use super::models::{Notification, NotificationType};

/// Trait for notification storage operations
pub trait NotificationStore: Send + Sync {
    /// Create a notification for a user.
    /// Enforces the 100-notification-per-user limit by deleting oldest if needed.
    /// Returns the created notification with its ID and timestamps set.
    fn create_notification(
        &self,
        user_id: usize,
        notification_type: NotificationType,
        title: String,
        body: Option<String>,
        data: serde_json::Value,
    ) -> Result<Notification>;

    /// Get all notifications for a user, ordered by created_at DESC.
    fn get_user_notifications(&self, user_id: usize) -> Result<Vec<Notification>>;

    /// Get a single notification by ID (verifies ownership).
    fn get_notification(
        &self,
        notification_id: &str,
        user_id: usize,
    ) -> Result<Option<Notification>>;

    /// Mark a notification as read. Returns the updated notification.
    /// Returns None if notification doesn't exist or doesn't belong to user.
    fn mark_notification_read(
        &self,
        notification_id: &str,
        user_id: usize,
    ) -> Result<Option<Notification>>;

    /// Get count of unread notifications for a user.
    fn get_unread_count(&self, user_id: usize) -> Result<usize>;
}
