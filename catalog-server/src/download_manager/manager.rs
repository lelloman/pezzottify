//! Main download manager orchestration.
//!
//! Coordinates download requests via Quentin Torrentino ticket-based API.
//! This is the main facade for all download manager operations.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::catalog_store::{CatalogStore, TrackAvailability};
use crate::config::DownloadManagerSettings;
use crate::search::{HashedItemType, SearchVault};

use super::audit_logger::AuditLogger;
use super::models::*;
use super::queue_store::DownloadQueueStore;
use super::retry_policy::RetryPolicy;
use super::sync_notifier::DownloadSyncNotifier;
use super::torrent_client::TorrentClient;
use super::torrent_types::{
    AudioSearchConstraints, CreateTicketRequest, ExpectedContent, ExpectedTrack, OutputConstraints,
    QueryContext, SearchConstraints, TicketStatus, TorrentEvent,
};

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
    /// Creates a queue item for the album (with null QT ticket ID).
    /// The ticket will be submitted to Quentin Torrentino later by the queue processor.
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

        // Create a single album queue item (with null ticket_id - will be set when submitted to QT)
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

        // Enqueue the item (status: PENDING, ticket_id: NULL)
        self.queue_store.enqueue(album_item.clone())?;
        self.audit_logger.log_request_created(&album_item, 0)?;

        // Increment user request count (once per album)
        self.queue_store.increment_user_requests(user_id)?;

        info!(
            "Queued album '{}' for download ({} tracks)",
            album.name, tracks_needing_download
        );

        // Submit the pending item to QT if connected
        if self.torrent_client.is_connected() {
            match self.submit_pending_tickets().await {
                Ok(count) => {
                    if count > 0 {
                        debug!("Submitted {} pending tickets after enqueue", count);
                    }
                }
                Err(e) => {
                    warn!("Failed to submit pending tickets after enqueue: {}", e);
                    // Continue anyway - item is in queue and will be submitted on reconnect
                }
            }
        }

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
    // Ticket Submission
    // =========================================================================

    /// Submit pending queue items as tickets to Quentin Torrentino.
    ///
    /// Called when WebSocket connects to submit any items queued while offline.
    /// Each pending Album item creates one ticket.
    /// Returns the number of tickets submitted.
    pub async fn submit_pending_tickets(&self) -> Result<usize> {
        // Get all pending queue items
        let pending_items = self.queue_store.list_all(
            Some(QueueStatus::Pending),
            true,  // exclude completed
            false, // don't include archived
            1000,  // reasonable limit
            0,
        )?;

        if pending_items.is_empty() {
            debug!("No pending items to submit");
            return Ok(0);
        }

        info!(
            "Found {} pending queue items to submit",
            pending_items.len()
        );

        let mut tickets_submitted = 0;

        // Process each pending item
        for item in pending_items {
            // Only process Album items (atomic album downloads)
            if item.content_type != DownloadContentType::Album {
                debug!("Skipping non-album item: {:?}", item.content_type);
                continue;
            }

            let album_id = &item.content_id;
            match self.create_and_submit_ticket(album_id, &item).await {
                Ok(ticket_id) => {
                    tickets_submitted += 1;
                    info!(
                        "Submitted ticket {} for album {}",
                        ticket_id, album_id
                    );
                }
                Err(e) => {
                    error!("Failed to submit ticket for album {}: {}", album_id, e);
                }
            }
        }

        Ok(tickets_submitted)
    }

    /// Create and submit a ticket for an album queue item.
    ///
    /// Creates a QT ticket, stores the mapping, and marks the item as in_progress.
    async fn create_and_submit_ticket(
        &self,
        album_id: &str,
        item: &QueueItem,
    ) -> Result<String> {
        // Get resolved album data
        let resolved_album = self
            .catalog_store
            .get_resolved_album(album_id)?
            .ok_or_else(|| anyhow!("Album {} not found", album_id))?;

        let album = &resolved_album.album;
        let artists = &resolved_album.artists;
        let primary_artist = artists.first().map(|a| a.name.clone());

        // Build expected tracks from album discs
        let mut expected_tracks: Vec<ExpectedTrack> = Vec::new();
        for disc in &resolved_album.discs {
            for track in &disc.tracks {
                expected_tracks.push(ExpectedTrack {
                    number: track.track_number as u32,
                    title: track.name.clone(),
                    duration_secs: Some((track.duration_ms / 1000) as u32),
                    disc_number: if disc.number > 1 {
                        Some(disc.number as u32)
                    } else {
                        None
                    },
                });
            }
        }

        // Extract release year from release_date (format: "YYYY", "YYYY-MM", or "YYYY-MM-DD")
        let release_year = album
            .release_date
            .as_ref()
            .and_then(|d| d.split('-').next())
            .filter(|y| y.len() == 4);

        // Build description for search (include year if available)
        let description = match (&primary_artist, release_year) {
            (Some(artist), Some(year)) => format!("{} {} ({})", artist, album.name, year),
            (Some(artist), None) => format!("{} {}", artist, album.name),
            (None, Some(year)) => format!("{} ({})", album.name, year),
            (None, None) => album.name.clone(),
        };

        // Create the ticket request (matches QT's CreateTicketBody format)
        let request = CreateTicketRequest {
            priority: Some(0),
            query_context: QueryContext {
                tags: vec!["music".to_string(), "album".to_string()],
                description,
                expected: Some(ExpectedContent::Album {
                    artist: primary_artist,
                    title: album.name.clone(),
                    tracks: expected_tracks,
                }),
                search_constraints: Some(SearchConstraints {
                    audio: Some(AudioSearchConstraints::default()),
                }),
            },
            dest_path: self.media_path.to_string_lossy().to_string(),
            output_constraints: Some(OutputConstraints::default()),
        };

        // Submit to QT - QT assigns the ticket ID
        let response = self.torrent_client.create_ticket(request).await?;
        let ticket_id = response.id;

        // Create ticket mapping (queue_item_id -> ticket_id -> album_id)
        self.queue_store
            .create_ticket_mapping(&item.id, &ticket_id, album_id)?;

        // Mark queue item as IN_PROGRESS (now has ticket_id)
        if let Err(e) = self.queue_store.claim_for_processing(&item.id) {
            warn!("Failed to claim item {} for processing: {}", item.id, e);
        }

        Ok(ticket_id)
    }

    /// Compute the destination path for a track file.
    fn compute_track_dest_path(&self, track_id: &str) -> String {
        // Use a simple directory structure based on track_id prefix
        // Format: audio/{first2}/{next2}/trackid.ogg
        let id_bytes = track_id.as_bytes();
        let dir1 = if id_bytes.len() >= 2 {
            &track_id[0..2]
        } else {
            "00"
        };
        let dir2 = if id_bytes.len() >= 4 {
            &track_id[2..4]
        } else {
            "00"
        };
        format!(
            "{}/audio/{}/{}/{}.ogg",
            self.media_path.display(),
            dir1,
            dir2,
            track_id
        )
    }

    // =========================================================================
    // Event Handling
    // =========================================================================

    /// Handle a ticket event from Quentin Torrentino.
    ///
    /// Updates queue status and track availability based on event type.
    pub async fn handle_ticket_event(&self, event: TorrentEvent) -> Result<()> {
        match event {
            TorrentEvent::Completed {
                ticket_id,
                items_placed,
            } => {
                info!(
                    "Ticket {} completed with {} items placed",
                    ticket_id, items_placed
                );
                self.handle_ticket_completed(&ticket_id).await?;
            }
            TorrentEvent::Failed {
                ticket_id,
                error,
                retryable,
            } => {
                warn!(
                    "Ticket {} failed: {} (retryable: {})",
                    ticket_id, error, retryable
                );
                self.handle_ticket_failed(&ticket_id, &error, retryable)
                    .await?;
            }
            TorrentEvent::TicketUpdated { ticket_id, state } => {
                debug!("Ticket {} updated to state: {}", ticket_id, state);
                // Update ticket state in DB
                if let Err(e) = self.queue_store.update_ticket_state(&ticket_id, &state) {
                    warn!("Failed to update ticket state: {}", e);
                }
            }
            TorrentEvent::TicketDeleted { ticket_id } => {
                debug!("Ticket {} deleted", ticket_id);
                // Could handle ticket deletion if needed
            }
            TorrentEvent::Progress {
                ticket_id,
                progress_pct,
                ..
            } => {
                debug!("Ticket {} progress: {:.1}%", ticket_id, progress_pct);
                // Progress updates are handled via QT's WebSocket - no sync broadcast needed
                // The sync_notifier is for parent-child progress within our queue model
            }
            TorrentEvent::NeedsApproval {
                ticket_id,
                candidates,
            } => {
                info!(
                    "Ticket {} needs approval ({} candidates)",
                    ticket_id,
                    candidates.len()
                );
                // Update state to NEEDS_APPROVAL
                if let Err(e) = self
                    .queue_store
                    .update_ticket_state(&ticket_id, TicketStatus::NeedsApproval.as_str())
                {
                    warn!("Failed to update ticket state: {}", e);
                }
                // Candidates are handled via admin API - QT stores them
            }
        }
        Ok(())
    }

    /// Handle successful ticket completion.
    async fn handle_ticket_completed(&self, ticket_id: &str) -> Result<()> {
        // Get the album_id from ticket mapping
        let mapping = self.queue_store.get_active_tickets()?;
        let ticket_mapping = mapping
            .iter()
            .find(|m| m.ticket_id == ticket_id)
            .ok_or_else(|| anyhow!("Ticket mapping not found for {}", ticket_id))?;

        let album_id = ticket_mapping.album_id.clone();

        // Collect track availability updates for batch processing
        let mut availability_updates: Vec<(String, HashedItemType, bool)> = Vec::new();

        // Get all tracks for this album and update their availability
        match self.catalog_store.get_resolved_album(&album_id) {
            Ok(Some(resolved_album)) => {
                for disc in &resolved_album.discs {
                    for track in &disc.tracks {
                        // Check if audio file now exists
                        let dest_path = self.compute_track_dest_path(&track.id);
                        if std::path::Path::new(&dest_path).exists() {
                            // Update track availability to Available
                            let mut updated_track = track.clone();
                            updated_track.availability = TrackAvailability::Available;
                            updated_track.audio_uri = Some(dest_path.clone());

                            if let Err(e) = self.catalog_store.update_track(&updated_track, None) {
                                warn!("Failed to update track {} availability: {}", track.id, e);
                            }

                            // Collect for batch update to search index
                            availability_updates.push((
                                track.id.clone(),
                                HashedItemType::Track,
                                true, // available
                            ));
                        }
                    }
                }
            }
            Ok(None) => {
                warn!(
                    "Album {} not found in catalog during ticket completion - tracks won't be updated",
                    album_id
                );
            }
            Err(e) => {
                error!(
                    "Failed to get album {} from catalog during ticket completion: {}",
                    album_id, e
                );
            }
        }

        // Update search index if available (batch update)
        if !availability_updates.is_empty() {
            if let Some(vault) = self.search_vault.read().await.as_ref() {
                vault.update_availability(&availability_updates);
            }
        }

        // Find and complete all queue items for this album
        let items =
            self.queue_store
                .list_all(Some(QueueStatus::InProgress), true, false, 1000, 0)?;

        for item in items {
            if item.content_type == DownloadContentType::TrackAudio {
                if let Ok(Some(track)) = self.catalog_store.get_track(&item.content_id) {
                    if track.album_id == album_id {
                        // Mark completed with 0 bytes/duration since we don't track per-track stats
                        if let Err(e) = self.queue_store.mark_completed(&item.id, 0, 0) {
                            warn!("Failed to complete queue item {}: {}", item.id, e);
                        }
                        // Decrement user queue count
                        if let Some(user_id) = &item.requested_by_user_id {
                            let _ = self.queue_store.decrement_user_queue(user_id);
                        }
                    }
                }
            }
        }

        // Update ticket state
        self.queue_store
            .update_ticket_state(ticket_id, TicketStatus::Completed.as_str())?;

        // Log audit event
        if let Some(queue_item_id) = self.queue_store.get_queue_item_for_ticket(ticket_id)? {
            if let Ok(Some(item)) = self.queue_store.get_item(&queue_item_id) {
                // Log with 0 bytes/duration since we don't track per-ticket stats
                let _ = self.audit_logger.log_download_completed(&item, 0, 0, None);
            }
        }

        Ok(())
    }

    /// Handle ticket failure.
    async fn handle_ticket_failed(
        &self,
        ticket_id: &str,
        error_msg: &str,
        retryable: bool,
    ) -> Result<()> {
        // Get the album_id from ticket mapping
        let mapping = self.queue_store.get_active_tickets()?;
        let ticket_mapping = mapping
            .iter()
            .find(|m| m.ticket_id == ticket_id)
            .ok_or_else(|| anyhow!("Ticket mapping not found for {}", ticket_id))?;

        let album_id = ticket_mapping.album_id.clone();

        // Create error struct - use Unknown type, retryability is determined by the retryable param
        let download_error = DownloadError::new(
            if retryable {
                DownloadErrorType::Unknown // Generic retryable error
            } else {
                DownloadErrorType::NotFound // Permanent (non-retryable) error
            },
            error_msg,
        );

        // Find all queue items for this album
        let items =
            self.queue_store
                .list_all(Some(QueueStatus::InProgress), true, false, 1000, 0)?;

        for item in items {
            if item.content_type == DownloadContentType::TrackAudio {
                if let Ok(Some(track)) = self.catalog_store.get_track(&item.content_id) {
                    if track.album_id == album_id {
                        if retryable && item.retry_count < item.max_retries {
                            // Schedule retry - use exponential backoff
                            let retry_delay_secs = 60 * (1 << item.retry_count.min(6)); // max ~1 hour
                            let next_retry_at =
                                chrono::Utc::now().timestamp() + retry_delay_secs as i64;
                            if let Err(e) = self.queue_store.mark_retry_waiting(
                                &item.id,
                                next_retry_at,
                                &download_error,
                            ) {
                                warn!("Failed to mark retry for {}: {}", item.id, e);
                            }
                        } else {
                            // Mark as permanently failed
                            if let Err(e) = self.queue_store.mark_failed(&item.id, &download_error)
                            {
                                warn!("Failed to mark item {} as failed: {}", item.id, e);
                            }

                            // Update track availability to FetchError
                            let mut updated_track = track.clone();
                            updated_track.availability = TrackAvailability::FetchError;
                            let _ = self.catalog_store.update_track(&updated_track, None);
                        }
                    }
                }
            }
        }

        // Update ticket state
        self.queue_store
            .update_ticket_state(ticket_id, TicketStatus::Failed.as_str())?;

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
            connected: self.torrent_client.is_connected(),
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
    /// Whether connected to Quentin Torrentino.
    pub connected: bool,
    /// Number of pending items in queue.
    pub pending_count: usize,
}

#[cfg(test)]
mod tests {
    // TODO: Add tests in Phase 3
}
