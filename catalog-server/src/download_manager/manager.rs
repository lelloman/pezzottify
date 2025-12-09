//! Main download manager orchestration.
//!
//! Coordinates the download queue, processor, and related components.
//! This is the main facade for all download manager operations.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;

use crate::catalog_store::CatalogStore;
use crate::config::DownloadManagerSettings;

use super::audit_logger::AuditLogger;
use super::downloader_client::DownloaderClient;
use super::models::*;
use super::queue_store::DownloadQueueStore;
use super::retry_policy::RetryPolicy;

/// Main download manager that orchestrates all download operations.
///
/// Provides a unified interface for:
/// - Search proxy (forwarding searches to external downloader service)
/// - User request management (queuing album/discography downloads)
/// - Rate limiting (per-user and global capacity checks)
/// - Queue processing (executing downloads from the queue)
/// - Admin operations (viewing stats, retrying failed items)
/// - Audit logging (tracking all download operations)
pub struct DownloadManager {
    /// Queue store for persisting download requests.
    queue_store: Arc<dyn DownloadQueueStore>,
    /// HTTP client for communicating with the external downloader service.
    #[allow(dead_code)]
    downloader_client: DownloaderClient,
    /// Catalog store for checking existing content.
    #[allow(dead_code)]
    catalog_store: Arc<dyn CatalogStore>,
    /// Path to media files directory.
    media_path: PathBuf,
    /// Configuration settings.
    config: DownloadManagerSettings,
    /// Retry policy for failed downloads.
    #[allow(dead_code)]
    retry_policy: RetryPolicy,
    /// Audit logger for tracking operations.
    audit_logger: AuditLogger,
}

impl DownloadManager {
    /// Create a new DownloadManager.
    ///
    /// # Arguments
    /// * `queue_store` - Store for persisting download queue state
    /// * `downloader_client` - HTTP client for the downloader service
    /// * `catalog_store` - Store for checking existing catalog content
    /// * `media_path` - Path to the media files directory
    /// * `config` - Download manager configuration settings
    pub fn new(
        queue_store: Arc<dyn DownloadQueueStore>,
        downloader_client: DownloaderClient,
        catalog_store: Arc<dyn CatalogStore>,
        media_path: PathBuf,
        config: DownloadManagerSettings,
    ) -> Self {
        let retry_policy = RetryPolicy::new(&config);
        let audit_logger = AuditLogger::new(queue_store.clone());

        Self {
            queue_store,
            downloader_client,
            catalog_store,
            media_path,
            config,
            retry_policy,
            audit_logger,
        }
    }

    /// Check if the download manager is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the download manager configuration.
    pub fn config(&self) -> &DownloadManagerSettings {
        &self.config
    }

    /// Get the media path.
    pub fn media_path(&self) -> &PathBuf {
        &self.media_path
    }

    // =========================================================================
    // Search Proxy Methods (async - calls external downloader service)
    // =========================================================================

    /// Search for content via the external downloader service.
    ///
    /// Forwards the search request to the downloader and returns results.
    pub async fn search(
        &self,
        _query: &str,
        _search_type: SearchType,
    ) -> Result<SearchResults> {
        // TODO: Implement in DM-1.7.2
        todo!("search not yet implemented")
    }

    /// Search for an artist's discography via the external downloader service.
    pub async fn search_discography(&self, _artist_id: &str) -> Result<DiscographyResult> {
        // TODO: Implement in DM-1.7.2
        todo!("search_discography not yet implemented")
    }

    // =========================================================================
    // User Request Methods (sync - only touches local queue store)
    // =========================================================================

    /// Request download of an album.
    ///
    /// # Logic:
    /// 1. Check user rate limits
    /// 2. Check if already in catalog
    /// 3. Check if already in queue
    /// 4. Enqueue with appropriate priority
    /// 5. Log audit event
    /// 6. Return result with queue position
    pub fn request_album(
        &self,
        _user_id: &str,
        _request: AlbumRequest,
    ) -> Result<RequestResult> {
        // TODO: Implement in DM-1.7.3
        todo!("request_album not yet implemented")
    }

    /// Request download of an artist's full discography.
    pub fn request_discography(
        &self,
        _user_id: &str,
        _request: DiscographyRequest,
    ) -> Result<DiscographyRequestResult> {
        // TODO: Implement in DM-1.7.3
        todo!("request_discography not yet implemented")
    }

