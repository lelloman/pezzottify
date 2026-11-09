//! Track embedding synchronization job.
//!
//! Keeps configured audio embedding namespaces populated for available tracks by
//! calling the external Simple-AI audio embedding endpoint.

use crate::background_jobs::{
    context::JobContext,
    job::{
        BackgroundJob, JobError, JobExecutionPolicy, JobResourceClass, JobSchedule,
        ShutdownBehavior,
    },
    JobAuditLogger, SchedulerHandle,
};
use crate::catalog_store::EntityEmbeddingUpsert;
use crate::config::{AudioEmbeddingSpec, AudioEmbeddingsSettings};
use crate::db_executor::DbPriority;
use reqwest::blocking::{multipart, Client};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Scheduler id of the track embedding sync job.
pub const TRACK_EMBEDDING_SYNC_JOB_ID: &str = "track_embedding_sync";

/// How often a request for new tracks retries while the job is already running.
const NEW_TRACKS_RETRY_INTERVAL: Duration = Duration::from_secs(30);
/// How long a request for new tracks keeps retrying; covers the scheduled run's
/// two-hour runtime budget. After that the scheduled run picks the tracks up.
const NEW_TRACKS_RETRY_WINDOW: Duration = Duration::from_secs(3 * 60 * 60);

/// Embed freshly ingested tracks now instead of waiting for the scheduled run.
///
/// Triggers the track embedding sync for just these tracks in the background.
/// While the job is already running (the scheduled run, or another album's
/// request) it retries; any other refusal (job not configured, paused, circuit
/// open) leaves the tracks to the scheduled run, which embeds every track still
/// missing an embedding.
pub fn request_track_embeddings(scheduler: SchedulerHandle, track_ids: Vec<String>) {
    if track_ids.is_empty() {
        return;
    }
    tokio::spawn(trigger_with_retry(
        scheduler,
        track_ids,
        NEW_TRACKS_RETRY_INTERVAL,
        NEW_TRACKS_RETRY_WINDOW,
    ));
}

async fn trigger_with_retry(
    scheduler: SchedulerHandle,
    track_ids: Vec<String>,
    retry_interval: Duration,
    retry_window: Duration,
) -> Result<(), JobError> {
    let count = track_ids.len();
    let params = json!({ "track_ids": track_ids });
    let deadline = tokio::time::Instant::now() + retry_window;
    loop {
        match scheduler
            .trigger_job(TRACK_EMBEDDING_SYNC_JOB_ID, Some(params.clone()))
            .await
        {
            Ok(()) => {
                info!("Started track embedding sync for {count} newly ingested track(s)");
                return Ok(());
            }
            Err(JobError::AlreadyRunning) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(retry_interval).await;
            }
            Err(JobError::NotFound) => {
                debug!("Track embedding sync is not configured; skipping new tracks");
                return Err(JobError::NotFound);
            }
            Err(e) => {
                info!(
                    "Leaving {count} newly ingested track(s) to the scheduled embedding sync: {e}"
                );
                return Err(e);
            }
        }
    }
}

#[derive(Clone)]
pub struct TrackEmbeddingSyncJob {
    settings: AudioEmbeddingsSettings,
}

#[derive(Debug, Deserialize, Default)]
struct TrackEmbeddingSyncParams {
    max_tracks: Option<usize>,
    force: Option<bool>,
    /// Only embed these tracks (still skipping namespaces they already have,
    /// unless `force`). Used right after ingestion.
    track_ids: Option<Vec<String>>,
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

/// Why an embedding request did not produce an embedding.
#[derive(Debug)]
enum EmbeddingRequestError {
    /// Simple-AI rejected this input (for example a file over the runner's size
    /// limit). Retrying will not help and it says nothing about the service.
    Rejected(String),
    /// Simple-AI, a runner or the network failed.
    Failed(String),
}

impl std::fmt::Display for EmbeddingRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(msg) => write!(f, "rejected: {msg}"),
            Self::Failed(msg) => write!(f, "{msg}"),
        }
    }
}

