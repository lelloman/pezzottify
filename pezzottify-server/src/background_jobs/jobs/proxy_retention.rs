//! Reconcile proxy-materialized tracks against durable listening history.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobExecutionPolicy, JobResourceClass, JobSchedule},
    JobAuditLogger,
};
use crate::catalog_store::AlbumAvailability;
use crate::search::HashedItemType;
use anyhow::{Context, Result};
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

pub struct ProxyRetentionJob {
    media_path: PathBuf,
}

impl ProxyRetentionJob {
    pub fn new(media_path: PathBuf) -> Self {
        Self { media_path }
    }

    fn proxy_audio_paths(&self, track_id: &str) -> Result<Vec<PathBuf>> {
        let (dir1, dir2) = crate::ingestion::FileHandler::shard_dirs(track_id);
        let directory = self.media_path.join("audio").join(dir1).join(dir2);
        let Ok(entries) = std::fs::read_dir(directory) else {
            return Ok(Vec::new());
        };
        let prefix = format!("{track_id}.");
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            if name.to_string_lossy().starts_with(&prefix) && entry.file_type()?.is_file() {
                paths.push(entry.path());
            }
        }
        Ok(paths)
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
        let keep = meets_retention_threshold(listened_seconds, track.duration_ms);
        if keep {
            ctx.server_store
                .delete_proxy_materialization(&materialization.track_id)?;
            return Ok(true);
        }

        let resolved_album = ctx
            .catalog_store
            .get_resolved_album(&track.album_id)?
            .with_context(|| format!("album {} is missing", track.album_id))?;
        ctx.catalog_store
            .clear_track_audio_uri(&materialization.track_id)?;
        let album_availability = ctx
            .catalog_store
            .recompute_album_availability(&track.album_id)?;

        let mut unavailable = vec![(materialization.track_id.clone(), HashedItemType::Track)];
        if album_availability == AlbumAvailability::Missing {
            unavailable.push((track.album_id.clone(), HashedItemType::Album));
        }
        for artist in resolved_album.artists {
            if !ctx
                .catalog_store
                .recompute_artist_availability(&artist.id)?
            {
                unavailable.push((artist.id, HashedItemType::Artist));
            }
        }
        if let Some(search) = &ctx.search_vault {
            search.unpublish_proxy_items(&unavailable)?;
        }

        for path in self.proxy_audio_paths(&materialization.track_id)? {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
        ctx.server_store
            .delete_proxy_materialization(&materialization.track_id)?;
        Ok(false)
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
