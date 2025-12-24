//! Missing files watchdog.
//!
//! Scans for missing media files and queues repairs. The watchdog periodically
//! checks that all catalog content has corresponding media files on disk,
//! and queues download requests for any missing content.

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use tracing::{debug, info, warn};

use crate::catalog_store::CatalogStore;

use super::audit_logger::AuditLogger;
use super::models::{
    DownloadContentType, MissingFilesMode, MissingFilesReport, MissingImageInfo, MissingTrackInfo,
    QueueItem, QueuePriority, QueueStatus, RequestSource,
};
use super::queue_store::DownloadQueueStore;

/// Missing files watchdog that scans catalog for missing media files.
///
/// The watchdog:
/// 1. Scans all tracks to find those missing audio files
/// 2. Scans all album images to find missing cover images
/// 3. Scans all artist images to find missing portrait images
/// 4. Queues repair downloads for missing content (if not already queued)
pub struct MissingFilesWatchdog {
    catalog_store: Arc<dyn CatalogStore>,
    queue_store: Arc<dyn DownloadQueueStore>,
    audit_logger: AuditLogger,
}

impl MissingFilesWatchdog {
    /// Create a new MissingFilesWatchdog.
    ///
    /// # Arguments
    /// * `catalog_store` - Catalog store for querying content
    /// * `queue_store` - Download queue store for checking/adding queue items
    /// * `audit_logger` - Audit logger for recording watchdog events
    pub fn new(
        catalog_store: Arc<dyn CatalogStore>,
        queue_store: Arc<dyn DownloadQueueStore>,
        audit_logger: AuditLogger,
    ) -> Self {
        Self {
            catalog_store,
            queue_store,
            audit_logger,
        }
    }

    /// Run a full scan and queue repairs for missing media files.
    ///
    /// Returns a report detailing what was found and what was queued.
    ///
    /// # Arguments
    /// * `mode` - The execution mode (DryRun or Actual)
    pub fn run_scan(&self, mode: MissingFilesMode) -> Result<MissingFilesReport> {
        let start = Instant::now();

        info!("Starting missing files scan in {:?} mode", mode);

        // Log scan start
        if let Err(e) = self.audit_logger.log_watchdog_scan_started() {
            warn!("Failed to log watchdog scan start: {}", e);
        }

        // Get totals for the scan summary
        let all_track_ids = self.catalog_store.list_all_track_ids()?;
        let all_album_image_ids = self.catalog_store.list_all_album_image_ids()?;
        let all_artist_image_ids = self.catalog_store.list_all_artist_image_ids()?;

        let total_tracks_scanned = all_track_ids.len();
        let total_album_images_scanned = all_album_image_ids.len();
        let total_artist_images_scanned = all_artist_image_ids.len();

        // Scan for missing content with detailed info
        let (missing_track_audio, missing_track_details) =
            self.scan_missing_track_audio_detailed(&all_track_ids)?;
        let (missing_album_images, missing_album_image_details) =
            self.scan_missing_album_images_detailed(&all_album_image_ids)?;
        let (missing_artist_images, missing_artist_image_details) =
            self.scan_missing_artist_images_detailed(&all_artist_image_ids)?;

        info!(
            "Missing files scan found {} missing track audio, {} missing album images, {} missing artist images",
            missing_track_audio.len(),
            missing_album_images.len(),
            missing_artist_images.len()
        );

        // Queue repairs for missing content only if in Actual mode
        let (items_queued, items_skipped) = match mode {
            MissingFilesMode::DryRun => {
                info!(
                    "Dry-run mode: would queue {} items",
                    missing_track_audio.len()
                        + missing_album_images.len()
                        + missing_artist_images.len()
                );
                (0, 0)
            }
            MissingFilesMode::Actual => self.queue_repairs(
                &missing_track_audio,
                &missing_album_images,
                &missing_artist_images,
            )?,
        };

        let scan_duration_ms = start.elapsed().as_millis() as i64;

        let report = MissingFilesReport {
            mode,
            total_tracks_scanned,
            total_album_images_scanned,
            total_artist_images_scanned,
            missing_track_audio,
            missing_track_details,
            missing_album_images,
            missing_album_image_details,
            missing_artist_images,
            missing_artist_image_details,
            items_queued,
            items_skipped,
            scan_duration_ms,
        };

        // Log scan completion
        if let Err(e) = self.audit_logger.log_missing_files_scan_completed(&report) {
            warn!("Failed to log watchdog scan completion: {}", e);
        }

        Ok(report)
    }

