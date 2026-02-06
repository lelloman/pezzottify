//! Main download manager orchestration.
//!
//! Coordinates download requests via a passive queue. Requests are stored and
//! fulfilled by an external script through the ingestion system.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::catalog_store::{CatalogStore, TrackAvailability};
use crate::config::DownloadManagerSettings;
use crate::ingestion::{DownloadManagerTrait, QueueItemInfo};
use crate::search::SearchVault;

use super::audit_logger::AuditLogger;
use super::models::*;
use super::queue_store::DownloadQueueStore;
use super::retry_policy::RetryPolicy;
use super::sync_notifier::DownloadSyncNotifier;

impl DownloadManagerTrait for DownloadManager {
    fn get_queue_item(&self, item_id: &str) -> Result<Option<QueueItemInfo>> {
        let item = self.queue_store.get_item(item_id)?;
        Ok(item.map(|i| QueueItemInfo {
            id: i.id,
            content_id: i.content_id,
            content_name: i.content_name,
            artist_name: i.artist_name,
        }))
    }

    fn mark_request_completed(
        &self,
        item_id: &str,
        bytes_downloaded: u64,
        duration_ms: i64,
    ) -> Result<()> {
        self.queue_store
            .mark_completed(item_id, bytes_downloaded, duration_ms)?;

        // Log audit event
        if let Ok(Some(item)) = self.queue_store.get_item(item_id) {
            self.audit_logger.log_download_completed(
                &item,
                bytes_downloaded,
                duration_ms,
                None, // No track count available from ingestion
            )?;
        }

        Ok(())
    }