/// HTTP statuses that mean the request itself was unacceptable, as opposed to
/// authentication, routing, throttling or timeouts.
fn is_input_rejection_status(code: u16) -> bool {
    matches!(code, 400 | 413 | 415 | 422)
}

/// Classify a non-success Simple-AI response. The gateway reports a runner's
/// 4xx as its own HTTP 500 with "Runner returned HTTP <code>" in the body, so
/// both the outer status and the wrapped runner status are checked.
fn classify_http_failure(status: u16, body: &str) -> EmbeddingRequestError {
    let message = format!("Simple-AI returned HTTP {status}: {body}");
    let runner_status = body
        .split_once("Runner returned HTTP ")
        .and_then(|(_, rest)| rest.get(..3))
        .and_then(|code| code.parse::<u16>().ok());
    if is_input_rejection_status(status) || runner_status.is_some_and(is_input_rejection_status) {
        EmbeddingRequestError::Rejected(message)
    } else {
        EmbeddingRequestError::Failed(message)
    }
}

/// Whether a run should be reported as failed: it reached Simple-AI or local
/// storage and hit real failures, yet stored nothing. Input rejections are
/// excluded so permanently unembeddable tracks cannot fail every run.
fn run_stored_nothing_despite_failures(embeddings_stored: usize, failures: usize) -> bool {
    embeddings_stored == 0 && failures > 0
}

impl TrackEmbeddingSyncJob {
    pub fn new(settings: AudioEmbeddingsSettings) -> Self {
        Self { settings }
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
            let track_id = track_id.to_owned();
            let namespace = spec.namespace.clone();
            let existing = ctx
                .catalog_db
                .run_blocking(DbPriority::Background, move |store| {
                    store.get_entity_embedding("track", &track_id, &namespace, false)
                })
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
        audio: crate::media::LocalAudio,
        spec: &AudioEmbeddingSpec,
    ) -> Result<SimpleAiAudioEmbeddingResponse, EmbeddingRequestError> {
        let failed = |e: &dyn std::fmt::Display| EmbeddingRequestError::Failed(e.to_string());
        let options = json!({
            "model": spec.model,
            "namespace": spec.namespace,
        });
        let (reader, filename) = audio.into_reader();
        let length = reader.metadata().map_err(|e| failed(&e))?.len();
        let mime = mime_guess::from_path(&filename).first_or_octet_stream();
        let part = multipart::Part::reader_with_length(reader, length)
            .file_name(filename)
            .mime_str(mime.as_ref())
            .map_err(|e| failed(&e))?;
        let form = multipart::Form::new()
            .part("file", part)
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
            .map_err(|e| failed(&e))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(classify_http_failure(status, &body));
        }

