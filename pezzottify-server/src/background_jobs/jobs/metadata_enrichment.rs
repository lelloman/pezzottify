//! Metadata enrichment queue background job.
//!
//! The job uses the existing agent LLM settings to turn catalog context into
//! typed v1 enrichment rows plus queryable child tables.

use crate::agent::{CompletionOptions, LlmProvider, Message, OllamaProvider, OpenAIProvider};
use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::config::{AgentSettings, MetadataEnrichmentJobSettings};
use crate::enrichment_store::{
    AlbumEnrichmentV1, ArtistEnrichmentV1, EnrichmentQueueItemV1, EnrichmentStore, EntityAliasV1,
    EntityContributorV1, EntityEvidenceV1, EntityExternalIdV1, EntityRelationV1, EntitySourceV1,
    EntityTagV1, TrackEnrichmentV1,
};
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
    agent: AgentSettings,
}

impl MetadataEnrichmentJob {
    pub fn from_settings(settings: &MetadataEnrichmentJobSettings, agent: AgentSettings) -> Self {
        Self {
            settings: settings.clone(),
            agent,
        }
    }

    fn execute_disabled(
        &self,
        store: &dyn EnrichmentStore,
        batch: Vec<EnrichmentQueueItemV1>,
    ) -> Result<usize, JobError> {
        let mut failed = 0usize;
        for item in batch {
            warn!(
                "Metadata enrichment agent LLM is disabled; leaving {} {} queued for retry",
                item.entity_type, item.entity_id
            );
            store
                .fail_enrichment_queue_item(
                    item.id,
                    "agent LLM is disabled",
                    Some(self.settings.retry_after_secs as i64),
                )
                .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;
            failed += 1;
        }
        Ok(failed)
    }

    async fn enrich_queue_item(
        &self,
        ctx: &JobContext,
        store: &dyn EnrichmentStore,
        provider: &dyn LlmProvider,
        item: &EnrichmentQueueItemV1,
    ) -> std::result::Result<(), ItemError> {
        let (context, external_ids) = match item.entity_type.as_str() {
            "artist" => {
                let artist = ctx
                    .catalog_store
                    .get_resolved_artist(&item.entity_id)
                    .map_err(|e| {
                        ItemError::retryable(format!("catalog artist lookup failed: {e}"))
                    })?
                    .ok_or_else(|| {
                        ItemError::permanent(format!("catalog artist {} not found", item.entity_id))
                    })?;
                (
                    serde_json::to_value(artist).map_err(|e| {
                        ItemError::retryable(format!("artist context serialization failed: {e}"))
                    })?,
                    Vec::new(),
                )
            }
            "album" => {
                let album = ctx
                    .catalog_store
                    .get_resolved_album(&item.entity_id)
                    .map_err(|e| ItemError::retryable(format!("catalog album lookup failed: {e}")))?
                    .ok_or_else(|| {
                        ItemError::permanent(format!("catalog album {} not found", item.entity_id))
                    })?;
                let external_ids = album
                    .album
                    .external_id_upc
                    .as_ref()
                    .map(|upc| EntityExternalIdV1 {
                        provider: "upc".to_string(),
                        external_id: Some(upc.clone()),
                        url: None,
                        confidence: Some(1.0),
                    })
                    .into_iter()
                    .collect();
                (
                    serde_json::to_value(album).map_err(|e| {
                        ItemError::retryable(format!("album context serialization failed: {e}"))
                    })?,
                    external_ids,
                )
            }
            "track" => {
                let track = ctx
                    .catalog_store
                    .get_resolved_track(&item.entity_id)
                    .map_err(|e| ItemError::retryable(format!("catalog track lookup failed: {e}")))?
                    .ok_or_else(|| {
                        ItemError::permanent(format!("catalog track {} not found", item.entity_id))
                    })?;
                let external_ids = track
                    .track
                    .external_id_isrc
                    .as_ref()
                    .map(|isrc| EntityExternalIdV1 {
                        provider: "isrc".to_string(),
                        external_id: Some(isrc.clone()),
                        url: None,
                        confidence: Some(1.0),
                    })
                    .into_iter()
                    .collect();
                (
                    serde_json::to_value(track).map_err(|e| {
                        ItemError::retryable(format!("track context serialization failed: {e}"))
                    })?,
                    external_ids,
                )
            }
            other => {
                return Err(ItemError::permanent(format!(
                    "unsupported enrichment entity type: {other}"
                )));
            }
        };

        let (output, raw_payload) = self
            .generate_metadata(provider, &item.entity_type, &item.entity_id, &context)
            .await?;
        self.store_metadata(
            store,
            provider,
            &item.entity_type,
            &item.entity_id,
            output,
            raw_payload,
            external_ids,
        )?;
        store
            .complete_enrichment_queue_item(item.id)
            .map_err(|e| ItemError::retryable(format!("queue completion failed: {e}")))?;
        Ok(())
    }