    fn mark_request_in_progress(&self, item_id: &str) -> Result<()> {
        match self.queue_store.claim_for_processing(item_id) {
            Ok(true) => {
                info!(
                    "Marked download request {} as IN_PROGRESS (ingestion started)",
                    item_id
                );
                // Log audit event
                if let Ok(Some(item)) = self.queue_store.get_item(item_id) {
                    let _ = self.audit_logger.log_download_started(&item);
                }
                Ok(())
            }
            Ok(false) => {
                warn!(
                    "Download request {} was not PENDING (already claimed or missing)",
                    item_id
                );
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn mark_request_failed(&self, item_id: &str, error_message: &str) -> Result<()> {
        // Check current status — don't overwrite COMPLETED with FAILED
        // (possible in collection uploads where multiple jobs share one queue item)
        if let Ok(Some(item)) = self.queue_store.get_item(item_id) {
            if item.status == QueueStatus::Completed {
                info!(
                    "Download request {} already COMPLETED, not marking as failed",
                    item_id
                );
                return Ok(());
            }
        }

        let error = DownloadError::new(DownloadErrorType::Unknown, error_message);
        self.queue_store.mark_failed(item_id, &error)?;

        // Log audit event
        if let Ok(Some(item)) = self.queue_store.get_item(item_id) {
            let _ = self.audit_logger.log_download_failed(&item, &error);
        }

        info!(
            "Marked download request {} as FAILED: {}",
            item_id, error_message
        );

        Ok(())
    }

    fn complete_requests_for_album(
        &self,
        album_id: &str,
        bytes_downloaded: u64,
        duration_ms: i64,
    ) -> Result<Vec<String>> {
        let pending_requests = self
            .queue_store
            .find_pending_by_content(DownloadContentType::Album, album_id)?;

        let mut completed_ids = Vec::new();

        for item in pending_requests {
            if let Err(e) = self.mark_request_completed(&item.id, bytes_downloaded, duration_ms) {
                warn!(
                    "Failed to auto-complete download request {} for album {}: {}",
                    item.id, album_id, e
                );
            } else {
                info!(
                    "Auto-completed download request {} for album {} (ingestion fulfilled)",
                    item.id, album_id
                );
                completed_ids.push(item.id);
            }
        }

        Ok(completed_ids)
    }
}

/// Main download manager that orchestrates all download operations.
///
/// Provides a unified interface for:
/// - User request management (queuing track/album downloads)
/// - Rate limiting (per-user checks)
/// - Admin operations (viewing stats, retrying failed items)
/// - Audit logging (tracking all download operations)
pub struct DownloadManager {
    /// Queue store for persisting download requests.
    queue_store: Arc<dyn DownloadQueueStore>,
    /// Catalog store for checking existing content and updating availability.
    catalog_store: Arc<dyn CatalogStore>,
    /// Path to media files directory.
    #[allow(dead_code)]
    media_path: PathBuf,
    /// Configuration settings.
    config: DownloadManagerSettings,
    /// Retry policy for failed downloads.
    #[allow(dead_code)]
    retry_policy: RetryPolicy,
    /// Audit logger for tracking operations.
    audit_logger: AuditLogger,
    /// Sync event notifier for WebSocket updates (optional, late-init).
    sync_notifier: RwLock<Option<Arc<DownloadSyncNotifier>>>,
    /// Search vault for updating availability in the search index (optional, late-init).
    #[allow(dead_code)]
    search_vault: RwLock<Option<Arc<dyn SearchVault>>>,
}

impl DownloadManager {
    /// Create a new DownloadManager.
    pub fn new(
        queue_store: Arc<dyn DownloadQueueStore>,
        catalog_store: Arc<dyn CatalogStore>,
        media_path: PathBuf,
        config: DownloadManagerSettings,
    ) -> Self {
        let retry_policy = RetryPolicy::new(&config);
        let audit_logger = AuditLogger::new(queue_store.clone());

        Self {
            queue_store,
            catalog_store,
            media_path,
            config,
            retry_policy,
            audit_logger,
            sync_notifier: RwLock::new(None),
            search_vault: RwLock::new(None),
        }
    }

    /// Set the sync notifier for WebSocket updates.
    pub async fn set_sync_notifier(&self, notifier: Arc<DownloadSyncNotifier>) {
        *self.sync_notifier.write().await = Some(notifier);
    }

    /// Set the search vault for availability updates.
    pub async fn set_search_vault(&self, vault: Arc<dyn SearchVault>) {
        *self.search_vault.write().await = Some(vault);
    }

    // =========================================================================
    // User Request Methods
    // =========================================================================

    /// Request download of a single track.
    ///
    /// Creates a queue item for the track if:
    /// - Track exists in catalog
    /// - Track is not already available
    /// - Track is not already in queue
    pub async fn request_track(&self, user_id: &str, track_id: &str) -> Result<QueueItem> {
        // Check user limits
        let limits = self.queue_store.get_user_stats(user_id)?;
        if !limits.can_request {
            return Err(anyhow!("User has reached request limit"));
        }

        // Check if track exists in catalog
        let track = self
            .catalog_store
            .get_track(track_id)?
            .ok_or_else(|| anyhow!("Track not found in catalog: {}", track_id))?;

        // Check if already available
        if track.availability == TrackAvailability::Available {
            return Err(anyhow!("Track is already available"));
        }

        // Check if already in active queue
        if self
            .queue_store
            .is_in_active_queue(DownloadContentType::TrackAudio, track_id)?
        {
            return Err(anyhow!("Track is already in download queue"));
        }

        // Create queue item
        let item = QueueItem::new(
            uuid::Uuid::new_v4().to_string(),
            DownloadContentType::TrackAudio,
            track_id.to_string(),
            QueuePriority::User,
            RequestSource::User,
            self.config.max_retries as i32,
        )
        .with_names(Some(track.name.clone()), None)
        .with_user(user_id.to_string());

        // Enqueue
        self.queue_store.enqueue(item.clone())?;
        self.queue_store.increment_user_requests(user_id)?;

        // Log audit event (queue_position 0 for now - will be computed later)
        self.audit_logger.log_request_created(&item, 0)?;

        info!("Queued track download: {} ({})", track.name, track_id);

        Ok(item)
    }

    /// Request download of all tracks in an album.
    ///
    /// Creates a queue item for the album. The item will be picked up by an
    /// external script that downloads content and uploads it via the ingestion system.
    /// Returns the queued item wrapped in a Vec (for compatibility).
    pub async fn request_album(&self, user_id: &str, album_id: &str) -> Result<Vec<QueueItem>> {
        // Check user limits
        let limits = self.queue_store.get_user_stats(user_id)?;
        if !limits.can_request {
            return Err(anyhow!("User has reached request limit"));
        }

        // Get resolved album with all tracks
        let resolved_album = self
            .catalog_store
            .get_resolved_album(album_id)?
            .ok_or_else(|| anyhow!("Album not found in catalog: {}", album_id))?;

        let album = &resolved_album.album;
        let primary_artist = resolved_album
            .artists
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_default();

        // Count tracks that need downloading (for validation)
        let mut tracks_needing_download = 0;
        for disc in &resolved_album.discs {
            for track in &disc.tracks {
                if track.availability != TrackAvailability::Available {
                    tracks_needing_download += 1;
                }
            }
        }

        if tracks_needing_download == 0 {
            return Err(anyhow!(
                "No tracks to download - all tracks in '{}' are already available",
                album.name
            ));
        }

        // Check if album is already in queue
        if self
            .queue_store
            .is_in_active_queue(DownloadContentType::Album, album_id)?
        {
            return Err(anyhow!("Album is already in download queue"));
        }

        // Create a single album queue item
        let album_item = QueueItem::new(
            uuid::Uuid::new_v4().to_string(),
            DownloadContentType::Album,
            album_id.to_string(),
            QueuePriority::User,
            RequestSource::User,
            self.config.max_retries as i32,
        )
        .with_names(Some(album.name.clone()), Some(primary_artist.clone()))
        .with_user(user_id.to_string());

        // Enqueue the item (status: PENDING)
        self.queue_store.enqueue(album_item.clone())?;
        self.audit_logger.log_request_created(&album_item, 0)?;

        // Increment user request count (once per album)
        self.queue_store.increment_user_requests(user_id)?;

        info!(
            "Queued album '{}' for download ({} tracks)",
            album.name, tracks_needing_download
        );

        Ok(vec![album_item])
    }

    /// Get rate limit status for a user.
    pub fn get_user_limits(&self, user_id: &str) -> Result<UserLimitStatus> {
        self.queue_store.get_user_stats(user_id)
    }

    /// Get user's download requests.
    pub fn get_user_requests(
        &self,
        user_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>> {
        self.queue_store.get_user_requests(user_id, limit, offset)
    }

    // =========================================================================
    // Admin Methods
    // =========================================================================

    /// Get overall queue statistics.
    pub fn get_queue_stats(&self) -> Result<QueueStats> {
        self.queue_store.get_queue_stats()
    }

    /// Get failed queue items for review.
    pub fn get_failed_items(&self, limit: usize, offset: usize) -> Result<Vec<QueueItem>> {
        self.queue_store.get_failed_items(limit, offset)
    }

    /// Retry a failed queue item.
    pub fn retry_failed(&self, item_id: &str, admin_user_id: &str) -> Result<()> {
        self.queue_store.reset_to_pending(item_id)?;

        // Log audit event
        if let Ok(Some(item)) = self.queue_store.get_item(item_id) {
            self.audit_logger.log_admin_retry(&item, admin_user_id)?;
        }

        Ok(())
    }

    /// Delete a download request.
    ///
    /// Returns true if the item was deleted.
    pub fn delete_request(&self, item_id: &str, _admin_user_id: &str) -> Result<bool> {
        // Get the queue item first
        let item = self
            .queue_store
            .get_item(item_id)?
            .ok_or_else(|| anyhow!("Queue item not found: {}", item_id))?;

        // Delete the queue item
        let deleted = self.queue_store.delete_item(item_id)?;

        if deleted {
            // Decrement user queue count if there was a user
            if let Some(user_id) = &item.requested_by_user_id {
                let _ = self.queue_store.decrement_user_queue(user_id);
            }

            info!(
                "Deleted queue item: {} ({})",
                item_id,
                item.content_name.unwrap_or_default()
            );
        }

        Ok(deleted)
    }

    /// Get all requests (admin view) - legacy version with hardcoded filters.
    pub fn get_all_requests(
        &self,
        status: Option<QueueStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>> {
        self.queue_store
            .list_all(status, false, true, limit, offset)
    }

    /// Get all requests with explicit filter parameters.
    pub fn get_all_requests_filtered(
        &self,
        status: Option<QueueStatus>,
        exclude_completed: bool,
        top_level_only: bool,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>> {
        self.queue_store
            .list_all(status, exclude_completed, top_level_only, limit, offset)
    }

    /// Get audit log with filtering.
    pub fn get_audit_log(&self, filter: AuditLogFilter) -> Result<(Vec<AuditLogEntry>, usize)> {
        self.queue_store.get_audit_log(filter)
    }

    // =========================================================================
    // Status Methods
    // =========================================================================

    /// Get download manager status.
    pub fn get_status(&self) -> DownloadManagerStatus {
        DownloadManagerStatus {
            enabled: true,
            pending_count: self
                .queue_store
                .get_queue_stats()
                .map(|s| s.pending as usize)
                .unwrap_or(0),
        }
    }
}

/// Download manager status information.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadManagerStatus {
    /// Whether the download manager is enabled.
    pub enabled: bool,
    /// Number of pending items in queue.
    pub pending_count: usize,
}

#[cfg(test)]
mod tests {
    // TODO: Add tests in Phase 3
}
