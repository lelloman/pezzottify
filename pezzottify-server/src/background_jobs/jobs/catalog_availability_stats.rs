//! Catalog availability stats background job.
//!
//! Periodically reconciles catalog availability with filesystem truth,
//! computes aggregate availability counts, and stores a snapshot in server state.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::config::CatalogAvailabilityStatsJobSettings;
use crate::search::HashedItemType;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{info, warn};

const SNAPSHOT_STATE_KEY: &str = "catalog_availability_stats_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogAvailabilityStatsSnapshot {
    pub computed_at: String,
    pub duration_ms: u64,
    pub counts: crate::catalog_store::CatalogAvailabilityStats,
    pub job: CatalogAvailabilityJobInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogAvailabilityJobInfo {
    pub id: String,
    pub version: u32,
}

/// Periodic job that verifies and snapshots catalog availability.
pub struct CatalogAvailabilityStatsJob {
    interval_hours: u64,
    startup_delay_minutes: u64,
}

impl CatalogAvailabilityStatsJob {
    pub fn from_settings(settings: &CatalogAvailabilityStatsJobSettings) -> Self {
        Self {
            interval_hours: settings.interval_hours,
            startup_delay_minutes: settings.startup_delay_minutes,
        }
    }

    pub fn snapshot_state_key() -> &'static str {
        SNAPSHOT_STATE_KEY
    }
}

impl BackgroundJob for CatalogAvailabilityStatsJob {
    fn id(&self) -> &'static str {
        "catalog_availability_stats"
    }

    fn name(&self) -> &'static str {
        "Catalog Availability Stats"
    }

    fn description(&self) -> &'static str {
        "Reconcile availability from filesystem and persist aggregate catalog availability stats"
    }

    fn schedule(&self) -> JobSchedule {
        JobSchedule::Interval(Duration::from_secs(self.interval_hours * 60 * 60))
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        let audit = JobAuditLogger::new(ctx.server_store.clone(), self.id());
        let started_at = Instant::now();
        audit.log_started(Some(serde_json::json!({
            "interval_hours": self.interval_hours,
            "startup_delay_minutes": self.startup_delay_minutes,
        })));

        let refresh = match ctx
            .catalog_store
            .refresh_availability_and_stats_with_cancel(&|| ctx.is_cancelled())
        {
            Ok(r) => r,
            Err(e) => {
                if ctx.is_cancelled() {
                    audit.log_failed("Cancelled", None);
                    return Err(JobError::Cancelled);
                }
                let msg = format!("Failed to refresh catalog availability: {}", e);
                audit.log_failed(&msg, None);
                return Err(JobError::ExecutionFailed(msg));
            }
        };

        if ctx.is_cancelled() {
            audit.log_failed("Cancelled", None);
            return Err(JobError::Cancelled);
        }

        // Keep search availability index consistent with repaired catalog flags.
        if (refresh.repaired.tracks_updated
            + refresh.repaired.albums_updated
            + refresh.repaired.artists_updated)
            > 0
        {
            if let Some(search_vault) = &ctx.search_vault {
                let total_updates = refresh.track_updates.len()
                    + refresh.album_updates.len()
                    + refresh.artist_updates.len();
                let mut updates = Vec::with_capacity(total_updates);

                for item in &refresh.track_updates {
                    if ctx.is_cancelled() {
                        audit.log_failed("Cancelled", None);
                        return Err(JobError::Cancelled);
                    }
                    updates.push((item.id.clone(), HashedItemType::Track, item.available));
                }
                for item in &refresh.album_updates {
                    if ctx.is_cancelled() {
                        audit.log_failed("Cancelled", None);
                        return Err(JobError::Cancelled);
                    }
                    updates.push((item.id.clone(), HashedItemType::Album, item.available));
                }
                for item in &refresh.artist_updates {
                    if ctx.is_cancelled() {
                        audit.log_failed("Cancelled", None);
                        return Err(JobError::Cancelled);
                    }
                    updates.push((item.id.clone(), HashedItemType::Artist, item.available));
                }

                if !updates.is_empty() {
                    search_vault.update_availability(&updates);
                } else {
                    warn!("Reconciliation reported repairs but did not provide per-item updates");
                }
            }
        }

        let snapshot = CatalogAvailabilityStatsSnapshot {
            computed_at: Utc::now().to_rfc3339(),
            duration_ms: started_at.elapsed().as_millis() as u64,
            counts: refresh.stats.clone(),
            job: CatalogAvailabilityJobInfo {
                id: self.id().to_string(),
                version: 1,
            },
        };

        let snapshot_json = match serde_json::to_string(&snapshot) {
            Ok(v) => v,
            Err(e) => {
                let msg = format!("Failed to serialize availability snapshot: {}", e);
                audit.log_failed(&msg, None);
                return Err(JobError::ExecutionFailed(msg));
            }
        };

        if let Err(e) = ctx
            .server_store
            .set_state(Self::snapshot_state_key(), &snapshot_json)
        {
            let msg = format!("Failed to persist availability snapshot: {}", e);
            audit.log_failed(&msg, None);
            return Err(JobError::ExecutionFailed(msg));
        }

        info!(
            "Catalog availability snapshot saved: artists {}/{} albums {}/{} tracks {}/{} (repairs: t={} a={} ar={})",
            snapshot.counts.artists.available,
            snapshot.counts.artists.total,
            snapshot.counts.albums.available,
            snapshot.counts.albums.total,
            snapshot.counts.tracks.available,
            snapshot.counts.tracks.total,
            refresh.repaired.tracks_updated,
            refresh.repaired.albums_updated,
            refresh.repaired.artists_updated
        );

        audit.log_completed(Some(serde_json::json!({
            "duration_ms": snapshot.duration_ms,
            "repaired": refresh.repaired,
            "counts": snapshot.counts,
        })));
        Ok(())
    }
}
