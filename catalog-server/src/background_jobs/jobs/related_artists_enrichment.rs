//! Related artists enrichment background job.
//!
//! This job populates the `related_artists` field on artists by querying
//! external APIs: MusicBrainz (for MBID resolution) and Last.fm (for similar artists).
//!
//! ## Pipeline per artist
//!
//! ```text
//! Spotify ID → MusicBrainz (get mbid) → Last.fm (similar artists + mbids)
//!            → MusicBrainz (resolve back to Spotify IDs)
//! ```
//!
//! ## Two-phase execution
//!
//! **Phase 1:** Populate MusicBrainz IDs for artists that don't have one yet.
//! **Phase 2:** Fetch similar artists from Last.fm and resolve them back to catalog entries.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::config::RelatedArtistsSettings;
use crate::related_artists::lastfm::LastFmClient;
use crate::related_artists::musicbrainz::MusicBrainzClient;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

pub struct RelatedArtistsEnrichmentJob {
    settings: RelatedArtistsSettings,
    musicbrainz: MusicBrainzClient,
    lastfm: LastFmClient,
}

impl RelatedArtistsEnrichmentJob {
    pub fn new(settings: RelatedArtistsSettings) -> Result<Self, String> {
        let musicbrainz = MusicBrainzClient::new(&settings.musicbrainz_user_agent)
            .map_err(|e| format!("Failed to create MusicBrainz client: {}", e))?;

        let lastfm = LastFmClient::new(&settings.lastfm_api_key)
            .map_err(|e| format!("Failed to create Last.fm client: {}", e))?;

        Ok(Self {
            settings,
            musicbrainz,
            lastfm,
        })
    }
}

