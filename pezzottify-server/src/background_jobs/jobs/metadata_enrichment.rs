//! Metadata enrichment queue background job.
//!
//! The queue and typed storage are implemented here even when no external
//! metadata provider is configured. Until a source/LLM provider is attached,
//! the job claims queued work and leaves a clear retryable failure on each row.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::config::MetadataEnrichmentJobSettings;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

pub struct MetadataEnrichmentJob {
    settings: MetadataEnrichmentJobSettings,
}

impl MetadataEnrichmentJob {
    pub fn from_settings(settings: &MetadataEnrichmentJobSettings) -> Self {
        Self {
            settings: settings.clone(),
        }
    }
}

impl BackgroundJob for MetadataEnrichmentJob {
    fn id(&self) -> &'static str {
        "metadata_enrichment_v1"
    }

    fn name(&self) -> &'static str {
        "Metadata Enrichment v1"
    }

    fn description(&self) -> &'static str {
        "Process queued artist, album, and track metadata enrichment requests"
    }

    fn schedule(&self) -> JobSchedule {
        JobSchedule::Interval(Duration::from_secs(self.settings.interval_hours * 60 * 60))
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        let store = ctx.enrichment_store.as_ref().ok_or_else(|| {
            JobError::ExecutionFailed("Enrichment store not available in job context".to_string())
        })?;
        let audit = JobAuditLogger::new(Arc::clone(&ctx.server_store), self.id());
        audit.log_started(Some(serde_json::json!({
            "batch_size": self.settings.batch_size,
        })));

        let batch = store
            .claim_enrichment_queue_batch(self.settings.batch_size)
            .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;

        if batch.is_empty() {
            audit.log_completed(Some(serde_json::json!({ "processed": 0 })));
            return Ok(());
        }

        let mut failed = 0usize;
        for item in batch {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            warn!(
                "Metadata enrichment provider is not configured; leaving {} {} queued for retry",
                item.entity_type, item.entity_id
            );
            store
                .fail_enrichment_queue_item(
                    item.id,
                    "metadata enrichment provider not configured",
                    Some(self.settings.retry_after_secs as i64),
                )
                .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;
            failed += 1;
        }

        info!("Metadata enrichment processed {} queued items", failed);
        audit.log_completed(Some(serde_json::json!({
            "processed": failed,
            "retryable_failures": failed,
            "reason": "provider_not_configured",
        })));
        Ok(())
    }
}