    async fn generate_metadata(
        &self,
        provider: &dyn LlmProvider,
        entity_type: &str,
        entity_id: &str,
        context: &Value,
    ) -> std::result::Result<(MetadataOutput, Value), ItemError> {
        let context_json = serde_json::to_string_pretty(context)
            .map_err(|e| ItemError::retryable(format!("context serialization failed: {e}")))?;
        let messages = vec![
            Message::system(
                "You enrich a music catalog as strict JSON. Return only one valid JSON object, no markdown. Use null when a fact is unknown. Do not invent exact dates, URLs, catalog numbers, or external identifiers. Confidence values must be from 0.0 to 1.0.",
            ),
            Message::user(format!(
                "Entity type: {entity_type}\nEntity id: {entity_id}\nCatalog context JSON:\n{context_json}\n\nReturn JSON using this schema. Include only facts you can infer with reasonable confidence from the context or widely-known music metadata. Unknown scalar fields must be null and arrays may be empty.\n\n{schema}",
                schema = output_schema(entity_type),
            )),
        ];
        let options = CompletionOptions {
            temperature: self.agent.llm.temperature,
            max_tokens: Some(4_000),
            timeout: Duration::from_secs(self.agent.llm.timeout_secs),
        };

        let response = provider
            .complete(&messages, None, &options)
            .await
            .map_err(|e| ItemError::retryable(format!("LLM completion failed: {e}")))?;
        let json = extract_json_object(&response.message.content).ok_or_else(|| {
            ItemError::retryable("LLM response did not contain a JSON object".to_string())
        })?;
        let raw_payload: Value = serde_json::from_str(&json)
            .map_err(|e| ItemError::retryable(format!("LLM response was not valid JSON: {e}")))?;
        let output: RawMetadataOutput =
            serde_json::from_value(raw_payload.clone()).map_err(|e| {
                ItemError::retryable(format!("LLM JSON did not match enrichment schema: {e}"))
            })?;
        Ok((output.normalized(entity_type, entity_id), raw_payload))
    }

