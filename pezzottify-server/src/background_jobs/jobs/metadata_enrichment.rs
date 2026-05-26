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
use crate::catalog_store::ResolvedTrack;
use crate::config::{AgentSettings, MetadataEnrichmentJobSettings};
use crate::enrichment_store::{
    AlbumEnrichmentV1, ArtistEnrichmentV1, EnrichmentQueueItemV1, EnrichmentStore, EntityAliasV1,
    EntityContributorV1, EntityEvidenceV1, EntityExternalIdV1, EntityRelationV1, EntitySourceV1,
    EntityTagV1, TrackEnrichmentV1,
};
use crate::user::user_models::TrackPlayCount;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

const ENRICHMENT_STALE_AFTER_SECS: i64 = 90 * 24 * 60 * 60;
const ALL_TIME_LISTENING_START_DATE: u32 = 0;
const ALL_TIME_LISTENING_END_DATE: u32 = 99_991_231;
const LISTENING_BACKFILL_REASON: &str = "listening_backfill";
const WIKIDATA_LLM_COMPLETION_REASON: &str = "wikidata_llm_completion";
const GENERATED_SOURCE_STATUS: &str = "llm_inferred_v2";
const WIKIDATA_SOURCE_STATUS: &str = "wikidata";
const WIKIDATA_SPARQL_URL: &str = "https://query.wikidata.org/sparql";

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ListeningBackfillCandidate {
    entity_type: String,
    entity_id: String,
    priority: i64,
}

#[derive(Debug, Clone)]
struct ListenedTrackContext {
    track_id: String,
    play_count: u64,
    album_id: Option<String>,
    artist_ids: Vec<String>,
}

fn listening_priority(play_count: u64) -> i64 {
    play_count.min(i64::MAX as u64) as i64
}

fn wants_entity_type(selected: &HashSet<String>, entity_type: &str) -> bool {
    selected.is_empty() || selected.contains(entity_type)
}

fn listening_backfill_candidates(
    tracks: &[ListenedTrackContext],
    selected_entity_types: &[String],
) -> Vec<ListeningBackfillCandidate> {
    let selected = selected_entity_types
        .iter()
        .cloned()
        .collect::<HashSet<String>>();
    let mut candidates = Vec::new();

    if wants_entity_type(&selected, "track") {
        candidates.extend(
            tracks
                .iter()
                .filter(|track| !track.track_id.trim().is_empty() && track.play_count > 0)
                .map(|track| ListeningBackfillCandidate {
                    entity_type: "track".to_string(),
                    entity_id: track.track_id.clone(),
                    priority: listening_priority(track.play_count),
                }),
        );
    }

    if wants_entity_type(&selected, "album") {
        let mut album_plays: HashMap<String, u64> = HashMap::new();
        for track in tracks {
            let Some(album_id) = track.album_id.as_ref() else {
                continue;
            };
            if album_id.trim().is_empty() || track.play_count == 0 {
                continue;
            }
            let entry = album_plays.entry(album_id.clone()).or_insert(0);
            *entry = entry.saturating_add(track.play_count);
        }
        candidates.extend(album_plays.into_iter().map(|(album_id, play_count)| {
            ListeningBackfillCandidate {
                entity_type: "album".to_string(),
                entity_id: album_id,
                priority: listening_priority(play_count),
            }
        }));
    }

    if wants_entity_type(&selected, "artist") {
        let mut artist_plays: HashMap<String, u64> = HashMap::new();
        for track in tracks {
            if track.play_count == 0 {
                continue;
            }
            let mut seen_for_track = HashSet::new();
            for artist_id in &track.artist_ids {
                if artist_id.trim().is_empty() || !seen_for_track.insert(artist_id) {
                    continue;
                }
                let entry = artist_plays.entry(artist_id.clone()).or_insert(0);
                *entry = entry.saturating_add(track.play_count);
            }
        }
        candidates.extend(artist_plays.into_iter().map(|(artist_id, play_count)| {
            ListeningBackfillCandidate {
                entity_type: "artist".to_string(),
                entity_id: artist_id,
                priority: listening_priority(play_count),
            }
        }));
    }

    candidates.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.entity_type.cmp(&b.entity_type))
            .then_with(|| a.entity_id.cmp(&b.entity_id))
    });
    candidates
}