        response.json().map_err(|e| failed(&e))
    }

    fn execute_inner(
        &self,
        ctx: &JobContext,
        params: TrackEmbeddingSyncParams,
    ) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(ctx.server_db.clone(), self.id());
        let started_at = Instant::now();
        let max_tracks = params
            .max_tracks
            .unwrap_or(self.settings.max_tracks_per_run)
            .max(1);
        let force = params.force.unwrap_or(false);
        let namespaces = self.namespaces();
        let requested_ids = params.track_ids;

        audit.log_started(Some(json!({
            "max_tracks": max_tracks,
            "force": force,
            "namespaces": namespaces,
            "track_ids": requested_ids,
        })));

        let tracks = ctx
            .catalog_db
            .run_blocking(DbPriority::Background, move |store| {
                if let Some(ids) = requested_ids {
                    let mut seen = std::collections::HashSet::new();
                    let mut tracks = Vec::new();
                    for id in ids.into_iter().filter(|id| seen.insert(id.clone())) {
                        if let Some(audio_uri) = store.get_track(&id)?.and_then(|t| t.audio_uri) {
                            tracks.push((id, audio_uri));
                        }
                    }
                    tracks.truncate(max_tracks);
                    Ok(tracks)
                } else if force {
                    store.list_available_track_ids_with_audio_uri(max_tracks, 0)
                } else {
                    store.list_available_tracks_missing_embeddings(&namespaces, max_tracks)
                }
            })
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
        let mut rejected = 0usize;

        for (track_id, audio_uri) in tracks {
            if ctx.is_cancelled() {
                audit.log_failed("Cancelled", None);
                return Err(JobError::Cancelled);
            }

            // Check local availability before counting a processed track, retaining
            // the descriptor for the first upload instead of reopening its path.
            let mut first_audio = match ctx.media.open_local_audio_blocking(&track_id) {
                Ok(Some(audio)) => Some(audio),
                result => {
                    failures += 1;
                    warn!(
                        track_id,
                        failed = result.is_err(),
                        "Skipping track because local audio cannot be opened"
                    );
                    continue;
                }
            };

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

                let opened = match first_audio.take() {
                    Some(audio) => Ok(Some(audio)),
                    None => ctx.media.open_local_audio_blocking(&track_id),
                };
                let audio = match opened {
                    Ok(Some(audio)) => audio,
                    result => {
                        failures += 1;
                        warn!(
                            track_id,
                            failed = result.is_err(),
                            "Cannot open local audio for embedding"
                        );
                        continue;
                    }
                };
                match self.request_embedding(&client, audio, &spec) {
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
                        match ctx
                            .catalog_db
                            .run_blocking(DbPriority::Background, move |store| {
                                store.upsert_entity_embedding(&upsert)
                            }) {
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
                    Err(EmbeddingRequestError::Rejected(msg)) => {
                        rejected += 1;
                        warn!(
                            "Simple-AI rejected track {} for namespace {}: {}",
                            track_id, spec.namespace, msg
                        );
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

            if tracks_processed.is_multiple_of(50) {
                audit.log_progress(json!({
                    "tracks_processed": tracks_processed,
                    "tracks_skipped": tracks_skipped,
                    "embeddings_stored": embeddings_stored,
                    "failures": failures,
                    "rejected": rejected,
                }));
            }
        }

        let duration_ms = started_at.elapsed().as_millis() as u64;
        let summary = json!({
            "duration_ms": duration_ms,
            "tracks_processed": tracks_processed,
            "tracks_skipped": tracks_skipped,
            "embeddings_stored": embeddings_stored,
            "failures": failures,
            "rejected": rejected,
        });

        // A run that stores nothing while hitting failures means the embedding
        // pipeline is broken (runner environment, gateway, media mount). Report it
        // as a failed run so the job failure metric and alert fire.
        if run_stored_nothing_despite_failures(embeddings_stored, failures) {
            let msg = format!(
                "Track embedding sync stored no embeddings: failures={failures} rejected={rejected} processed={tracks_processed}"
            );
            warn!("{msg}");
            audit.log_failed(&msg, Some(summary));
            return Err(JobError::ExecutionFailed(msg));
        }

        info!(
            "Track embedding sync completed: processed={} skipped={} stored={} failures={} rejected={} duration_ms={}",
            tracks_processed, tracks_skipped, embeddings_stored, failures, rejected, duration_ms
        );
        audit.log_completed(Some(summary));

        Ok(())
    }
}

impl BackgroundJob for TrackEmbeddingSyncJob {
    fn id(&self) -> &'static str {
        TRACK_EMBEDDING_SYNC_JOB_ID
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

    fn execution_policy(&self) -> JobExecutionPolicy {
        JobExecutionPolicy::new(JobResourceClass::IoBound)
            .with_queue_timeout(Duration::from_secs(60))
            .with_max_runtime(Duration::from_secs(2 * 60 * 60))
            .with_circuit_breaker(3, Duration::from_secs(60 * 60))
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
            concepts: Default::default(),
        }
    }

    #[test]
    fn schedule_uses_configured_interval_and_jitter() {
        let job = TrackEmbeddingSyncJob::new(test_settings());

        match job.schedule() {
            JobSchedule::JitteredInterval { interval, jitter } => {
                assert_eq!(interval, Duration::from_secs(6 * 60 * 60));
                assert_eq!(jitter, Duration::from_secs(15 * 60));
            }
            other => panic!("unexpected schedule: {:?}", other),
        }
    }

    #[test]
    fn execution_policy_isolates_remote_embedding_generation() {
        let policy = TrackEmbeddingSyncJob::new(test_settings()).execution_policy();

        assert_eq!(policy.resource_class, JobResourceClass::IoBound);
        assert_eq!(policy.queue_timeout, Duration::from_secs(60));
        assert_eq!(policy.max_runtime, Some(Duration::from_secs(2 * 60 * 60)));
        let breaker = policy.circuit_breaker.unwrap();
        assert_eq!(breaker.failure_threshold, 3);
        assert_eq!(breaker.cooldown, Duration::from_secs(60 * 60));
    }

    #[test]
    fn namespaces_are_derived_from_configured_specs() {
        let settings = AudioEmbeddingsSettings {
            specs: vec![
                AudioEmbeddingSpec {
                    model: "model-a".to_string(),
                    namespace: "namespace.a".to_string(),
                    serve: true,
                },
                AudioEmbeddingSpec {
                    model: "model-b".to_string(),
                    namespace: "namespace.b".to_string(),
                    // Unserved namespaces are still backfilled.
                    serve: false,
                },
            ],
            ..test_settings()
        };
        let job = TrackEmbeddingSyncJob::new(settings);

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
        assert_eq!(params.track_ids, None);

        let params: TrackEmbeddingSyncParams =
            serde_json::from_value(json!({"track_ids": ["a", "b"]})).unwrap();
        assert_eq!(
            params.track_ids,
            Some(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn gateway_wrapped_runner_size_rejection_is_not_a_failure() {
        let body = r#"Runner returned HTTP 400: {"error":{"message":"Invalid request: file exceeds runner audio embedding limit of 104857600 bytes"}}"#;
        assert!(matches!(
            classify_http_failure(500, body),
            EmbeddingRequestError::Rejected(_)
        ));
        assert!(matches!(
            classify_http_failure(413, ""),
            EmbeddingRequestError::Rejected(_)
        ));
    }

    #[test]
    fn service_auth_and_runner_errors_are_failures() {
        for (status, body) in [
            (500, "Runner returned HTTP 500: provider exited"),
            (500, "Runner returned HTTP 503: draining"),
            (502, "bad gateway"),
            (503, ""),
            (401, "missing key"),
            (429, "slow down"),
            (404, "not found"),
        ] {
            assert!(
                matches!(
                    classify_http_failure(status, body),
                    EmbeddingRequestError::Failed(_)
                ),
                "{status} {body}"
            );
        }
    }

    #[test]
    fn run_fails_only_when_failures_stored_nothing() {
        assert!(run_stored_nothing_despite_failures(0, 3));
        assert!(!run_stored_nothing_despite_failures(1, 3));
        assert!(!run_stored_nothing_despite_failures(0, 0));
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

    mod new_tracks {
        use super::*;
        use crate::background_jobs::circuit_breaker::CircuitBreakerRegistry;
        use crate::background_jobs::handle::{SchedulerCommand, SharedJobState};
        use crate::background_jobs::JobPauseState;
        use crate::backup::DbRegistry;
        use crate::catalog_store::{
            Album, AlbumAvailability, AlbumType, Artist, CatalogStore, SqliteCatalogStore, Track,
            TrackAvailability,
        };
        use crate::server_store::SqliteServerStore;
        use crate::user::{SqliteUserStore, UserManager};
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use tokio::sync::{mpsc, RwLock};
        use tokio_util::sync::CancellationToken;

        const NAMESPACE: &str = "test.ns.v1";

        /// Minimal Simple-AI stand-in: answers every request with a 2-d embedding.
        fn fake_simple_ai() -> (String, Arc<AtomicUsize>) {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let requests = Arc::new(AtomicUsize::new(0));
            let counter = requests.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { return };
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut length = 0usize;
                    let mut chunked = false;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 {
                            break;
                        }
                        let lower = line.to_ascii_lowercase();
                        if let Some(v) = lower.strip_prefix("content-length:") {
                            length = v.trim().parse().unwrap_or(0);
                        }
                        if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
                            chunked = true;
                        }
                        if line == "\r\n" {
                            break;
                        }
                    }
                    if chunked {
                        loop {
                            let mut size = String::new();
                            reader.read_line(&mut size).unwrap();
                            let n = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
                            let mut chunk = vec![0u8; n + 2];
                            reader.read_exact(&mut chunk).unwrap();
                            if n == 0 {
                                break;
                            }
                        }
                    } else {
                        let mut body = vec![0u8; length];
                        reader.read_exact(&mut body).unwrap();
                    }
                    counter.fetch_add(1, Ordering::SeqCst);
                    let body = json!({
                        "model": "test-model",
                        "namespace": NAMESPACE,
                        "embedding": [0.5, 0.25],
                        "dim": 2,
                        "dtype": "float32",
                    })
                    .to_string();
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                }
            });
            (url, requests)
        }

        fn seed(store: &SqliteCatalogStore, dir: &std::path::Path, album: &str, tracks: usize) {
            store
                .create_artist(&Artist {
                    id: "artist1".to_string(),
                    name: "Artist 1".to_string(),
                    genres: Vec::new(),
                    followers_total: 0,
                    popularity: 0,
                    available: true,
                })
                .ok();
            store
                .create_album(
                    &Album {
                        id: album.to_string(),
                        name: album.to_string(),
                        album_type: AlbumType::Album,
                        label: None,
                        release_date: None,
                        release_date_precision: None,
                        external_id_upc: None,
                        popularity: 0,
                        album_availability: AlbumAvailability::Complete,
                    },
                    &["artist1".to_string()],
                )
                .unwrap();
            for idx in 0..tracks {
                let id = format!("{album}_t{idx}");
                let uri = format!("{id}.ogg");
                std::fs::write(dir.join(&uri), b"audio").unwrap();
                store
                    .create_track(
                        &Track {
                            id,
                            name: format!("Track {idx}"),
                            album_id: album.to_string(),
                            disc_number: 1,
                            track_number: idx as i32 + 1,
                            duration_ms: 1000,
                            explicit: false,
                            popularity: 0,
                            language: None,
                            external_id_isrc: None,
                            audio_uri: Some(uri),
                            availability: TrackAvailability::Available,
                        },
                        &["artist1".to_string()],
                    )
                    .unwrap();
            }
        }

        fn has_embedding(store: &SqliteCatalogStore, track_id: &str) -> bool {
            store
                .get_entity_embedding("track", track_id, NAMESPACE, false)
                .unwrap()
                .is_some()
        }

        #[test]
        fn track_ids_param_embeds_only_the_requested_tracks() {
            let (url, requests) = fake_simple_ai();
            let temp_dir = tempfile::TempDir::new().unwrap();
            let registry = DbRegistry::new();
            let store = Arc::new(
                SqliteCatalogStore::new(
                    temp_dir.path().join("catalog.db"),
                    temp_dir.path(),
                    2,
                    &registry,
                )
                .unwrap(),
            );
            seed(&store, temp_dir.path(), "new", 2);
            seed(&store, temp_dir.path(), "old", 2);
            let user_store =
                Arc::new(SqliteUserStore::new(temp_dir.path().join("user.db"), &registry).unwrap());
            let server_store = Arc::new(
                SqliteServerStore::new(temp_dir.path().join("server.db"), &registry).unwrap(),
            );
            let ctx = JobContext::new(
                CancellationToken::new(),
                store.clone(),
                user_store.clone(),
                server_store,
                Arc::new(UserManager::new(user_store)),
            );
            let job = TrackEmbeddingSyncJob::new(AudioEmbeddingsSettings {
                simple_ai_base_url: url,
                specs: vec![AudioEmbeddingSpec {
                    model: "test-model".to_string(),
                    namespace: NAMESPACE.to_string(),
                    serve: true,
                }],
                ..test_settings()
            });

            job.execute_with_params(
                &ctx,
                Some(json!({"track_ids": ["new_t0", "new_t1", "new_t0", "missing"]})),
            )
            .unwrap();

            assert!(has_embedding(&store, "new_t0"));
            assert!(has_embedding(&store, "new_t1"));
            assert!(!has_embedding(&store, "old_t0"));
            assert!(!has_embedding(&store, "old_t1"));
            assert_eq!(requests.load(Ordering::SeqCst), 2);

            // Already-embedded tracks are not requested again.
            job.execute_with_params(&ctx, Some(json!({"track_ids": ["new_t0"]})))
                .unwrap();
            assert_eq!(requests.load(Ordering::SeqCst), 2);
        }

        /// A scheduler handle whose trigger replies come from `replies`, in order.
        fn fake_scheduler(
            replies: Vec<Result<(), JobError>>,
        ) -> (SchedulerHandle, Arc<std::sync::Mutex<Vec<Option<Value>>>>) {
            let (tx, mut rx) = mpsc::channel(8);
            let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
            let log = seen.clone();
            tokio::spawn(async move {
                let mut replies = replies.into_iter();
                while let Some(cmd) = rx.recv().await {
                    if let SchedulerCommand::TriggerJob {
                        job_id,
                        params,
                        response,
                    } = cmd
                    {
                        assert_eq!(job_id, TRACK_EMBEDDING_SYNC_JOB_ID);
                        log.lock().unwrap().push(params);
                        let _ = response.send(replies.next().unwrap_or(Ok(())));
                    }
                }
            });
            let temp_dir = tempfile::TempDir::new().unwrap();
            let server_store = Arc::new(
                SqliteServerStore::new(temp_dir.path().join("server.db"), &DbRegistry::new())
                    .unwrap(),
            );
            std::mem::forget(temp_dir);
            let handle = SchedulerHandle::new(
                tx,
                Arc::new(RwLock::new(SharedJobState {
                    jobs: Default::default(),
                    running_jobs: Default::default(),
                })),
                server_store,
                Arc::new(RwLock::new(JobPauseState::default())),
                Arc::new(RwLock::new(CircuitBreakerRegistry::default())),
            );
            (handle, seen)
        }

        #[tokio::test]
        async fn new_tracks_retry_while_the_job_is_running() {
            let (handle, seen) = fake_scheduler(vec![
                Err(JobError::AlreadyRunning),
                Err(JobError::AlreadyRunning),
                Ok(()),
            ]);
            let result = trigger_with_retry(
                handle,
                vec!["a".to_string(), "b".to_string()],
                Duration::from_millis(1),
                Duration::from_secs(5),
            )
            .await;
            assert!(result.is_ok());
            let seen = seen.lock().unwrap();
            assert_eq!(seen.len(), 3);
            assert_eq!(seen[2], Some(json!({"track_ids": ["a", "b"]})));
        }

        #[tokio::test]
        async fn new_tracks_are_left_to_the_scheduled_run_when_refused() {
            for refusal in [JobError::Paused, JobError::CircuitOpen, JobError::NotFound] {
                let (handle, seen) = fake_scheduler(vec![Err(refusal)]);
                let result = trigger_with_retry(
                    handle,
                    vec!["a".to_string()],
                    Duration::from_millis(1),
                    Duration::from_secs(5),
                )
                .await;
                assert!(result.is_err());
                assert_eq!(seen.lock().unwrap().len(), 1);
            }
        }

        #[tokio::test]
        async fn new_tracks_stop_retrying_after_the_window() {
            let (handle, seen) =
                fake_scheduler((0..1000).map(|_| Err(JobError::AlreadyRunning)).collect());
            let result = trigger_with_retry(
                handle,
                vec!["a".to_string()],
                Duration::from_millis(5),
                Duration::from_millis(30),
            )
            .await;
            assert!(matches!(result, Err(JobError::AlreadyRunning)));
            let attempts = seen.lock().unwrap().len();
            assert!((2..100).contains(&attempts), "{attempts}");
        }
    }
}