    fn store_metadata(
        &self,
        store: &dyn EnrichmentStore,
        provider: &dyn LlmProvider,
        entity_type: &str,
        entity_id: &str,
        output: MetadataOutput,
        raw_payload: Value,
        default_external_ids: Vec<EntityExternalIdV1>,
    ) -> std::result::Result<(), ItemError> {
        let now = now_secs();
        match entity_type {
            "artist" => store
                .upsert_artist_enrichment_v1(&ArtistEnrichmentV1 {
                    artist_id: entity_id.to_string(),
                    kind: output.kind,
                    birth_date: output.birth_date,
                    death_date: output.death_date,
                    foundation_date: output.foundation_date,
                    dissolution_date: output.dissolution_date,
                    origin_place: output.origin_place,
                    origin_country: output.origin_country,
                    primary_language: output.primary_language,
                    is_person: output.is_person,
                    is_group: output.is_group,
                    is_composer: output.is_composer,
                    is_performer: output.is_performer,
                    is_conductor: output.is_conductor,
                    is_producer: output.is_producer,
                    confidence: output.confidence,
                    summary: output.summary.clone(),
                    bio: output.bio,
                    enriched_at: now,
                    last_verified_at: Some(now),
                    source_status: output
                        .source_status
                        .clone()
                        .or_else(|| Some("llm_inferred".to_string())),
                })
                .map_err(|e| {
                    ItemError::retryable(format!("artist enrichment write failed: {e}"))
                })?,
            "album" => store
                .upsert_album_enrichment_v1(&AlbumEnrichmentV1 {
                    album_id: entity_id.to_string(),
                    album_kind: output.album_kind,
                    original_release_date: output.original_release_date,
                    recording_start_date: output.recording_start_date,
                    recording_end_date: output.recording_end_date,
                    release_country: output.release_country,
                    label: output.label,
                    catalog_number: output.catalog_number,
                    is_live: output.is_live,
                    is_compilation: output.is_compilation,
                    is_soundtrack: output.is_soundtrack,
                    is_concept_album: output.is_concept_album,
                    is_remix_album: output.is_remix_album,
                    is_archival: output.is_archival,
                    confidence: output.confidence,
                    summary: output.summary.clone(),
                    notes: output.notes.clone(),
                    enriched_at: now,
                    last_verified_at: Some(now),
                    source_status: output
                        .source_status
                        .clone()
                        .or_else(|| Some("llm_inferred".to_string())),
                })
                .map_err(|e| ItemError::retryable(format!("album enrichment write failed: {e}")))?,
            "track" => store
                .upsert_track_enrichment_v1(&TrackEnrichmentV1 {
                    track_id: entity_id.to_string(),
                    track_kind: output.track_kind,
                    work_title: output.work_title,
                    composition_date: output.composition_date,
                    recording_date: output.recording_date,
                    language: output.language,
                    is_instrumental: output.is_instrumental,
                    is_live: output.is_live,
                    is_cover: output.is_cover,
                    is_remix: output.is_remix,
                    is_remaster: output.is_remaster,
                    is_arrangement: output.is_arrangement,
                    movement_number: output.movement_number,
                    movement_title: output.movement_title,
                    key_signature: output.key_signature,
                    opus_number: output.opus_number,
                    catalog_number: output.catalog_number,
                    form: output.form,
                    confidence: output.confidence,
                    summary: output.summary.clone(),
                    notes: output.notes.clone(),
                    performance_context: output.performance_context,
                    enriched_at: now,
                    last_verified_at: Some(now),
                    source_status: output
                        .source_status
                        .clone()
                        .or_else(|| Some("llm_inferred".to_string())),
                })
                .map_err(|e| ItemError::retryable(format!("track enrichment write failed: {e}")))?,
            other => {
                return Err(ItemError::permanent(format!(
                    "unsupported enrichment entity type: {other}"
                )));
            }
        }

        store
            .replace_entity_tags(entity_type, entity_id, &output.tags)
            .map_err(|e| ItemError::retryable(format!("tag replacement failed: {e}")))?;
        store
            .replace_entity_contributors(entity_type, entity_id, &output.contributors)
            .map_err(|e| ItemError::retryable(format!("contributor replacement failed: {e}")))?;
        store
            .replace_entity_relations(entity_type, entity_id, &output.relations)
            .map_err(|e| ItemError::retryable(format!("relation replacement failed: {e}")))?;
        store
            .replace_entity_sources(
                entity_type,
                entity_id,
                &sources_with_llm(output.sources, provider, now, output.confidence),
            )
            .map_err(|e| ItemError::retryable(format!("source replacement failed: {e}")))?;
        store
            .replace_entity_aliases(entity_type, entity_id, &output.aliases)
            .map_err(|e| ItemError::retryable(format!("alias replacement failed: {e}")))?;
        let mut external_ids = default_external_ids;
        external_ids.extend(output.external_ids);
        store
            .replace_entity_external_ids(entity_type, entity_id, &dedupe_external_ids(external_ids))
            .map_err(|e| ItemError::retryable(format!("external id replacement failed: {e}")))?;
        store
            .replace_entity_evidence(
                entity_type,
                entity_id,
                &evidence_with_llm_response(
                    output.evidence,
                    raw_payload,
                    output.summary.as_deref().or(output.notes.as_deref()),
                ),
            )
            .map_err(|e| ItemError::retryable(format!("evidence replacement failed: {e}")))?;
        Ok(())
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

        if !self.agent.enabled {
            let failed = self.execute_disabled(store.as_ref(), batch)?;
            audit.log_completed(Some(serde_json::json!({
                "processed": failed,
                "retryable_failures": failed,
                "batch_size": batch_size,
                "entity_types": selected_entity_types.clone(),
                "reason": "agent_llm_disabled",
            })));
            return Ok(());
        }

        let provider = build_provider(&self.agent);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                JobError::ExecutionFailed(format!(
                    "Failed to create metadata enrichment runtime: {e}"
                ))
            })?;

        let mut processed = 0usize;
        let mut retryable_failures = 0usize;
        let mut permanent_failures = 0usize;
        for item in batch {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }

            match runtime.block_on(self.enrich_queue_item(
                ctx,
                store.as_ref(),
                provider.as_ref(),
                &item,
            )) {
                Ok(()) => processed += 1,
                Err(ItemError::Retryable(message)) => {
                    warn!(
                        "Metadata enrichment failed for {} {}: {}",
                        item.entity_type, item.entity_id, message
                    );
                    store
                        .fail_enrichment_queue_item(
                            item.id,
                            &message,
                            Some(self.settings.retry_after_secs as i64),
                        )
                        .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;
                    retryable_failures += 1;
                }
                Err(ItemError::Permanent(message)) => {
                    warn!(
                        "Metadata enrichment permanently failed for {} {}: {}",
                        item.entity_type, item.entity_id, message
                    );
                    store
                        .fail_enrichment_queue_item(item.id, &message, None)
                        .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;
                    permanent_failures += 1;
                }
            }
        }

        info!(
            "Metadata enrichment processed {} queued items ({} retryable failures, {} permanent failures)",
            processed, retryable_failures, permanent_failures
        );
        audit.log_completed(Some(serde_json::json!({
            "processed": processed,
            "retryable_failures": retryable_failures,
            "permanent_failures": permanent_failures,
            "batch_size": batch_size,
            "entity_types": selected_entity_types.clone(),
            "provider": provider.name(),
            "model": provider.model(),
        })));
        Ok(())
    }
}

