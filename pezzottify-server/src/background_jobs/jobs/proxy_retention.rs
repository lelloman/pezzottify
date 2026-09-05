//! Reconcile proxy-materialized tracks against durable listening history.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobExecutionPolicy, JobResourceClass, JobSchedule},
    JobAuditLogger,
};
use anyhow::Result;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

const RETENTION_AGE: Duration = Duration::from_secs(48 * 60 * 60);
const RETENTION_PERCENT: u64 = 50;

fn meets_retention_threshold(listened_seconds: u64, duration_ms: i64) -> bool {
    let listened_ms = u128::from(listened_seconds) * 1_000;
    let duration_ms = duration_ms.max(0) as u128;
    duration_ms > 0 && listened_ms * 100 >= duration_ms * u128::from(RETENTION_PERCENT)
}

pub struct ProxyRetentionJob;

impl ProxyRetentionJob {
    pub fn new(_media_path: PathBuf) -> Self {
        Self
    }

    fn process_candidate(
        &self,
        ctx: &JobContext,
        materialization: &crate::server_store::ProxyMaterialization,
    ) -> Result<bool> {
        let Some(track) = ctx.catalog_store.get_track(&materialization.track_id)? else {
            ctx.server_store
                .delete_proxy_materialization(&materialization.track_id)?;
            return Ok(false);
        };
        let listened_seconds = ctx.user_store.get_track_listening_seconds_since(
            &materialization.track_id,
            materialization.materialized_at,
        )?;
        let Some(copy) = ctx
            .media
            .proxy_copy(&materialization.track_id, materialization.materialized_at)?
        else {
            // Legacy/ambiguous copies are protected. Retire the stale schedule only.
            ctx.server_store
                .delete_proxy_materialization(&materialization.track_id)?;
            return Ok(true);
        };
        if meets_retention_threshold(listened_seconds, track.duration_ms) {
            ctx.media.retain_copy(&copy)?;
            return Ok(true);
        }
        // A concurrent replacement is kept, rather than counted as deleted.
        Ok(!ctx.media.remove_copy(&copy)?)
    }
}

impl BackgroundJob for ProxyRetentionJob {
    fn id(&self) -> &'static str {
        "proxy_retention"
    }

    fn name(&self) -> &'static str {
        "Proxy Track Retention"
    }

    fn description(&self) -> &'static str {
        "Delete proxy tracks older than 48 hours with less than 50% recorded listening"
    }

    fn schedule(&self) -> JobSchedule {
        JobSchedule::Interval(Duration::from_secs(24 * 60 * 60))
    }

    fn run_on_startup(&self) -> bool {
        false
    }

    fn execution_policy(&self) -> JobExecutionPolicy {
        JobExecutionPolicy::new(JobResourceClass::IoBound)
            .with_queue_timeout(Duration::from_secs(60))
            .with_max_runtime(Duration::from_secs(60 * 60))
            .with_circuit_breaker(3, Duration::from_secs(30 * 60))
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(ctx.server_db.clone(), self.id());
        let cutoff = chrono::Utc::now().timestamp() - RETENTION_AGE.as_secs() as i64;
        audit.log_started(Some(serde_json::json!({
            "cutoff": cutoff,
            "retention_percent": RETENTION_PERCENT,
        })));
        let candidates = ctx
            .server_store
            .list_proxy_materializations_before(cutoff, usize::MAX)
            .map_err(|error| JobError::ExecutionFailed(error.to_string()))?;

        let mut kept = 0usize;
        let mut deleted = 0usize;
        let mut failed = 0usize;
        for candidate in &candidates {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            match self.process_candidate(ctx, candidate) {
                Ok(true) => kept += 1,
                Ok(false) => deleted += 1,
                Err(error) => {
                    failed += 1;
                    warn!(track_id = %candidate.track_id, %error, "Proxy retention candidate failed");
                }
            }
        }

        let details = serde_json::json!({
            "candidates": candidates.len(),
            "kept": kept,
            "deleted": deleted,
            "failed": failed,
        });
        audit.log_completed(Some(details));
        info!(
            candidates = candidates.len(),
            kept, deleted, failed, "Proxy retention completed"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_runs_daily_without_startup_cleanup() {
        let job = ProxyRetentionJob::new(std::path::Path::new("/media").to_path_buf());
        assert!(!job.run_on_startup());
        match job.schedule() {
            JobSchedule::Interval(interval) => {
                assert_eq!(interval, Duration::from_secs(24 * 60 * 60));
            }
            _ => panic!("expected interval schedule"),
        }
    }

    #[test]
    fn retention_requires_at_least_half_of_track_duration() {
        assert!(!meets_retention_threshold(99, 200_000));
        assert!(meets_retention_threshold(100, 200_000));
        assert!(meets_retention_threshold(101, 200_000));
        assert!(!meets_retention_threshold(100, 0));
    }
}
