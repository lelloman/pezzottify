//! Audit logging for download operations.
//!
//! Provides a higher-level interface for logging download events,
//! building on the queue store's raw audit log functionality.

use std::sync::Arc;

use anyhow::Result;

use super::models::{
    AuditEventType, AuditLogEntry, DownloadError, QueueItem, RequestSource, WatchdogReport,
};
use super::queue_store::DownloadQueueStore;

/// Helper for logging audit events during download operations.
///
/// Provides convenient methods that automatically populate audit entries
/// from queue items and other context.
pub struct AuditLogger {
    queue_store: Arc<dyn DownloadQueueStore>,
}

impl AuditLogger {
    /// Create a new AuditLogger with the given queue store.
    pub fn new(queue_store: Arc<dyn DownloadQueueStore>) -> Self {
        Self { queue_store }
    }

    /// Log a new download request being created.
    ///
    /// Records the queue position at the time of creation.
    pub fn log_request_created(&self, queue_item: &QueueItem, queue_position: usize) -> Result<()> {
        let entry = AuditLogEntry::new(AuditEventType::RequestCreated)
            .with_queue_item(queue_item.id.clone())
            .with_content(queue_item.content_type, queue_item.content_id.clone())
            .with_source(queue_item.request_source)
            .with_details(serde_json::json!({
                "queue_position": queue_position,
                "priority": queue_item.priority.as_i32(),
                "content_name": queue_item.content_name,
                "artist_name": queue_item.artist_name,
            }));

        let entry = if let Some(user_id) = &queue_item.requested_by_user_id {
            entry.with_user(user_id.clone())
        } else {
            entry
        };

        self.queue_store.log_audit_event(entry)
    }

    /// Log a download starting processing.
    pub fn log_download_started(&self, queue_item: &QueueItem) -> Result<()> {
        let entry = AuditLogEntry::new(AuditEventType::DownloadStarted)
            .with_queue_item(queue_item.id.clone())
            .with_content(queue_item.content_type, queue_item.content_id.clone())
            .with_source(queue_item.request_source);

        let entry = if let Some(user_id) = &queue_item.requested_by_user_id {
            entry.with_user(user_id.clone())
        } else {
            entry
        };

        self.queue_store.log_audit_event(entry)
    }

    /// Log children being created for a parent album item.
    ///
    /// Records the breakdown of track audio and image children.
    pub fn log_children_created(
        &self,
        parent_item: &QueueItem,
        children_count: usize,
        track_count: usize,
        image_count: usize,
    ) -> Result<()> {
        let entry = AuditLogEntry::new(AuditEventType::ChildrenCreated)
            .with_queue_item(parent_item.id.clone())
            .with_content(parent_item.content_type, parent_item.content_id.clone())
            .with_source(parent_item.request_source)
            .with_details(serde_json::json!({
                "children_count": children_count,
                "track_count": track_count,
                "image_count": image_count,
            }));

        let entry = if let Some(user_id) = &parent_item.requested_by_user_id {
            entry.with_user(user_id.clone())
        } else {
            entry
        };

        self.queue_store.log_audit_event(entry)
    }

    /// Log a download completing successfully.
    ///
    /// For album completions, includes the track count.
    pub fn log_download_completed(
        &self,
        queue_item: &QueueItem,
        bytes_downloaded: u64,
        duration_ms: i64,
        tracks_downloaded: Option<usize>,
    ) -> Result<()> {
        let mut details = serde_json::json!({
            "bytes_downloaded": bytes_downloaded,
            "duration_ms": duration_ms,
        });

        if let Some(tracks) = tracks_downloaded {
            details["tracks_downloaded"] = serde_json::json!(tracks);
        }

        let entry = AuditLogEntry::new(AuditEventType::DownloadCompleted)
            .with_queue_item(queue_item.id.clone())
            .with_content(queue_item.content_type, queue_item.content_id.clone())
            .with_source(queue_item.request_source)
            .with_details(details);

        let entry = if let Some(user_id) = &queue_item.requested_by_user_id {
            entry.with_user(user_id.clone())
        } else {
            entry
        };

        self.queue_store.log_audit_event(entry)
    }