#[derive(Debug)]
enum ItemError {
    Retryable(String),
    Permanent(String),
}

impl ItemError {
    fn retryable(message: String) -> Self {
        Self::Retryable(message)
    }

    fn permanent(message: String) -> Self {
        Self::Permanent(message)
    }
}

#[derive(Debug, Default)]
struct MetadataOutput {
    kind: Option<String>,
    birth_date: Option<String>,
    death_date: Option<String>,
    foundation_date: Option<String>,
    dissolution_date: Option<String>,
    origin_place: Option<String>,
    origin_country: Option<String>,
    primary_language: Option<String>,
    is_person: Option<bool>,
    is_group: Option<bool>,
    is_composer: Option<bool>,
    is_performer: Option<bool>,
    is_conductor: Option<bool>,
    is_producer: Option<bool>,
    album_kind: Option<String>,
    original_release_date: Option<String>,
    recording_start_date: Option<String>,
    recording_end_date: Option<String>,
    release_country: Option<String>,
    label: Option<String>,
    catalog_number: Option<String>,
    is_live: Option<bool>,
    is_compilation: Option<bool>,
    is_soundtrack: Option<bool>,
    is_concept_album: Option<bool>,
    is_remix_album: Option<bool>,
    is_archival: Option<bool>,
    track_kind: Option<String>,
    work_title: Option<String>,
    composition_date: Option<String>,
    recording_date: Option<String>,
    language: Option<String>,
    is_instrumental: Option<bool>,
    is_cover: Option<bool>,
    is_remix: Option<bool>,
    is_remaster: Option<bool>,
    is_arrangement: Option<bool>,
    movement_number: Option<i64>,
    movement_title: Option<String>,
    key_signature: Option<String>,
    opus_number: Option<String>,
    form: Option<String>,
    confidence: Option<f64>,
    summary: Option<String>,
    bio: Option<String>,
    notes: Option<String>,
    performance_context: Option<String>,
    source_status: Option<String>,
    tags: Vec<EntityTagV1>,
    contributors: Vec<EntityContributorV1>,
    relations: Vec<EntityRelationV1>,
    aliases: Vec<EntityAliasV1>,
    external_ids: Vec<EntityExternalIdV1>,
    sources: Vec<EntitySourceV1>,
    evidence: Vec<EntityEvidenceV1>,
}

#[derive(Debug, Deserialize, Default)]
struct RawMetadataOutput {
    kind: Option<String>,
    birth_date: Option<String>,
    death_date: Option<String>,
    foundation_date: Option<String>,
    dissolution_date: Option<String>,
    origin_place: Option<String>,
    origin_country: Option<String>,
    primary_language: Option<String>,
    is_person: Option<bool>,
    is_group: Option<bool>,
    is_composer: Option<bool>,
    is_performer: Option<bool>,
    is_conductor: Option<bool>,
    is_producer: Option<bool>,
    album_kind: Option<String>,
    original_release_date: Option<String>,
    recording_start_date: Option<String>,
    recording_end_date: Option<String>,
    release_country: Option<String>,
    label: Option<String>,
    catalog_number: Option<String>,
    is_live: Option<bool>,
    is_compilation: Option<bool>,
    is_soundtrack: Option<bool>,
    is_concept_album: Option<bool>,
    is_remix_album: Option<bool>,
    is_archival: Option<bool>,
    track_kind: Option<String>,
    work_title: Option<String>,
    composition_date: Option<String>,
    recording_date: Option<String>,
    language: Option<String>,
    is_instrumental: Option<bool>,
    is_cover: Option<bool>,
    is_remix: Option<bool>,
    is_remaster: Option<bool>,
    is_arrangement: Option<bool>,
    movement_number: Option<i64>,
    movement_title: Option<String>,
    key_signature: Option<String>,
    opus_number: Option<String>,
    form: Option<String>,
    confidence: Option<f64>,
    summary: Option<String>,
    bio: Option<String>,
    notes: Option<String>,
    performance_context: Option<String>,
    source_status: Option<String>,
    #[serde(default)]
    tags: Vec<TagOutput>,
    #[serde(default)]
    contributors: Vec<ContributorOutput>,
    #[serde(default)]
    relations: Vec<RelationOutput>,
    #[serde(default)]
    aliases: Vec<AliasOutput>,
    #[serde(default)]
    external_ids: Vec<ExternalIdOutput>,
    #[serde(default)]
    sources: Vec<SourceOutput>,
    #[serde(default)]
    evidence: Vec<EvidenceOutput>,
}