fn track_context_from_resolved(
    track_count: &TrackPlayCount,
    resolved: Option<ResolvedTrack>,
) -> ListenedTrackContext {
    match resolved {
        Some(resolved) => ListenedTrackContext {
            track_id: track_count.track_id.clone(),
            play_count: track_count.play_count,
            album_id: Some(resolved.album.id),
            artist_ids: resolved
                .artists
                .into_iter()
                .map(|artist| artist.artist.id)
                .collect(),
        },
        None => ListenedTrackContext {
            track_id: track_count.track_id.clone(),
            play_count: track_count.play_count,
            album_id: None,
            artist_ids: Vec::new(),
        },
    }
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

    fn build_listening_backfill_candidates(
        &self,
        ctx: &JobContext,
        selected_entity_types: &[String],
    ) -> Result<Vec<ListeningBackfillCandidate>, JobError> {
        let track_counts = ctx
            .user_store
            .get_all_track_play_counts(ALL_TIME_LISTENING_START_DATE, ALL_TIME_LISTENING_END_DATE)
            .map_err(|e| {
                JobError::ExecutionFailed(format!("Failed to get listening counts: {e}"))
            })?;

        if track_counts.is_empty() {
            return Ok(Vec::new());
        }

        let needs_resolved_tracks = selected_entity_types.is_empty()
            || selected_entity_types
                .iter()
                .any(|entity_type| matches!(entity_type.as_str(), "artist" | "album"));
        let tracks = track_counts
            .iter()
            .map(|track_count| {
                let resolved = if needs_resolved_tracks {
                    ctx.catalog_store
                        .get_resolved_track(&track_count.track_id)
                        .map_err(|e| {
                            JobError::ExecutionFailed(format!(
                                "Failed to resolve listened track {}: {e}",
                                track_count.track_id
                            ))
                        })?
                } else {
                    None
                };
                Ok(track_context_from_resolved(track_count, resolved))
            })
            .collect::<Result<Vec<_>, JobError>>()?;

        Ok(listening_backfill_candidates(
            &tracks,
            selected_entity_types,
        ))
    }

    fn seed_listening_backfill(
        &self,
        ctx: &JobContext,
        store: &dyn EnrichmentStore,
        selected_entity_types: &[String],
    ) -> Result<usize, JobError> {
        let candidates = self.build_listening_backfill_candidates(ctx, selected_entity_types)?;
        let mut seeded = 0usize;
        for candidate in candidates {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            let needs_llm_completion = candidate.entity_type == "artist"
                && self.wikidata_only_artist_needs_llm_completion(store, &candidate.entity_id)?;
            let reason = if needs_llm_completion {
                WIKIDATA_LLM_COMPLETION_REASON
            } else {
                LISTENING_BACKFILL_REASON
            };
            let stale_after_secs = if needs_llm_completion {
                0
            } else {
                ENRICHMENT_STALE_AFTER_SECS
            };
            let queued = store
                .enqueue_enrichment_if_missing_or_stale(
                    &candidate.entity_type,
                    &candidate.entity_id,
                    reason,
                    candidate.priority,
                    stale_after_secs,
                )
                .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;
            if queued {
                seeded += 1;
            }
        }
        Ok(seeded)
    }

    fn wikidata_only_artist_needs_llm_completion(
        &self,
        store: &dyn EnrichmentStore,
        artist_id: &str,
    ) -> Result<bool, JobError> {
        if !self.agent.enabled {
            return Ok(false);
        }
        let profile = store
            .get_artist_enrichment_v1(artist_id)
            .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;
        Ok(profile
            .as_ref()
            .and_then(|profile| profile.source_status.as_deref())
            .map(source_status_needs_llm_completion)
            .unwrap_or(false))
    }

    async fn enrich_queue_item_without_llm(
        &self,
        ctx: &JobContext,
        store: &dyn EnrichmentStore,
        item: &EnrichmentQueueItemV1,
    ) -> std::result::Result<(), ItemError> {
        if item.entity_type == "artist" {
            match self
                .try_enrich_artist_from_wikidata(ctx, store, &item.entity_id)
                .await
            {
                Ok(Some(_)) => {
                    store.complete_enrichment_queue_item(item.id).map_err(|e| {
                        ItemError::retryable(format!("queue completion failed: {e}"))
                    })?;
                    return Ok(());
                }
                Ok(None) => {}
                Err(err) => warn!(
                    "Wikidata enrichment failed for artist {} while LLM is disabled: {}",
                    item.entity_id, err
                ),
            }
        }

        Err(ItemError::retryable(
            "agent LLM is disabled and no deterministic enrichment was available".to_string(),
        ))
    }

    async fn enrich_queue_item(
        &self,
        ctx: &JobContext,
        store: &dyn EnrichmentStore,
        provider: &dyn LlmProvider,
        item: &EnrichmentQueueItemV1,
    ) -> std::result::Result<(), ItemError> {
        let mut wikidata_enrichment = None;
        if item.entity_type == "artist" {
            match self
                .try_enrich_artist_from_wikidata(ctx, store, &item.entity_id)
                .await
            {
                Ok(Some(enrichment)) => {
                    wikidata_enrichment = Some(enrichment);
                }
                Ok(None) => {}
                Err(err) => warn!(
                    "Wikidata enrichment failed for artist {}; falling back to LLM: {}",
                    item.entity_id, err
                ),
            }
        }

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
                let mut context = serde_json::to_value(artist).map_err(|e| {
                    ItemError::retryable(format!("artist context serialization failed: {e}"))
                })?;
                if let Some(enrichment) = wikidata_enrichment.as_ref() {
                    context = serde_json::json!({
                        "catalog": context,
                        "source_backed_wikidata": enrichment.prompt_context(),
                    });
                }
                let external_ids = wikidata_enrichment
                    .as_ref()
                    .map(|enrichment| enrichment.external_ids.clone())
                    .unwrap_or_default();
                (context, external_ids)
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

        let (mut output, raw_payload) = self
            .generate_metadata(provider, &item.entity_type, &item.entity_id, &context)
            .await?;
        if let Some(enrichment) = wikidata_enrichment {
            output.sources.extend(enrichment.sources);
            output.evidence.extend(enrichment.evidence);
        }
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

    async fn try_enrich_artist_from_wikidata(
        &self,
        ctx: &JobContext,
        store: &dyn EnrichmentStore,
        artist_id: &str,
    ) -> anyhow::Result<Option<WikidataArtistEnrichment>> {
        let Some(mbid) = ctx.catalog_store.get_artist_mbid(artist_id)? else {
            return Ok(None);
        };
        let Some(facts) = WikidataClient::new()?.lookup_artist_by_mbid(&mbid).await? else {
            return Ok(None);
        };
        if !facts.has_profile_data() {
            return Ok(None);
        }

        let now = now_secs();
        let enrichment = WikidataArtistEnrichment::new(artist_id, mbid, facts, now);
        store.upsert_artist_enrichment_v1(&enrichment.profile)?;
        store.replace_entity_tags("artist", artist_id, &[])?;
        store.replace_entity_contributors("artist", artist_id, &[])?;
        store.replace_entity_relations("artist", artist_id, &[])?;
        store.replace_entity_aliases("artist", artist_id, &[])?;
        store.replace_entity_external_ids("artist", artist_id, &enrichment.external_ids)?;
        store.replace_entity_sources("artist", artist_id, &enrichment.sources)?;
        store.replace_entity_evidence("artist", artist_id, &enrichment.evidence)?;
        Ok(Some(enrichment))
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
                "You enrich a music catalog as strict JSON. Return only one valid JSON object, no markdown. Prefer durable, public music reference facts over guesswork. Source-backed facts in the catalog context are authoritative; do not contradict them. For established artists, use widely known biographical facts even when the local catalog context only contains name and discography. Use null only when the fact is not known to you or is ambiguous. Do not invent URLs, catalog numbers, external identifiers, or obscure exact dates. Confidence values must be from 0.0 to 1.0.",
            ),
            Message::user(format!(
                "Entity type: {entity_type}\nEntity id: {entity_id}\nCatalog context JSON:\n{context_json}\n\nReturn JSON using this schema. Include facts you can infer with reasonable confidence from the context or widely-known music metadata. For person artists, populate birth_date, death_date, and origin_place when they are well-known public facts; origin_place means birthplace for people. Unknown scalar fields must be null and arrays may be empty.\n\n{schema}",
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
            "artist" => {
                let wikidata_profile = store
                    .get_artist_enrichment_v1(entity_id)
                    .map_err(|e| {
                        ItemError::retryable(format!(
                            "artist enrichment pre-merge read failed: {e}"
                        ))
                    })?
                    .filter(is_wikidata_backed_profile);
                store
                    .upsert_artist_enrichment_v1(&artist_profile_from_output(
                        entity_id,
                        &output,
                        wikidata_profile.as_ref(),
                        now,
                    ))
                    .map_err(|e| {
                        ItemError::retryable(format!("artist enrichment write failed: {e}"))
                    })?
            }
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
                        .or_else(|| Some(GENERATED_SOURCE_STATUS.to_string())),
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
                        .or_else(|| Some(GENERATED_SOURCE_STATUS.to_string())),
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

        let claim_batch = || {
            if entity_types.is_empty() {
                store.claim_enrichment_queue_batch(batch_size)
            } else {
                store.claim_enrichment_queue_batch_for_types(batch_size, &entity_types)
            }
            .map_err(|e| JobError::ExecutionFailed(e.to_string()))
        };

        let mut seeded = 0usize;
        let mut batch = claim_batch()?;
        if batch.is_empty() {
            seeded = self.seed_listening_backfill(ctx, store.as_ref(), &entity_types)?;
            if seeded > 0 {
                batch = claim_batch()?;
            }
        }

        if batch.is_empty() {
            audit.log_completed(Some(serde_json::json!({
                "processed": 0,
                "seeded": seeded,
                "batch_size": batch_size,
                "entity_types": selected_entity_types.clone(),
            })));
            return Ok(());
        }

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                JobError::ExecutionFailed(format!(
                    "Failed to create metadata enrichment runtime: {e}"
                ))
            })?;

        let provider = if self.agent.enabled {
            Some(build_provider(&self.agent))
        } else {
            None
        };
        let mut processed = 0usize;
        let mut retryable_failures = 0usize;
        let mut permanent_failures = 0usize;
        for item in batch {
            if ctx.is_cancelled() {
                return Err(JobError::Cancelled);
            }

            let result = match provider.as_ref() {
                Some(provider) => runtime.block_on(self.enrich_queue_item(
                    ctx,
                    store.as_ref(),
                    provider.as_ref(),
                    &item,
                )),
                None => {
                    runtime.block_on(self.enrich_queue_item_without_llm(ctx, store.as_ref(), &item))
                }
            };

            match result {
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
            "seeded": seeded,
            "batch_size": batch_size,
            "entity_types": selected_entity_types.clone(),
            "provider": provider.as_ref().map(|provider| provider.name()).unwrap_or("deterministic"),
            "model": provider.as_ref().map(|provider| provider.model()).unwrap_or("wikidata"),
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

#[derive(Debug, Clone)]
struct WikidataArtistEnrichment {
    profile: ArtistEnrichmentV1,
    external_ids: Vec<EntityExternalIdV1>,
    sources: Vec<EntitySourceV1>,
    evidence: Vec<EntityEvidenceV1>,
}

impl WikidataArtistEnrichment {
    fn new(artist_id: &str, mbid: String, facts: WikidataArtistFacts, now: i64) -> Self {
        let wikidata_url = facts.wikidata_url();
        let qid = facts.qid.clone();
        let profile = ArtistEnrichmentV1 {
            artist_id: artist_id.to_string(),
            kind: Some(facts.kind.clone()),
            birth_date: facts.birth_date.clone(),
            death_date: facts.death_date.clone(),
            foundation_date: facts.foundation_date.clone(),
            dissolution_date: facts.dissolution_date.clone(),
            origin_place: facts.origin_place.clone(),
            origin_country: facts.origin_country.clone(),
            primary_language: None,
            is_person: Some(facts.kind == "person"),
            is_group: Some(facts.kind != "person"),
            is_composer: None,
            is_performer: None,
            is_conductor: None,
            is_producer: None,
            confidence: Some(1.0),
            summary: facts.description.clone(),
            bio: None,
            enriched_at: now,
            last_verified_at: Some(now),
            source_status: Some(WIKIDATA_SOURCE_STATUS.to_string()),
        };
        Self {
            profile,
            external_ids: dedupe_external_ids(vec![
                EntityExternalIdV1 {
                    provider: "musicbrainz".to_string(),
                    external_id: Some(mbid),
                    url: None,
                    confidence: Some(1.0),
                },
                EntityExternalIdV1 {
                    provider: "wikidata".to_string(),
                    external_id: Some(qid),
                    url: Some(wikidata_url.clone()),
                    confidence: Some(1.0),
                },
            ]),
            sources: vec![EntitySourceV1 {
                source_name: "wikidata".to_string(),
                source_url: Some(wikidata_url.clone()),
                retrieved_at: Some(now),
                confidence: Some(1.0),
            }],
            evidence: vec![EntityEvidenceV1 {
                source_name: Some("wikidata_sparql".to_string()),
                source_url: Some(wikidata_url),
                snippet: facts.description,
                raw_payload: facts.raw_payload,
            }],
        }
    }

    fn prompt_context(&self) -> Value {
        serde_json::json!({
            "kind": self.profile.kind.clone(),
            "birth_date": self.profile.birth_date.clone(),
            "death_date": self.profile.death_date.clone(),
            "foundation_date": self.profile.foundation_date.clone(),
            "dissolution_date": self.profile.dissolution_date.clone(),
            "origin_place": self.profile.origin_place.clone(),
            "origin_country": self.profile.origin_country.clone(),
            "summary": self.profile.summary.clone(),
            "external_ids": self.external_ids.clone(),
        })
    }
}

fn source_status_needs_llm_completion(source_status: &str) -> bool {
    let mut parts = source_status.split('+');
    let has_wikidata = parts.clone().any(|part| part == WIKIDATA_SOURCE_STATUS);
    let has_llm = parts.any(|part| part == GENERATED_SOURCE_STATUS);
    has_wikidata && !has_llm
}

fn is_wikidata_backed_profile(profile: &ArtistEnrichmentV1) -> bool {
    profile
        .source_status
        .as_deref()
        .map(|status| status.split('+').any(|part| part == WIKIDATA_SOURCE_STATUS))
        .unwrap_or(false)
}

fn artist_profile_from_output(
    artist_id: &str,
    output: &MetadataOutput,
    wikidata: Option<&ArtistEnrichmentV1>,
    now: i64,
) -> ArtistEnrichmentV1 {
    ArtistEnrichmentV1 {
        artist_id: artist_id.to_string(),
        kind: wikidata
            .and_then(|profile| profile.kind.clone())
            .or_else(|| output.kind.clone()),
        birth_date: wikidata
            .and_then(|profile| profile.birth_date.clone())
            .or_else(|| output.birth_date.clone()),
        death_date: wikidata
            .and_then(|profile| profile.death_date.clone())
            .or_else(|| output.death_date.clone()),
        foundation_date: wikidata
            .and_then(|profile| profile.foundation_date.clone())
            .or_else(|| output.foundation_date.clone()),
        dissolution_date: wikidata
            .and_then(|profile| profile.dissolution_date.clone())
            .or_else(|| output.dissolution_date.clone()),
        origin_place: wikidata
            .and_then(|profile| profile.origin_place.clone())
            .or_else(|| output.origin_place.clone()),
        origin_country: wikidata
            .and_then(|profile| profile.origin_country.clone())
            .or_else(|| output.origin_country.clone()),
        primary_language: output
            .primary_language
            .clone()
            .or_else(|| wikidata.and_then(|profile| profile.primary_language.clone())),
        is_person: wikidata
            .and_then(|profile| profile.is_person)
            .or(output.is_person),
        is_group: wikidata
            .and_then(|profile| profile.is_group)
            .or(output.is_group),
        is_composer: output
            .is_composer
            .or_else(|| wikidata.and_then(|profile| profile.is_composer)),
        is_performer: output
            .is_performer
            .or_else(|| wikidata.and_then(|profile| profile.is_performer)),
        is_conductor: output
            .is_conductor
            .or_else(|| wikidata.and_then(|profile| profile.is_conductor)),
        is_producer: output
            .is_producer
            .or_else(|| wikidata.and_then(|profile| profile.is_producer)),
        confidence: match (
            wikidata.and_then(|profile| profile.confidence),
            output.confidence,
        ) {
            (Some(wikidata_confidence), Some(output_confidence)) => {
                Some(wikidata_confidence.max(output_confidence))
            }
            (Some(wikidata_confidence), None) => Some(wikidata_confidence),
            (None, output_confidence) => output_confidence,
        },
        summary: output
            .summary
            .clone()
            .or_else(|| wikidata.and_then(|profile| profile.summary.clone())),
        bio: output
            .bio
            .clone()
            .or_else(|| wikidata.and_then(|profile| profile.bio.clone())),
        enriched_at: now,
        last_verified_at: Some(now),
        source_status: Some(artist_source_status(
            output.source_status.as_deref(),
            wikidata,
        )),
    }
}

fn artist_source_status(
    output_status: Option<&str>,
    wikidata: Option<&ArtistEnrichmentV1>,
) -> String {
    let output_status = output_status.unwrap_or(GENERATED_SOURCE_STATUS);
    if wikidata.is_some()
        && !output_status
            .split('+')
            .any(|part| part == WIKIDATA_SOURCE_STATUS)
    {
        format!("{WIKIDATA_SOURCE_STATUS}+{output_status}")
    } else {
        output_status.to_string()
    }
}

struct WikidataClient {
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct SparqlResponse {
    results: SparqlResults,
}

#[derive(Debug, Deserialize)]
struct SparqlResults {
    bindings: Vec<WikidataArtistBinding>,
}

#[derive(Debug, Deserialize)]
struct SparqlValue {
    value: String,
}

#[derive(Debug, Deserialize)]
struct WikidataArtistBinding {
    item: SparqlValue,
    #[serde(rename = "itemDescription")]
    item_description: Option<SparqlValue>,
    birth: Option<SparqlValue>,
    death: Option<SparqlValue>,
    #[serde(rename = "birthplaceLabel")]
    birthplace_label: Option<SparqlValue>,
    #[serde(rename = "countryLabel")]
    country_label: Option<SparqlValue>,
    inception: Option<SparqlValue>,
    dissolved: Option<SparqlValue>,
    #[serde(rename = "formationPlaceLabel")]
    formation_place_label: Option<SparqlValue>,
}

#[derive(Debug, Clone)]
struct WikidataArtistFacts {
    qid: String,
    kind: String,
    birth_date: Option<String>,
    death_date: Option<String>,
    foundation_date: Option<String>,
    dissolution_date: Option<String>,
    origin_place: Option<String>,
    origin_country: Option<String>,
    description: Option<String>,
    raw_payload: Option<Value>,
}

impl WikidataClient {
    fn new() -> anyhow::Result<Self> {
        let user_agent = format!(
            "pezzottify-server/{} metadata-enrichment (Wikidata lookup)",
            env!("CARGO_PKG_VERSION")
        );
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent(user_agent)
                .timeout(Duration::from_secs(20))
                .build()?,
        })
    }

    async fn lookup_artist_by_mbid(
        &self,
        mbid: &str,
    ) -> anyhow::Result<Option<WikidataArtistFacts>> {
        let query = wikidata_artist_query(mbid);
        let raw_payload: Value = self
            .client
            .get(WIKIDATA_SPARQL_URL)
            .query(&[("query", query.as_str()), ("format", "json")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let response: SparqlResponse = serde_json::from_value(raw_payload.clone())?;
        Ok(response
            .results
            .bindings
            .into_iter()
            .next()
            .and_then(|binding| WikidataArtistFacts::from_binding(binding, Some(raw_payload))))
    }
}

impl WikidataArtistFacts {
    fn from_binding(binding: WikidataArtistBinding, raw_payload: Option<Value>) -> Option<Self> {
        let qid = qid_from_wikidata_url(&binding.item.value)?;
        let birth_date = binding.birth.as_ref().and_then(|v| wikidata_date(&v.value));
        let death_date = binding.death.as_ref().and_then(|v| wikidata_date(&v.value));
        let foundation_date = binding
            .inception
            .as_ref()
            .and_then(|v| wikidata_date(&v.value));
        let dissolution_date = binding
            .dissolved
            .as_ref()
            .and_then(|v| wikidata_date(&v.value));
        let birthplace = binding.birthplace_label.map(|v| v.value);
        let formation_place = binding.formation_place_label.map(|v| v.value);
        let country = binding.country_label.map(|v| v.value);
        let kind = if birth_date.is_some() || death_date.is_some() || birthplace.is_some() {
            "person"
        } else {
            "group"
        }
        .to_string();
        Some(Self {
            qid,
            kind,
            birth_date,
            death_date,
            foundation_date,
            dissolution_date,
            origin_place: birthplace.or(formation_place),
            origin_country: country,
            description: binding.item_description.map(|v| v.value),
            raw_payload,
        })
    }

    fn has_profile_data(&self) -> bool {
        self.birth_date.is_some()
            || self.death_date.is_some()
            || self.foundation_date.is_some()
            || self.dissolution_date.is_some()
            || self.origin_place.is_some()
            || self.origin_country.is_some()
            || self.description.is_some()
    }

    fn wikidata_url(&self) -> String {
        format!("https://www.wikidata.org/wiki/{}", self.qid)
    }
}

fn wikidata_artist_query(mbid: &str) -> String {
    let escaped_mbid = mbid.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        r#"SELECT ?item ?itemDescription ?birth ?death ?birthplaceLabel ?countryLabel ?inception ?dissolved ?formationPlaceLabel WHERE {{
  ?item wdt:P434 "{escaped_mbid}".
  OPTIONAL {{ ?item wdt:P569 ?birth. }}
  OPTIONAL {{ ?item wdt:P570 ?death. }}
  OPTIONAL {{ ?item wdt:P19 ?birthplace. }}
  OPTIONAL {{ ?item wdt:P27 ?country. }}
  OPTIONAL {{ ?item wdt:P571 ?inception. }}
  OPTIONAL {{ ?item wdt:P576 ?dissolved. }}
  OPTIONAL {{ ?item wdt:P740 ?formationPlace. }}
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "en". }}
}}
LIMIT 1"#
    )
}

