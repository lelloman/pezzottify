//! Background job to clean up orphaned ingestion temp directories.
//!
//! This job provides a safety net for cleaning up temp files that weren't
//! cleaned up during normal job completion or failure (e.g., server crash,
//! bug in cleanup code).
//!
//! It runs periodically and removes directories that:
//! - Don't correspond to any active (non-terminal) ingestion job
//! - Are older than a configurable minimum age (to avoid race conditions)

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::config::IngestionCleanupJobSettings;
use crate::ingestion::IngestionStore;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Background job that cleans up orphaned ingestion temp directories.
///
/// This job runs at a configured interval and removes directories from the
/// ingestion temp folder that don't correspond to active jobs and are older
/// than `min_age_secs` (default 5 minutes).
pub struct IngestionCleanupJob {
    /// Interval in hours between runs
    interval_hours: u64,
    /// The ingestion store to query for active jobs.
    ingestion_store: Arc<dyn IngestionStore>,
    /// Directory containing job temp directories.
    temp_dir: PathBuf,
    /// Minimum age in seconds before a directory is eligible for cleanup.
    /// This prevents race conditions with newly created jobs.
    min_age_secs: u64,
}

impl IngestionCleanupJob {
    /// Create a new IngestionCleanupJob with default settings.
    pub fn new(ingestion_store: Arc<dyn IngestionStore>, temp_dir: PathBuf) -> Self {
        Self::from_settings(ingestion_store, temp_dir, &IngestionCleanupJobSettings::default())
    }

    /// Create a new IngestionCleanupJob from settings.
    pub fn from_settings(
        ingestion_store: Arc<dyn IngestionStore>,
        temp_dir: PathBuf,
        settings: &IngestionCleanupJobSettings,
    ) -> Self {
        Self {
            interval_hours: settings.interval_hours,
            ingestion_store,
            temp_dir,
            min_age_secs: settings.min_age_secs,
        }
    }

    /// Create with a custom minimum age (deprecated: use from_settings).
    #[deprecated(note = "Use from_settings instead for better configurability")]
    #[allow(dead_code)]
    pub fn with_min_age(mut self, min_age_secs: u64) -> Self {
        self.min_age_secs = min_age_secs;
        self
    }
}