    /// Scan for tracks missing audio files with detailed information.
    ///
    /// Returns a tuple of (track IDs, detailed track info) for tracks missing audio files.
    fn scan_missing_track_audio_detailed(
        &self,
        track_ids: &[String],
    ) -> Result<(Vec<String>, Vec<MissingTrackInfo>)> {
        let mut missing_ids = Vec::new();
        let mut missing_details = Vec::new();

        for track_id in track_ids {
            let is_missing =
                if let Some(audio_path) = self.catalog_store.get_track_audio_path(track_id) {
                    if !audio_path.exists() {
                        debug!(
                            "Missing audio file for track {}: {:?}",
                            track_id, audio_path
                        );
                        true
                    } else {
                        false
                    }
                } else {
                    // Track has no audio_uri set - this is also considered missing
                    debug!("Track {} has no audio_uri set", track_id);
                    true
                };

            if is_missing {
                missing_ids.push(track_id.clone());

                // Gather detailed info from resolved track JSON
                let detail = self.get_track_detail(track_id);
                missing_details.push(detail);
            }
        }

        Ok((missing_ids, missing_details))
    }

    /// Get detailed information about a track for the report.
    fn get_track_detail(&self, track_id: &str) -> MissingTrackInfo {
        let mut track_name = String::from("Unknown");
        let mut album_id = None;
        let mut album_name = None;
        let mut artist_names = Vec::new();

        if let Ok(Some(track_json)) = self.catalog_store.get_resolved_track_json(track_id) {
            // Extract track name from resolved track JSON
            // ResolvedTrack has: track: Track, album: Album, artists: Vec<TrackArtist>
            if let Some(track) = track_json.get("track") {
                if let Some(name) = track.get("name").and_then(|n| n.as_str()) {
                    track_name = name.to_string();
                }
            }

            if let Some(album) = track_json.get("album") {
                if let Some(id) = album.get("id").and_then(|i| i.as_str()) {
                    album_id = Some(id.to_string());
                }
                if let Some(name) = album.get("name").and_then(|n| n.as_str()) {
                    album_name = Some(name.to_string());
                }
            }

            // ResolvedTrack.artists is Vec<TrackArtist> with { artist: Artist, role: ... }
            if let Some(artists) = track_json.get("artists").and_then(|a| a.as_array()) {
                for track_artist in artists {
                    if let Some(artist) = track_artist.get("artist") {
                        if let Some(name) = artist.get("name").and_then(|n| n.as_str()) {
                            artist_names.push(name.to_string());
                        }
                    }
                }
            }
        }

        MissingTrackInfo {
            track_id: track_id.to_string(),
            track_name,
            album_id,
            album_name,
            artist_names,
        }
    }

    /// Scan for album images missing files with detailed information.
    ///
    /// Returns a tuple of (image IDs, detailed image info) for missing album images.
    fn scan_missing_album_images_detailed(
        &self,
        image_ids: &[String],
    ) -> Result<(Vec<String>, Vec<MissingImageInfo>)> {
        let mut missing_ids = Vec::new();
        let mut missing_details = Vec::new();

        for image_id in image_ids {
            let image_path = self.catalog_store.get_image_path(image_id);
            if !image_path.exists() {
                debug!(
                    "Missing album image file for {}: {:?}",
                    image_id, image_path
                );
                missing_ids.push(image_id.clone());

                // Try to find which album uses this image
                let detail = self.get_album_image_detail(image_id);
                missing_details.push(detail);
            }
        }

        Ok((missing_ids, missing_details))
    }

