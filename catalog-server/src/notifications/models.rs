//! Notification data models

use serde::{Deserialize, Serialize};

/// Notification type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    DownloadCompleted,
    // Future: DownloadFailed, NewRelease, SystemAnnouncement
}

/// A user notification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub notification_type: NotificationType,
    pub title: String,
    pub body: Option<String>,
    pub data: serde_json::Value,
    pub read_at: Option<i64>,
    pub created_at: i64,
}

/// Data payload for DownloadCompleted notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadCompletedData {
    pub album_id: String,
    pub album_name: String,
    pub artist_name: String,
    pub image_id: Option<String>,
    pub request_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_type_serialization() {
        let download_completed = NotificationType::DownloadCompleted;
        let serialized = serde_json::to_string(&download_completed).unwrap();
        assert_eq!(serialized, "\"download_completed\"");

        let deserialized: NotificationType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, NotificationType::DownloadCompleted);
    }

    #[test]
    fn test_notification_serialization() {
        let notification = Notification {
            id: "notif-123".to_string(),
            notification_type: NotificationType::DownloadCompleted,
            title: "Album Ready".to_string(),
            body: Some("Your album is ready to listen".to_string()),
            data: serde_json::json!({
                "album_id": "album-456",
                "album_name": "Test Album"
            }),
            read_at: None,
            created_at: 1700000000,
        };

        let serialized = serde_json::to_string(&notification).unwrap();
        let deserialized: Notification = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.id, "notif-123");
        assert_eq!(deserialized.notification_type, NotificationType::DownloadCompleted);
        assert_eq!(deserialized.title, "Album Ready");
        assert_eq!(deserialized.body, Some("Your album is ready to listen".to_string()));
        assert!(deserialized.read_at.is_none());
        assert_eq!(deserialized.created_at, 1700000000);
    }

    #[test]
    fn test_notification_with_read_at() {
        let notification = Notification {
            id: "notif-123".to_string(),
            notification_type: NotificationType::DownloadCompleted,
            title: "Album Ready".to_string(),
            body: None,
            data: serde_json::Value::Null,
            read_at: Some(1700001000),
            created_at: 1700000000,
        };

        let serialized = serde_json::to_string(&notification).unwrap();
        let deserialized: Notification = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.read_at, Some(1700001000));
    }

    #[test]
    fn test_download_completed_data_serialization() {
        let data = DownloadCompletedData {
            album_id: "album-123".to_string(),
            album_name: "Test Album".to_string(),
            artist_name: "Test Artist".to_string(),
            image_id: Some("img-456".to_string()),
            request_id: "req-789".to_string(),
        };

        let serialized = serde_json::to_string(&data).unwrap();
        let deserialized: DownloadCompletedData = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.album_id, "album-123");
        assert_eq!(deserialized.album_name, "Test Album");
        assert_eq!(deserialized.artist_name, "Test Artist");
        assert_eq!(deserialized.image_id, Some("img-456".to_string()));
        assert_eq!(deserialized.request_id, "req-789");
    }

    #[test]
    fn test_download_completed_data_without_image() {
        let data = DownloadCompletedData {
            album_id: "album-123".to_string(),
            album_name: "Test Album".to_string(),
            artist_name: "Test Artist".to_string(),
            image_id: None,
            request_id: "req-789".to_string(),
        };

        let serialized = serde_json::to_string(&data).unwrap();
        let deserialized: DownloadCompletedData = serde_json::from_str(&serialized).unwrap();

        assert!(deserialized.image_id.is_none());
    }
}
