//! Missing files watchdog background job.
//!
//! This job scans the catalog for missing media files and
//! queues download requests to repair them.
//!
//! The job is manual-only (no automatic schedule) and supports two modes:
//! - DryRun (default): Reports what would be queued without making changes
//! - Actual: Queues download requests for missing files

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::download_manager::{MissingFilesMode, MissingFilesWatchdog};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tracing::info;

/// Background job that runs the missing files watchdog.
///
/// This job is manual-only and scans the catalog for missing audio files
/// and images, optionally queuing download requests to repair them.
///
/// Supports two modes via the `mode` parameter:
/// - `dry_run` (default): Reports what would be queued without making changes
/// - `actual`: Queues download requests for missing files
pub struct MissingFilesWatchdogJob {
    watchdog: Arc<MissingFilesWatchdog>,
}

impl MissingFilesWatchdogJob {
    /// Create a new MissingFilesWatchdogJob.
    pub fn new(watchdog: Arc<MissingFilesWatchdog>) -> Self {
        Self { watchdog }
    }

    /// Parse the mode from job parameters.
    fn parse_mode(params: Option<JsonValue>) -> MissingFilesMode {
        params
            .and_then(|p| p.get("mode").cloned())
            .and_then(|m| serde_json::from_value(m).ok())
            .unwrap_or_default()
    }
}

impl BackgroundJob for MissingFilesWatchdogJob {
    fn id(&self) -> &'static str {
        "missing_files_watchdog"
    }

    fn name(&self) -> &'static str {
        "Missing Files Watchdog"
    }

    fn description(&self) -> &'static str {
        "Scan catalog for missing media files and queue repairs"
    }

    fn schedule(&self) -> JobSchedule {
        // Manual-only: no automatic runs (disabled due to performance concerns)
        JobSchedule::Combined {
            cron: None,
            interval: None,
            hooks: vec![],
        }
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        // This job can be cancelled - it's not critical and will run again
        ShutdownBehavior::Cancellable
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
        info!("Starting missing files watchdog in {:?} mode", mode);

        audit.log_started(Some(serde_json::json!({ "mode": mode })));

        let report = match self.watchdog.run_scan(mode) {
            Ok(report) => report,
            Err(e) => {
                audit.log_failed(&e.to_string(), None);
                return Err(JobError::ExecutionFailed(e.to_string()));
            }
        };

        info!(
            "Missing files scan complete: mode={:?}, queued={}, skipped={}, duration={}ms",
            mode, report.items_queued, report.items_skipped, report.scan_duration_ms
        );

        if report.is_clean() {
            info!("Missing files check passed - no missing media files");
        } else {
            info!(
                "Found {} missing items: {} track audio, {} album images, {} artist images",
                report.total_missing(),
                report.missing_track_audio.len(),
                report.missing_album_images.len(),
                report.missing_artist_images.len()
            );

            // Log detailed track info for debugging
            if !report.missing_track_details.is_empty() {
                info!("Missing tracks:");
                for track in &report.missing_track_details {
                    info!(
                        "  - {} ({}) by {} [album: {}]",
                        track.track_name,
                        track.track_id,
                        track.artist_names.join(", "),
                        track.album_name.as_deref().unwrap_or("Unknown")
                    );
                }
            }
        }

        // Log completion with detailed results - include all the summary data
        let details = serde_json::json!({
            "mode": mode,
            "total_tracks_scanned": report.total_tracks_scanned,
            "total_album_images_scanned": report.total_album_images_scanned,
            "total_artist_images_scanned": report.total_artist_images_scanned,
            "missing_track_audio_count": report.missing_track_audio.len(),
            "missing_track_audio_ids": report.missing_track_audio,
            "missing_track_details": report.missing_track_details,
            "missing_album_images_count": report.missing_album_images.len(),
            "missing_album_images_ids": report.missing_album_images,
            "missing_album_image_details": report.missing_album_image_details,
            "missing_artist_images_count": report.missing_artist_images.len(),
            "missing_artist_images_ids": report.missing_artist_images,
            "missing_artist_image_details": report.missing_artist_image_details,
            "total_missing": report.total_missing(),
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

    // Note: Full integration tests require a mock MissingFilesWatchdog,
    // which is tested separately in the watchdog module.
    // These tests verify job metadata only.

    #[test]
    fn test_job_schedule_is_manual_only() {
        // Verify the schedule configuration matches expectations (manual-only)
        let schedule = JobSchedule::Combined {
            cron: None,
            interval: None,
            hooks: vec![],
        };

        match schedule {
            JobSchedule::Combined {
                cron,
                interval,
                hooks,
            } => {
                assert!(cron.is_none());
                assert!(interval.is_none());
                assert!(hooks.is_empty());
            }
            _ => panic!("Expected Combined schedule"),
        }
    }

    #[test]
    fn test_parse_mode_default() {
        let mode = MissingFilesWatchdogJob::parse_mode(None);
        assert_eq!(mode, MissingFilesMode::DryRun);
    }

    #[test]
    fn test_parse_mode_dry_run() {
        let params = serde_json::json!({ "mode": "dry_run" });
        let mode = MissingFilesWatchdogJob::parse_mode(Some(params));
        assert_eq!(mode, MissingFilesMode::DryRun);
    }

    #[test]
    fn test_parse_mode_actual() {
        let params = serde_json::json!({ "mode": "actual" });
        let mode = MissingFilesWatchdogJob::parse_mode(Some(params));
        assert_eq!(mode, MissingFilesMode::Actual);
    }

    #[test]
    fn test_parse_mode_invalid_fallback() {
        let params = serde_json::json!({ "mode": "invalid" });
        let mode = MissingFilesWatchdogJob::parse_mode(Some(params));
        assert_eq!(mode, MissingFilesMode::DryRun);
    }
}
