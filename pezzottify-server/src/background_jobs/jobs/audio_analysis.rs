//! Audio analysis background job.
//!
//! Extracts audio features from tracks using rustentia.
//! Results are stored in the enrichment database for use by recommendation
//! and playlist generation features.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::config::AudioAnalysisSettings;
use crate::enrichment_store::AudioFeatures;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

pub struct AudioAnalysisJob {
    settings: AudioAnalysisSettings,
}

impl AudioAnalysisJob {
    pub fn new(settings: AudioAnalysisSettings) -> Self {
        Self { settings }
    }
}

impl BackgroundJob for AudioAnalysisJob {
    fn id(&self) -> &'static str {
        "audio_analysis"
    }

    fn name(&self) -> &'static str {
        "Audio Analysis"
    }

    fn description(&self) -> &'static str {
        "Extract audio features from tracks using rustentia"
    }

    fn schedule(&self) -> JobSchedule {
        JobSchedule::Interval(Duration::from_secs(self.settings.interval_hours * 60 * 60))
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        let enrichment_store = ctx.enrichment_store.as_ref().ok_or_else(|| {
            JobError::ExecutionFailed("Enrichment store not available in job context".to_string())
        })?;

        let audit = JobAuditLogger::new(Arc::clone(&ctx.server_store), self.id());
        let batch_size = self.settings.batch_size;

        audit.log_started(Some(serde_json::json!({
            "batch_size": batch_size,
            "analyzer": format!("rustentia-{}", rustentia::version()),
        })));

        let result = self.run_analysis(ctx, enrichment_store.as_ref(), &audit, batch_size);

        match &result {
            Ok(()) => {}
            Err(JobError::Cancelled) => {
                audit.log_failed("Cancelled", None);
            }
            Err(e) => {
                audit.log_failed(&e.to_string(), None);
            }
        }

        result
    }
}

impl AudioAnalysisJob {
    fn run_analysis(
        &self,
        ctx: &JobContext,
        enrichment_store: &dyn crate::enrichment_store::EnrichmentStore,
        audit: &JobAuditLogger,
        batch_size: usize,
    ) -> Result<(), JobError> {
        // Page through available tracks from the catalog, checking each page against
        // the enrichment store, until we collect `batch_size` tracks to analyze.
        // This avoids loading all available track IDs into memory at once.
        const PAGE_SIZE: usize = 1000;
        let mut needing_analysis: Vec<String> = Vec::with_capacity(batch_size);
        let mut offset = 0usize;
        let mut pages_scanned = 0usize;

        loop {
            let page = ctx
                .catalog_store
                .list_available_track_ids_with_audio_uri(PAGE_SIZE, offset)
                .map_err(|e| {
                    JobError::ExecutionFailed(format!("Failed to list available tracks: {}", e))
                })?;

            if page.is_empty() {
                break;
            }

            let page_ids: Vec<String> = page.into_iter().map(|(id, _)| id).collect();
            let remaining = batch_size - needing_analysis.len();

            let mut unanalyzed = enrichment_store
                .get_tracks_needing_analysis(&page_ids, remaining)
                .map_err(|e| {
                    JobError::ExecutionFailed(format!(
                        "Failed to check tracks needing analysis: {}",
                        e
                    ))
                })?;

            needing_analysis.append(&mut unanalyzed);
            offset += PAGE_SIZE;
            pages_scanned += 1;

            if needing_analysis.len() >= batch_size {
                break;
            }
        }

        if needing_analysis.is_empty() {
            info!("No tracks need audio analysis");
            audit.log_completed(Some(serde_json::json!({
                "analyzed": 0,
                "pages_scanned": pages_scanned,
                "skipped": "all tracks already analyzed",
            })));
            return Ok(());
        }

        info!(
            "Processing {} tracks for audio analysis (scanned {} pages)",
            needing_analysis.len(),
            pages_scanned
        );

        let mut analyzed = 0u32;
        let mut errors = 0u32;

        for track_id in &needing_analysis {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }

            // Resolve the full audio path via catalog store
            let Some(audio_path) = ctx.catalog_store.get_track_audio_path(track_id) else {
                debug!("Audio file not found for track {}, skipping", track_id);
                errors += 1;
                continue;
            };

            if !audio_path.exists() {
                debug!(
                    "Audio file missing on disk for track {}, skipping",
                    track_id
                );
                errors += 1;
                continue;
            }

            match self.analyze_track(track_id, &audio_path) {
                Ok(features) => {
                    if let Err(e) = enrichment_store.upsert_audio_features(&features) {
                        error!("Failed to store features for {}: {}", track_id, e);
                        errors += 1;
                    } else {
                        debug!(
                            "Analyzed track {} (bpm={:.1}, key={})",
                            track_id, features.bpm, features.key
                        );
                        analyzed += 1;
                    }
                }
                Err(e) => {
                    warn!("Audio analysis failed for {}: {}", track_id, e);
                    errors += 1;
                }
            }

            // Rate limit to avoid CPU overload
            if self.settings.delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(self.settings.delay_ms));
            }
        }

        info!(
            "Audio analysis complete: {} analyzed, {} errors",
            analyzed, errors
        );

        audit.log_completed(Some(serde_json::json!({
            "pages_scanned": pages_scanned,
            "needing_analysis": needing_analysis.len(),
            "analyzed": analyzed,
            "errors": errors,
        })));

        Ok(())
    }

    fn analyze_track(&self, track_id: &str, audio_path: &Path) -> Result<AudioFeatures, String> {
        let features = rustentia::analyze(audio_path)
            .map_err(|e| format!("Analysis failed: {}", e))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        Ok(AudioFeatures {
            track_id: track_id.to_string(),
            bpm: features.bpm,
            danceability: features.danceability,
            key: features.key,
            chords_key: features.chords_key,
            chords_scale: features.chords_scale,
            chords_changes_rate: features.chords_changes_rate,
            loudness: features.loudness,
            average_loudness: features.average_loudness,
            dynamic_complexity: features.dynamic_complexity,
            spectral_complexity: features.spectral_complexity,
            vocal_instrumental: features.vocal_instrumental,
            valence: features.valence,
            analyzed_at: now,
            analyzer_version: format!("rustentia-{}", rustentia::version()),
        })
    }
}