    /// Get detailed information about an album image for the report.
    fn get_album_image_detail(&self, image_id: &str) -> MissingImageInfo {
        // We don't have a direct query for "which album uses this image"
        // For now, we'll just report the image ID. In a future enhancement,
        // we could add a reverse lookup.
        MissingImageInfo {
            image_id: image_id.to_string(),
            entity_id: String::new(),
            entity_name: String::from("(album lookup not implemented)"),
        }
    }

    /// Scan for artist images missing files with detailed information.
    ///
    /// Returns a tuple of (image IDs, detailed image info) for missing artist images.
    fn scan_missing_artist_images_detailed(
        &self,
        image_ids: &[String],
    ) -> Result<(Vec<String>, Vec<MissingImageInfo>)> {
        let mut missing_ids = Vec::new();
        let mut missing_details = Vec::new();

        for image_id in image_ids {
            let image_path = self.catalog_store.get_image_path(image_id);
            if !image_path.exists() {
                debug!(
                    "Missing artist image file for {}: {:?}",
                    image_id, image_path
                );
                missing_ids.push(image_id.clone());

                // Try to find which artist uses this image
                let detail = self.get_artist_image_detail(image_id);
                missing_details.push(detail);
            }
        }

        Ok((missing_ids, missing_details))
    }

    /// Get detailed information about an artist image for the report.
    fn get_artist_image_detail(&self, image_id: &str) -> MissingImageInfo {
        // We don't have a direct query for "which artist uses this image"
        // For now, we'll just report the image ID. In a future enhancement,
        // we could add a reverse lookup.
        MissingImageInfo {
            image_id: image_id.to_string(),
            entity_id: String::new(),
            entity_name: String::from("(artist lookup not implemented)"),
        }
    }

    /// Queue repairs for missing content.
    ///
    /// For each missing item:
    /// 1. Check if already in queue (any status)
    /// 2. Skip if already queued
    /// 3. Otherwise, create new queue item with Watchdog priority
    ///
    /// Returns (items_queued, items_skipped).
    fn queue_repairs(
        &self,
        missing_track_audio: &[String],
        missing_album_images: &[String],
        missing_artist_images: &[String],
    ) -> Result<(usize, usize)> {
        let mut queued = 0;
        let mut skipped = 0;

        // Queue missing track audio
        for track_id in missing_track_audio {
            if self.is_already_in_queue(DownloadContentType::TrackAudio, track_id)? {
                debug!("Track audio {} already in queue, skipping", track_id);
                skipped += 1;
                continue;
            }

            let queue_item = self.create_watchdog_queue_item(
                DownloadContentType::TrackAudio,
                track_id.clone(),
                "missing_audio_file",
            );

            match self.queue_store.enqueue(queue_item.clone()) {
                Ok(_) => {
                    if let Err(e) = self
                        .audit_logger
                        .log_watchdog_queued(&queue_item, "missing_audio_file")
                    {
                        warn!("Failed to log watchdog queue event: {}", e);
                    }
                    queued += 1;
                }
                Err(e) => {
                    warn!("Failed to queue track audio repair for {}: {}", track_id, e);
                }
            }
        }

        // Queue missing album images
        for image_id in missing_album_images {
            if self.is_already_in_queue(DownloadContentType::AlbumImage, image_id)? {
                debug!("Album image {} already in queue, skipping", image_id);
                skipped += 1;
                continue;
            }

            let queue_item = self.create_watchdog_queue_item(
                DownloadContentType::AlbumImage,
                image_id.clone(),
                "missing_album_image",
            );

            match self.queue_store.enqueue(queue_item.clone()) {
                Ok(_) => {
                    if let Err(e) = self
                        .audit_logger
                        .log_watchdog_queued(&queue_item, "missing_album_image")
                    {
                        warn!("Failed to log watchdog queue event: {}", e);
                    }
                    queued += 1;
                }
                Err(e) => {
                    warn!("Failed to queue album image repair for {}: {}", image_id, e);
                }
            }
        }

        // Queue missing artist images
        for image_id in missing_artist_images {
            if self.is_already_in_queue(DownloadContentType::ArtistImage, image_id)? {
                debug!("Artist image {} already in queue, skipping", image_id);
                skipped += 1;
                continue;
            }

            let queue_item = self.create_watchdog_queue_item(
                DownloadContentType::ArtistImage,
                image_id.clone(),
                "missing_artist_image",
            );

            match self.queue_store.enqueue(queue_item.clone()) {
                Ok(_) => {
                    if let Err(e) = self
                        .audit_logger
                        .log_watchdog_queued(&queue_item, "missing_artist_image")
                    {
                        warn!("Failed to log watchdog queue event: {}", e);
                    }
                    queued += 1;
                }
                Err(e) => {
                    warn!(
                        "Failed to queue artist image repair for {}: {}",
                        image_id, e
                    );
                }
            }
        }

        Ok((queued, skipped))
    }

