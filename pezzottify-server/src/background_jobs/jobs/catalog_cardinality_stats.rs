//! Manual repair job for persisted catalog cardinalities.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::server::metrics;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

pub struct CatalogCardinalityStatsJob;

impl BackgroundJob for CatalogCardinalityStatsJob {
    fn id(&self) -> &'static str {
        "catalog_cardinality_stats"
    }

    fn name(&self) -> &'static str {
        "Rebuild Catalog Counts"
    }

    fn description(&self) -> &'static str {
        "Manually reconcile persisted artist, album, and track counts"
    }

    fn schedule(&self) -> JobSchedule {
        JobSchedule::Manual
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(ctx.server_store.clone(), self.id());
        let started_at = Instant::now();
        audit.log_started(None);

        let cancellation_token = ctx.cancellation_token.clone();
        let is_cancelled: Arc<dyn Fn() -> bool + Send + Sync> =
            Arc::new(move || cancellation_token.is_cancelled());
        let stats = match ctx
            .catalog_store
            .rebuild_catalog_cardinality_stats(is_cancelled)
        {
            Ok(stats) => stats,
            Err(error) if ctx.is_cancelled() || error.to_string() == "cancelled" => {
                audit.log_failed("Cancelled", None);
                return Err(JobError::Cancelled);
            }
            Err(error) => {
                let message = format!("Failed to rebuild catalog counts: {error}");
                audit.log_failed(&message, None);
                return Err(JobError::ExecutionFailed(message));
            }
        };

        metrics::init_catalog_metrics(stats.artists, stats.albums, stats.tracks);
        let duration_ms = started_at.elapsed().as_millis() as u64;
        audit.log_completed(Some(serde_json::json!({
            "artists": stats.artists,
            "albums": stats.albums,
            "tracks": stats.tracks,
            "mutation_version": stats.mutation_version,
            "updated_at": stats.updated_at,
            "duration_ms": duration_ms,
        })));
        info!(
            "Catalog counts rebuilt in {} ms: {} artists, {} albums, {} tracks",
            duration_ms, stats.artists, stats.albums, stats.tracks
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_is_manual_only() {
        assert!(matches!(
            CatalogCardinalityStatsJob.schedule(),
            JobSchedule::Manual
        ));
    }
}
