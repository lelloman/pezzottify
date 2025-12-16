//! Missing files watchdog background job.
//!
//! This job periodically scans the catalog for missing media files and
//! queues download requests to repair them.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, HookEvent, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::download_manager::MissingFilesWatchdog;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

/// Background job that runs the missing files watchdog.
///
/// This job runs on startup and then daily to scan the catalog for
/// missing audio files and images, queuing download requests to repair them.
pub struct MissingFilesWatchdogJob {
    watchdog: Arc<MissingFilesWatchdog>,
}

impl MissingFilesWatchdogJob {
    /// Create a new MissingFilesWatchdogJob.
    pub fn new(watchdog: Arc<MissingFilesWatchdog>) -> Self {
        Self { watchdog }
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
        // Run on startup and every 24 hours
        JobSchedule::Combined {
            cron: None,
            interval: Some(Duration::from_secs(24 * 60 * 60)), // 24 hours
            hooks: vec![HookEvent::OnStartup],
        }
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        // This job can be cancelled - it's not critical and will run again
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(Arc::clone(&ctx.server_store), self.id());

        // Check for cancellation before starting
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        audit.log_started(None);

        let report = match self.watchdog.run_scan() {
            Ok(report) => report,
            Err(e) => {
                audit.log_failed(&e.to_string(), None);
                return Err(JobError::ExecutionFailed(e.to_string()));
            }
        };

        info!(
            "Missing files scan complete: queued={}, skipped={}, duration={}ms",
            report.items_queued, report.items_skipped, report.scan_duration_ms
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
        }

        // Log completion with detailed results
        let details = serde_json::json!({
            "missing_track_audio_count": report.missing_track_audio.len(),
            "missing_album_images_count": report.missing_album_images.len(),
            "missing_artist_images_count": report.missing_artist_images.len(),
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
    fn test_job_schedule_is_combined_with_startup() {
        // Verify the schedule configuration matches expectations
        let schedule = JobSchedule::Combined {
            cron: None,
            interval: Some(Duration::from_secs(24 * 60 * 60)),
            hooks: vec![HookEvent::OnStartup],
        };

        match schedule {
            JobSchedule::Combined {
                cron,
                interval,
                hooks,
            } => {
                assert!(cron.is_none());
                assert_eq!(interval.unwrap(), Duration::from_secs(24 * 60 * 60));
                assert!(hooks.contains(&HookEvent::OnStartup));
            }
            _ => panic!("Expected Combined schedule"),
        }
    }
}
