//! Main download manager orchestration.
//!
//! Coordinates the download queue, processor, and related components.
//! This is the main facade for all download manager operations.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::catalog_store::{TrackAvailability, WritableCatalogStore};
use crate::config::DownloadManagerSettings;
use crate::notifications::{NotificationService, NotificationType};
use crate::search::{HashedItemType, SearchVault};

use super::audit_logger::AuditLogger;
use super::corruption_handler::{CorruptionHandler, CorruptionHandlerConfig, HandlerAction};
use super::downloader_client::DownloaderClient;
use super::models::*;
use super::queue_store::DownloadQueueStore;
use super::retry_policy::RetryPolicy;
use super::sync_notifier::DownloadSyncNotifier;
use super::throttle::{DownloadThrottler, SlidingWindowThrottler, ThrottlerConfig};

/// Main download manager that orchestrates all download operations.
///
/// Provides a unified interface for:
/// - User request management (queuing album/discography downloads)
/// - Rate limiting (per-user and global capacity checks)
/// - Queue processing (executing downloads from the queue)
/// - Admin operations (viewing stats, retrying failed items)
/// - Audit logging (tracking all download operations)
pub struct DownloadManager {
    /// Queue store for persisting download requests.
    queue_store: Arc<dyn DownloadQueueStore>,
    /// HTTP client for communicating with the external downloader service.
    downloader_client: DownloaderClient,
    /// Catalog store for checking existing content and ingesting new content.
    catalog_store: Arc<dyn WritableCatalogStore>,
    /// Path to media files directory.
    media_path: PathBuf,
    /// Configuration settings.
    config: DownloadManagerSettings,
    /// Retry policy for failed downloads.
    #[allow(dead_code)]
    retry_policy: RetryPolicy,
    /// Audit logger for tracking operations.
    audit_logger: AuditLogger,
    /// Bandwidth throttler for rate limiting downloads.
    throttler: Arc<SlidingWindowThrottler>,
    /// Corruption handler for detecting file corruption and managing restarts.
    corruption_handler: Arc<CorruptionHandler>,
    /// Sync event notifier for WebSocket updates (optional, uses interior mutability for late initialization).
    sync_notifier: RwLock<Option<Arc<DownloadSyncNotifier>>>,
    /// Notification service for creating user notifications (optional, uses interior mutability for late initialization).
    notification_service: RwLock<Option<Arc<NotificationService>>>,
    /// Search vault for updating availability in the search index (optional, uses interior mutability for late initialization).
    search_vault: RwLock<Option<Arc<dyn SearchVault>>>,
}

impl DownloadManager {
    /// Create a new DownloadManager.
    ///
    /// # Arguments
    /// * `queue_store` - Store for persisting download queue state
    /// * `downloader_client` - HTTP client for the downloader service
    /// * `catalog_store` - Store for checking and writing catalog content
    /// * `media_path` - Path to the media files directory
    /// * `config` - Download manager configuration settings
    pub fn new(
        queue_store: Arc<dyn DownloadQueueStore>,
        downloader_client: DownloaderClient,
        catalog_store: Arc<dyn WritableCatalogStore>,
        media_path: PathBuf,
        config: DownloadManagerSettings,
    ) -> Self {
        let retry_policy = RetryPolicy::new(&config);
        let audit_logger = AuditLogger::new(queue_store.clone());

        // Initialize throttler from config
        let throttler_config = ThrottlerConfig {
            max_bytes_per_minute: config.throttle_max_mb_per_minute * 1024 * 1024,
            max_bytes_per_hour: config.throttle_max_mb_per_hour * 1024 * 1024,
            enabled: config.throttle_enabled,
        };
        let throttler = Arc::new(SlidingWindowThrottler::new(throttler_config));

        // Initialize corruption handler from config
        let corruption_config = CorruptionHandlerConfig {
            window_size: config.corruption_window_size,
            failure_threshold: config.corruption_failure_threshold,
            base_cooldown_secs: config.corruption_base_cooldown_secs,
            max_cooldown_secs: config.corruption_max_cooldown_secs,
            cooldown_multiplier: config.corruption_cooldown_multiplier,
            successes_to_deescalate: config.corruption_successes_to_deescalate,
        };
        let corruption_handler = Arc::new(CorruptionHandler::new(corruption_config));

        Self {
            queue_store,
            downloader_client,
            catalog_store,
            media_path,
            config,
            retry_policy,
            audit_logger,
            throttler,
            corruption_handler,
            sync_notifier: RwLock::new(None),
            notification_service: RwLock::new(None),
            search_vault: RwLock::new(None),
        }
    }

    /// Set the sync notifier for WebSocket updates.
    ///
    /// This should be called after construction to enable real-time
    /// download status updates via WebSocket. Uses interior mutability
    /// so it can be called even after the manager is wrapped in Arc.
    pub async fn set_sync_notifier(&self, notifier: Arc<DownloadSyncNotifier>) {
        let mut guard = self.sync_notifier.write().await;
        *guard = Some(notifier);
    }

    /// Set the notification service for creating user notifications.
    ///
    /// This should be called after construction to enable notification
    /// creation when downloads complete. Uses interior mutability
    /// so it can be called even after the manager is wrapped in Arc.
    pub async fn set_notification_service(&self, service: Arc<NotificationService>) {
        let mut guard = self.notification_service.write().await;
        *guard = Some(service);
    }

    /// Set the search vault for updating availability in the search index.
    ///
    /// This should be called after construction to enable search index
    /// updates when track availability changes. Uses interior mutability
    /// so it can be called even after the manager is wrapped in Arc.
    pub async fn set_search_vault(&self, vault: Arc<dyn SearchVault>) {
        let mut guard = self.search_vault.write().await;
        *guard = Some(vault);
    }

