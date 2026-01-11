//! Main download manager orchestration.
//!
//! Coordinates download requests via Quentin Torrentino ticket-based API.
//! This is the main facade for all download manager operations.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::catalog_store::{CatalogStore, TrackAvailability};
use crate::config::DownloadManagerSettings;
use crate::search::SearchVault;

use super::audit_logger::AuditLogger;
use super::models::*;
use super::queue_store::DownloadQueueStore;
use super::retry_policy::RetryPolicy;
use super::sync_notifier::DownloadSyncNotifier;
use super::torrent_client::TorrentClient;
use super::torrent_types::{MusicTicket, TicketStatus, TorrentEvent};

/// Main download manager that orchestrates all download operations.
///
/// Provides a unified interface for:
/// - User request management (queuing track/album downloads)
/// - Rate limiting (per-user checks)
/// - Ticket-based download via Quentin Torrentino
/// - Admin operations (viewing stats, retrying failed items)
/// - Audit logging (tracking all download operations)
pub struct DownloadManager {
    /// Queue store for persisting download requests.
    queue_store: Arc<dyn DownloadQueueStore>,
    /// Torrent client for communicating with Quentin Torrentino.
    torrent_client: Arc<TorrentClient>,
    /// Catalog store for checking existing content and updating availability.
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
    /// Sync event notifier for WebSocket updates (optional, late-init).
    sync_notifier: RwLock<Option<Arc<DownloadSyncNotifier>>>,
    /// Search vault for updating availability in the search index (optional, late-init).
    search_vault: RwLock<Option<Arc<dyn SearchVault>>>,
}

impl DownloadManager {
    /// Create a new DownloadManager.
    pub fn new(
        queue_store: Arc<dyn DownloadQueueStore>,
        torrent_client: Arc<TorrentClient>,
        catalog_store: Arc<dyn CatalogStore>,
        media_path: PathBuf,
        config: DownloadManagerSettings,
    ) -> Self {
        let retry_policy = RetryPolicy::new(&config);
        let audit_logger = AuditLogger::new(queue_store.clone());

        Self {
            queue_store,
            torrent_client,
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

    /// Check if Quentin Torrentino is connected (WebSocket alive).
    pub fn is_connected(&self) -> bool {
        self.torrent_client.is_connected()
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
    pub async fn request_track(
        &self,
        user_id: &str,
        track_id: &str,
    ) -> Result<QueueItem> {
        // Check user limits
        let limits = self.queue_store.get_user_stats(user_id)?;
        if !limits.can_request {
            return Err(anyhow!("User has reached request limit"));
        }

        // Check if track exists in catalog
        let track = self.catalog_store.get_track(track_id)?
            .ok_or_else(|| anyhow!("Track not found in catalog: {}", track_id))?;

        // Check if already available
        if track.availability == TrackAvailability::Available {
            return Err(anyhow!("Track is already available"));
        }

        // Check if already in active queue
        if self.queue_store.is_in_active_queue(DownloadContentType::TrackAudio, track_id)? {
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

        info!(
            "Queued track download: {} ({})",
            track.name, track_id
        );

        Ok(item)
    }

    /// Request download of all tracks in an album.
    ///
    /// Creates queue items for each track that is not already available.
    /// Returns the list of queued items.
    ///
    /// TODO: Implement when CatalogStore trait has get_album() and get_album_tracks()
    pub async fn request_album(
        &self,
        _user_id: &str,
        _album_id: &str,
    ) -> Result<Vec<QueueItem>> {
        // TODO: Implement album download request
        // Need to add get_album() and get_album_tracks() to CatalogStore trait
        Err(anyhow!("Album download not yet implemented"))
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
    // Ticket Submission
    // =========================================================================

    /// Submit pending queue items as tickets to Quentin Torrentino.
    ///
    /// Called when WebSocket connects to submit any items queued while offline.
    /// Returns the number of tickets submitted.
    pub async fn submit_pending_tickets(&self) -> Result<usize> {
        // TODO: Implement ticket submission in Phase 3
        // For now, just return 0
        warn!("submit_pending_tickets not yet implemented");
        Ok(0)
    }

    // =========================================================================
    // Event Handling
    // =========================================================================

    /// Handle a ticket event from Quentin Torrentino.
    ///
    /// Updates queue status and track availability based on event type.
    pub async fn handle_ticket_event(&self, event: TorrentEvent) -> Result<()> {
        match event {
            TorrentEvent::Completed { ticket_id, items_placed } => {
                info!("Ticket {} completed with {} items placed", ticket_id, items_placed);
                // TODO: Update track availability to Available
                // TODO: Update queue item status
                // TODO: Notify connected clients
            }
            TorrentEvent::Failed { ticket_id, error, retryable } => {
                warn!("Ticket {} failed: {} (retryable: {})", ticket_id, error, retryable);
                // TODO: Update track availability to FetchError
                // TODO: Update queue item status
            }
            TorrentEvent::StateChange { ticket_id, old_state, new_state, .. } => {
                debug!("Ticket {} state: {} -> {}", ticket_id, old_state, new_state);
                // TODO: Update ticket state in DB
            }
            TorrentEvent::Progress { ticket_id, progress_pct, .. } => {
                debug!("Ticket {} progress: {:.1}%", ticket_id, progress_pct);
                // TODO: Broadcast progress to connected clients
            }
            TorrentEvent::NeedsApproval { ticket_id, candidates } => {
                info!("Ticket {} needs approval ({} candidates)", ticket_id, candidates.len());
                // TODO: Store candidates for admin UI
            }
        }
        Ok(())
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

    /// Get all requests (admin view).
    pub fn get_all_requests(
        &self,
        status: Option<QueueStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>> {
        self.queue_store.list_all(status, false, true, limit, offset)
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
            connected: self.torrent_client.is_connected(),
            pending_count: self.queue_store.get_queue_stats()
                .map(|s| s.pending as usize)
                .unwrap_or(0),
        }
    }
}

/// Download manager status information.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadManagerStatus {
    /// Whether connected to Quentin Torrentino.
    pub connected: bool,
    /// Number of pending items in queue.
    pub pending_count: usize,
}

#[cfg(test)]
mod tests {
    // TODO: Add tests in Phase 3
}