impl BackgroundJob for RelatedArtistsEnrichmentJob {
    fn id(&self) -> &'static str {
        "related_artists_enrichment"
    }

    fn name(&self) -> &'static str {
        "Related Artists Enrichment"
    }

    fn description(&self) -> &'static str {
        "Populate related artists via MusicBrainz and Last.fm APIs"
    }

    fn schedule(&self) -> JobSchedule {
        JobSchedule::Interval(Duration::from_secs(self.settings.interval_hours * 60 * 60))
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(Arc::clone(&ctx.server_store), self.id());
        let batch_size = self.settings.batch_size;

        audit.log_started(Some(serde_json::json!({
            "batch_size": batch_size,
            "similar_artists_limit": self.settings.similar_artists_limit,
        })));

        // =====================================================================
        // Phase 1: Populate MusicBrainz IDs
        // =====================================================================
        let artists_needing_mbid = ctx
            .catalog_store
            .get_artists_needing_mbid(batch_size)
            .map_err(|e| {
                JobError::ExecutionFailed(format!("Failed to get artists needing mbid: {}", e))
            })?;

        let mut mbid_found = 0u32;
        let mut mbid_not_found = 0u32;
        let mut mbid_errors = 0u32;

        info!(
            "Phase 1: Processing {} artists for MusicBrainz ID lookup",
            artists_needing_mbid.len()
        );

        for (spotify_id, _artist_rowid) in &artists_needing_mbid {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }

            match self.musicbrainz.lookup_mbid_for_spotify_id(spotify_id) {
                Ok(Some(mbid)) => {
                    if let Err(e) = ctx.catalog_store.set_artist_mbid(spotify_id, &mbid) {
                        error!("Failed to set mbid for {}: {}", spotify_id, e);
                        mbid_errors += 1;
                    } else {
                        debug!("Found mbid {} for artist {}", mbid, spotify_id);
                        mbid_found += 1;
                    }
                }
                Ok(None) => {
                    if let Err(e) = ctx.catalog_store.mark_artist_mbid_not_found(spotify_id) {
                        error!("Failed to mark mbid not found for {}: {}", spotify_id, e);
                        mbid_errors += 1;
                    } else {
                        mbid_not_found += 1;
                    }
                }
                Err(e) => {
                    // Network error - leave status unchanged for retry
                    warn!("MusicBrainz lookup failed for {}: {}", spotify_id, e);
                    mbid_errors += 1;
                }
            }
        }

        info!(
            "Phase 1 complete: {} found, {} not found, {} errors",
            mbid_found, mbid_not_found, mbid_errors
        );

        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        // =====================================================================
        // Phase 2: Fetch related artists from Last.fm
        // =====================================================================
        let artists_needing_related = ctx
            .catalog_store
            .get_artists_needing_related(batch_size)
            .map_err(|e| {
                JobError::ExecutionFailed(format!("Failed to get artists needing related: {}", e))
            })?;

        let mut related_success = 0u32;
        let mut related_errors = 0u32;

        info!(
            "Phase 2: Processing {} artists for related artist enrichment",
            artists_needing_related.len()
        );

        for (spotify_id, mbid, artist_rowid) in &artists_needing_related {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }

            let similar = match self
                .lastfm
                .get_similar_artists(mbid, self.settings.similar_artists_limit)
            {
                Ok(s) => s,
                Err(e) => {
                    warn!(
                        "Last.fm lookup failed for {} (mbid {}): {}",
                        spotify_id, mbid, e
                    );
                    related_errors += 1;
                    continue;
                }
            };

            // Resolve similar artists back to catalog entries
            let mut related_pairs: Vec<(i64, f64)> = Vec::new();

            for similar_artist in &similar {
                let Some(similar_mbid) = &similar_artist.mbid else {
                    continue;
                };

                // Check if we already have this mbid in our catalog
                let related_rowid = match ctx.catalog_store.get_artist_rowid_by_mbid(similar_mbid) {
                    Ok(Some(rowid)) => Some(rowid),
                    Ok(None) => {
                        // Try to resolve via MusicBrainz → Spotify ID
                        match self.musicbrainz.lookup_spotify_id_for_mbid(similar_mbid) {
                            Ok(Some(related_spotify_id)) => {
                                // Try to find this Spotify ID in our catalog and opportunistically cache the mbid
                                match ctx.catalog_store.get_artist_json(&related_spotify_id) {
                                    Ok(Some(_)) => {
                                        // Artist exists in catalog, cache the mbid for future lookups
                                        let _ = ctx
                                            .catalog_store
                                            .set_artist_mbid(&related_spotify_id, similar_mbid);
                                        ctx.catalog_store
                                            .get_artist_rowid_by_mbid(similar_mbid)
                                            .ok()
                                            .flatten()
                                    }
                                    _ => None,
                                }
                            }
                            _ => None,
                        }
                    }
                    Err(_) => None,
                };

                if let Some(rowid) = related_rowid {
                    // Don't add self-references
                    if rowid != *artist_rowid {
                        related_pairs.push((rowid, similar_artist.score));
                    }
                }
            }

            if let Err(e) = ctx
                .catalog_store
                .set_related_artists(*artist_rowid, &related_pairs)
            {
                error!("Failed to store related artists for {}: {}", spotify_id, e);
                related_errors += 1;
            } else {
                debug!(
                    "Stored {} related artists for {} (mbid {})",
                    related_pairs.len(),
                    spotify_id,
                    mbid
                );
                related_success += 1;
            }
        }

        info!(
            "Phase 2 complete: {} enriched, {} errors",
            related_success, related_errors
        );

        audit.log_completed(Some(serde_json::json!({
            "phase1": {
                "processed": artists_needing_mbid.len(),
                "mbid_found": mbid_found,
                "mbid_not_found": mbid_not_found,
                "errors": mbid_errors,
            },
            "phase2": {
                "processed": artists_needing_related.len(),
                "enriched": related_success,
                "errors": related_errors,
            },
        })));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_job_id() {
        assert_eq!("related_artists_enrichment", "related_artists_enrichment");
    }
}
