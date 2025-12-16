//! Expand Artists Base background job.
//!
//! This job scans the catalog for artists that need enrichment:
//! 1. Artists without related artists populated
//! 2. Orphan related artist IDs (referenced but not in catalog)
//!
//! It supports two modes:
//! - DryRun (default): Reports what would be queued without making changes
//! - Actual: Queues download requests for missing artist data

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule},
    JobAuditLogger,
};
use crate::catalog_store::CatalogStore;
use crate::download_manager::{
    DownloadContentType, DownloadQueueStore, QueueItem, QueuePriority, QueueStatus, RequestSource,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

/// Execution mode for the job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExpandArtistsMode {
    /// Report what would be done without making changes.
    #[default]
    DryRun,
    /// Actually queue download requests.
    Actual,
}

/// Report from the expand artists scan.
#[derive(Debug, Clone, Serialize)]
pub struct ExpandArtistsReport {
    /// The mode that was used for this run.
    pub mode: ExpandArtistsMode,
    /// Artist IDs that have no related artists populated.
    pub artists_without_related: Vec<String>,
    /// Related artist IDs that don't exist in the catalog.
    pub orphan_related_artist_ids: Vec<String>,
    /// Number of items queued (0 in dry-run mode).
    pub items_queued: usize,
    /// Number of items skipped (already in queue).
    pub items_skipped: usize,
    /// Duration of the scan in milliseconds.
    pub scan_duration_ms: i64,
}

impl ExpandArtistsReport {
    /// Total number of artists needing enrichment.
    pub fn total_enrichment_needed(&self) -> usize {
        self.artists_without_related.len() + self.orphan_related_artist_ids.len()
    }

    /// Returns true if no enrichment is needed.
    pub fn is_clean(&self) -> bool {
        self.artists_without_related.is_empty() && self.orphan_related_artist_ids.is_empty()
    }
}

/// Background job that scans for missing artist data and queues enrichment downloads.
///
/// This job is manual-only (no automatic schedule) and supports two modes:
/// - DryRun: Reports what would be queued
/// - Actual: Queues download requests for missing artist data
pub struct ExpandArtistsBaseJob {
    catalog_store: Arc<dyn CatalogStore>,
    queue_store: Arc<dyn DownloadQueueStore>,
}

impl ExpandArtistsBaseJob {
    /// Create a new ExpandArtistsBaseJob.
    pub fn new(
        catalog_store: Arc<dyn CatalogStore>,
        queue_store: Arc<dyn DownloadQueueStore>,
    ) -> Self {
        Self {
            catalog_store,
            queue_store,
        }
    }

    /// Parse the mode from job parameters.
    fn parse_mode(params: Option<JsonValue>) -> ExpandArtistsMode {
        params
            .and_then(|p| p.get("mode").cloned())
            .and_then(|m| serde_json::from_value(m).ok())
            .unwrap_or_default()
    }

    /// Run the scan with the specified mode.
    fn run_scan(&self, mode: ExpandArtistsMode) -> anyhow::Result<ExpandArtistsReport> {
        let start = Instant::now();

        // Scan for artists needing enrichment
        let artists_without_related = self.catalog_store.get_artists_without_related()?;
        let orphan_related_artist_ids = self.catalog_store.get_orphan_related_artist_ids()?;

        info!(
            "Expand artists scan found {} artists without related, {} orphan related artist IDs",
            artists_without_related.len(),
            orphan_related_artist_ids.len()
        );

        // Queue enrichment items if in actual mode
        let (items_queued, items_skipped) = match mode {
            ExpandArtistsMode::DryRun => {
                info!(
                    "Dry-run mode: would queue {} items",
                    artists_without_related.len() + orphan_related_artist_ids.len()
                );
                (0, 0)
            }
            ExpandArtistsMode::Actual => {
                self.queue_artist_enrichment(&artists_without_related, &orphan_related_artist_ids)?
            }
        };

        let scan_duration_ms = start.elapsed().as_millis() as i64;

        Ok(ExpandArtistsReport {
            mode,
            artists_without_related,
            orphan_related_artist_ids,
            items_queued,
            items_skipped,
            scan_duration_ms,
        })
    }

    /// Queue artist enrichment items.
    ///
    /// Returns (items_queued, items_skipped).
    fn queue_artist_enrichment(
        &self,
        artists_without_related: &[String],
        orphan_related_artist_ids: &[String],
    ) -> anyhow::Result<(usize, usize)> {
        let mut queued = 0;
        let mut skipped = 0;

        // Queue artists without related artists
        for artist_id in artists_without_related {
            if self.is_already_in_queue(DownloadContentType::ArtistRelated, artist_id)? {
                info!("Artist related {} already in queue, skipping", artist_id);
                skipped += 1;
                continue;
            }

            let queue_item =
                self.create_queue_item(DownloadContentType::ArtistRelated, artist_id.clone());

            match self.queue_store.enqueue(queue_item) {
                Ok(_) => {
                    queued += 1;
                }
                Err(e) => {
                    warn!(
                        "Failed to queue artist related fetch for {}: {}",
                        artist_id, e
                    );
                }
            }
        }

        // Queue orphan related artist IDs (need full metadata)
        for artist_id in orphan_related_artist_ids {
            if self.is_already_in_queue(DownloadContentType::ArtistMetadata, artist_id)? {
                info!("Artist metadata {} already in queue, skipping", artist_id);
                skipped += 1;
                continue;
            }

            let queue_item =
                self.create_queue_item(DownloadContentType::ArtistMetadata, artist_id.clone());

            match self.queue_store.enqueue(queue_item) {
                Ok(_) => {
                    queued += 1;
                }
                Err(e) => {
                    warn!(
                        "Failed to queue artist metadata fetch for {}: {}",
                        artist_id, e
                    );
                }
            }
        }

        Ok((queued, skipped))
    }

