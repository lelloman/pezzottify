//! Track embedding synchronization job.
//!
//! Keeps configured audio embedding namespaces populated for available tracks by
//! calling the external Simple-AI audio embedding endpoint.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::catalog_store::EntityEmbeddingUpsert;
use crate::config::{AudioEmbeddingSpec, AudioEmbeddingsSettings};
use reqwest::blocking::{multipart, Client};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{info, warn};

#[derive(Clone)]
pub struct TrackEmbeddingSyncJob {
    settings: AudioEmbeddingsSettings,
    media_path: PathBuf,
}

#[derive(Debug, Deserialize, Default)]
struct TrackEmbeddingSyncParams {
    max_tracks: Option<usize>,
    force: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimpleAiAudioEmbeddingResponse {
    model: String,
    namespace: String,
    embedding: Vec<f32>,
    dim: u32,
    dtype: String,
    #[serde(default)]
    metadata: Value,
    #[serde(default)]
    model_info: Value,
}

impl TrackEmbeddingSyncJob {
    pub fn new(settings: AudioEmbeddingsSettings, media_path: PathBuf) -> Self {
        Self {
            settings,
            media_path,
        }
    }

    fn namespaces(&self) -> Vec<String> {
        self.settings
            .specs
            .iter()
            .map(|spec| spec.namespace.clone())
            .collect()
    }

    fn specs_for_track(
        &self,
        ctx: &JobContext,
        track_id: &str,
        force: bool,
    ) -> Result<Vec<AudioEmbeddingSpec>, JobError> {
        if force {
            return Ok(self.settings.specs.clone());
        }

        let mut missing = Vec::new();
        for spec in &self.settings.specs {
            let existing = ctx
                .catalog_store
                .get_entity_embedding("track", track_id, &spec.namespace, false)
                .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;
            if existing.is_none() {
                missing.push(spec.clone());
            }
        }
        Ok(missing)
    }

    fn request_embedding(
        &self,
        client: &Client,
        audio_path: &Path,
        spec: &AudioEmbeddingSpec,
    ) -> Result<SimpleAiAudioEmbeddingResponse, JobError> {
        let options = json!({
            "model": spec.model,
            "namespace": spec.namespace,
        });
        let form = multipart::Form::new()
            .file("file", audio_path)
            .map_err(|e| JobError::ExecutionFailed(e.to_string()))?
            .text("options", options.to_string());

        let url = format!(
            "{}/v1/audio/embeddings",
            self.settings.simple_ai_base_url.trim_end_matches('/')
        );
        let response = client
            .post(url)
            .bearer_auth(&self.settings.api_key)
            .multipart(form)
            .send()
            .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(JobError::ExecutionFailed(format!(
                "Simple-AI returned HTTP {status}: {body}"
            )));
        }

        response
            .json()
            .map_err(|e| JobError::ExecutionFailed(e.to_string()))
    }

    fn execute_inner(
        &self,
        ctx: &JobContext,
        params: TrackEmbeddingSyncParams,
    ) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(ctx.server_store.clone(), self.id());
        let started_at = Instant::now();
        let max_tracks = params
            .max_tracks
            .unwrap_or(self.settings.max_tracks_per_run)
            .max(1);
        let force = params.force.unwrap_or(false);
        let namespaces = self.namespaces();

        audit.log_started(Some(json!({
            "max_tracks": max_tracks,
            "force": force,
            "namespaces": namespaces,
        })));

        let tracks = if force {
            ctx.catalog_store
                .list_available_track_ids_with_audio_uri(max_tracks, 0)
        } else {
            ctx.catalog_store
                .list_available_tracks_missing_embeddings(&namespaces, max_tracks)
        }
        .map_err(|e| {
            let msg = format!("Failed to select tracks for embedding sync: {e}");
            audit.log_failed(&msg, None);
            JobError::ExecutionFailed(msg)
        })?;

        let client = Client::builder()
            .timeout(Duration::from_secs(self.settings.request_timeout_secs))
            .build()
            .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;

        let mut tracks_processed = 0usize;
        let mut tracks_skipped = 0usize;
        let mut embeddings_stored = 0usize;
        let mut failures = 0usize;

