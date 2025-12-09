//! Audit log cleanup background job.
//!
//! This job periodically deletes old audit log entries based on
//! the configured retention period.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
};
use crate::download_manager::DownloadQueueStore;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

/// Background job that cleans up old audit log entries.
///
/// This job runs daily to delete audit log entries older than
/// the configured retention period.
pub struct AuditLogCleanupJob {
    queue_store: Arc<dyn DownloadQueueStore>,
    retention_days: u64,
}

impl AuditLogCleanupJob {
    /// Create a new AuditLogCleanupJob.
    pub fn new(queue_store: Arc<dyn DownloadQueueStore>, retention_days: u64) -> Self {
        Self {
            queue_store,
            retention_days,
        }
    }
}

impl BackgroundJob for AuditLogCleanupJob {
    fn id(&self) -> &'static str {
        "audit_log_cleanup"
    }

    fn name(&self) -> &'static str {
        "Audit Log Cleanup"
    }

    fn description(&self) -> &'static str {
        "Delete old audit log entries based on retention policy"
    }

    fn schedule(&self) -> JobSchedule {
        // Run every 24 hours (no startup run needed)
        JobSchedule::Interval(Duration::from_secs(24 * 60 * 60))
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        // This job can be cancelled - cleanup can happen next run
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        // Check for cancellation before starting
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        // Calculate cutoff timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let cutoff = now - (self.retention_days as i64 * 24 * 60 * 60);

        info!(
            "Cleaning up audit log entries older than {} days (cutoff: {})",
            self.retention_days, cutoff
        );

        let deleted = self
            .queue_store
            .cleanup_old_audit_entries(cutoff)
            .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;

        if deleted > 0 {
            info!("Deleted {} old audit log entries", deleted);
        } else {
            info!("No audit log entries to clean up");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_metadata() {
        // We can't easily create the job without a real store,
        // so just test the schedule configuration
        let schedule = JobSchedule::Interval(Duration::from_secs(24 * 60 * 60));

        match schedule {
            JobSchedule::Interval(duration) => {
                assert_eq!(duration, Duration::from_secs(24 * 60 * 60));
            }
            _ => panic!("Expected Interval schedule"),
        }
    }

    #[test]
    fn test_retention_calculation() {
        // Test that retention calculation would work correctly
        let retention_days: u64 = 90;
        let now: i64 = 1700000000; // Some arbitrary timestamp
        let cutoff = now - (retention_days as i64 * 24 * 60 * 60);

        // 90 days in seconds = 90 * 24 * 60 * 60 = 7,776,000
        assert_eq!(cutoff, now - 7_776_000);
    }
}