impl RawMetadataOutput {
    fn normalized(self, entity_type: &str, entity_id: &str) -> MetadataOutput {
        MetadataOutput {
            kind: clean_opt_string(self.kind),
            birth_date: clean_opt_string(self.birth_date),
            death_date: clean_opt_string(self.death_date),
            foundation_date: clean_opt_string(self.foundation_date),
            dissolution_date: clean_opt_string(self.dissolution_date),
            origin_place: clean_opt_string(self.origin_place),
            origin_country: clean_opt_string(self.origin_country),
            primary_language: clean_opt_string(self.primary_language),
            is_person: self.is_person,
            is_group: self.is_group,
            is_composer: self.is_composer,
            is_performer: self.is_performer,
            is_conductor: self.is_conductor,
            is_producer: self.is_producer,
            album_kind: clean_opt_string(self.album_kind),
            original_release_date: clean_opt_string(self.original_release_date),
            recording_start_date: clean_opt_string(self.recording_start_date),
            recording_end_date: clean_opt_string(self.recording_end_date),
            release_country: clean_opt_string(self.release_country),
            label: clean_opt_string(self.label),
            catalog_number: clean_opt_string(self.catalog_number),
            is_live: self.is_live,
            is_compilation: self.is_compilation,
            is_soundtrack: self.is_soundtrack,
            is_concept_album: self.is_concept_album,
            is_remix_album: self.is_remix_album,
            is_archival: self.is_archival,
            track_kind: clean_opt_string(self.track_kind),
            work_title: clean_opt_string(self.work_title),
            composition_date: clean_opt_string(self.composition_date),
            recording_date: clean_opt_string(self.recording_date),
            language: clean_opt_string(self.language),
            is_instrumental: self.is_instrumental,
            is_cover: self.is_cover,
            is_remix: self.is_remix,
            is_remaster: self.is_remaster,
            is_arrangement: self.is_arrangement,
            movement_number: self.movement_number,
            movement_title: clean_opt_string(self.movement_title),
            key_signature: clean_opt_string(self.key_signature),
            opus_number: clean_opt_string(self.opus_number),
            form: clean_opt_string(self.form),
            confidence: clamp_confidence(self.confidence),
            summary: clean_opt_string(self.summary),
            bio: clean_opt_string(self.bio),
            notes: clean_opt_string(self.notes),
            performance_context: clean_opt_string(self.performance_context),
            source_status: clean_opt_string(self.source_status),
            tags: self
                .tags
                .into_iter()
                .filter_map(TagOutput::into_model)
                .collect(),
            contributors: self
                .contributors
                .into_iter()
                .filter_map(ContributorOutput::into_model)
                .collect(),
            relations: self
                .relations
                .into_iter()
                .filter_map(|relation| relation.into_model(entity_type, entity_id))
                .collect(),
            aliases: self
                .aliases
                .into_iter()
                .filter_map(AliasOutput::into_model)
                .collect(),
            external_ids: self
                .external_ids
                .into_iter()
                .filter_map(ExternalIdOutput::into_model)
                .collect(),
            sources: self
                .sources
                .into_iter()
                .filter_map(SourceOutput::into_model)
                .collect(),
            evidence: self
                .evidence
                .into_iter()
                .filter_map(EvidenceOutput::into_model)
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TagOutput {
    tag_type: Option<String>,
    tag: Option<String>,
    confidence: Option<f64>,
    source: Option<String>,
}

impl TagOutput {
    fn into_model(self) -> Option<EntityTagV1> {
        Some(EntityTagV1 {
            tag_type: clean_opt_string(self.tag_type)?,
            tag: clean_opt_string(self.tag)?,
            confidence: clamp_confidence(self.confidence),
            source: clean_opt_string(self.source),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ContributorOutput {
    contributor_name: Option<String>,
    contributor_id: Option<String>,
    role: Option<String>,
    confidence: Option<f64>,
}

impl ContributorOutput {
    fn into_model(self) -> Option<EntityContributorV1> {
        Some(EntityContributorV1 {
            contributor_name: clean_opt_string(self.contributor_name)?,
            contributor_id: clean_opt_string(self.contributor_id),
            role: clean_opt_string(self.role)?,
            confidence: clamp_confidence(self.confidence),
        })
    }
}

#[derive(Debug, Deserialize)]
struct RelationOutput {
    relation_type: Option<String>,
    target_entity_type: Option<String>,
    target_entity_id: Option<String>,
    external_target_name: Option<String>,
    external_target_url: Option<String>,
    confidence: Option<f64>,
    evidence: Option<Value>,
}

impl RelationOutput {
    fn into_model(self, entity_type: &str, entity_id: &str) -> Option<EntityRelationV1> {
        let confidence = clamp_confidence(self.confidence);
        let target_entity_type = clean_opt_string(self.target_entity_type);
        let target_entity_id = clean_opt_string(self.target_entity_id);
        let external_target_name = clean_opt_string(self.external_target_name);
        let external_target_url = clean_opt_string(self.external_target_url);
        if target_entity_id.is_none()
            && external_target_name.is_none()
            && external_target_url.is_none()
        {
            return None;
        }
        Some(EntityRelationV1 {
            source_entity_type: entity_type.to_string(),
            source_entity_id: entity_id.to_string(),
            relation_type: clean_opt_string(self.relation_type)?,
            target_entity_type,
            target_entity_id,
            external_target_name,
            external_target_url,
            visible: confidence.unwrap_or(0.0) >= 0.8,
            confidence,
            evidence: self.evidence,
        })
    }
}

#[derive(Debug, Deserialize)]
struct AliasOutput {
    alias: Option<String>,
    locale: Option<String>,
    source: Option<String>,
    confidence: Option<f64>,
}

impl AliasOutput {
    fn into_model(self) -> Option<EntityAliasV1> {
        Some(EntityAliasV1 {
            alias: clean_opt_string(self.alias)?,
            locale: clean_opt_string(self.locale),
            source: clean_opt_string(self.source),
            confidence: clamp_confidence(self.confidence),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ExternalIdOutput {
    provider: Option<String>,
    external_id: Option<String>,
    url: Option<String>,
    confidence: Option<f64>,
}

impl ExternalIdOutput {
    fn into_model(self) -> Option<EntityExternalIdV1> {
        let external_id = clean_opt_string(self.external_id);
        let url = clean_opt_string(self.url);
        if external_id.is_none() && url.is_none() {
            return None;
        }
        Some(EntityExternalIdV1 {
            provider: clean_opt_string(self.provider)?,
            external_id,
            url,
            confidence: clamp_confidence(self.confidence),
        })
    }
}

#[derive(Debug, Deserialize)]
struct SourceOutput {
    source_name: Option<String>,
    source_url: Option<String>,
    retrieved_at: Option<i64>,
    confidence: Option<f64>,
}

impl SourceOutput {
    fn into_model(self) -> Option<EntitySourceV1> {
        Some(EntitySourceV1 {
            source_name: clean_opt_string(self.source_name)?,
            source_url: clean_opt_string(self.source_url),
            retrieved_at: self.retrieved_at,
            confidence: clamp_confidence(self.confidence),
        })
    }
}

#[derive(Debug, Deserialize)]
struct EvidenceOutput {
    source_name: Option<String>,
    source_url: Option<String>,
    snippet: Option<String>,
    raw_payload: Option<Value>,
}

impl EvidenceOutput {
    fn into_model(self) -> Option<EntityEvidenceV1> {
        let snippet = clean_opt_string(self.snippet);
        let raw_payload = self.raw_payload;
        if snippet.is_none() && raw_payload.is_none() {
            return None;
        }
        Some(EntityEvidenceV1 {
            source_name: clean_opt_string(self.source_name),
            source_url: clean_opt_string(self.source_url),
            snippet,
            raw_payload,
        })
    }
}

fn build_provider(agent: &AgentSettings) -> Box<dyn LlmProvider> {
    match agent.llm.provider.as_str() {
        "openai" => match &agent.llm.api_key_command {
            Some(command) => Box::new(OpenAIProvider::with_key_command(
                agent.llm.base_url.clone(),
                agent.llm.model.clone(),
                command.clone(),
            )),
            None => Box::new(OpenAIProvider::new(
                agent.llm.base_url.clone(),
                agent.llm.model.clone(),
                agent.llm.api_key.clone(),
            )),
        },
        _ => Box::new(OllamaProvider::new(
            agent.llm.base_url.clone(),
            agent.llm.model.clone(),
        )),
    }
}

fn output_schema(entity_type: &str) -> &'static str {
    match entity_type {
        "artist" => ARTIST_OUTPUT_SCHEMA,
        "album" => ALBUM_OUTPUT_SCHEMA,
        _ => TRACK_OUTPUT_SCHEMA,
    }
}

const ARTIST_OUTPUT_SCHEMA: &str = r#"{
  "kind": "person|group|orchestra|choir|producer|label|null",
  "birth_date": "YYYY-MM-DD|YYYY-MM|YYYY|null",
  "death_date": "YYYY-MM-DD|YYYY-MM|YYYY|null",
  "foundation_date": "YYYY-MM-DD|YYYY-MM|YYYY|null",
  "dissolution_date": "YYYY-MM-DD|YYYY-MM|YYYY|null",
  "origin_place": "string|null",
  "origin_country": "string|null",
  "primary_language": "ISO 639-1 or language name|null",
  "is_person": true,
  "is_group": false,
  "is_composer": false,
  "is_performer": true,
  "is_conductor": false,
  "is_producer": false,
  "confidence": 0.0,
  "summary": "one sentence|null",
  "bio": "short paragraph|null",
  "source_status": "llm_inferred",
  "tags": [{"tag_type": "genre|style|mood|scene|theme", "tag": "string", "confidence": 0.0, "source": "llm"}],
  "contributors": [],
  "relations": [{"relation_type": "influenced_by|member_of|collaborated_with|similar_to", "target_entity_type": null, "target_entity_id": null, "external_target_name": "string|null", "external_target_url": null, "confidence": 0.0, "evidence": null}],
  "aliases": [{"alias": "string", "locale": null, "source": "llm", "confidence": 0.0}],
  "external_ids": [],
  "sources": [],
  "evidence": []
}"#;

const ALBUM_OUTPUT_SCHEMA: &str = r#"{
  "album_kind": "album|single|ep|compilation|soundtrack|live|remix|null",
  "original_release_date": "YYYY-MM-DD|YYYY-MM|YYYY|null",
  "recording_start_date": "YYYY-MM-DD|YYYY-MM|YYYY|null",
  "recording_end_date": "YYYY-MM-DD|YYYY-MM|YYYY|null",
  "release_country": "string|null",
  "label": "string|null",
  "catalog_number": "string|null",
  "is_live": false,
  "is_compilation": false,
  "is_soundtrack": false,
  "is_concept_album": false,
  "is_remix_album": false,
  "is_archival": false,
  "confidence": 0.0,
  "summary": "one sentence|null",
  "notes": "short notes|null",
  "source_status": "llm_inferred",
  "tags": [{"tag_type": "genre|style|mood|scene|theme", "tag": "string", "confidence": 0.0, "source": "llm"}],
  "contributors": [{"contributor_name": "string", "contributor_id": "local artist id|null", "role": "artist|producer|composer|conductor|engineer", "confidence": 0.0}],
  "relations": [{"relation_type": "part_of_series|influenced_by|alternate_version_of|related_to", "target_entity_type": null, "target_entity_id": null, "external_target_name": "string|null", "external_target_url": null, "confidence": 0.0, "evidence": null}],
  "aliases": [],
  "external_ids": [],
  "sources": [],
  "evidence": []
}"#;

const TRACK_OUTPUT_SCHEMA: &str = r#"{
  "track_kind": "song|instrumental|movement|spoken_word|remix|null",
  "work_title": "string|null",
  "composition_date": "YYYY-MM-DD|YYYY-MM|YYYY|null",
  "recording_date": "YYYY-MM-DD|YYYY-MM|YYYY|null",
  "language": "ISO 639-1 or language name|null",
  "is_instrumental": false,
  "is_live": false,
  "is_cover": false,
  "is_remix": false,
  "is_remaster": false,
  "is_arrangement": false,
  "movement_number": null,
  "movement_title": "string|null",
  "key_signature": "string|null",
  "opus_number": "string|null",
  "catalog_number": "string|null",
  "form": "string|null",
  "confidence": 0.0,
  "summary": "one sentence|null",
  "notes": "short notes|null",
  "performance_context": "string|null",
  "source_status": "llm_inferred",
  "tags": [{"tag_type": "genre|style|mood|scene|theme", "tag": "string", "confidence": 0.0, "source": "llm"}],
  "contributors": [{"contributor_name": "string", "contributor_id": "local artist id|null", "role": "artist|composer|producer|remixer|conductor", "confidence": 0.0}],
  "relations": [{"relation_type": "cover_of|remix_of|movement_of|samples|influenced_by|related_to", "target_entity_type": null, "target_entity_id": null, "external_target_name": "string|null", "external_target_url": null, "confidence": 0.0, "evidence": null}],
  "aliases": [],
  "external_ids": [],
  "sources": [],
  "evidence": []
}"#;

fn sources_with_llm(
    mut sources: Vec<EntitySourceV1>,
    provider: &dyn LlmProvider,
    now: i64,
    confidence: Option<f64>,
) -> Vec<EntitySourceV1> {
    sources.push(EntitySourceV1 {
        source_name: format!("llm:{}:{}", provider.name(), provider.model()),
        source_url: None,
        retrieved_at: Some(now),
        confidence,
    });
    sources
}

fn evidence_with_llm_response(
    mut evidence: Vec<EntityEvidenceV1>,
    raw_payload: Value,
    snippet: Option<&str>,
) -> Vec<EntityEvidenceV1> {
    evidence.push(EntityEvidenceV1 {
        source_name: Some("llm_response".to_string()),
        source_url: None,
        snippet: snippet.map(|s| s.chars().take(512).collect()),
        raw_payload: Some(raw_payload),
    });
    evidence
}

fn dedupe_external_ids(external_ids: Vec<EntityExternalIdV1>) -> Vec<EntityExternalIdV1> {
    let mut deduped = Vec::new();
    for external_id in external_ids {
        let exists = deduped.iter().any(|existing: &EntityExternalIdV1| {
            existing.provider == external_id.provider
                && existing.external_id == external_id.external_id
                && existing.url == external_id.url
        });
        if !exists {
            deduped.push(external_id);
        }
    }
    deduped
}

fn clean_opt_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "null")
}

fn clamp_confidence(value: Option<f64>) -> Option<f64> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0))
}

fn extract_json_object(content: &str) -> Option<String> {
    let start = content.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in content[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return Some(content[start..end].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_object_handles_fences_and_nested_strings() {
        let content = "```json\n{\"summary\":\"brace } in string\",\"nested\":{\"ok\":true}}\n```";
        assert_eq!(
            extract_json_object(content).unwrap(),
            "{\"summary\":\"brace } in string\",\"nested\":{\"ok\":true}}"
        );
    }

    #[test]
    fn normalizes_confidence_and_empty_strings() {
        let output = RawMetadataOutput {
            summary: Some("  ".to_string()),
            confidence: Some(2.0),
            tags: vec![TagOutput {
                tag_type: Some(" genre ".to_string()),
                tag: Some("  Jazz ".to_string()),
                confidence: Some(-0.5),
                source: Some("llm".to_string()),
            }],
            ..RawMetadataOutput::default()
        }
        .normalized("artist", "a1");

        assert_eq!(output.summary, None);
        assert_eq!(output.confidence, Some(1.0));
        assert_eq!(output.tags.len(), 1);
        assert_eq!(output.tags[0].tag_type, "genre");
        assert_eq!(output.tags[0].tag, "Jazz");
        assert_eq!(output.tags[0].confidence, Some(0.0));
    }

    #[test]
    fn relation_visibility_requires_high_confidence() {
        let relation = RelationOutput {
            relation_type: Some("cover_of".to_string()),
            target_entity_type: None,
            target_entity_id: None,
            external_target_name: Some("Original".to_string()),
            external_target_url: None,
            confidence: Some(0.79),
            evidence: None,
        }
        .into_model("track", "t1")
        .unwrap();
        assert!(!relation.visible);

        let relation = RelationOutput {
            relation_type: Some("cover_of".to_string()),
            target_entity_type: None,
            target_entity_id: None,
            external_target_name: Some("Original".to_string()),
            external_target_url: None,
            confidence: Some(0.8),
            evidence: None,
        }
        .into_model("track", "t1")
        .unwrap();
        assert!(relation.visible);
    }
}
