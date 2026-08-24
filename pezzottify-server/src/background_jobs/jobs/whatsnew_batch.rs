//! What's New batch job.
//!
//! This job periodically creates batches from pending albums in the What's New
//! staging area. Albums are accumulated as they're ingested, and this job
//! closes them into a batch every 6 hours.

use crate::background_jobs::{
    context::JobContext,
    job::{
        BackgroundJob, JobError, JobExecutionPolicy, JobResourceClass, JobSchedule,
        ShutdownBehavior,
    },
    JobAuditLogger,
};
use crate::config::IntervalJobSettings;
use crate::db_executor::DbPriority;
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

/// Background job that batches pending What's New albums.
///
/// Albums are added to a pending list when they're ingested. This job
/// runs every configured interval to close the pending albums into a batch.
pub struct WhatsNewBatchJob {
    interval_hours: u64,
}

impl WhatsNewBatchJob {
    /// Create a new WhatsNewBatchJob with default settings.
    pub fn new() -> Self {
        Self::from_settings(&IntervalJobSettings::default())
    }

    /// Create a new WhatsNewBatchJob from settings.
    pub fn from_settings(settings: &IntervalJobSettings) -> Self {
        Self {
            interval_hours: settings.interval_hours,
        }
    }

    /// Create a new WhatsNewBatchJob with custom interval.
    pub fn with_interval_hours(interval_hours: u64) -> Self {
        Self { interval_hours }
    }
}

impl Default for WhatsNewBatchJob {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundJob for WhatsNewBatchJob {
    fn id(&self) -> &'static str {
        "whatsnew_batch"
    }

    fn name(&self) -> &'static str {
        "What's New Batch"
    }

    fn description(&self) -> &'static str {
        "Create batches from pending What's New albums"
    }

    fn schedule(&self) -> JobSchedule {
        // Run every configured interval (no startup run - wait for albums to accumulate)
        JobSchedule::Interval(Duration::from_secs(self.interval_hours * 60 * 60))
    }

    fn execution_policy(&self) -> JobExecutionPolicy {
        JobExecutionPolicy::new(JobResourceClass::Lightweight)
            .with_queue_timeout(Duration::from_secs(10))
            .with_max_runtime(Duration::from_secs(2 * 60))
            .with_circuit_breaker(3, Duration::from_secs(15 * 60))
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        // This job can be cancelled - batching can happen next run
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(ctx.server_db.clone(), self.id());

        // Check for cancellation before starting
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        audit.log_started(None);

        // Get pending albums
        let pending = match ctx.server_db.run_blocking(DbPriority::Background, |store| {
            store.get_pending_whatsnew_albums()
        }) {
            Ok(p) => p,
            Err(e) => {
                let error_msg = format!("Failed to get pending albums: {}", e);
                audit.log_failed(&error_msg, None);
                return Err(JobError::ExecutionFailed(error_msg));
            }
        };

        if pending.is_empty() {
            info!("No pending albums to batch");
            audit.log_completed(Some(serde_json::json!({
                "pending_count": 0,
                "batch_created": false,
            })));
            return Ok(());
        }

        let album_ids: Vec<String> = pending.into_iter().map(|(id, _)| id).collect();
        let album_count = album_ids.len();

        info!("Creating What's New batch with {} albums", album_count);

        // Create the batch
        let batch_id = Uuid::new_v4().to_string();
        let closed_at = chrono::Utc::now().timestamp();

        let batch_id_for_store = batch_id.clone();
        let album_ids_for_store = album_ids.clone();
        if let Err(e) = ctx
            .server_db
            .run_blocking(DbPriority::Background, move |store| {
                store.create_whatsnew_batch(&batch_id_for_store, closed_at, &album_ids_for_store)
            })
        {
            let error_msg = format!("Failed to create batch: {}", e);
            audit.log_failed(&error_msg, None);
            return Err(JobError::ExecutionFailed(error_msg));
        }

        // Clear the pending list
        if let Err(e) = ctx.server_db.run_blocking(DbPriority::Background, |store| {
            store.clear_pending_whatsnew_albums()
        }) {
            // Log but don't fail - batch was created successfully
            info!("Warning: Failed to clear pending albums: {}", e);
        }

        info!(
            "Created What's New batch {} with {} albums",
            batch_id, album_count
        );

        audit.log_completed(Some(serde_json::json!({
            "batch_id": batch_id,
            "album_count": album_count,
            "closed_at": closed_at,
        })));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_metadata() {
        let job = WhatsNewBatchJob::new();

        assert_eq!(job.id(), "whatsnew_batch");
        assert_eq!(job.name(), "What's New Batch");
        assert!(!job.description().is_empty());
        assert_eq!(job.shutdown_behavior(), ShutdownBehavior::Cancellable);
    }

    #[test]
    fn test_job_schedule() {
        let job = WhatsNewBatchJob::new();

        match job.schedule() {
            JobSchedule::Interval(duration) => {
                assert_eq!(duration, Duration::from_secs(6 * 60 * 60));
            }
            _ => panic!("Expected Interval schedule"),
        }
    }

    #[test]
    fn execution_policy_bounds_lightweight_batch_work() {
        let policy = WhatsNewBatchJob::new().execution_policy();

        assert_eq!(policy.resource_class, JobResourceClass::Lightweight);
        assert_eq!(policy.queue_timeout, Duration::from_secs(10));
        assert_eq!(policy.max_runtime, Some(Duration::from_secs(2 * 60)));
        let breaker = policy.circuit_breaker.unwrap();
        assert_eq!(breaker.failure_threshold, 3);
        assert_eq!(breaker.cooldown, Duration::from_secs(15 * 60));
    }

    // Integration tests for the job are in e2e_changelog_tests.rs
    // They use the test server infrastructure which provides all the necessary
    // stores and can verify the full flow through the API.
}