    /// Log a download failing permanently (after max retries).
    pub fn log_download_failed(&self, queue_item: &QueueItem, error: &DownloadError) -> Result<()> {
        let entry = AuditLogEntry::new(AuditEventType::DownloadFailed)
            .with_queue_item(queue_item.id.clone())
            .with_content(queue_item.content_type, queue_item.content_id.clone())
            .with_source(queue_item.request_source)
            .with_details(serde_json::json!({
                "error_type": error.error_type.as_str(),
                "error_message": error.message,
                "retry_count": queue_item.retry_count,
            }));

        let entry = if let Some(user_id) = &queue_item.requested_by_user_id {
            entry.with_user(user_id.clone())
        } else {
            entry
        };

        self.queue_store.log_audit_event(entry)
    }

    /// Log a retry being scheduled for a failed download.
    pub fn log_retry_scheduled(
        &self,
        queue_item: &QueueItem,
        next_retry_at: i64,
        backoff_secs: u64,
        error: &DownloadError,
    ) -> Result<()> {
        let entry = AuditLogEntry::new(AuditEventType::RetryScheduled)
            .with_queue_item(queue_item.id.clone())
            .with_content(queue_item.content_type, queue_item.content_id.clone())
            .with_source(queue_item.request_source)
            .with_details(serde_json::json!({
                "retry_count": queue_item.retry_count,
                "next_retry_at": next_retry_at,
                "backoff_secs": backoff_secs,
                "error_type": error.error_type.as_str(),
                "error_message": error.message,
            }));

        let entry = if let Some(user_id) = &queue_item.requested_by_user_id {
            entry.with_user(user_id.clone())
        } else {
            entry
        };

        self.queue_store.log_audit_event(entry)
    }

    /// Log an admin manually retrying a failed item.
    pub fn log_admin_retry(&self, queue_item: &QueueItem, admin_user_id: &str) -> Result<()> {
        let entry = AuditLogEntry::new(AuditEventType::AdminRetry)
            .with_queue_item(queue_item.id.clone())
            .with_content(queue_item.content_type, queue_item.content_id.clone())
            .with_source(queue_item.request_source)
            .with_user(admin_user_id.to_string())
            .with_details(serde_json::json!({
                "previous_retry_count": queue_item.retry_count,
                "previous_error_type": queue_item.error_type.as_ref().map(|e| e.as_str()),
                "previous_error_message": queue_item.error_message,
            }));

        self.queue_store.log_audit_event(entry)
    }

    /// Log the watchdog queuing an item for repair.
    pub fn log_watchdog_queued(&self, queue_item: &QueueItem, reason: &str) -> Result<()> {
        let entry = AuditLogEntry::new(AuditEventType::WatchdogQueued)
            .with_queue_item(queue_item.id.clone())
            .with_content(queue_item.content_type, queue_item.content_id.clone())
            .with_source(RequestSource::Watchdog)
            .with_details(serde_json::json!({
                "reason": reason,
            }));

        self.queue_store.log_audit_event(entry)
    }

    /// Log a watchdog scan starting.
    pub fn log_watchdog_scan_started(&self) -> Result<()> {
        let entry = AuditLogEntry::new(AuditEventType::WatchdogScanStarted)
            .with_source(RequestSource::Watchdog);

        self.queue_store.log_audit_event(entry)
    }

    /// Log a watchdog scan completing.
    pub fn log_watchdog_scan_completed(&self, report: &WatchdogReport) -> Result<()> {
        let entry = AuditLogEntry::new(AuditEventType::WatchdogScanCompleted)
            .with_source(RequestSource::Watchdog)
            .with_details(serde_json::json!({
                "missing_track_audio_count": report.missing_track_audio.len(),
                "missing_album_images_count": report.missing_album_images.len(),
                "missing_artist_images_count": report.missing_artist_images.len(),
                "total_missing": report.total_missing(),
                "items_queued": report.items_queued,
                "items_skipped": report.items_skipped,
                "scan_duration_ms": report.scan_duration_ms,
                "is_clean": report.is_clean(),
            }));

        self.queue_store.log_audit_event(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download_manager::{
        DownloadContentType, DownloadErrorType, QueuePriority, QueueStatus,
        SqliteDownloadQueueStore,
    };
    use std::sync::Arc;

    fn create_test_store() -> Arc<SqliteDownloadQueueStore> {
        Arc::new(SqliteDownloadQueueStore::in_memory().unwrap())
    }

    fn create_test_queue_item() -> QueueItem {
        QueueItem {
            id: "test-item-123".to_string(),
            parent_id: None,
            status: QueueStatus::Pending,
            priority: QueuePriority::User,
            content_type: DownloadContentType::Album,
            content_id: "album-456".to_string(),
            content_name: Some("Test Album".to_string()),
            artist_name: Some("Test Artist".to_string()),
            request_source: RequestSource::User,
            requested_by_user_id: Some("user-789".to_string()),
            created_at: chrono::Utc::now().timestamp(),
            started_at: None,
            completed_at: None,
            last_attempt_at: None,
            next_retry_at: None,
            retry_count: 0,
            max_retries: 5,
            error_type: None,
            error_message: None,
            bytes_downloaded: None,
            processing_duration_ms: None,
        }
    }

    #[test]
    fn test_log_request_created() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());
        let item = create_test_queue_item();

        let result = logger.log_request_created(&item, 5);
        assert!(result.is_ok());

        let entries = store.get_audit_for_item(&item.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::RequestCreated);
        assert_eq!(entries[0].queue_item_id, Some(item.id.clone()));
        assert_eq!(entries[0].content_type, Some(DownloadContentType::Album));
        assert_eq!(entries[0].content_id, Some("album-456".to_string()));
        assert_eq!(entries[0].user_id, Some("user-789".to_string()));
        assert_eq!(entries[0].request_source, Some(RequestSource::User));

        let details = entries[0].details.as_ref().unwrap();
        assert_eq!(details["queue_position"], 5);
    }