fn qid_from_wikidata_url(url: &str) -> Option<String> {
    let qid = url.rsplit('/').next()?.trim();
    if qid.starts_with('Q') && qid[1..].chars().all(|c| c.is_ascii_digit()) {
        Some(qid.to_string())
    } else {
        None
    }
}

fn wikidata_date(value: &str) -> Option<String> {
    let date = value
        .trim_start_matches('+')
        .split('T')
        .next()
        .unwrap_or(value)
        .trim();
    if date.len() < 4 || date.starts_with("0000") {
        return None;
    }
    if date.len() >= 10 && !date[5..7].eq("00") && !date[8..10].eq("00") {
        Some(date[..10].to_string())
    } else if date.len() >= 7 && !date[5..7].eq("00") {
        Some(date[..7].to_string())
    } else {
        Some(date[..4].to_string())
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
  "origin_place": "birthplace for people, formation/origin city for groups|null",
  "origin_country": "birth country for people, formation/origin country for groups|null",
  "primary_language": "ISO 639-1 or language name|null",
  "is_person": true,
  "is_group": false,
  "is_composer": false,
  "is_performer": true,
  "is_conductor": false,
  "is_producer": false,
  "confidence": 0.0,
  "summary": "one listener-facing sentence with the artist's significance|null",
  "bio": "short listener-facing paragraph including major roles such as singer, composer, songwriter, author, or producer when well-known|null",
  "source_status": "llm_inferred_v2",
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
  "source_status": "llm_inferred_v2",
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
  "source_status": "llm_inferred_v2",
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

    fn track_context(
        track_id: &str,
        play_count: u64,
        album_id: Option<&str>,
        artist_ids: &[&str],
    ) -> ListenedTrackContext {
        ListenedTrackContext {
            track_id: track_id.to_string(),
            play_count,
            album_id: album_id.map(str::to_string),
            artist_ids: artist_ids.iter().map(|id| id.to_string()).collect(),
        }
    }

    #[test]
    fn listening_backfill_candidates_aggregate_and_sort_all_types() {
        let tracks = vec![
            track_context("t1", 3, Some("album1"), &["artist1", "artist2"]),
            track_context("t2", 7, Some("album1"), &["artist1"]),
            track_context("t3", 5, Some("album2"), &["artist2", "artist2"]),
        ];

        let candidates = listening_backfill_candidates(&tracks, &[]);

        assert_eq!(
            candidates,
            vec![
                ListeningBackfillCandidate {
                    entity_type: "album".to_string(),
                    entity_id: "album1".to_string(),
                    priority: 10,
                },
                ListeningBackfillCandidate {
                    entity_type: "artist".to_string(),
                    entity_id: "artist1".to_string(),
                    priority: 10,
                },
                ListeningBackfillCandidate {
                    entity_type: "artist".to_string(),
                    entity_id: "artist2".to_string(),
                    priority: 8,
                },
                ListeningBackfillCandidate {
                    entity_type: "track".to_string(),
                    entity_id: "t2".to_string(),
                    priority: 7,
                },
                ListeningBackfillCandidate {
                    entity_type: "album".to_string(),
                    entity_id: "album2".to_string(),
                    priority: 5,
                },
                ListeningBackfillCandidate {
                    entity_type: "track".to_string(),
                    entity_id: "t3".to_string(),
                    priority: 5,
                },
                ListeningBackfillCandidate {
                    entity_type: "track".to_string(),
                    entity_id: "t1".to_string(),
                    priority: 3,
                },
            ]
        );
    }

    #[test]
    fn listening_backfill_candidates_respect_entity_type_filter_and_clamp_priority() {
        let tracks = vec![track_context("t1", u64::MAX, Some("album1"), &["artist1"])];

        let candidates = listening_backfill_candidates(&tracks, &["artist".to_string()]);

        assert_eq!(
            candidates,
            vec![ListeningBackfillCandidate {
                entity_type: "artist".to_string(),
                entity_id: "artist1".to_string(),
                priority: i64::MAX,
            }]
        );
    }

    fn listening_event(
        user_id: usize,
        track_id: &str,
        started_at: u64,
    ) -> crate::user::ListeningEvent {
        crate::user::ListeningEvent {
            id: None,
            user_id,
            track_id: track_id.to_string(),
            session_id: Some(format!("session-{track_id}-{started_at}")),
            started_at,
            ended_at: Some(started_at + 180),
            duration_seconds: 180,
            track_duration_seconds: 180,
            completed: true,
            seek_count: 0,
            pause_count: 0,
            playback_context: None,
            client_type: None,
            date: 20260524,
        }
    }

    fn test_job_context(
        temp_dir: &tempfile::TempDir,
    ) -> (
        JobContext,
        Arc<crate::user::SqliteUserStore>,
        Arc<crate::enrichment_store::SqliteEnrichmentStore>,
    ) {
        let registry = crate::backup::DbRegistry::new();
        let catalog_store: Arc<dyn crate::catalog_store::CatalogStore> =
            Arc::new(crate::catalog_store::NullCatalogStore);
        let user_store_impl = Arc::new(
            crate::user::SqliteUserStore::new(temp_dir.path().join("user.db"), &registry).unwrap(),
        );
        let user_store: Arc<dyn crate::user::FullUserStore> = user_store_impl.clone();
        let server_store: Arc<dyn crate::server_store::ServerStore> = Arc::new(
            crate::server_store::SqliteServerStore::new(
                temp_dir.path().join("server.db"),
                &registry,
            )
            .unwrap(),
        );
        let user_manager = Arc::new(std::sync::Mutex::new(crate::user::UserManager::new(
            catalog_store.clone(),
            user_store.clone(),
        )));
        let enrichment_store_impl = Arc::new(
            crate::enrichment_store::SqliteEnrichmentStore::new(
                temp_dir.path().join("enrichment.db"),
                &registry,
            )
            .unwrap(),
        );
        let enrichment_store: Arc<dyn EnrichmentStore> = enrichment_store_impl.clone();
        let ctx = JobContext::new(
            tokio_util::sync::CancellationToken::new(),
            catalog_store,
            user_store,
            server_store,
            user_manager,
        )
        .with_enrichment_store(enrichment_store);

        (ctx, user_store_impl, enrichment_store_impl)
    }

    #[test]
    fn empty_queue_seeds_listened_tracks_and_claims_batch() {
        use crate::user::{UserListeningStore, UserStore};

        let temp_dir = tempfile::TempDir::new().unwrap();
        let (ctx, user_store, enrichment_store) = test_job_context(&temp_dir);
        let user_id = user_store.create_user("listener").unwrap();
        user_store
            .record_listening_event(listening_event(user_id, "track1", 1_000))
            .unwrap();
        user_store
            .record_listening_event(listening_event(user_id, "track1", 2_000))
            .unwrap();
        user_store
            .record_listening_event(listening_event(user_id, "track2", 3_000))
            .unwrap();
        user_store
            .record_listening_event(listening_event(user_id, "track3", 4_000))
            .unwrap();

        let job = MetadataEnrichmentJob::from_settings(
            &MetadataEnrichmentJobSettings {
                interval_hours: 6,
                batch_size: 2,
                retry_after_secs: 60,
            },
            AgentSettings {
                enabled: false,
                ..AgentSettings::default()
            },
        );

        job.execute_with_params(
            &ctx,
            Some(serde_json::json!({
                "entity_types": ["track"]
            })),
        )
        .unwrap();

        let items = ["track1", "track2", "track3"]
            .into_iter()
            .map(|track_id| {
                enrichment_store
                    .get_enrichment_queue_item("track", track_id)
                    .unwrap()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(items.iter().filter(|item| item.attempts == 1).count(), 2);
        assert_eq!(items.iter().filter(|item| item.attempts == 0).count(), 1);
        assert_eq!(
            items
                .iter()
                .filter(|item| item.reason.as_deref() == Some(LISTENING_BACKFILL_REASON))
                .count(),
            3
        );
        assert_eq!(
            enrichment_store
                .get_enrichment_queue_item("track", "track1")
                .unwrap()
                .unwrap()
                .priority,
            2
        );
    }

    #[test]
    fn empty_queue_with_no_listening_data_seeds_nothing() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let (ctx, _user_store, enrichment_store) = test_job_context(&temp_dir);
        let job = MetadataEnrichmentJob::from_settings(
            &MetadataEnrichmentJobSettings {
                interval_hours: 6,
                batch_size: 2,
                retry_after_secs: 60,
            },
            AgentSettings {
                enabled: false,
                ..AgentSettings::default()
            },
        );

        job.execute_with_params(
            &ctx,
            Some(serde_json::json!({
                "entity_types": ["track"]
            })),
        )
        .unwrap();

        assert!(enrichment_store
            .claim_enrichment_queue_batch(10)
            .unwrap()
            .is_empty());
    }

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
    fn wikidata_helpers_parse_qids_dates_and_artist_facts() {
        assert_eq!(
            qid_from_wikidata_url("https://www.wikidata.org/entity/Q123"),
            Some("Q123".to_string())
        );
        assert_eq!(
            wikidata_date("+1945-03-23T00:00:00Z"),
            Some("1945-03-23".to_string())
        );
        assert_eq!(
            wikidata_date("+1945-00-00T00:00:00Z"),
            Some("1945".to_string())
        );

        let binding = WikidataArtistBinding {
            item: SparqlValue {
                value: "http://www.wikidata.org/entity/Q364723".to_string(),
            },
            item_description: Some(SparqlValue {
                value: "Italian singer-songwriter and composer".to_string(),
            }),
            birth: Some(SparqlValue {
                value: "1945-03-23T00:00:00Z".to_string(),
            }),
            death: Some(SparqlValue {
                value: "2021-05-18T00:00:00Z".to_string(),
            }),
            birthplace_label: Some(SparqlValue {
                value: "Ionia".to_string(),
            }),
            country_label: Some(SparqlValue {
                value: "Italy".to_string(),
            }),
            inception: None,
            dissolved: None,
            formation_place_label: None,
        };

        let facts = WikidataArtistFacts::from_binding(binding, None).unwrap();
        assert_eq!(facts.qid, "Q364723");
        assert_eq!(facts.kind, "person");
        assert_eq!(facts.birth_date.as_deref(), Some("1945-03-23"));
        assert_eq!(facts.death_date.as_deref(), Some("2021-05-18"));
        assert_eq!(facts.origin_place.as_deref(), Some("Ionia"));
        assert_eq!(facts.origin_country.as_deref(), Some("Italy"));
        assert!(facts.has_profile_data());
    }

    #[test]
    fn source_status_completion_detects_wikidata_without_llm() {
        assert!(source_status_needs_llm_completion("wikidata"));
        assert!(source_status_needs_llm_completion("wikidata+verified"));
        assert!(!source_status_needs_llm_completion(
            "wikidata+llm_inferred_v2"
        ));
        assert!(!source_status_needs_llm_completion("llm_inferred_v2"));
    }

    #[test]
    fn artist_profile_merge_preserves_wikidata_facts_and_uses_llm_fillers() {
        let wikidata = ArtistEnrichmentV1 {
            artist_id: "artist1".to_string(),
            kind: Some("person".to_string()),
            birth_date: Some("1945-03-23".to_string()),
            death_date: None,
            foundation_date: None,
            dissolution_date: None,
            origin_place: Some("Ionia".to_string()),
            origin_country: Some("Italy".to_string()),
            primary_language: None,
            is_person: Some(true),
            is_group: Some(false),
            is_composer: None,
            is_performer: None,
            is_conductor: None,
            is_producer: None,
            confidence: Some(1.0),
            summary: Some("Wikidata description".to_string()),
            bio: None,
            enriched_at: 10,
            last_verified_at: Some(10),
            source_status: Some(WIKIDATA_SOURCE_STATUS.to_string()),
        };
        let output = MetadataOutput {
            kind: Some("group".to_string()),
            birth_date: Some("1900".to_string()),
            origin_place: Some("Wrong place".to_string()),
            primary_language: Some("it".to_string()),
            is_person: Some(false),
            is_group: Some(true),
            is_composer: Some(true),
            is_performer: Some(true),
            confidence: Some(0.6),
            summary: Some("LLM summary".to_string()),
            bio: Some("LLM bio".to_string()),
            ..MetadataOutput::default()
        };

        let merged = artist_profile_from_output("artist1", &output, Some(&wikidata), 20);

        assert_eq!(merged.kind.as_deref(), Some("person"));
        assert_eq!(merged.birth_date.as_deref(), Some("1945-03-23"));
        assert_eq!(merged.origin_place.as_deref(), Some("Ionia"));
        assert_eq!(merged.origin_country.as_deref(), Some("Italy"));
        assert_eq!(merged.primary_language.as_deref(), Some("it"));
        assert_eq!(merged.is_person, Some(true));
        assert_eq!(merged.is_group, Some(false));
        assert_eq!(merged.is_composer, Some(true));
        assert_eq!(merged.summary.as_deref(), Some("LLM summary"));
        assert_eq!(merged.bio.as_deref(), Some("LLM bio"));
        assert_eq!(merged.confidence, Some(1.0));
        assert_eq!(
            merged.source_status.as_deref(),
            Some("wikidata+llm_inferred_v2")
        );
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