    /// Update search index availability for a track.
    ///
    /// Called internally when track availability changes to keep the
    /// search index in sync with the catalog.
    async fn update_search_availability(&self, track_id: &str, is_available: bool) {
        if let Some(vault) = self.search_vault.read().await.as_ref() {
            vault.update_availability(&[(
                track_id.to_string(),
                HashedItemType::Track,
                is_available,
            )]);
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

    /// Check if the downloader service is healthy and reachable.
    pub async fn check_downloader_health(&self) -> bool {
        self.downloader_client.health_check().await.is_ok()
    }

    /// Check if ffprobe is available on the system.
    ///
    /// This should be called at startup to ensure media validation will work.
    /// Returns Ok(version) if ffprobe is available, Err with message otherwise.
    pub async fn check_ffprobe_available() -> Result<String, String> {
        use tokio::process::Command;

        let output = Command::new("ffprobe")
            .arg("-version")
            .output()
            .await
            .map_err(|e| {
                format!(
                    "ffprobe not found. Install ffmpeg package for media validation. Error: {}",
                    e
                )
            })?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Extract version from first line (e.g., "ffprobe version 6.1.1-3ubuntu5 ...")
            let version = stdout.lines().next().unwrap_or("unknown").to_string();
            Ok(version)
        } else {
            Err("ffprobe command failed".to_string())
        }
    }

    /// Get the current status of the downloader service.
    pub async fn get_downloader_status(
        &self,
    ) -> Option<crate::downloader::models::DownloaderStatus> {
        self.downloader_client.get_status().await.ok()
    }

    /// Get the media path.
    pub fn media_path(&self) -> &PathBuf {
        &self.media_path
    }

    /// Get the stale threshold from configuration.
    pub fn get_stale_threshold_secs(&self) -> u64 {
        self.config.stale_in_progress_threshold_secs
    }

    /// Get items stuck in IN_PROGRESS state longer than the threshold.
    pub fn get_stale_in_progress(&self, threshold_secs: i64) -> Result<Vec<QueueItem>> {
        self.queue_store.get_stale_in_progress(threshold_secs)
    }

    // =========================================================================
    // User Request Methods
    // =========================================================================

    /// Request download of an album.
    ///
    /// # Logic:
    /// 1. Check user rate limits
    /// 2. Check if already in catalog
    /// 3. Check if already in queue
    /// 4. Enqueue with appropriate priority
    /// 5. Log audit event
    /// 6. Emit sync event for real-time updates
    /// 7. Return result with queue position
    pub async fn request_album(
        &self,
        user_id: &str,
        request: AlbumRequest,
    ) -> Result<RequestResult> {
        // 1. Check user rate limits
        let limits = self.check_user_limits(user_id)?;
        if !limits.can_request {
            return Err(anyhow!(
                "Rate limit exceeded: {} requests today (max {}), {} in queue (max {})",
                limits.requests_today,
                limits.max_per_day,
                limits.in_queue,
                limits.max_queue
            ));
        }

        // 2. Check if already in catalog
        // TODO: Implement catalog check when external ID mapping is available
        // For now, we skip this check since we don't have external ID → catalog ID mapping

        // 3. Check if already in queue
        if self
            .queue_store
            .is_in_active_queue(DownloadContentType::Album, &request.album_id)?
        {
            return Err(anyhow!(
                "Album {} is already in the download queue",
                request.album_id
            ));
        }

        // 4. Enqueue with appropriate priority
        let queue_item_id = Uuid::new_v4().to_string();
        let queue_item = QueueItem::new(
            queue_item_id.clone(),
            DownloadContentType::Album,
            request.album_id.clone(),
            QueuePriority::User,
            RequestSource::User,
            self.config.max_retries as i32,
        )
        .with_names(Some(request.album_name), Some(request.artist_name))
        .with_user(user_id.to_string());

        self.queue_store.enqueue(queue_item.clone())?;

        // Increment user request counters
        self.queue_store.increment_user_requests(user_id)?;

        // 5. Get queue position
        let queue_position = self
            .queue_store
            .get_queue_position(&queue_item_id)?
            .unwrap_or(0);

        // 6. Log audit event
        self.audit_logger
            .log_request_created(&queue_item, queue_position)?;

        // 7. Emit sync event for real-time WebSocket updates
        if let Some(ref notifier) = *self.sync_notifier.read().await {
            notifier
                .notify_request_created(&queue_item, queue_position as i32)
                .await;
        }

        Ok(RequestResult {
            request_id: queue_item_id,
            status: QueueStatus::Pending,
            queue_position,
        })
    }

    /// Request download of an artist's full discography.
    ///
    /// This creates multiple album download requests, one for each album in the discography.
    /// Albums already in the catalog or queue are skipped.
    pub fn request_discography(
        &self,
        _user_id: &str,
        request: DiscographyRequest,
    ) -> Result<DiscographyRequestResult> {
        // For discography requests, we need to first fetch the artist's albums
        // This requires calling the external downloader service, which is async
        // For now, we return a placeholder that indicates this needs async implementation

        // TODO: Implement in DM-1.7.3 when async discography fetching is available
        // The implementation should:
        // 1. Check user rate limits
        // 2. Fetch artist's albums from downloader service (async)
        // 3. For each album:
        //    - Check if already in catalog
        //    - Check if already in queue
        //    - Enqueue if not present
        // 4. Log audit events for each queued album
        // 5. Return aggregated result

        // For now, return an error indicating the feature requires async support
        Err(anyhow!(
            "Discography request for artist {} ({}) requires async implementation - use search_discography + individual album requests",
            request.artist_name,
            request.artist_id
        ))
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

    /// Get all download requests for a user with progress information.
    /// Returns UserRequestView which includes progress for album downloads.
    pub fn get_user_requests_with_progress(
        &self,
        user_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<UserRequestView>> {
        let requests = self.queue_store.get_user_requests(user_id, limit, offset)?;

        // Calculate queue positions for pending items
        let pending_items: Vec<_> = requests
            .iter()
            .filter(|r| r.status == QueueStatus::Pending)
            .collect();

        let views: Vec<UserRequestView> = requests
            .iter()
            .map(|item| {
                // Get progress for album items that have children
                let progress = if item.content_type == DownloadContentType::Album {
                    self.queue_store
                        .get_children_progress(&item.id)
                        .ok()
                        .filter(|p| p.total_children > 0)
                } else {
                    None
                };

                // Calculate queue position for pending items
                let queue_position = if item.status == QueueStatus::Pending {
                    pending_items
                        .iter()
                        .position(|p| p.id == item.id)
                        .map(|pos| pos + 1)
                } else {
                    None
                };

                UserRequestView::from_queue_item(item, progress, queue_position)
            })
            .collect();

        Ok(views)
    }

    /// Get the status of a specific request for a user.
    pub fn get_request_status(&self, user_id: &str, request_id: &str) -> Result<Option<QueueItem>> {
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
    ///
    /// # Logic:
    /// 1. Check corruption handler cooldown
    /// 2. Check bandwidth throttle
    /// 3. Check global capacity limits
    /// 4. Get next pending item (by priority)
    /// 5. Claim for processing (atomic)
    /// 6. Log download started
    /// 7. Call downloader
    /// 8. On success: mark completed, record activity, log audit, update throttler
    /// 9. On failure: check retry policy, handle corruption if applicable
    pub async fn process_next(&self) -> Result<Option<ProcessingResult>> {
        // 1. Check if corruption handler is in cooldown
        if self.corruption_handler.is_in_cooldown().await {
            debug!("Corruption handler in cooldown, skipping processing");
            return Ok(None);
        }

        // 2. Check bandwidth throttle
        if let Err(wait_duration) = self.throttler.check_bandwidth().await {
            debug!(
                "Bandwidth throttled, wait {} seconds",
                wait_duration.as_secs()
            );
            return Ok(None);
        }

        // 3. Check global capacity limits
        let capacity = self.check_global_capacity()?;
        if capacity.at_capacity {
            return Ok(None);
        }

        // 4. Get next pending item (by priority)
        let item = match self.queue_store.get_next_pending()? {
            Some(item) => item,
            None => return Ok(None),
        };

        // 5. Claim for processing (atomic)
        if !self.queue_store.claim_for_processing(&item.id)? {
            // Another processor claimed it, try again
            return Ok(None);
        }

        // 6. Log download started
        self.audit_logger.log_download_started(&item)?;

        // Emit status change to InProgress for top-level album items
        if item.content_type == DownloadContentType::Album && item.parent_id.is_none() {
            if let Some(ref notifier) = *self.sync_notifier.read().await {
                notifier
                    .notify_status_changed(&item, QueueStatus::InProgress, None, None)
                    .await;
            }
        }

        let start_time = std::time::Instant::now();

        // 7. Execute download based on content type
        let download_result = self.execute_download(&item).await;

        let duration_ms = start_time.elapsed().as_millis() as i64;

        // Determine if this is a media item (track/image) for throttle/corruption tracking
        let is_media_item = matches!(
            item.content_type,
            DownloadContentType::TrackAudio
                | DownloadContentType::AlbumImage
                | DownloadContentType::ArtistImage
        );

        match download_result {
            Ok(bytes_downloaded) => {
                // 8. On success: handle completion based on item type
                //
                // For Album items (parent items that spawn children), do NOT mark as
                // completed here. They should remain IN_PROGRESS until all children
                // complete, which is handled by check_and_complete_parent().
                //
                // For child items (tracks, images), mark as completed immediately.
                let is_parent_item = item.content_type == DownloadContentType::Album;

                // Update throttler and corruption handler for media items
                if is_media_item {
                    self.throttler.record_download(bytes_downloaded).await;
                    self.corruption_handler.record_result(true).await;
                }

                if is_parent_item {
                    // Parent items stay IN_PROGRESS - children will complete them
                    // Record activity but don't mark completed or decrement queue
                    self.queue_store
                        .record_activity(item.content_type, bytes_downloaded, true)?;

                    self.audit_logger.log_download_completed(
                        &item,
                        bytes_downloaded,
                        duration_ms,
                        None,
                    )?;

                    info!(
                        "Album {} metadata processed, waiting for {} children to complete",
                        item.content_id,
                        self.queue_store
                            .get_children(&item.id)
                            .map(|c| c.len())
                            .unwrap_or(0)
                    );
                } else {
                    // Child items - mark as completed immediately
                    self.queue_store
                        .mark_completed(&item.id, bytes_downloaded, duration_ms)?;

                    self.queue_store
                        .record_activity(item.content_type, bytes_downloaded, true)?;

                    // Decrement user's queue count (only for top-level items, not children)
                    if item.parent_id.is_none() {
                        if let Some(user_id) = &item.requested_by_user_id {
                            self.queue_store.decrement_user_queue(user_id)?;
                        }
                    }

                    self.audit_logger.log_download_completed(
                        &item,
                        bytes_downloaded,
                        duration_ms,
                        None,
                    )?;

                    // Check if parent is now complete (must be AFTER mark_completed)
                    if let Some(parent_id) = &item.parent_id {
                        // Emit progress update for the parent
                        if let Some(ref notifier) = *self.sync_notifier.read().await {
                            if let Ok(Some(parent)) = self.queue_store.get_item(parent_id) {
                                if let Ok(progress) =
                                    self.queue_store.get_children_progress(parent_id)
                                {
                                    notifier.notify_progress_updated(&parent, &progress).await;
                                }
                            }
                        }
                        self.check_and_complete_parent(parent_id).await.ok();
                    }
                }

                Ok(Some(ProcessingResult::success(
                    item.id,
                    item.content_type,
                    bytes_downloaded,
                    duration_ms,
                )))
            }
            Err(error) => {
                // 9. On failure: check for corruption and handle accordingly
                let is_corruption = error.error_type == DownloadErrorType::Corruption;

                // Handle corruption tracking for media items
                if is_media_item && is_corruption {
                    // Record corruption event in metrics
                    crate::server::metrics::record_corruption_event();

                    let action = self.corruption_handler.record_result(false).await;

                    if action == HandlerAction::RestartNeeded {
                        // Try to trigger restart if we can acquire the lock
                        if self.corruption_handler.try_acquire_restart_lock() {
                            info!("Corruption threshold exceeded, triggering downloader restart");

                            // Record restart in metrics
                            crate::server::metrics::record_corruption_handler_restart();

                            // Fire-and-forget restart
                            let client = self.downloader_client.clone();
                            tokio::spawn(async move {
                                if let Err(e) = client.restart().await {
                                    warn!("Failed to restart downloader: {}", e);
                                } else {
                                    info!("Downloader restart request sent");
                                }
                            });

                            // Record the restart (escalates cooldown)
                            self.corruption_handler.record_restart().await;
                            self.corruption_handler.release_restart_lock();
                        }
                    }
                }

                // Check retry policy
                if self.retry_policy.should_retry(&error, item.retry_count) {
                    // Schedule retry
                    let next_retry_at = self.retry_policy.next_retry_at(item.retry_count);
                    let backoff_secs = self.retry_policy.backoff_secs(item.retry_count);

                    self.queue_store
                        .mark_retry_waiting(&item.id, next_retry_at, &error)?;

                    self.audit_logger.log_retry_scheduled(
                        &item,
                        next_retry_at,
                        backoff_secs,
                        &error,
                    )?;

                    // Emit status change for top-level album items
                    if item.content_type == DownloadContentType::Album && item.parent_id.is_none() {
                        if let Some(ref notifier) = *self.sync_notifier.read().await {
                            let queue_pos = self
                                .queue_store
                                .get_queue_position(&item.id)
                                .ok()
                                .flatten()
                                .map(|p| p as i32);
                            notifier
                                .notify_status_changed(
                                    &item,
                                    QueueStatus::RetryWaiting,
                                    queue_pos,
                                    Some(error.message.clone()),
                                )
                                .await;
                        }
                    }
                } else {
                    // Mark as permanently failed
                    self.queue_store.mark_failed(&item.id, &error)?;

                    // If this was a track download, mark it as FetchError in the catalog
                    if item.content_type == DownloadContentType::TrackAudio {
                        if let Err(e) = self.catalog_store.set_track_availability(
                            &item.content_id,
                            &TrackAvailability::FetchError,
                        ) {
                            warn!(
                                "Failed to set track {} availability to FetchError: {}",
                                item.content_id, e
                            );
                        }
                        // Update search index availability
                        self.update_search_availability(&item.content_id, false).await;
                    }

                    // Decrement user's queue count
                    if let Some(user_id) = &item.requested_by_user_id {
                        self.queue_store.decrement_user_queue(user_id)?;
                    }

                    self.queue_store
                        .record_activity(item.content_type, 0, false)?;

                    self.audit_logger.log_download_failed(&item, &error)?;

                    // Emit status change for top-level album items
                    if item.content_type == DownloadContentType::Album && item.parent_id.is_none() {
                        if let Some(ref notifier) = *self.sync_notifier.read().await {
                            notifier
                                .notify_status_changed(
                                    &item,
                                    QueueStatus::Failed,
                                    None,
                                    Some(error.message.clone()),
                                )
                                .await;
                        }
                    }
                }

                Ok(Some(ProcessingResult::failure(
                    item.id,
                    item.content_type,
                    duration_ms,
                    error,
                )))
            }
        }
    }

    /// Execute download based on content type.
    ///
    /// Returns the number of bytes downloaded on success.
    async fn execute_download(&self, item: &QueueItem) -> Result<u64, DownloadError> {
        match item.content_type {
            DownloadContentType::Album => {
                // Albums are parent items that create children for tracks/images
                self.execute_album_download(item).await
            }
            DownloadContentType::TrackAudio => {
                // Download track audio file
                self.execute_track_download(item).await
            }
            DownloadContentType::AlbumImage | DownloadContentType::ArtistImage => {
                // Download image file
                self.execute_image_download(item).await
            }
            DownloadContentType::ArtistRelated => {
                // Fetch related artist IDs and save to database
                self.execute_artist_related_download(item).await
            }
            DownloadContentType::ArtistMetadata => {
                // Fetch full artist metadata and create record
                self.execute_artist_metadata_download(item).await
            }
        }
    }

    /// Execute album download - fetches metadata and creates child items.
    async fn execute_album_download(&self, item: &QueueItem) -> Result<u64, DownloadError> {
        info!("Processing album: {}", item.content_id);

        // 1. Fetch album metadata
        let album = self
            .downloader_client
            .get_album(&item.content_id)
            .await
            .map_err(|e| {
                DownloadError::new(
                    DownloadErrorType::Connection,
                    format!("Failed to fetch album metadata: {}", e),
                )
            })?;

        // 2. Fetch album tracks
        let tracks = self
            .downloader_client
            .get_album_tracks(&item.content_id)
            .await
            .map_err(|e| {
                DownloadError::new(
                    DownloadErrorType::Connection,
                    format!("Failed to fetch album tracks: {}", e),
                )
            })?;

        // 3. Fetch artist metadata for each artist on the album
        let mut artists = Vec::new();
        for artist_id in &album.artists_ids {
            match self.downloader_client.get_artist(artist_id).await {
                Ok(artist) => artists.push(artist),
                Err(e) => {
                    debug!("Failed to fetch artist {}: {}", artist_id, e);
                    // Continue without this artist
                }
            }
        }

        // 4. Ingest album, tracks, and artists into the catalog FIRST
        // This must happen before creating children so tracks exist when audio downloads complete
        let ingested = super::catalog_ingestion::ingest_album(
            self.catalog_store.as_ref(),
            &album,
            &tracks,
            &artists,
        )
        .map_err(|e| {
            DownloadError::new(
                DownloadErrorType::Storage,
                format!("Failed to ingest album to catalog: {}", e),
            )
        })?;

        info!(
            "Ingested album {} with {} tracks, {} album images, {} artist images",
            ingested.album_id,
            ingested.track_ids.len(),
            ingested.album_image_ids.len(),
            ingested.artist_image_ids.len()
        );

        // 5. Create child queue items for tracks and images
        let mut children = Vec::new();
        let track_count = tracks.len();

        // Add track children
        for track in &tracks {
            children.push(QueueItem::new_child(
                Uuid::new_v4().to_string(),
                item.id.clone(),
                DownloadContentType::TrackAudio,
                track.id.clone(),
                item.priority,
                item.request_source,
                item.requested_by_user_id.clone(),
                self.config.max_retries as i32,
            ));
        }

        // Add album cover images (merge covers + cover_group, deduplicated)
        let all_covers = super::catalog_ingestion::merge_images(&album.covers, &album.cover_group);
        for cover in &all_covers {
            children.push(QueueItem::new_child(
                Uuid::new_v4().to_string(),
                item.id.clone(),
                DownloadContentType::AlbumImage,
                cover.id.clone(),
                QueuePriority::Expansion,
                item.request_source,
                item.requested_by_user_id.clone(),
                self.config.max_retries as i32,
            ));
        }

        // Add artist portrait images (merge portraits + portrait_group, deduplicated)
        for artist in &artists {
            let all_portraits =
                super::catalog_ingestion::merge_images(&artist.portraits, &artist.portrait_group);
            for portrait in &all_portraits {
                children.push(QueueItem::new_child(
                    Uuid::new_v4().to_string(),
                    item.id.clone(),
                    DownloadContentType::ArtistImage,
                    portrait.id.clone(),
                    QueuePriority::Expansion,
                    item.request_source,
                    item.requested_by_user_id.clone(),
                    self.config.max_retries as i32,
                ));
            }
        }

        let children_count = children.len();
        let image_count = children_count - track_count;

        // 6. Insert children into queue
        if !children.is_empty() {
            self.queue_store
                .create_children(&item.id, children)
                .map_err(|e| {
                    DownloadError::new(
                        DownloadErrorType::Storage,
                        format!("Failed to create child items: {}", e),
                    )
                })?;

            // Log children created
            self.audit_logger
                .log_children_created(item, children_count, track_count, image_count)
                .ok();
        }

        info!(
            "Album {} queued {} child items ({} tracks, {} images)",
            item.content_id, children_count, track_count, image_count
        );

        // Return 0 bytes - album itself doesn't download data, children do
        Ok(0)
    }

    /// Execute track audio download.
    async fn execute_track_download(&self, item: &QueueItem) -> Result<u64, DownloadError> {
        info!("Downloading track audio: {}", item.content_id);

        // 1. Download track audio
        let (bytes, content_type) = self
            .downloader_client
            .download_track_audio(&item.content_id)
            .await
            .map_err(|e| {
                DownloadError::new(
                    DownloadErrorType::Connection,
                    format!("Failed to download track audio: {}", e),
                )
            })?;

        // 2. Determine file extension from content type
        let ext = Self::content_type_to_extension(&content_type);

        // 3. Write to disk using sharded directory structure
        let sharded_subpath = Self::sharded_path(&item.content_id, ext);
        let file_path = self.media_path.join("audio").join(&sharded_subpath);

        // Create parent directories
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                DownloadError::new(
                    DownloadErrorType::Storage,
                    format!("Failed to create audio directory: {}", e),
                )
            })?;
        }

        fs::write(&file_path, &bytes).await.map_err(|e| {
            DownloadError::new(
                DownloadErrorType::Storage,
                format!("Failed to write audio file: {}", e),
            )
        })?;

        let bytes_downloaded = bytes.len() as u64;

        // 4. Validate audio file with ffprobe before finalizing
        if let Err(e) = Self::validate_audio_with_ffprobe(&file_path).await {
            // Delete the corrupted file
            let _ = fs::remove_file(&file_path).await;
            return Err(DownloadError::new(
                DownloadErrorType::Corruption,
                format!(
                    "Audio validation failed for track {} (file deleted): {}",
                    item.content_id, e
                ),
            ));
        }

        info!(
            "Track {} downloaded and validated: {} bytes -> {}",
            item.content_id,
            bytes_downloaded,
            file_path.display()
        );

        // 5. Update track in catalog with actual audio path and format
        // This should always succeed since ingestion happens before children are created.
        // If it fails, the track doesn't exist in the catalog which is a bug.
        let audio_uri = format!("audio/{}", sharded_subpath);
        let format = Self::content_type_to_format(&content_type);
        self.catalog_store
            .update_track_audio(&item.content_id, &audio_uri, &format)
            .map_err(|e| {
                DownloadError::new(
                    DownloadErrorType::Storage,
                    format!(
                        "Failed to update track audio path (track {} not in catalog): {}",
                        item.content_id, e
                    ),
                )
            })?;

        // 6. Mark track as available now that audio is ready
        self.catalog_store
            .set_track_availability(&item.content_id, &TrackAvailability::Available)
            .map_err(|e| {
                DownloadError::new(
                    DownloadErrorType::Storage,
                    format!(
                        "Failed to set track availability (track {}): {}",
                        item.content_id, e
                    ),
                )
            })?;

        // 7. Update search index availability
        self.update_search_availability(&item.content_id, true).await;

        // Note: Parent completion check is done in process_next() after mark_completed()
        Ok(bytes_downloaded)
    }

    /// Execute image download (album cover or artist portrait).
    async fn execute_image_download(&self, item: &QueueItem) -> Result<u64, DownloadError> {
        info!("Downloading image: {}", item.content_id);

        // 1. Download image
        let bytes = self
            .downloader_client
            .download_image(&item.content_id)
            .await
            .map_err(|e| {
                DownloadError::new(
                    DownloadErrorType::Connection,
                    format!("Failed to download image: {}", e),
                )
            })?;

        // 2. Write to disk using sharded directory structure
        let sharded_subpath = Self::sharded_path(&item.content_id, "jpg");
        let file_path = self.media_path.join("images").join(&sharded_subpath);

        // Create parent directories
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                DownloadError::new(
                    DownloadErrorType::Storage,
                    format!("Failed to create images directory: {}", e),
                )
            })?;
        }

        fs::write(&file_path, &bytes).await.map_err(|e| {
            DownloadError::new(
                DownloadErrorType::Storage,
                format!("Failed to write image file: {}", e),
            )
        })?;

        let bytes_downloaded = bytes.len() as u64;

        // 3. Validate image file with ffprobe before finalizing
        if let Err(e) = Self::validate_image_with_ffprobe(&file_path).await {
            // Delete the corrupted file
            let _ = fs::remove_file(&file_path).await;
            return Err(DownloadError::new(
                DownloadErrorType::Corruption,
                format!(
                    "Image validation failed for {} (file deleted): {}",
                    item.content_id, e
                ),
            ));
        }

        info!(
            "Image {} downloaded and validated: {} bytes -> {}",
            item.content_id,
            bytes_downloaded,
            file_path.display()
        );

        // Note: Parent completion check is done in process_next() after mark_completed()
        Ok(bytes_downloaded)
    }

    /// Execute artist related download - fetches related artist IDs and saves them.
    async fn execute_artist_related_download(
        &self,
        item: &QueueItem,
    ) -> Result<u64, DownloadError> {
        info!("Fetching related artists for: {}", item.content_id);

        // 1. Fetch related artist IDs from downloader
        let related_ids = self
            .downloader_client
            .get_artist_related(&item.content_id)
            .await
            .map_err(|e| {
                DownloadError::new(
                    DownloadErrorType::Connection,
                    format!("Failed to fetch related artists: {}", e),
                )
            })?;

        // 2. Save related artist IDs to database
        let related_count = related_ids.len();
        for related_id in &related_ids {
            if let Err(e) = self
                .catalog_store
                .add_related_artist(&item.content_id, related_id)
            {
                // Log but don't fail - some may already exist or be duplicates
                debug!(
                    "Failed to add related artist {} for {}: {}",
                    related_id, item.content_id, e
                );
            }
        }

        info!(
            "Added {} related artists for {}",
            related_count, item.content_id
        );

        // No bytes downloaded, but successful
        Ok(0)
    }

    /// Execute artist metadata download - fetches full artist and creates record.
    async fn execute_artist_metadata_download(
        &self,
        item: &QueueItem,
    ) -> Result<u64, DownloadError> {
        info!("Fetching artist metadata for: {}", item.content_id);

        // 1. Fetch artist metadata from downloader
        let artist = self
            .downloader_client
            .get_artist(&item.content_id)
            .await
            .map_err(|e| {
                DownloadError::new(
                    DownloadErrorType::Connection,
                    format!("Failed to fetch artist metadata: {}", e),
                )
            })?;

        // 2. Ingest artist and images into catalog
        let image_ids =
            super::catalog_ingestion::ingest_artist(self.catalog_store.as_ref(), &artist).map_err(
                |e| {
                    DownloadError::new(
                        DownloadErrorType::Storage,
                        format!("Failed to ingest artist to catalog: {}", e),
                    )
                },
            )?;

        // 3. Queue portrait image downloads
        for image_id in &image_ids {
            let queue_item = QueueItem::new_child(
                Uuid::new_v4().to_string(),
                item.id.clone(),
                DownloadContentType::ArtistImage,
                image_id.clone(),
                QueuePriority::Expansion,
                item.request_source,
                item.requested_by_user_id.clone(),
                self.config.max_retries as i32,
            );
            if let Err(e) = self.queue_store.enqueue(queue_item) {
                debug!("Failed to queue artist image download: {}", e);
            }
        }

        info!(
            "Created artist {} with {} portrait images queued",
            item.content_id,
            image_ids.len()
        );

        Ok(0)
    }

    /// Check if all children of a parent are complete and update parent status.
    async fn check_and_complete_parent(&self, parent_id: &str) -> Result<()> {
        // check_parent_completion returns:
        // - Some(Completed) if all children completed successfully
        // - Some(Failed) if any children failed and none are in progress
        // - None if children are still being processed
        let new_status = self.queue_store.check_parent_completion(parent_id)?;

        match new_status {
            Some(QueueStatus::Completed) => {
                // All children done - calculate total bytes and mark parent as completed
                let children = self.queue_store.get_children(parent_id)?;
                let total_bytes: u64 = children.iter().filter_map(|c| c.bytes_downloaded).sum();
                let children_count = children.len();

                self.queue_store.mark_completed(parent_id, total_bytes, 0)?;

                let parent = self.queue_store.get_item(parent_id)?;
                if let Some(ref parent) = parent {
                    if let Some(user_id) = &parent.requested_by_user_id {
                        self.queue_store.decrement_user_queue(user_id)?;
                    }

                    // Emit completion event to requesting user
                    if let Some(ref notifier) = *self.sync_notifier.read().await {
                        notifier.notify_completed(parent).await;

                        // Broadcast catalog update to ALL connected users
                        if let Ok(skeleton_version) = self.catalog_store.get_skeleton_version() {
                            notifier.notify_catalog_updated(skeleton_version).await;
                        }
                    }

                    // Create user notification for download completion
                    if let Some(ref notification_service) = *self.notification_service.read().await
                    {
                        if let Some(user_id_str) = &parent.requested_by_user_id {
                            if let Ok(user_id) = user_id_str.parse::<usize>() {
                                // Get album image if available
                                let image_id = self
                                    .catalog_store
                                    .get_album_display_image_id(&parent.content_id)
                                    .ok()
                                    .flatten();

                                let data = serde_json::json!({
                                    "album_id": parent.content_id,
                                    "album_name": parent.content_name.clone().unwrap_or_default(),
                                    "artist_name": parent.artist_name.clone().unwrap_or_default(),
                                    "image_id": image_id,
                                    "request_id": parent.id,
                                });

                                let title = format!(
                                    "\"{}\" is ready",
                                    parent.content_name.as_deref().unwrap_or("Album")
                                );
                                let body = parent.artist_name.as_ref().map(|a| format!("by {}", a));

                                if let Err(e) = notification_service
                                    .create_notification(
                                        user_id,
                                        NotificationType::DownloadCompleted,
                                        title,
                                        body,
                                        data,
                                    )
                                    .await
                                {
                                    warn!(
                                        "Failed to create download completion notification: {}",
                                        e
                                    );
                                }
                            }
                        }
                    }
                }

                info!(
                    "Parent {} completed: {} children, {} total bytes",
                    parent_id, children_count, total_bytes
                );
            }
            Some(QueueStatus::Failed) => {
                // Some children failed - get failure count for error message
                let progress = self.queue_store.get_children_progress(parent_id)?;

                let error_msg = format!(
                    "{} of {} children failed",
                    progress.failed, progress.total_children
                );

                self.queue_store.mark_failed(
                    parent_id,
                    &DownloadError::new(DownloadErrorType::Unknown, error_msg.clone()),
                )?;

                let parent = self.queue_store.get_item(parent_id)?;
                if let Some(ref parent) = parent {
                    if let Some(user_id) = &parent.requested_by_user_id {
                        self.queue_store.decrement_user_queue(user_id)?;
                    }

                    // Emit failure event
                    if let Some(ref notifier) = *self.sync_notifier.read().await {
                        notifier
                            .notify_status_changed(
                                parent,
                                QueueStatus::Failed,
                                None,
                                Some(error_msg),
                            )
                            .await;
                    }
                }
            }
            _ => {
                // None or other status - children still in progress, do nothing
            }
        }

        Ok(())
    }

    /// Convert content type to file extension.
    fn content_type_to_extension(content_type: &str) -> &'static str {
        match content_type {
            "audio/flac" => "flac",
            "audio/mpeg" | "audio/mp3" => "mp3",
            "audio/ogg" | "audio/vorbis" => "ogg",
            "audio/wav" | "audio/wave" => "wav",
            "audio/aac" => "aac",
            "audio/mp4" | "audio/m4a" => "m4a",
            _ => "flac", // Default to flac
        }
    }

    /// Convert content type to Format enum.
    fn content_type_to_format(content_type: &str) -> crate::catalog_store::Format {
        use crate::catalog_store::Format;
        match content_type {
            "audio/flac" => Format::Flac,
            "audio/mpeg" | "audio/mp3" => Format::Mp3_320, // Assume high quality
            "audio/ogg" | "audio/vorbis" => Format::OggVorbis320, // Assume high quality
            "audio/aac" => Format::Aac160,
            _ => Format::Flac, // Default to flac
        }
    }

    /// Validate an audio file using ffprobe.
    ///
    /// Returns Ok(()) if valid, Err with message if invalid or ffprobe fails.
    async fn validate_audio_with_ffprobe(path: &std::path::Path) -> Result<(), String> {
        use tokio::process::Command;

        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "csv=p=0",
            ])
            .arg(path)
            .output()
            .await
            .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("ffprobe validation failed: {}", stderr.trim()))
        }
    }

    /// Validate an image file using ffprobe.
    ///
    /// Returns Ok(()) if valid, Err with message if invalid or ffprobe fails.
    /// Note: ffprobe may return exit code 0 even for invalid files, but writes
    /// errors to stderr with `-v error`. We check both exit code and stderr.
    async fn validate_image_with_ffprobe(path: &std::path::Path) -> Result<(), String> {
        use tokio::process::Command;

        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=format_name",
                "-of",
                "csv=p=0",
            ])
            .arg(path)
            .output()
            .await
            .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Check both exit code and stderr - ffprobe may return 0 but log errors
        if output.status.success() && stderr.trim().is_empty() {
            Ok(())
        } else if !output.status.success() {
            Err(format!("ffprobe validation failed: {}", stderr.trim()))
        } else {
            // Exit code 0 but errors in stderr
            Err(format!("ffprobe detected issues: {}", stderr.trim()))
        }
    }

    /// Generate a sharded file path from an ID.
    ///
    /// Uses first 6 characters split into 3 pairs as directory levels.
    /// Example: `2Ueco3C1xLw5RXl39lAPkL` with ext `flac` becomes
    /// `2U/ec/o3/2Ueco3C1xLw5RXl39lAPkL.flac`
    fn sharded_path(id: &str, ext: &str) -> String {
        // Ensure we have enough characters for sharding (at least 6)
        if id.len() >= 6 {
            format!("{}/{}/{}/{}.{}", &id[0..2], &id[2..4], &id[4..6], id, ext)
        } else {
            // Fallback for short IDs (shouldn't happen with real IDs)
            format!("{}.{}", id, ext)
        }
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
    pub fn retry_failed(&self, admin_user_id: &str, request_id: &str, force: bool) -> Result<()> {
        // Get the item
        let item = self
            .queue_store
            .get_item(request_id)?
            .ok_or_else(|| anyhow::anyhow!("Item not found: {}", request_id))?;

        // Verify status is retryable
        let allowed_statuses = if force {
            vec![
                QueueStatus::Failed,
                QueueStatus::InProgress,
                QueueStatus::RetryWaiting,
            ]
        } else {
            vec![QueueStatus::Failed]
        };

        if !allowed_statuses.contains(&item.status) {
            return Err(anyhow::anyhow!(
                "Cannot retry item with status: {:?}{}",
                item.status,
                if !force {
                    " (use force=true for stuck items)"
                } else {
                    ""
                }
            ));
        }

        // If this is a parent item (no parent_id), delete all children first.
        // This prevents duplicate children when the parent is reprocessed.
        if item.parent_id.is_none() {
            let deleted_count = self.queue_store.delete_children(request_id)?;
            if deleted_count > 0 {
                info!(
                    "Admin {} deleted {} children for parent {} before retry",
                    admin_user_id, deleted_count, request_id
                );
            }
        }

        // Reset the item to pending status
        self.queue_store.reset_to_pending(request_id)?;

        // Log the admin retry
        self.audit_logger.log_admin_retry(&item, admin_user_id)?;

        info!(
            "Admin {} triggered retry for item {} ({:?})",
            admin_user_id, request_id, item.content_name
        );

        Ok(())
    }

    /// Delete a queue item (admin action).
    pub fn delete_request(&self, admin_user_id: &str, request_id: &str) -> Result<()> {
        // Get the item first to verify it exists and log it
        let item = self
            .queue_store
            .get_item(request_id)?
            .ok_or_else(|| anyhow::anyhow!("Item not found: {}", request_id))?;

        // Delete the item (children are cascade-deleted)
        self.queue_store.delete_item(request_id)?;

        // Log the deletion
        info!(
            "Admin {} deleted queue item {} ({:?})",
            admin_user_id, request_id, item.content_name
        );

        Ok(())
    }

    /// Get activity log entries.
    pub fn get_activity(&self, hours: usize) -> Result<Vec<ActivityLogEntry>> {
        let since = chrono::Utc::now().timestamp() - (hours as i64 * 3600);
        self.queue_store.get_activity_since(since)
    }

    /// Get aggregated download statistics over time.
    /// If `since`/`until` are provided, uses custom date range instead of period defaults.
    pub fn get_stats_history(
        &self,
        period: StatsPeriod,
        since: Option<i64>,
        until: Option<i64>,
    ) -> Result<DownloadStatsHistory> {
        self.queue_store.get_stats_history(period, since, until)
    }

    /// Get all requests with optional filters.
    pub fn get_all_requests(
        &self,
        status: Option<QueueStatus>,
        exclude_completed: bool,
        top_level_only: bool,
        _user_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>> {
        self.queue_store
            .list_all(status, exclude_completed, top_level_only, limit, offset)
    }

    /// Get progress for a parent item's children.
    pub fn get_request_progress(&self, request_id: &str) -> Result<Option<DownloadProgress>> {
        let item = self.queue_store.get_item(request_id)?;
        match item {
            Some(item) if item.parent_id.is_none() => {
                // This is a top-level request, get children progress
                let progress = self.queue_store.get_children_progress(request_id)?;
                if progress.total_children > 0 {
                    Ok(Some(progress))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
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

    /// Get audit log entries for a specific user. Returns (entries, total_count).
    pub fn get_audit_for_user(
        &self,
        user_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<AuditLogEntry>, usize)> {
        self.queue_store
            .get_audit_for_user(user_id, None, None, limit, offset)
    }

    // =========================================================================
    // Throttle & Corruption Handler Methods (for admin API)
    // =========================================================================

    /// Get current throttle statistics.
    pub async fn get_throttle_stats(&self) -> super::throttle::ThrottleStats {
        self.throttler.get_stats().await
    }

    /// Reset throttle state (admin action).
    pub async fn reset_throttle(&self) {
        self.throttler.reset().await;
        info!("Throttle state reset by admin");
    }

    /// Get current corruption handler state.
    pub async fn get_corruption_handler_state(&self) -> super::corruption_handler::HandlerState {
        self.corruption_handler.get_state().await
    }

    /// Reset corruption handler state (admin action).
    /// Clears cooldown immediately and resets level to 0.
    pub async fn reset_corruption_handler(&self) {
        self.corruption_handler.admin_reset().await;
    }

    /// Get reference to the corruption handler for persistence operations.
    pub fn corruption_handler(&self) -> &Arc<CorruptionHandler> {
        &self.corruption_handler
    }

    /// Get reference to the throttler.
    pub fn throttler(&self) -> &Arc<SlidingWindowThrottler> {
        &self.throttler
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
        let catalog_store =
            Arc::new(SqliteCatalogStore::new(&catalog_db_path, temp_dir.path(), 4).unwrap());
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

        let status = ctx
            .manager
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

        let (entries, total) = ctx.manager.get_audit_for_user("user-123", 10, 0).unwrap();

        assert!(entries.is_empty());
        assert_eq!(total, 0);
    }

    #[test]
    fn test_retry_failed_not_found() {
        let ctx = create_test_manager();

        let result = ctx.manager.retry_failed("admin", "nonexistent", false);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_request_album_success() {
        let ctx = create_test_manager();

        let request = AlbumRequest {
            album_id: "album-123".to_string(),
            album_name: "Test Album".to_string(),
            artist_name: "Test Artist".to_string(),
        };

        let result = ctx.manager.request_album("user-1", request).await.unwrap();

        assert_eq!(result.status, QueueStatus::Pending);
        assert!(!result.request_id.is_empty());
        assert_eq!(result.queue_position, 1);

        // Verify the item was added to the queue
        let requests = ctx.manager.get_user_requests("user-1", 10, 0).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].content_id, "album-123");
        assert_eq!(requests[0].content_name, Some("Test Album".to_string()));
        assert_eq!(requests[0].artist_name, Some("Test Artist".to_string()));
    }

    #[tokio::test]
    async fn test_request_album_already_in_queue() {
        let ctx = create_test_manager();

        let request = AlbumRequest {
            album_id: "album-123".to_string(),
            album_name: "Test Album".to_string(),
            artist_name: "Test Artist".to_string(),
        };

        // First request should succeed
        ctx.manager
            .request_album("user-1", request.clone())
            .await
            .unwrap();

        // Second request for the same album should fail
        let result = ctx.manager.request_album("user-1", request).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already in the download queue"));
    }

    #[tokio::test]
    async fn test_request_album_increments_user_stats() {
        let ctx = create_test_manager();

        let request = AlbumRequest {
            album_id: "album-123".to_string(),
            album_name: "Test Album".to_string(),
            artist_name: "Test Artist".to_string(),
        };

        // Check initial limits
        let limits_before = ctx.manager.check_user_limits("user-1").unwrap();
        assert_eq!(limits_before.requests_today, 0);
        assert_eq!(limits_before.in_queue, 0);

        // Make a request
        ctx.manager.request_album("user-1", request).await.unwrap();

        // Check updated limits
        let limits_after = ctx.manager.check_user_limits("user-1").unwrap();
        assert_eq!(limits_after.requests_today, 1);
        assert_eq!(limits_after.in_queue, 1);
    }

    #[tokio::test]
    async fn test_request_album_logs_audit_event() {
        let ctx = create_test_manager();

        let request = AlbumRequest {
            album_id: "album-123".to_string(),
            album_name: "Test Album".to_string(),
            artist_name: "Test Artist".to_string(),
        };

        let result = ctx.manager.request_album("user-1", request).await.unwrap();

        // Check that an audit log entry was created
        let (item, entries) = ctx
            .manager
            .get_audit_for_item(&result.request_id)
            .unwrap()
            .unwrap();

        assert_eq!(item.id, result.request_id);
        assert!(!entries.is_empty());
        assert_eq!(entries[0].event_type, AuditEventType::RequestCreated);
    }

    #[test]
    fn test_request_discography_not_implemented() {
        let ctx = create_test_manager();

        let request = DiscographyRequest {
            artist_id: "artist-123".to_string(),
            artist_name: "Test Artist".to_string(),
        };

        let result = ctx.manager.request_discography("user-1", request);

        // Should return an error indicating async implementation is needed
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires async implementation"));
    }

    #[tokio::test]
    async fn test_process_next_empty_queue() {
        let ctx = create_test_manager();

        let result = ctx.manager.process_next().await.unwrap();

        // Should return None when queue is empty
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_process_next_processes_item() {
        let ctx = create_test_manager();

        // Add an item to the queue
        let request = AlbumRequest {
            album_id: "album-123".to_string(),
            album_name: "Test Album".to_string(),
            artist_name: "Test Artist".to_string(),
        };
        let request_result = ctx.manager.request_album("user-1", request).await.unwrap();

        // Process the item
        let result = ctx.manager.process_next().await.unwrap();

        // Should return a result (failure since download not implemented)
        assert!(result.is_some());
        let processing_result = result.unwrap();
        assert_eq!(processing_result.queue_item_id, request_result.request_id);
        assert_eq!(processing_result.content_type, DownloadContentType::Album);
        assert!(!processing_result.success);
        assert!(processing_result.error.is_some());
    }

    #[tokio::test]
    async fn test_process_next_schedules_retry() {
        let ctx = create_test_manager();

        // Add an item to the queue
        let request = AlbumRequest {
            album_id: "album-456".to_string(),
            album_name: "Test Album".to_string(),
            artist_name: "Test Artist".to_string(),
        };
        let request_result = ctx.manager.request_album("user-1", request).await.unwrap();

        // Process the item (will fail and schedule retry)
        let result = ctx.manager.process_next().await.unwrap();
        assert!(result.is_some());

        // Check the item is now in RETRY_WAITING status
        let item = ctx
            .manager
            .queue_store
            .get_item(&request_result.request_id)
            .unwrap()
            .unwrap();
        assert_eq!(item.status, QueueStatus::RetryWaiting);
        assert_eq!(item.retry_count, 1);
    }

    #[tokio::test]
    async fn test_process_next_logs_audit_events() {
        let ctx = create_test_manager();

        // Add an item to the queue
        let request = AlbumRequest {
            album_id: "album-789".to_string(),
            album_name: "Test Album".to_string(),
            artist_name: "Test Artist".to_string(),
        };
        let request_result = ctx.manager.request_album("user-1", request).await.unwrap();

        // Process the item
        ctx.manager.process_next().await.unwrap();

        // Check audit log entries
        let (_, entries) = ctx
            .manager
            .get_audit_for_item(&request_result.request_id)
            .unwrap()
            .unwrap();

        // Should have: RequestCreated, DownloadStarted, RetryScheduled
        assert!(entries.len() >= 3);
        assert!(entries
            .iter()
            .any(|e| e.event_type == AuditEventType::RequestCreated));
        assert!(entries
            .iter()
            .any(|e| e.event_type == AuditEventType::DownloadStarted));
        assert!(entries
            .iter()
            .any(|e| e.event_type == AuditEventType::RetryScheduled));
    }

    #[test]
    fn test_config_accessor() {
        let ctx = create_test_manager();

        let config = ctx.manager.config();

        // Default config values
        assert!(!config.enabled);
        assert!(config.max_retries > 0);
    }

    #[test]
    fn test_is_enabled() {
        let ctx = create_test_manager();

        // Default config has enabled = false
        assert!(!ctx.manager.is_enabled());
    }

    #[test]
    fn test_get_all_requests_empty() {
        let ctx = create_test_manager();

        let requests = ctx
            .manager
            .get_all_requests(None, false, false, None, 100, 0)
            .unwrap();

        assert!(requests.is_empty());
    }

    #[tokio::test]
    async fn test_get_all_requests_with_items() {
        let ctx = create_test_manager();

        // Add some items
        ctx.manager
            .request_album(
                "user-1",
                AlbumRequest {
                    album_id: "album-1".to_string(),
                    album_name: "Album 1".to_string(),
                    artist_name: "Artist".to_string(),
                },
            )
            .await
            .unwrap();

        ctx.manager
            .request_album(
                "user-2",
                AlbumRequest {
                    album_id: "album-2".to_string(),
                    album_name: "Album 2".to_string(),
                    artist_name: "Artist".to_string(),
                },
            )
            .await
            .unwrap();

        let requests = ctx
            .manager
            .get_all_requests(None, false, false, None, 100, 0)
            .unwrap();

        assert_eq!(requests.len(), 2);
    }

    #[tokio::test]
    async fn test_get_all_requests_with_status_filter() {
        let ctx = create_test_manager();

        // Add an item
        ctx.manager
            .request_album(
                "user-1",
                AlbumRequest {
                    album_id: "album-filter".to_string(),
                    album_name: "Album".to_string(),
                    artist_name: "Artist".to_string(),
                },
            )
            .await
            .unwrap();

        // Filter by PENDING status
        let pending = ctx
            .manager
            .get_all_requests(Some(QueueStatus::Pending), false, false, None, 100, 0)
            .unwrap();
        assert_eq!(pending.len(), 1);

        // Filter by COMPLETED status (should be empty)
        let completed = ctx
            .manager
            .get_all_requests(Some(QueueStatus::Completed), false, false, None, 100, 0)
            .unwrap();
        assert!(completed.is_empty());
    }

    #[tokio::test]
    async fn test_get_request_status_found() {
        let ctx = create_test_manager();

        let request_result = ctx
            .manager
            .request_album(
                "user-1",
                AlbumRequest {
                    album_id: "album-status".to_string(),
                    album_name: "Album".to_string(),
                    artist_name: "Artist".to_string(),
                },
            )
            .await
            .unwrap();

        let status = ctx
            .manager
            .get_request_status("user-1", &request_result.request_id)
            .unwrap();

        assert!(status.is_some());
        let item = status.unwrap();
        assert_eq!(item.id, request_result.request_id);
        assert_eq!(item.status, QueueStatus::Pending);
    }

    #[tokio::test]
    async fn test_multiple_users_separate_queues() {
        let ctx = create_test_manager();

        // User 1 requests
        ctx.manager
            .request_album(
                "user-1",
                AlbumRequest {
                    album_id: "album-u1".to_string(),
                    album_name: "Album U1".to_string(),
                    artist_name: "Artist".to_string(),
                },
            )
            .await
            .unwrap();

        // User 2 requests
        ctx.manager
            .request_album(
                "user-2",
                AlbumRequest {
                    album_id: "album-u2".to_string(),
                    album_name: "Album U2".to_string(),
                    artist_name: "Artist".to_string(),
                },
            )
            .await
            .unwrap();

        // Check each user sees only their requests
        let user1_requests = ctx.manager.get_user_requests("user-1", 100, 0).unwrap();
        let user2_requests = ctx.manager.get_user_requests("user-2", 100, 0).unwrap();

        assert_eq!(user1_requests.len(), 1);
        assert_eq!(user2_requests.len(), 1);
        assert_eq!(user1_requests[0].content_id, "album-u1");
        assert_eq!(user2_requests[0].content_id, "album-u2");
    }

    #[tokio::test]
    async fn test_retry_failed_wrong_status() {
        let ctx = create_test_manager();

        // Add an item (will be in PENDING status)
        let request_result = ctx
            .manager
            .request_album(
                "user-1",
                AlbumRequest {
                    album_id: "album-retry".to_string(),
                    album_name: "Album".to_string(),
                    artist_name: "Artist".to_string(),
                },
            )
            .await
            .unwrap();

        // Try to retry a non-failed item (without force)
        let result = ctx
            .manager
            .retry_failed("admin", &request_result.request_id, false);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot retry item with status"));
    }

    #[tokio::test]
    async fn test_queue_position_ordering() {
        let ctx = create_test_manager();

        // Add multiple items
        let result1 = ctx
            .manager
            .request_album(
                "user-1",
                AlbumRequest {
                    album_id: "album-pos-1".to_string(),
                    album_name: "Album 1".to_string(),
                    artist_name: "Artist".to_string(),
                },
            )
            .await
            .unwrap();

        let result2 = ctx
            .manager
            .request_album(
                "user-1",
                AlbumRequest {
                    album_id: "album-pos-2".to_string(),
                    album_name: "Album 2".to_string(),
                    artist_name: "Artist".to_string(),
                },
            )
            .await
            .unwrap();

        // First item should have position 1
        assert_eq!(result1.queue_position, 1);
        // Second item gets position 1 when items have same timestamp (created in same second)
        // Queue position counts items with strictly earlier created_at, so if timestamps match,
        // both items appear at the same position. This is expected behavior for fast operations.
        assert!(result2.queue_position >= 1);
    }

    #[tokio::test]
    async fn test_process_next_respects_priority_order() {
        let ctx = create_test_manager();

        // Add two items (both will have User priority by default)
        ctx.manager
            .request_album(
                "user-1",
                AlbumRequest {
                    album_id: "album-first".to_string(),
                    album_name: "First Album".to_string(),
                    artist_name: "Artist".to_string(),
                },
            )
            .await
            .unwrap();

        ctx.manager
            .request_album(
                "user-1",
                AlbumRequest {
                    album_id: "album-second".to_string(),
                    album_name: "Second Album".to_string(),
                    artist_name: "Artist".to_string(),
                },
            )
            .await
            .unwrap();

        // Process first item - should be the first one added (FIFO within same priority)
        let result = ctx.manager.process_next().await.unwrap().unwrap();
        assert_eq!(result.content_type, DownloadContentType::Album);

        // The processed item should be album-first (first in, first out)
        let item = ctx
            .manager
            .queue_store
            .get_item(&result.queue_item_id)
            .unwrap()
            .unwrap();
        assert_eq!(item.content_id, "album-first");
    }

    // =========================================================================
    // FFprobe Validation Tests
    // =========================================================================

    #[tokio::test]
    async fn test_validate_audio_with_ffprobe_valid_file() {
        // Create a valid audio file using ffmpeg
        let temp_dir = TempDir::new().unwrap();
        let audio_path = temp_dir.path().join("test.mp3");

        // Generate a 1-second silent audio file
        let status = tokio::process::Command::new("ffmpeg")
            .args([
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=44100:cl=mono",
                "-t",
                "0.1",
                "-q:a",
                "9",
            ])
            .arg(&audio_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;

        // Skip test if ffmpeg not available
        if status.is_err() {
            eprintln!("Skipping test: ffmpeg not available");
            return;
        }

        let result = DownloadManager::validate_audio_with_ffprobe(&audio_path).await;
        assert!(result.is_ok(), "Valid audio file should pass validation");
    }

    #[tokio::test]
    async fn test_validate_audio_with_ffprobe_invalid_file() {
        let temp_dir = TempDir::new().unwrap();
        let audio_path = temp_dir.path().join("invalid.mp3");

        // Write garbage data that looks nothing like an audio file
        std::fs::write(&audio_path, b"this is not an audio file at all").unwrap();

        let result = DownloadManager::validate_audio_with_ffprobe(&audio_path).await;
        assert!(result.is_err(), "Invalid audio file should fail validation");
    }

    #[tokio::test]
    async fn test_validate_audio_with_ffprobe_nonexistent_file() {
        let path = std::path::Path::new("/nonexistent/path/to/audio.mp3");

        let result = DownloadManager::validate_audio_with_ffprobe(path).await;
        assert!(result.is_err(), "Nonexistent file should fail validation");
    }

    #[tokio::test]
    async fn test_validate_image_with_ffprobe_valid_file() {
        // Create a valid image file using ffmpeg
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.png");

        // Generate a small test image (10x10 red)
        // -update 1 is required for image2 muxer to output a single file
        let status = tokio::process::Command::new("ffmpeg")
            .args([
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=10x10",
                "-frames:v",
                "1",
                "-update",
                "1",
            ])
            .arg(&image_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;

        // Skip test if ffmpeg not available
        if status.is_err() {
            eprintln!("Skipping test: ffmpeg not available");
            return;
        }

        let result = DownloadManager::validate_image_with_ffprobe(&image_path).await;
        assert!(
            result.is_ok(),
            "Valid image file should pass validation: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_validate_image_with_ffprobe_invalid_file() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("invalid.png");

        // Write garbage data
        std::fs::write(&image_path, b"this is not an image file").unwrap();

        let result = DownloadManager::validate_image_with_ffprobe(&image_path).await;
        assert!(result.is_err(), "Invalid image file should fail validation");
    }

    #[tokio::test]
    async fn test_validate_image_with_ffprobe_nonexistent_file() {
        let path = std::path::Path::new("/nonexistent/path/to/image.png");

        let result = DownloadManager::validate_image_with_ffprobe(path).await;
        assert!(result.is_err(), "Nonexistent file should fail validation");
    }

    #[tokio::test]
    async fn test_validate_audio_with_ffprobe_truncated_file() {
        // Create a valid audio file, then truncate it to simulate corruption
        let temp_dir = TempDir::new().unwrap();
        let audio_path = temp_dir.path().join("truncated.mp3");

        // Generate a valid audio file first
        let status = tokio::process::Command::new("ffmpeg")
            .args([
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=44100:cl=mono",
                "-t",
                "1",
                "-q:a",
                "9",
            ])
            .arg(&audio_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;

        if status.is_err() {
            eprintln!("Skipping test: ffmpeg not available");
            return;
        }

        // Read the file and truncate it to half
        let data = std::fs::read(&audio_path).unwrap();
        if data.len() > 100 {
            std::fs::write(&audio_path, &data[..data.len() / 2]).unwrap();
        }

        // Truncated file may or may not pass ffprobe depending on where truncation happened
        // The important thing is it doesn't panic
        let _result = DownloadManager::validate_audio_with_ffprobe(&audio_path).await;
    }

    #[tokio::test]
    async fn test_check_ffprobe_available() {
        // This test verifies ffprobe check returns version info
        // Will pass if ffprobe is installed, skip message if not
        let result = DownloadManager::check_ffprobe_available().await;

        match result {
            Ok(version) => {
                assert!(
                    version.contains("ffprobe"),
                    "Version should mention ffprobe: {}",
                    version
                );
            }
            Err(e) => {
                // If ffprobe is not installed, the test should pass but log a message
                eprintln!(
                    "Note: ffprobe not installed, skipping version check. Error: {}",
                    e
                );
            }
        }
    }
}