    /// Check if an item is already in the active download queue (pending/in-progress).
    /// Completed items are not considered "in queue" so they can be retried.
    fn is_already_in_queue(
        &self,
        content_type: DownloadContentType,
        content_id: &str,
    ) -> anyhow::Result<bool> {
        self.queue_store
            .is_in_active_queue(content_type, content_id)
    }

    /// Create a queue item for expand artists job.
    fn create_queue_item(
        &self,
        content_type: DownloadContentType,
        content_id: String,
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
            request_source: RequestSource::Expansion,
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

impl BackgroundJob for ExpandArtistsBaseJob {
    fn id(&self) -> &'static str {
        "expand_artists_base"
    }

    fn name(&self) -> &'static str {
        "Expand Artists Base"
    }

    fn description(&self) -> &'static str {
        "Scan for missing artist data and queue enrichment downloads"
    }

    fn schedule(&self) -> JobSchedule {
        // Manual-only: no automatic runs
        JobSchedule::Combined {
            cron: None,
            interval: None,
            hooks: vec![],
        }
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        // Default execution with dry-run mode
        self.execute_with_params(ctx, None)
    }

    fn execute_with_params(
        &self,
        ctx: &JobContext,
        params: Option<JsonValue>,
    ) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(Arc::clone(&ctx.server_store), self.id());

        // Check for cancellation before starting
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        let mode = Self::parse_mode(params);
        info!("Starting expand artists base job in {:?} mode", mode);

        audit.log_started(Some(serde_json::json!({ "mode": mode })));

        let report = match self.run_scan(mode) {
            Ok(report) => report,
            Err(e) => {
                audit.log_failed(&e.to_string(), None);
                return Err(JobError::ExecutionFailed(e.to_string()));
            }
        };

        info!(
            "Expand artists scan complete: mode={:?}, queued={}, skipped={}, duration={}ms",
            mode, report.items_queued, report.items_skipped, report.scan_duration_ms
        );

        if report.is_clean() {
            info!("No artist enrichment needed - catalog is complete");
        } else {
            info!(
                "Found {} items needing enrichment: {} artists without related, {} orphan related artists",
                report.total_enrichment_needed(),
                report.artists_without_related.len(),
                report.orphan_related_artist_ids.len()
            );
        }

        // Log completion with detailed results
        let details = serde_json::json!({
            "mode": mode,
            "artists_without_related_count": report.artists_without_related.len(),
            "orphan_related_artist_ids_count": report.orphan_related_artist_ids.len(),
            "total_enrichment_needed": report.total_enrichment_needed(),
            "items_queued": report.items_queued,
            "items_skipped": report.items_skipped,
            "is_clean": report.is_clean(),
            "scan_duration_ms": report.scan_duration_ms,
        });
        audit.log_completed(Some(details));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mode_default() {
        let mode = ExpandArtistsBaseJob::parse_mode(None);
        assert_eq!(mode, ExpandArtistsMode::DryRun);
    }

    #[test]
    fn test_parse_mode_dry_run() {
        let params = serde_json::json!({ "mode": "dry_run" });
        let mode = ExpandArtistsBaseJob::parse_mode(Some(params));
        assert_eq!(mode, ExpandArtistsMode::DryRun);
    }

    #[test]
    fn test_parse_mode_actual() {
        let params = serde_json::json!({ "mode": "actual" });
        let mode = ExpandArtistsBaseJob::parse_mode(Some(params));
        assert_eq!(mode, ExpandArtistsMode::Actual);
    }

    #[test]
    fn test_parse_mode_invalid_fallback() {
        let params = serde_json::json!({ "mode": "invalid" });
        let mode = ExpandArtistsBaseJob::parse_mode(Some(params));
        assert_eq!(mode, ExpandArtistsMode::DryRun);
    }

    #[test]
    fn test_expand_artists_report_is_clean() {
        let report = ExpandArtistsReport {
            mode: ExpandArtistsMode::DryRun,
            artists_without_related: vec![],
            orphan_related_artist_ids: vec![],
            items_queued: 0,
            items_skipped: 0,
            scan_duration_ms: 100,
        };
        assert!(report.is_clean());
        assert_eq!(report.total_enrichment_needed(), 0);
    }

    #[test]
    fn test_expand_artists_report_not_clean() {
        let report = ExpandArtistsReport {
            mode: ExpandArtistsMode::Actual,
            artists_without_related: vec!["artist1".to_string()],
            orphan_related_artist_ids: vec!["artist2".to_string(), "artist3".to_string()],
            items_queued: 3,
            items_skipped: 0,
            scan_duration_ms: 200,
        };
        assert!(!report.is_clean());
        assert_eq!(report.total_enrichment_needed(), 3);
    }

    #[test]
    fn test_expand_artists_mode_serialization() {
        let dry_run = serde_json::to_string(&ExpandArtistsMode::DryRun).unwrap();
        assert_eq!(dry_run, "\"dry_run\"");

        let actual = serde_json::to_string(&ExpandArtistsMode::Actual).unwrap();
        assert_eq!(actual, "\"actual\"");
    }
}
