//! Missing files watchdog.
//!
//! Scans for missing audio files and queues repairs. The watchdog periodically
//! checks that all catalog tracks have corresponding audio files on disk,
//! and queues download requests for any missing content.

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use tracing::{debug, info, warn};

use crate::catalog_store::CatalogStore;

use super::audit_logger::AuditLogger;
use super::models::{
    DownloadContentType, MissingFilesMode, MissingFilesReport, MissingTrackInfo, QueueItem,
    QueuePriority, RequestSource,
};
use super::queue_store::DownloadQueueStore;

/// Missing files watchdog that scans catalog for missing audio files.
///
/// The watchdog:
/// 1. Scans all tracks to find those missing audio files
/// 2. Queues repair downloads for missing content (if not already queued)
///
/// Note: Only scans audio files (image scanning not implemented).
pub struct MissingFilesWatchdog {
    catalog_store: Arc<dyn CatalogStore>,
    queue_store: Arc<dyn DownloadQueueStore>,
    audit_logger: AuditLogger,
}

impl MissingFilesWatchdog {
    /// Create a new MissingFilesWatchdog.
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

    /// Run a full scan and queue repairs for missing audio files.
    ///
    /// Returns a report detailing what was found and what was queued.
    pub fn run_scan(&self, mode: MissingFilesMode) -> Result<MissingFilesReport> {
        let start = Instant::now();

        info!("Starting missing files scan in {:?} mode", mode);

        // Log scan start
        if let Err(e) = self.audit_logger.log_watchdog_scan_started() {
            warn!("Failed to log watchdog scan start: {}", e);
        }

        // Get all track IDs
        let all_track_ids = self.catalog_store.list_all_track_ids()?;
        let total_tracks_scanned = all_track_ids.len();

        // Scan for missing audio files
        let (missing_track_audio, missing_track_details) =
            self.scan_missing_track_audio_detailed(&all_track_ids)?;

        info!(
            "Missing files scan found {} tracks missing audio out of {} total",
            missing_track_audio.len(),
            total_tracks_scanned
        );

        // Queue repairs only if in Actual mode
        let (items_queued, items_skipped) = match mode {
            MissingFilesMode::DryRun => {
                info!(
                    "Dry-run mode: would queue {} items",
                    missing_track_audio.len()
                );
                (0, 0)
            }
            MissingFilesMode::Actual => self.queue_repairs(&missing_track_audio)?,
        };

        let scan_duration_ms = start.elapsed().as_millis() as i64;

        let report = MissingFilesReport {
            mode,
            total_tracks_scanned,
            total_album_images_scanned: 0,  // Not scanning images
            total_artist_images_scanned: 0, // Not scanning images
            missing_track_audio: missing_track_audio.clone(),
            missing_track_details,
            missing_album_images: vec![],
            missing_album_image_details: vec![],
            missing_artist_images: vec![],
            missing_artist_image_details: vec![],
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
    fn scan_missing_track_audio_detailed(
        &self,
        track_ids: &[String],
    ) -> Result<(Vec<String>, Vec<MissingTrackInfo>)> {
        let mut missing_ids = Vec::new();
        let mut missing_details = Vec::new();

        for track_id in track_ids {
            let is_missing =
                if let Some(audio_path) = self.catalog_store.get_track_audio_path(track_id) {
                    !audio_path.exists()
                } else {
                    // No audio_uri set = missing
                    true
                };

            if is_missing {
                missing_ids.push(track_id.clone());

                // Try to get track details for the report
                if let Ok(Some(track)) = self.catalog_store.get_track(track_id) {
                    missing_details.push(MissingTrackInfo {
                        track_id: track_id.clone(),
                        track_name: track.name,
                        album_id: Some(track.album_id),
                        album_name: None,     // Would need additional lookup
                        artist_names: vec![], // Would need additional lookup
                    });
                }
            }
        }

        debug!(
            "Scanned {} tracks, found {} missing audio files",
            track_ids.len(),
            missing_ids.len()
        );

        Ok((missing_ids, missing_details))
    }

    /// Queue repair downloads for missing content.
    ///
    /// Returns (items_queued, items_skipped).
    fn queue_repairs(&self, missing_track_ids: &[String]) -> Result<(usize, usize)> {
        let mut queued = 0;
        let mut skipped = 0;

        for track_id in missing_track_ids {
            // Check if already in active queue
            if self
                .queue_store
                .is_in_active_queue(DownloadContentType::TrackAudio, track_id)?
            {
                debug!("Track {} already in queue, skipping", track_id);
                skipped += 1;
                continue;
            }

            // Create queue item
            let item = QueueItem::new(
                uuid::Uuid::new_v4().to_string(),
                DownloadContentType::TrackAudio,
                track_id.clone(),
                QueuePriority::Background,
                RequestSource::Watchdog,
                3, // Lower max retries for watchdog items
            );

            // Enqueue
            if let Err(e) = self.queue_store.enqueue(item.clone()) {
                warn!("Failed to enqueue repair for track {}: {}", track_id, e);
                continue;
            }

            // Log audit event
            if let Err(e) = self
                .audit_logger
                .log_watchdog_queued(&item, "missing_audio")
            {
                warn!("Failed to log watchdog queue event: {}", e);
            }

            queued += 1;
        }

        info!(
            "Queued {} repair items, skipped {} already-queued items",
            queued, skipped
        );

        Ok((queued, skipped))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_files_mode_debug() {
        let mode = MissingFilesMode::DryRun;
        assert_eq!(format!("{:?}", mode), "DryRun");
    }
}