    /// Check if an item is already in the download queue.
    fn is_already_in_queue(
        &self,
        content_type: DownloadContentType,
        content_id: &str,
    ) -> Result<bool> {
        self.queue_store.is_in_queue(content_type, content_id)
    }

    /// Create a queue item for watchdog-initiated repair.
    fn create_watchdog_queue_item(
        &self,
        content_type: DownloadContentType,
        content_id: String,
        _reason: &str,
    ) -> QueueItem {
        let now = chrono::Utc::now().timestamp();
        QueueItem {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: None,
            status: QueueStatus::Pending,
            priority: QueuePriority::Background,
            content_type,
            content_id,
            content_name: None,
            artist_name: None,
            request_source: RequestSource::Watchdog,
            requested_by_user_id: None,
            created_at: now,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::SearchableItem;
    use crate::download_manager::SqliteDownloadQueueStore;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Mock CatalogStore for testing
    struct MockCatalogStore {
        track_ids: Vec<String>,
        album_image_ids: Vec<String>,
        artist_image_ids: Vec<String>,
        existing_audio_paths: Vec<String>,
        media_path: PathBuf,
    }

    impl MockCatalogStore {
        fn new(media_path: PathBuf) -> Self {
            Self {
                track_ids: vec![],
                album_image_ids: vec![],
                artist_image_ids: vec![],
                existing_audio_paths: vec![],
                media_path,
            }
        }

        fn with_tracks(mut self, tracks: Vec<&str>) -> Self {
            self.track_ids = tracks.iter().map(|s| s.to_string()).collect();
            self
        }

        fn with_album_images(mut self, images: Vec<&str>) -> Self {
            self.album_image_ids = images.iter().map(|s| s.to_string()).collect();
            self
        }

        fn with_artist_images(mut self, images: Vec<&str>) -> Self {
            self.artist_image_ids = images.iter().map(|s| s.to_string()).collect();
            self
        }

        #[allow(dead_code)]
        fn with_existing_audio(mut self, tracks: Vec<&str>) -> Self {
            self.existing_audio_paths = tracks.iter().map(|s| s.to_string()).collect();
            self
        }
    }

    impl CatalogStore for MockCatalogStore {
        fn get_artist_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
            Ok(None)
        }

        fn get_album_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
            Ok(None)
        }

        fn get_track_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
            Ok(None)
        }

        fn get_track(&self, _id: &str) -> Result<Option<crate::catalog_store::Track>> {
            Ok(None)
        }