    #[test]
    fn test_log_download_started() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());
        let item = create_test_queue_item();

        let result = logger.log_download_started(&item);
        assert!(result.is_ok());

        let entries = store.get_audit_for_item(&item.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::DownloadStarted);
    }

    #[test]
    fn test_log_children_created() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());
        let item = create_test_queue_item();

        let result = logger.log_children_created(&item, 15, 12, 3);
        assert!(result.is_ok());

        let entries = store.get_audit_for_item(&item.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::ChildrenCreated);

        let details = entries[0].details.as_ref().unwrap();
        assert_eq!(details["children_count"], 15);
        assert_eq!(details["track_count"], 12);
        assert_eq!(details["image_count"], 3);
    }

    #[test]
    fn test_log_download_completed() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());
        let item = create_test_queue_item();

        let result = logger.log_download_completed(&item, 1024000, 5000, Some(12));
        assert!(result.is_ok());

        let entries = store.get_audit_for_item(&item.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::DownloadCompleted);

        let details = entries[0].details.as_ref().unwrap();
        assert_eq!(details["bytes_downloaded"], 1024000);
        assert_eq!(details["duration_ms"], 5000);
        assert_eq!(details["tracks_downloaded"], 12);
    }

    #[test]
    fn test_log_download_completed_without_tracks() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());
        let mut item = create_test_queue_item();
        item.content_type = DownloadContentType::TrackAudio;

        let result = logger.log_download_completed(&item, 5000000, 1000, None);
        assert!(result.is_ok());

        let entries = store.get_audit_for_item(&item.id).unwrap();
        let details = entries[0].details.as_ref().unwrap();
        assert!(details.get("tracks_downloaded").is_none());
    }

    #[test]
    fn test_log_download_failed() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());
        let mut item = create_test_queue_item();
        item.retry_count = 5;

        let error = DownloadError::new(DownloadErrorType::NotFound, "Album not found on provider");

        let result = logger.log_download_failed(&item, &error);
        assert!(result.is_ok());

        let entries = store.get_audit_for_item(&item.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::DownloadFailed);

        let details = entries[0].details.as_ref().unwrap();
        assert_eq!(details["error_type"], "not_found");
        assert_eq!(details["error_message"], "Album not found on provider");
        assert_eq!(details["retry_count"], 5);
    }

    #[test]
    fn test_log_retry_scheduled() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());
        let mut item = create_test_queue_item();
        item.retry_count = 2;

        let error = DownloadError::new(DownloadErrorType::Connection, "Connection refused");
        let next_retry_at = chrono::Utc::now().timestamp() + 240;

        let result = logger.log_retry_scheduled(&item, next_retry_at, 240, &error);
        assert!(result.is_ok());

        let entries = store.get_audit_for_item(&item.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::RetryScheduled);

        let details = entries[0].details.as_ref().unwrap();
        assert_eq!(details["retry_count"], 2);
        assert_eq!(details["next_retry_at"], next_retry_at);
        assert_eq!(details["backoff_secs"], 240);
        assert_eq!(details["error_type"], "connection");
    }

    #[test]
    fn test_log_admin_retry() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());
        let mut item = create_test_queue_item();
        item.status = QueueStatus::Failed;
        item.retry_count = 5;
        item.error_type = Some(DownloadErrorType::Timeout);
        item.error_message = Some("Request timed out".to_string());

        let result = logger.log_admin_retry(&item, "admin-user-id");
        assert!(result.is_ok());

        let entries = store.get_audit_for_item(&item.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::AdminRetry);
        assert_eq!(entries[0].user_id, Some("admin-user-id".to_string()));

        let details = entries[0].details.as_ref().unwrap();
        assert_eq!(details["previous_retry_count"], 5);
        assert_eq!(details["previous_error_type"], "timeout");
        assert_eq!(details["previous_error_message"], "Request timed out");
    }

    #[test]
    fn test_log_watchdog_queued() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());
        let mut item = create_test_queue_item();
        item.content_type = DownloadContentType::TrackAudio;
        item.request_source = RequestSource::Watchdog;

        let result = logger.log_watchdog_queued(&item, "missing_audio_file");
        assert!(result.is_ok());

        let entries = store.get_audit_for_item(&item.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::WatchdogQueued);
        assert_eq!(entries[0].request_source, Some(RequestSource::Watchdog));

        let details = entries[0].details.as_ref().unwrap();
        assert_eq!(details["reason"], "missing_audio_file");
    }

    #[test]
    fn test_log_watchdog_scan_started() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());

        let result = logger.log_watchdog_scan_started();
        assert!(result.is_ok());

        let (entries, _) = store
            .get_audit_log(crate::download_manager::AuditLogFilter::new())
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::WatchdogScanStarted);
        assert_eq!(entries[0].request_source, Some(RequestSource::Watchdog));
    }

    #[test]
    fn test_log_watchdog_scan_completed() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());

        let report = WatchdogReport {
            missing_track_audio: vec!["track1".to_string(), "track2".to_string()],
            missing_album_images: vec!["img1".to_string()],
            missing_artist_images: vec![],
            items_queued: 3,
            items_skipped: 1,
            scan_duration_ms: 1500,
        };

        let result = logger.log_watchdog_scan_completed(&report);
        assert!(result.is_ok());

        let (entries, _) = store
            .get_audit_log(crate::download_manager::AuditLogFilter::new())
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].event_type,
            AuditEventType::WatchdogScanCompleted
        );

        let details = entries[0].details.as_ref().unwrap();
        assert_eq!(details["missing_track_audio_count"], 2);
        assert_eq!(details["missing_album_images_count"], 1);
        assert_eq!(details["missing_artist_images_count"], 0);
        assert_eq!(details["total_missing"], 3);
        assert_eq!(details["items_queued"], 3);
        assert_eq!(details["items_skipped"], 1);
        assert_eq!(details["scan_duration_ms"], 1500);
        assert_eq!(details["is_clean"], false);
    }

    #[test]
    fn test_log_watchdog_scan_completed_clean() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());

        let report = WatchdogReport::default();

        let result = logger.log_watchdog_scan_completed(&report);
        assert!(result.is_ok());

        let (entries, _) = store
            .get_audit_log(crate::download_manager::AuditLogFilter::new())
            .unwrap();
        let details = entries[0].details.as_ref().unwrap();
        assert_eq!(details["is_clean"], true);
        assert_eq!(details["total_missing"], 0);
    }

    #[test]
    fn test_log_request_without_user() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());
        let mut item = create_test_queue_item();
        item.requested_by_user_id = None;
        item.request_source = RequestSource::Watchdog;

        let result = logger.log_request_created(&item, 0);
        assert!(result.is_ok());

        let entries = store.get_audit_for_item(&item.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].user_id.is_none());
        assert_eq!(entries[0].request_source, Some(RequestSource::Watchdog));
    }

    #[test]
    fn test_multiple_events_for_same_item() {
        let store = create_test_store();
        let logger = AuditLogger::new(store.clone());
        let item = create_test_queue_item();

        // Log a sequence of events
        logger.log_request_created(&item, 0).unwrap();
        logger.log_download_started(&item).unwrap();
        logger.log_children_created(&item, 10, 8, 2).unwrap();
        logger
            .log_download_completed(&item, 50000000, 30000, Some(8))
            .unwrap();

        let entries = store.get_audit_for_item(&item.id).unwrap();
        assert_eq!(entries.len(), 4);

        // Verify event order (oldest first - ASC order)
        assert_eq!(entries[0].event_type, AuditEventType::RequestCreated);
        assert_eq!(entries[1].event_type, AuditEventType::DownloadStarted);
        assert_eq!(entries[2].event_type, AuditEventType::ChildrenCreated);
        assert_eq!(entries[3].event_type, AuditEventType::DownloadCompleted);
    }
}
