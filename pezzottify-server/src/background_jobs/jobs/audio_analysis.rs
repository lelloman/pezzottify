//! Audio analysis background job.
//!
//! Extracts audio features from tracks using rustentia.
//! Results are stored in the enrichment database for use by recommendation
//! and playlist generation features.
//!
//! TODO: Uncomment rustentia usage when the crate is published on crates.io.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::config::AudioAnalysisSettings;
// use crate::enrichment_store::AudioFeatures;
// use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

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
        let _enrichment_store = ctx.enrichment_store.as_ref().ok_or_else(|| {
            JobError::ExecutionFailed("Enrichment store not available in job context".to_string())
        })?;

        let audit = JobAuditLogger::new(Arc::clone(&ctx.server_store), self.id());

        audit.log_started(Some(serde_json::json!({
            "batch_size": self.settings.batch_size,
            "analyzer": "rustentia (not yet available)",
        })));

        info!("Audio analysis job skipped: rustentia crate not yet available");

        audit.log_completed(Some(serde_json::json!({
            "status": "skipped",
            "reason": "rustentia crate not yet available",
        })));

        Ok(())
    }
}