        for (track_id, audio_uri) in tracks {
            if ctx.is_cancelled() {
                audit.log_failed("Cancelled", None);
                return Err(JobError::Cancelled);
            }

            let audio_path = self.media_path.join(&audio_uri);
            if !audio_path.is_file() {
                failures += 1;
                warn!(
                    "Skipping track {} because audio file is missing: {}",
                    track_id,
                    audio_path.display()
                );
                continue;
            }

            let specs = self.specs_for_track(ctx, &track_id, force)?;
            if specs.is_empty() {
                tracks_skipped += 1;
                continue;
            }

            tracks_processed += 1;
            for spec in specs {
                if ctx.is_cancelled() {
                    audit.log_failed("Cancelled", None);
                    return Err(JobError::Cancelled);
                }

                match self.request_embedding(&client, &audio_path, &spec) {
                    Ok(response) => {
                        if response.embedding.len() != response.dim as usize {
                            failures += 1;
                            warn!(
                                "Skipping invalid embedding for track {} namespace {}: dim={} len={}",
                                track_id,
                                response.namespace,
                                response.dim,
                                response.embedding.len()
                            );
                            continue;
                        }

                        let upsert = EntityEmbeddingUpsert {
                            entity_type: "track".to_string(),
                            entity_id: track_id.clone(),
                            namespace: response.namespace.clone(),
                            vector: response.embedding,
                            dtype: response.dtype,
                            metadata: json!({
                                "simpleAiMetadata": response.metadata,
                                "sourceAudioUri": audio_uri,
                            }),
                            model: json!({
                                "id": response.model,
                                "info": response.model_info,
                            }),
                        };
                        match ctx.catalog_store.upsert_entity_embedding(&upsert) {
                            Ok(_) => embeddings_stored += 1,
                            Err(e) => {
                                failures += 1;
                                warn!(
                                    "Failed to store embedding for track {} namespace {}: {}",
                                    track_id, response.namespace, e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        failures += 1;
                        warn!(
                            "Failed to generate embedding for track {} namespace {}: {}",
                            track_id, spec.namespace, e
                        );
                    }
                }
            }

            if tracks_processed % 50 == 0 {
                audit.log_progress(json!({
                    "tracks_processed": tracks_processed,
                    "tracks_skipped": tracks_skipped,
                    "embeddings_stored": embeddings_stored,
                    "failures": failures,
                }));
            }
        }

        let duration_ms = started_at.elapsed().as_millis() as u64;
        info!(
            "Track embedding sync completed: processed={} skipped={} stored={} failures={} duration_ms={}",
            tracks_processed, tracks_skipped, embeddings_stored, failures, duration_ms
        );
        audit.log_completed(Some(json!({
            "duration_ms": duration_ms,
            "tracks_processed": tracks_processed,
            "tracks_skipped": tracks_skipped,
            "embeddings_stored": embeddings_stored,
            "failures": failures,
        })));

        Ok(())
    }
}

impl BackgroundJob for TrackEmbeddingSyncJob {
    fn id(&self) -> &'static str {
        "track_embedding_sync"
    }

    fn name(&self) -> &'static str {
        "Track Embedding Sync"
    }

    fn description(&self) -> &'static str {
        "Generate missing track audio embeddings through Simple-AI"
    }

    fn schedule(&self) -> JobSchedule {
        JobSchedule::JitteredInterval {
            interval: Duration::from_secs(self.settings.interval_hours * 60 * 60),
            jitter: Duration::from_secs(self.settings.jitter_minutes * 60),
        }
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        self.execute_inner(ctx, TrackEmbeddingSyncParams::default())
    }

    fn execute_with_params(&self, ctx: &JobContext, params: Option<Value>) -> Result<(), JobError> {
        let params = match params {
            Some(value) => serde_json::from_value(value)
                .map_err(|e| JobError::ExecutionFailed(format!("Invalid params: {e}")))?,
            None => TrackEmbeddingSyncParams::default(),
        };
        self.execute_inner(ctx, params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AlbumEmbeddingDerivationSpec, AlbumEmbeddingDerivationsSettings};

    fn test_settings() -> AudioEmbeddingsSettings {
        AudioEmbeddingsSettings {
            enabled: true,
            simple_ai_base_url: "http://simple-ai:8000".to_string(),
            api_key: "secret".to_string(),
            interval_hours: 6,
            jitter_minutes: 15,
            max_tracks_per_run: 123,
            request_timeout_secs: 30,
            specs: AudioEmbeddingSpec::defaults(),
            album_derivations: AlbumEmbeddingDerivationsSettings {
                enabled: true,
                interval_hours: 6,
                jitter_minutes: 15,
                max_albums_per_run: 1000,
                specs: AlbumEmbeddingDerivationSpec::defaults(),
            },
        }
    }

    #[test]
    fn schedule_uses_configured_interval_and_jitter() {
        let job = TrackEmbeddingSyncJob::new(test_settings(), PathBuf::from("/media"));

        match job.schedule() {
            JobSchedule::JitteredInterval { interval, jitter } => {
                assert_eq!(interval, Duration::from_secs(6 * 60 * 60));
                assert_eq!(jitter, Duration::from_secs(15 * 60));
            }
            other => panic!("unexpected schedule: {:?}", other),
        }
    }

    #[test]
    fn namespaces_are_derived_from_configured_specs() {
        let settings = AudioEmbeddingsSettings {
            specs: vec![
                AudioEmbeddingSpec {
                    model: "model-a".to_string(),
                    namespace: "namespace.a".to_string(),
                },
                AudioEmbeddingSpec {
                    model: "model-b".to_string(),
                    namespace: "namespace.b".to_string(),
                },
            ],
            ..test_settings()
        };
        let job = TrackEmbeddingSyncJob::new(settings, PathBuf::from("/media"));

        assert_eq!(
            job.namespaces(),
            vec!["namespace.a".to_string(), "namespace.b".to_string()]
        );
    }

    #[test]
    fn params_deserialize_manual_run_options() {
        let params: TrackEmbeddingSyncParams =
            serde_json::from_value(json!({"max_tracks": 7, "force": true})).unwrap();

        assert_eq!(params.max_tracks, Some(7));
        assert_eq!(params.force, Some(true));
    }

    #[test]
    fn simple_ai_response_deserializes_camel_case_model_info() {
        let response: SimpleAiAudioEmbeddingResponse = serde_json::from_value(json!({
            "model": "musicfm-msd",
            "namespace": "musicfm.mean.v1",
            "embedding": [0.1, 0.2],
            "dim": 2,
            "dtype": "float32",
            "metadata": {"pooling": "mean"},
            "modelInfo": {"embeddingDim": 1024}
        }))
        .unwrap();

        assert_eq!(response.model, "musicfm-msd");
        assert_eq!(response.namespace, "musicfm.mean.v1");
        assert_eq!(response.embedding, vec![0.1, 0.2]);
        assert_eq!(response.model_info["embeddingDim"], 1024);
    }
}