    /// Get all download requests for a user.
    pub fn get_user_requests(
        &self,
        user_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>> {
        self.queue_store.get_user_requests(user_id, limit, offset)
    }

    /// Get the status of a specific request for a user.
    pub fn get_request_status(
        &self,
        user_id: &str,
        request_id: &str,
    ) -> Result<Option<QueueItem>> {
        let requests = self.queue_store.get_user_requests(user_id, 1000, 0)?;
        Ok(requests.into_iter().find(|r| r.id == request_id))
    }

    // =========================================================================
    // Rate Limiting Methods (sync - only touches local queue store)
    // =========================================================================

    /// Check if a user has exceeded their rate limits.
    pub fn check_user_limits(&self, user_id: &str) -> Result<UserLimitStatus> {
        let stats = self.queue_store.get_user_stats(user_id)?;

        // Use config values for limits, stats provides the counts
        let max_per_day = self.config.user_max_requests_per_day as i32;
        let max_queue = self.config.user_max_queue_size as i32;

        let daily_limit_reached = stats.requests_today >= max_per_day;
        let queue_limit_reached = stats.in_queue >= max_queue;

        Ok(UserLimitStatus {
            requests_today: stats.requests_today,
            max_per_day,
            in_queue: stats.in_queue,
            max_queue,
            can_request: !daily_limit_reached && !queue_limit_reached,
        })
    }

    /// Check global system capacity.
    pub fn check_global_capacity(&self) -> Result<CapacityStatus> {
        let hourly = self.queue_store.get_hourly_counts()?;
        let daily = self.queue_store.get_daily_counts()?;

        let albums_this_hour = hourly.albums as i32;
        let albums_today = daily.albums as i32;

        let max_per_hour = self.config.max_albums_per_hour as i32;
        let max_per_day = self.config.max_albums_per_day as i32;

        let hourly_limit_reached = albums_this_hour >= max_per_hour;
        let daily_limit_reached = albums_today >= max_per_day;

        Ok(CapacityStatus {
            albums_this_hour,
            max_per_hour,
            albums_today,
            max_per_day,
            at_capacity: hourly_limit_reached || daily_limit_reached,
        })
    }

    // =========================================================================
    // Queue Processing Methods (async - calls external downloader service)
    // =========================================================================

    /// Process the next item in the queue.
    ///
    /// Returns `Ok(None)` if the queue is empty or capacity limits are reached.
    pub async fn process_next(&self) -> Result<Option<ProcessingResult>> {
        // TODO: Implement in DM-1.7.4
        todo!("process_next not yet implemented")
    }

    /// Promote items that are ready for retry back to pending status.
    ///
    /// Returns the number of items promoted.
    pub fn promote_ready_retries(&self) -> Result<usize> {
        let ready_items = self.queue_store.get_retry_ready()?;
        let count = ready_items.len();
        for item in &ready_items {
            self.queue_store.promote_retry_to_pending(&item.id)?;
        }
        Ok(count)
    }

    // =========================================================================
    // Admin Methods (sync - only touches local queue store)
    // =========================================================================

    /// Get queue statistics.
    pub fn get_queue_stats(&self) -> Result<QueueStats> {
        self.queue_store.get_queue_stats()
    }

    /// Get failed items from the queue.
    pub fn get_failed_items(&self, limit: usize, offset: usize) -> Result<Vec<QueueItem>> {
        self.queue_store.get_failed_items(limit, offset)
    }

    /// Retry a failed item (admin action).
    pub fn retry_failed(&self, admin_user_id: &str, request_id: &str) -> Result<()> {
        // Get the item
        let item = self
            .queue_store
            .get_item(request_id)?
            .ok_or_else(|| anyhow::anyhow!("Item not found: {}", request_id))?;

        // Verify it's in failed status
        if item.status != QueueStatus::Failed {
            return Err(anyhow::anyhow!(
                "Can only retry failed items, current status: {:?}",
                item.status
            ));
        }

        // TODO: Implement reset_failed_to_pending in DownloadQueueStore trait
        // For now, log the attempt but cannot actually reset the status
        // Need to add a trait method like `reset_to_pending(id)` that handles FAILED -> PENDING

        // Log the admin retry
        self.audit_logger.log_admin_retry(&item, admin_user_id)?;

        Ok(())
    }

    /// Get activity log entries.
    pub fn get_activity(&self, hours: usize) -> Result<Vec<ActivityLogEntry>> {
        let since = chrono::Utc::now().timestamp() - (hours as i64 * 3600);
        self.queue_store.get_activity_since(since)
    }

    /// Get all requests with optional filters.
    pub fn get_all_requests(
        &self,
        status: Option<QueueStatus>,
        _user_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>> {
        self.queue_store.list_all(status, limit, offset)
    }

    // =========================================================================
    // Audit Methods (sync - only touches local queue store)
    // =========================================================================

    /// Get audit log entries with filters.
    pub fn get_audit_log(&self, filter: AuditLogFilter) -> Result<(Vec<AuditLogEntry>, usize)> {
        self.queue_store.get_audit_log(filter)
    }

    /// Get a queue item with its audit history.
    pub fn get_audit_for_item(
        &self,
        queue_item_id: &str,
    ) -> Result<Option<(QueueItem, Vec<AuditLogEntry>)>> {
        let item = self.queue_store.get_item(queue_item_id)?;
        match item {
            Some(item) => {
                let entries = self.queue_store.get_audit_for_item(queue_item_id)?;
                Ok(Some((item, entries)))
            }
            None => Ok(None),
        }
    }

    /// Get audit log entries for a specific user.
    pub fn get_audit_for_user(
        &self,
        user_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AuditLogEntry>> {
        let (entries, _total) = self.queue_store.get_audit_for_user(user_id, None, None, limit, offset)?;
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::SqliteCatalogStore;
    use crate::download_manager::SqliteDownloadQueueStore;
    use std::sync::Arc;
    use tempfile::TempDir;

    struct TestContext {
        manager: DownloadManager,
        #[allow(dead_code)]
        temp_dir: TempDir,
    }

    fn create_test_manager() -> TestContext {
        let temp_dir = TempDir::new().unwrap();
        let queue_store = Arc::new(SqliteDownloadQueueStore::in_memory().unwrap());
        let catalog_db_path = temp_dir.path().join("catalog.db");
        let catalog_store = Arc::new(SqliteCatalogStore::new(&catalog_db_path, temp_dir.path()).unwrap());
        let downloader_client =
            DownloaderClient::new("http://localhost:8080".to_string(), 30).unwrap();
        let config = DownloadManagerSettings::default();
        let media_path = PathBuf::from("/tmp/media");

        let manager = DownloadManager::new(
            queue_store,
            downloader_client,
            catalog_store,
            media_path,
            config,
        );

        TestContext { manager, temp_dir }
    }

    #[test]
    fn test_new_manager() {
        let ctx = create_test_manager();
        assert!(!ctx.manager.is_enabled()); // Default config has enabled = false
        assert_eq!(ctx.manager.media_path(), &PathBuf::from("/tmp/media"));
    }

    #[test]
    fn test_check_user_limits_new_user() {
        let ctx = create_test_manager();

        let limits = ctx.manager.check_user_limits("new-user").unwrap();

        assert_eq!(limits.requests_today, 0);
        assert_eq!(limits.in_queue, 0);
        assert!(limits.can_request);
    }

    #[test]
    fn test_check_global_capacity_empty() {
        let ctx = create_test_manager();

        let capacity = ctx.manager.check_global_capacity().unwrap();

        assert_eq!(capacity.albums_this_hour, 0);
        assert_eq!(capacity.albums_today, 0);
        assert!(!capacity.at_capacity);
    }

    #[test]
    fn test_get_queue_stats_empty() {
        let ctx = create_test_manager();

        let stats = ctx.manager.get_queue_stats().unwrap();

        assert_eq!(stats.pending, 0);
        assert_eq!(stats.in_progress, 0);
        assert_eq!(stats.retry_waiting, 0);
        assert_eq!(stats.completed_today, 0);
        assert_eq!(stats.failed_today, 0);
    }

    #[test]
    fn test_get_failed_items_empty() {
        let ctx = create_test_manager();

        let failed = ctx.manager.get_failed_items(10, 0).unwrap();

        assert!(failed.is_empty());
    }

    #[test]
    fn test_get_user_requests_empty() {
        let ctx = create_test_manager();

        let requests = ctx.manager.get_user_requests("user-123", 100, 0).unwrap();

        assert!(requests.is_empty());
    }

    #[test]
    fn test_get_request_status_not_found() {
        let ctx = create_test_manager();

        let status = ctx.manager
            .get_request_status("user-123", "nonexistent")
            .unwrap();

        assert!(status.is_none());
    }

    #[test]
    fn test_promote_ready_retries_empty() {
        let ctx = create_test_manager();

        let promoted = ctx.manager.promote_ready_retries().unwrap();

        assert_eq!(promoted, 0);
    }

    #[test]
    fn test_get_activity_empty() {
        let ctx = create_test_manager();

        let activity = ctx.manager.get_activity(24).unwrap();

        assert!(activity.is_empty());
    }

    #[test]
    fn test_get_audit_log_empty() {
        let ctx = create_test_manager();

        let (entries, total) = ctx.manager.get_audit_log(AuditLogFilter::new()).unwrap();

        assert!(entries.is_empty());
        assert_eq!(total, 0);
    }

    #[test]
    fn test_get_audit_for_item_not_found() {
        let ctx = create_test_manager();

        let result = ctx.manager.get_audit_for_item("nonexistent").unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn test_get_audit_for_user_empty() {
        let ctx = create_test_manager();

        let entries = ctx.manager.get_audit_for_user("user-123", 10, 0).unwrap();

        assert!(entries.is_empty());
    }

    #[test]
    fn test_retry_failed_not_found() {
        let ctx = create_test_manager();

        let result = ctx.manager.retry_failed("admin", "nonexistent");

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}
