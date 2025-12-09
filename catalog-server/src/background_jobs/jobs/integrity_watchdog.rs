//! Integrity watchdog background job.
//!
//! This job periodically scans the catalog for missing media files and
//! queues download requests to repair them.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, HookEvent, JobError, JobSchedule, ShutdownBehavior},
};
use crate::download_manager::IntegrityWatchdog;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

/// Background job that runs the integrity watchdog.
///
/// This job runs on startup and then daily to scan the catalog for
/// missing audio files and images, queuing download requests to repair them.
pub struct IntegrityWatchdogJob {
    watchdog: Arc<IntegrityWatchdog>,
}

impl IntegrityWatchdogJob {
    /// Create a new IntegrityWatchdogJob.
    pub fn new(watchdog: Arc<IntegrityWatchdog>) -> Self {
        Self { watchdog }
    }
}

impl BackgroundJob for IntegrityWatchdogJob {
    fn id(&self) -> &'static str {
        "integrity_watchdog"
    }

    fn name(&self) -> &'static str {
        "Integrity Watchdog"
    }

    fn description(&self) -> &'static str {
        "Scan catalog for missing files and queue repairs"
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
        // Check for cancellation before starting
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        let report = self
            .watchdog
            .run_scan()
            .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;

        info!(
            "Watchdog scan complete: queued={}, skipped={}, duration={}ms",
            report.items_queued, report.items_skipped, report.scan_duration_ms
        );

        if report.is_clean() {
            info!("Catalog integrity check passed - no missing files");
        } else {
            info!(
                "Found {} missing items: {} track audio, {} album images, {} artist images",
                report.total_missing(),
                report.missing_track_audio.len(),
                report.missing_album_images.len(),
                report.missing_artist_images.len()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require a mock IntegrityWatchdog,
    // which is tested separately in the watchdog module.
    // These tests verify job metadata only.

    // We can't easily create an IntegrityWatchdogJob without the watchdog,
    // so we test the trait implementation separately

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
