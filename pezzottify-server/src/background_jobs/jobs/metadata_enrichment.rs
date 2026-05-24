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
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Debug, Deserialize, Default)]
struct MetadataEnrichmentRunParams {
    batch_size: Option<usize>,
    entity_types: Option<Vec<String>>,
}

fn normalize_entity_types(entity_types: Option<Vec<String>>) -> Vec<String> {
    entity_types
        .unwrap_or_default()
        .into_iter()
        .map(|entity_type| entity_type.trim().to_ascii_lowercase())
        .filter(|entity_type| matches!(entity_type.as_str(), "artist" | "album" | "track"))
        .collect()
}

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
        self.execute_with_params(ctx, None)
    }

    fn execute_with_params(&self, ctx: &JobContext, params: Option<Value>) -> Result<(), JobError> {
        let params = match params {
            Some(value) => serde_json::from_value::<MetadataEnrichmentRunParams>(value)
                .map_err(|e| JobError::ExecutionFailed(format!("Invalid params: {e}")))?,
            None => MetadataEnrichmentRunParams::default(),
        };
        let batch_size = params.batch_size.unwrap_or(self.settings.batch_size).max(1);
        let entity_types = normalize_entity_types(params.entity_types);
        let selected_entity_types = if entity_types.is_empty() {
            vec![
                "artist".to_string(),
                "album".to_string(),
                "track".to_string(),
            ]
        } else {
            entity_types.clone()
        };

        let store = ctx.enrichment_store.as_ref().ok_or_else(|| {
            JobError::ExecutionFailed("Enrichment store not available in job context".to_string())
        })?;
        let audit = JobAuditLogger::new(Arc::clone(&ctx.server_store), self.id());
        audit.log_started(Some(serde_json::json!({
            "batch_size": batch_size,
            "entity_types": selected_entity_types.clone(),
        })));

        let batch = if entity_types.is_empty() {
            store.claim_enrichment_queue_batch(batch_size)
        } else {
            store.claim_enrichment_queue_batch_for_types(batch_size, &entity_types)
        }
        .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;

        if batch.is_empty() {
            audit.log_completed(Some(serde_json::json!({
                "processed": 0,
                "batch_size": batch_size,
                "entity_types": selected_entity_types.clone(),
            })));
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
            "batch_size": batch_size,
            "entity_types": selected_entity_types.clone(),
            "reason": "provider_not_configured",
        })));
        Ok(())
    }
}