        fn get_resolved_artist_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
            Ok(None)
        }

        fn get_resolved_album_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
            Ok(None)
        }

        fn get_resolved_track_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
            Ok(None)
        }

        fn get_artist_discography_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
            Ok(None)
        }

        fn get_image_path(&self, id: &str) -> PathBuf {
            self.media_path.join("images").join(format!("{}.jpg", id))
        }

        fn get_track_audio_path(&self, track_id: &str) -> Option<PathBuf> {
            if self.existing_audio_paths.contains(&track_id.to_string()) {
                Some(
                    self.media_path
                        .join("audio")
                        .join(format!("{}.ogg", track_id)),
                )
            } else {
                // Return a path that won't exist
                Some(
                    self.media_path
                        .join("audio")
                        .join(format!("{}.ogg", track_id)),
                )
            }
        }

        fn get_track_album_id(&self, _track_id: &str) -> Option<String> {
            None
        }

        fn get_artists_count(&self) -> usize {
            0
        }

        fn get_albums_count(&self) -> usize {
            0
        }

        fn get_tracks_count(&self) -> usize {
            self.track_ids.len()
        }

        fn get_searchable_content(&self) -> Result<Vec<SearchableItem>> {
            Ok(vec![])
        }

        fn create_artist(&self, _data: serde_json::Value) -> Result<serde_json::Value> {
            unimplemented!()
        }

        fn update_artist(&self, _id: &str, _data: serde_json::Value) -> Result<serde_json::Value> {
            unimplemented!()
        }

        fn delete_artist(&self, _id: &str) -> Result<()> {
            unimplemented!()
        }

        fn create_album(&self, _data: serde_json::Value) -> Result<serde_json::Value> {
            unimplemented!()
        }

        fn update_album(&self, _id: &str, _data: serde_json::Value) -> Result<serde_json::Value> {
            unimplemented!()
        }

        fn delete_album(&self, _id: &str) -> Result<()> {
            unimplemented!()
        }

        fn create_track(&self, _data: serde_json::Value) -> Result<serde_json::Value> {
            unimplemented!()
        }

        fn update_track(&self, _id: &str, _data: serde_json::Value) -> Result<serde_json::Value> {
            unimplemented!()
        }

        fn delete_track(&self, _id: &str) -> Result<()> {
            unimplemented!()
        }

        fn create_image(&self, _data: serde_json::Value) -> Result<serde_json::Value> {
            unimplemented!()
        }

        fn update_image(&self, _id: &str, _data: serde_json::Value) -> Result<serde_json::Value> {
            unimplemented!()
        }

        fn delete_image(&self, _id: &str) -> Result<()> {
            unimplemented!()
        }

        fn create_changelog_batch(
            &self,
            _name: &str,
            _description: Option<&str>,
        ) -> Result<crate::catalog_store::CatalogBatch> {
            unimplemented!()
        }

        fn get_changelog_batch(
            &self,
            _id: &str,
        ) -> Result<Option<crate::catalog_store::CatalogBatch>> {
            unimplemented!()
        }

        fn get_active_changelog_batch(&self) -> Result<Option<crate::catalog_store::CatalogBatch>> {
            unimplemented!()
        }

        fn close_changelog_batch(&self, _id: &str) -> Result<()> {
            unimplemented!()
        }

        fn list_changelog_batches(
            &self,
            _is_open: Option<bool>,
        ) -> Result<Vec<crate::catalog_store::CatalogBatch>> {
            unimplemented!()
        }

        fn delete_changelog_batch(&self, _id: &str) -> Result<()> {
            unimplemented!()
        }

        fn get_changelog_batch_changes(
            &self,
            _batch_id: &str,
        ) -> Result<Vec<crate::catalog_store::ChangeEntry>> {
            unimplemented!()
        }

        fn get_changelog_entity_history(
            &self,
            _entity_type: crate::catalog_store::ChangeEntityType,
            _entity_id: &str,
        ) -> Result<Vec<crate::catalog_store::ChangeEntry>> {
            unimplemented!()
        }

        fn get_whats_new_batches(
            &self,
            _limit: usize,
        ) -> Result<Vec<crate::catalog_store::WhatsNewBatch>> {
            unimplemented!()
        }

        fn get_stale_batches(
            &self,
            _stale_threshold_hours: u64,
        ) -> Result<Vec<crate::catalog_store::CatalogBatch>> {
            unimplemented!()
        }

        fn close_stale_batches(&self) -> Result<usize> {
            unimplemented!()
        }

        fn get_changelog_batch_summary(
            &self,
            _batch_id: &str,
        ) -> Result<crate::catalog_store::BatchChangeSummary> {
            unimplemented!()
        }

        fn list_all_track_ids(&self) -> Result<Vec<String>> {
            Ok(self.track_ids.clone())
        }

        fn list_all_album_image_ids(&self) -> Result<Vec<String>> {
            Ok(self.album_image_ids.clone())
        }

        fn list_all_artist_image_ids(&self) -> Result<Vec<String>> {
            Ok(self.artist_image_ids.clone())
        }

        fn get_artists_without_related(&self) -> Result<Vec<String>> {
            Ok(Vec::new())
        }

        fn get_orphan_related_artist_ids(&self) -> Result<Vec<String>> {
            Ok(Vec::new())
        }

        fn add_artist_image(
            &self,
            _artist_id: &str,
            _image_id: &str,
            _image_type: &crate::catalog_store::ImageType,
            _position: i32,
        ) -> Result<()> {
            unimplemented!()
        }

        fn add_album_image(
            &self,
            _album_id: &str,
            _image_id: &str,
            _image_type: &crate::catalog_store::ImageType,
            _position: i32,
        ) -> Result<()> {
            unimplemented!()
        }

        fn set_artist_display_image(&self, _artist_id: &str, _image_id: &str) -> Result<()> {
            unimplemented!()
        }

        fn set_album_display_image(&self, _album_id: &str, _image_id: &str) -> Result<()> {
            unimplemented!()
        }

        fn get_album_display_image_id(&self, _album_id: &str) -> Result<Option<String>> {
            Ok(None)
        }

        fn get_skeleton_version(&self) -> Result<i64> {
            Ok(0)
        }

        fn get_skeleton_checksum(&self) -> Result<String> {
            Ok("sha256:mock".to_string())
        }

        fn get_skeleton_events_since(
            &self,
            _seq: i64,
        ) -> Result<Vec<crate::skeleton::SkeletonEvent>> {
            Ok(Vec::new())
        }

        fn get_skeleton_earliest_seq(&self) -> Result<i64> {
            Ok(0)
        }

        fn get_skeleton_latest_seq(&self) -> Result<i64> {
            Ok(0)
        }

        fn get_all_artist_ids(&self) -> Result<Vec<String>> {
            Ok(Vec::new())
        }

        fn get_all_albums_skeleton(&self) -> Result<Vec<crate::skeleton::SkeletonAlbumEntry>> {
            Ok(Vec::new())
        }

        fn get_all_tracks_skeleton(&self) -> Result<Vec<crate::skeleton::SkeletonTrackEntry>> {
            Ok(Vec::new())
        }
    }

    fn create_test_watchdog(
        catalog_store: Arc<dyn CatalogStore>,
    ) -> (MissingFilesWatchdog, Arc<SqliteDownloadQueueStore>) {
        let queue_store = Arc::new(SqliteDownloadQueueStore::in_memory().unwrap());
        let audit_logger = AuditLogger::new(queue_store.clone());

        let watchdog = MissingFilesWatchdog::new(catalog_store, queue_store.clone(), audit_logger);

        (watchdog, queue_store)
    }

    #[test]
    fn test_scan_empty_catalog() {
        let temp_dir = TempDir::new().unwrap();
        let media_path = temp_dir.path().to_path_buf();

        let catalog = Arc::new(MockCatalogStore::new(media_path));
        let (watchdog, _) = create_test_watchdog(catalog);

        let report = watchdog.run_scan(MissingFilesMode::Actual).unwrap();

        assert!(report.missing_track_audio.is_empty());
        assert!(report.missing_album_images.is_empty());
        assert!(report.missing_artist_images.is_empty());
        assert_eq!(report.items_queued, 0);
        assert_eq!(report.items_skipped, 0);
        assert!(report.is_clean());
    }

    #[test]
    fn test_scan_finds_missing_track_audio() {
        let temp_dir = TempDir::new().unwrap();
        let media_path = temp_dir.path().to_path_buf();

        // Create catalog with tracks but no audio files
        let catalog = Arc::new(
            MockCatalogStore::new(media_path.clone())
                .with_tracks(vec!["track1", "track2", "track3"]),
        );

        let (watchdog, queue_store) = create_test_watchdog(catalog);

        let report = watchdog.run_scan(MissingFilesMode::Actual).unwrap();

        assert_eq!(report.missing_track_audio.len(), 3);
        assert!(report.missing_track_audio.contains(&"track1".to_string()));
        assert!(report.missing_track_audio.contains(&"track2".to_string()));
        assert!(report.missing_track_audio.contains(&"track3".to_string()));

        // Should have queued 3 items
        assert_eq!(report.items_queued, 3);
        assert_eq!(report.items_skipped, 0);

        // Verify queue contains the items
        let status = queue_store.get_queue_stats().unwrap();
        assert_eq!(status.pending, 3);
    }

    #[test]
    fn test_scan_dry_run_does_not_queue() {
        let temp_dir = TempDir::new().unwrap();
        let media_path = temp_dir.path().to_path_buf();

        // Create catalog with tracks but no audio files
        let catalog = Arc::new(
            MockCatalogStore::new(media_path.clone())
                .with_tracks(vec!["track1", "track2", "track3"]),
        );

        let (watchdog, queue_store) = create_test_watchdog(catalog);

        // Run in dry-run mode
        let report = watchdog.run_scan(MissingFilesMode::DryRun).unwrap();

        assert_eq!(report.missing_track_audio.len(), 3);
        assert_eq!(report.mode, MissingFilesMode::DryRun);

        // Should NOT have queued any items in dry-run mode
        assert_eq!(report.items_queued, 0);
        assert_eq!(report.items_skipped, 0);

        // Verify queue is empty
        let status = queue_store.get_queue_stats().unwrap();
        assert_eq!(status.pending, 0);
    }

    #[test]
    fn test_scan_finds_missing_album_images() {
        let temp_dir = TempDir::new().unwrap();
        let media_path = temp_dir.path().to_path_buf();

        // Create images directory
        fs::create_dir_all(media_path.join("images")).unwrap();

        // Create one existing image
        fs::write(media_path.join("images").join("img1.jpg"), b"image data").unwrap();

        let catalog = Arc::new(
            MockCatalogStore::new(media_path.clone())
                .with_album_images(vec!["img1", "img2", "img3"]),
        );

        let (watchdog, _) = create_test_watchdog(catalog);

        let report = watchdog.run_scan(MissingFilesMode::Actual).unwrap();

        // img1 exists, img2 and img3 are missing
        assert_eq!(report.missing_album_images.len(), 2);
        assert!(report.missing_album_images.contains(&"img2".to_string()));
        assert!(report.missing_album_images.contains(&"img3".to_string()));
        assert!(!report.missing_album_images.contains(&"img1".to_string()));

        assert_eq!(report.items_queued, 2);
    }

    #[test]
    fn test_scan_finds_missing_artist_images() {
        let temp_dir = TempDir::new().unwrap();
        let media_path = temp_dir.path().to_path_buf();

        let catalog = Arc::new(
            MockCatalogStore::new(media_path.clone())
                .with_artist_images(vec!["artist_img1", "artist_img2"]),
        );

        let (watchdog, _) = create_test_watchdog(catalog);

        let report = watchdog.run_scan(MissingFilesMode::Actual).unwrap();

        assert_eq!(report.missing_artist_images.len(), 2);
        assert_eq!(report.items_queued, 2);
    }

    #[test]
    fn test_scan_skips_already_queued_items() {
        let temp_dir = TempDir::new().unwrap();
        let media_path = temp_dir.path().to_path_buf();

        let catalog = Arc::new(
            MockCatalogStore::new(media_path.clone()).with_tracks(vec!["track1", "track2"]),
        );

        let (watchdog, queue_store) = create_test_watchdog(catalog);

        // First scan - should queue both
        let report1 = watchdog.run_scan(MissingFilesMode::Actual).unwrap();
        assert_eq!(report1.items_queued, 2);
        assert_eq!(report1.items_skipped, 0);

        // Second scan - should skip both since they're already in queue
        let report2 = watchdog.run_scan(MissingFilesMode::Actual).unwrap();
        assert_eq!(report2.items_queued, 0);
        assert_eq!(report2.items_skipped, 2);

        // Queue should still have 2 items
        let status = queue_store.get_queue_stats().unwrap();
        assert_eq!(status.pending, 2);
    }

    #[test]
    fn test_queued_items_have_watchdog_priority() {
        let temp_dir = TempDir::new().unwrap();
        let media_path = temp_dir.path().to_path_buf();

        let catalog =
            Arc::new(MockCatalogStore::new(media_path.clone()).with_tracks(vec!["track1"]));

        let (watchdog, queue_store) = create_test_watchdog(catalog);

        watchdog.run_scan(MissingFilesMode::Actual).unwrap();

        // Check that the queued item has Watchdog priority
        let pending = queue_store
            .list_all(Some(QueueStatus::Pending), false, false, 10, 0)
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].priority, QueuePriority::Background);
        assert_eq!(pending[0].request_source, RequestSource::Watchdog);
    }

    #[test]
    fn test_scan_duration_recorded() {
        let temp_dir = TempDir::new().unwrap();
        let media_path = temp_dir.path().to_path_buf();

        let catalog = Arc::new(MockCatalogStore::new(media_path.clone()));
        let (watchdog, _) = create_test_watchdog(catalog);

        let report = watchdog.run_scan(MissingFilesMode::Actual).unwrap();

        // Duration should be recorded (might be 0 for very fast scans)
        assert!(report.scan_duration_ms >= 0);
    }

    #[test]
    fn test_total_missing_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let media_path = temp_dir.path().to_path_buf();

        let catalog = Arc::new(
            MockCatalogStore::new(media_path.clone())
                .with_tracks(vec!["t1", "t2"])
                .with_album_images(vec!["a1"])
                .with_artist_images(vec!["ar1", "ar2", "ar3"]),
        );

        let (watchdog, _) = create_test_watchdog(catalog);

        let report = watchdog.run_scan(MissingFilesMode::Actual).unwrap();

        assert_eq!(report.total_missing(), 6); // 2 + 1 + 3
        assert!(!report.is_clean());
    }

    #[test]
    fn test_report_includes_scan_totals() {
        let temp_dir = TempDir::new().unwrap();
        let media_path = temp_dir.path().to_path_buf();

        let catalog = Arc::new(
            MockCatalogStore::new(media_path.clone())
                .with_tracks(vec!["t1", "t2", "t3"])
                .with_album_images(vec!["a1", "a2"])
                .with_artist_images(vec!["ar1"]),
        );

        let (watchdog, _) = create_test_watchdog(catalog);

        let report = watchdog.run_scan(MissingFilesMode::DryRun).unwrap();

        // Report should include totals scanned
        assert_eq!(report.total_tracks_scanned, 3);
        assert_eq!(report.total_album_images_scanned, 2);
        assert_eq!(report.total_artist_images_scanned, 1);
    }
}