impl BackgroundJob for IngestionCleanupJob {
    fn id(&self) -> &'static str {
        "ingestion_cleanup"
    }

    fn name(&self) -> &'static str {
        "Ingestion Temp Cleanup"
    }

    fn description(&self) -> &'static str {
        "Cleans up orphaned ingestion temp directories that weren't cleaned up during job completion"
    }

    fn schedule(&self) -> JobSchedule {
        // Run at configured interval
        JobSchedule::Interval(Duration::from_secs(self.interval_hours * 60 * 60))
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        // This job can be cancelled - it's not critical
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(Arc::clone(&ctx.server_store), self.id());

        audit.log_started(Some(serde_json::json!({
            "temp_dir": self.temp_dir.to_string_lossy(),
            "min_age_secs": self.min_age_secs,
        })));

        // Check if temp directory exists
        if !self.temp_dir.exists() {
            debug!(
                "Ingestion temp directory does not exist: {:?}",
                self.temp_dir
            );
            audit.log_completed(Some(serde_json::json!({
                "skipped": true,
                "reason": "temp_dir_not_found",
            })));
            return Ok(());
        }

        // Get list of active job IDs
        let active_job_ids: std::collections::HashSet<String> =
            match self.ingestion_store.list_active_job_ids() {
                Ok(ids) => ids.into_iter().collect(),
                Err(e) => {
                    let error_msg = format!("Failed to list active job IDs: {}", e);
                    audit.log_failed(&error_msg, None);
                    return Err(JobError::ExecutionFailed(error_msg));
                }
            };

        debug!("Found {} active ingestion jobs", active_job_ids.len());

        // Check for cancellation
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        // List directories in temp folder
        let entries = match std::fs::read_dir(&self.temp_dir) {
            Ok(entries) => entries,
            Err(e) => {
                let error_msg = format!("Failed to read temp directory: {}", e);
                audit.log_failed(&error_msg, None);
                return Err(JobError::ExecutionFailed(error_msg));
            }
        };

        let now = std::time::SystemTime::now();
        let min_age = Duration::from_secs(self.min_age_secs);

        let mut scanned = 0;
        let mut deleted = 0;
        let mut skipped_active = 0;
        let mut skipped_young = 0;
        let mut errors = 0;

        for entry in entries {
            // Check for cancellation periodically
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }

            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("Failed to read directory entry: {}", e);
                    errors += 1;
                    continue;
                }
            };

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            scanned += 1;

            // Get directory name (should be job ID)
            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Skip if job is still active
            if active_job_ids.contains(&dir_name) {
                debug!("Skipping active job directory: {}", dir_name);
                skipped_active += 1;
                continue;
            }

            // Check directory age
            let metadata = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    warn!("Failed to get metadata for {:?}: {}", path, e);
                    errors += 1;
                    continue;
                }
            };

            let modified = match metadata.modified() {
                Ok(t) => t,
                Err(e) => {
                    warn!("Failed to get modified time for {:?}: {}", path, e);
                    errors += 1;
                    continue;
                }
            };

            let age = match now.duration_since(modified) {
                Ok(d) => d,
                Err(_) => {
                    // Modified time is in the future - skip
                    skipped_young += 1;
                    continue;
                }
            };

            if age < min_age {
                debug!(
                    "Skipping young directory: {} (age: {}s < {}s)",
                    dir_name,
                    age.as_secs(),
                    self.min_age_secs
                );
                skipped_young += 1;
                continue;
            }

            // Delete the orphaned directory
            match std::fs::remove_dir_all(&path) {
                Ok(()) => {
                    info!(
                        "Deleted orphaned ingestion temp directory: {} (age: {}s)",
                        dir_name,
                        age.as_secs()
                    );
                    deleted += 1;
                }
                Err(e) => {
                    warn!("Failed to delete {:?}: {}", path, e);
                    errors += 1;
                }
            }
        }

        info!(
            "Ingestion cleanup complete: scanned={}, deleted={}, skipped_active={}, skipped_young={}, errors={}",
            scanned, deleted, skipped_active, skipped_young, errors
        );

        audit.log_completed(Some(serde_json::json!({
            "scanned": scanned,
            "deleted": deleted,
            "skipped_active": skipped_active,
            "skipped_young": skipped_young,
            "errors": errors,
        })));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests would require mocking IngestionStore
    // and setting up temp directories. For now, we test the metadata.

    struct MockIngestionStore {
        active_ids: Vec<String>,
    }

    impl IngestionStore for MockIngestionStore {
        fn create_job(&self, _job: &crate::ingestion::IngestionJob) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn get_job(&self, _id: &str) -> anyhow::Result<Option<crate::ingestion::IngestionJob>> {
            unimplemented!()
        }
        fn update_job(&self, _job: &crate::ingestion::IngestionJob) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn delete_job(&self, _id: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn list_jobs_by_user(
            &self,
            _user_id: &str,
            _limit: usize,
        ) -> anyhow::Result<Vec<crate::ingestion::IngestionJob>> {
            unimplemented!()
        }
        fn list_jobs_by_status(
            &self,
            _status: crate::ingestion::IngestionJobStatus,
            _limit: usize,
        ) -> anyhow::Result<Vec<crate::ingestion::IngestionJob>> {
            unimplemented!()
        }
        fn list_all_jobs(
            &self,
            _limit: usize,
        ) -> anyhow::Result<Vec<crate::ingestion::IngestionJob>> {
            unimplemented!()
        }
        fn list_active_job_ids(&self) -> anyhow::Result<Vec<String>> {
            Ok(self.active_ids.clone())
        }
        fn create_file(&self, _file: &crate::ingestion::IngestionFile) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn get_file(&self, _id: &str) -> anyhow::Result<Option<crate::ingestion::IngestionFile>> {
            unimplemented!()
        }
        fn update_file(&self, _file: &crate::ingestion::IngestionFile) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn get_files_for_job(
            &self,
            _job_id: &str,
        ) -> anyhow::Result<Vec<crate::ingestion::IngestionFile>> {
            unimplemented!()
        }
        fn delete_files_for_job(&self, _job_id: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn log_reasoning_step(
            &self,
            _job_id: &str,
            _step: &crate::agent::reasoning::ReasoningStep,
        ) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn get_reasoning_steps(
            &self,
            _job_id: &str,
        ) -> anyhow::Result<Vec<crate::agent::reasoning::ReasoningStep>> {
            unimplemented!()
        }
        fn create_review_item(
            &self,
            _job_id: &str,
            _question: &str,
            _options: &str,
        ) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn get_pending_reviews(
            &self,
            _limit: usize,
        ) -> anyhow::Result<Vec<crate::ingestion::ReviewQueueItem>> {
            unimplemented!()
        }
        fn resolve_review(
            &self,
            _job_id: &str,
            _user_id: &str,
            _selected_option: &str,
        ) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn get_review_item(
            &self,
            _job_id: &str,
        ) -> anyhow::Result<Option<crate::ingestion::ReviewQueueItem>> {
            unimplemented!()
        }
    }

    #[test]
    fn test_job_metadata() {
        let store: Arc<dyn IngestionStore> = Arc::new(MockIngestionStore { active_ids: vec![] });
        let job = IngestionCleanupJob::new(store, PathBuf::from("/tmp/test"));

        assert_eq!(job.id(), "ingestion_cleanup");
        assert_eq!(job.name(), "Ingestion Temp Cleanup");
        assert!(!job.description().is_empty());
        assert_eq!(job.shutdown_behavior(), ShutdownBehavior::Cancellable);
    }

    #[test]
    fn test_job_schedule() {
        let store: Arc<dyn IngestionStore> = Arc::new(MockIngestionStore { active_ids: vec![] });
        let job = IngestionCleanupJob::new(store, PathBuf::from("/tmp/test"));

        match job.schedule() {
            JobSchedule::Interval(duration) => {
                assert_eq!(duration, Duration::from_secs(60 * 60)); // 1 hour
            }
            _ => panic!("Expected Interval schedule"),
        }
    }

    #[test]
    fn test_with_min_age() {
        let store: Arc<dyn IngestionStore> = Arc::new(MockIngestionStore { active_ids: vec![] });
        let job = IngestionCleanupJob::new(store, PathBuf::from("/tmp/test")).with_min_age(600);

        assert_eq!(job.min_age_secs, 600);
    }
}
