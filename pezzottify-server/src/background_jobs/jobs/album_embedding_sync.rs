//! Album embedding synchronization job.
//!
//! Materializes derived album-level embeddings from complete local track
//! embeddings into the generic entity_embeddings table.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::catalog_store::{AlbumTrackRef, AlbumTracklist, EntityEmbeddingUpsert};
use crate::config::{
    AlbumEmbeddingAggregation, AlbumEmbeddingDerivationSpec, AlbumEmbeddingDerivationsSettings,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{info, warn};

#[derive(Clone)]
pub struct AlbumEmbeddingSyncJob {
    settings: AlbumEmbeddingDerivationsSettings,
    media_path: PathBuf,
}

#[derive(Debug, Deserialize, Default)]
struct AlbumEmbeddingSyncParams {
    max_albums: Option<usize>,
    force: Option<bool>,
}

impl AlbumEmbeddingSyncJob {
    pub fn new(settings: AlbumEmbeddingDerivationsSettings, media_path: PathBuf) -> Self {
        Self {
            settings,
            media_path,
        }
    }

    fn target_namespaces(&self) -> Vec<String> {
        self.settings
            .specs
            .iter()
            .map(|spec| spec.target_namespace.clone())
            .collect()
    }

    fn complete_local_tracks(&self, album: &AlbumTracklist) -> Option<Vec<(String, String)>> {
        if album.tracks.is_empty() {
            return None;
        }

        let mut tracks = Vec::with_capacity(album.tracks.len());
        for AlbumTrackRef {
            track_id,
            audio_uri,
        } in &album.tracks
        {
            let audio_uri = audio_uri.as_deref()?.trim();
            if audio_uri.is_empty() || !self.media_path.join(audio_uri).is_file() {
                return None;
            }
            tracks.push((track_id.clone(), audio_uri.to_string()));
        }
        Some(tracks)
    }

    fn source_vectors(
        &self,
        ctx: &JobContext,
        track_ids: &[String],
        source_namespace: &str,
    ) -> Result<Option<Vec<Vec<f32>>>, JobError> {
        let mut vectors = Vec::with_capacity(track_ids.len());
        for track_id in track_ids {
            let embedding = ctx
                .catalog_store
                .get_entity_embedding("track", track_id, source_namespace, true)
                .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;
            let Some(embedding) = embedding else {
                return Ok(None);
            };
            let Some(vector) = embedding.vector else {
                return Ok(None);
            };
            vectors.push(vector);
        }
        Ok(Some(vectors))
    }

    fn derive_vector(
        vectors: &[Vec<f32>],
        aggregation: AlbumEmbeddingAggregation,
    ) -> Result<Vec<f32>, JobError> {
        if vectors.is_empty() {
            return Err(JobError::ExecutionFailed(
                "cannot aggregate empty vector set".to_string(),
            ));
        }
        let dim = vectors[0].len();
        if dim == 0 {
            return Err(JobError::ExecutionFailed(
                "cannot aggregate empty source vectors".to_string(),
            ));
        }
        if vectors.iter().any(|vector| vector.len() != dim) {
            return Err(JobError::ExecutionFailed(
                "source embedding dimensions do not match".to_string(),
            ));
        }

        let mut derived = Vec::with_capacity(dim);
        for dim_idx in 0..dim {
            let mut values = vectors
                .iter()
                .map(|vector| vector[dim_idx])
                .collect::<Vec<_>>();
            values.sort_by(|a, b| a.total_cmp(b));
            let value = match aggregation {
                AlbumEmbeddingAggregation::Median => percentile_sorted(&values, 0.5),
                AlbumEmbeddingAggregation::Quantile { quantile } => {
                    percentile_sorted(&values, quantile)
                }
            };
            derived.push(value);
        }
        Ok(derived)
    }

    fn metadata(
        spec: &AlbumEmbeddingDerivationSpec,
        track_ids: &[String],
        audio_uris: &[String],
    ) -> Value {
        let mut object = Map::new();
        object.insert("derived".to_string(), json!(true));
        object.insert("source_entity_type".to_string(), json!("track"));
        object.insert(
            "source_namespace".to_string(),
            json!(spec.source_namespace.clone()),
        );
        object.insert("aggregation".to_string(), json!(spec.aggregation.as_str()));
        if let Some(quantile) = spec.aggregation.quantile() {
            object.insert("quantile".to_string(), json!(quantile));
        }
        object.insert("album_track_count".to_string(), json!(track_ids.len()));
        object.insert("source_embedding_count".to_string(), json!(track_ids.len()));
        object.insert(
            "coverage_basis".to_string(),
            json!("complete_album_tracklist_on_disk"),
        );
        object.insert("source_track_ids".to_string(), json!(track_ids));
        object.insert("source_audio_uris".to_string(), json!(audio_uris));
        Value::Object(object)
    }

    fn model(spec: &AlbumEmbeddingDerivationSpec) -> Value {
        json!({
            "id": "pezzottify-derived-album-embeddings",
            "source_namespace": spec.source_namespace.clone(),
            "target_namespace": spec.target_namespace.clone(),
            "derivation_version": "v1",
        })
    }

    fn process_spec(
        &self,
        ctx: &JobContext,
        album_id: &str,
        track_ids: &[String],
        audio_uris: &[String],
        spec: &AlbumEmbeddingDerivationSpec,
        force: bool,
    ) -> Result<SpecOutcome, JobError> {
        if !force {
            let existing = ctx
                .catalog_store
                .get_entity_embedding("album", album_id, &spec.target_namespace, false)
                .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;
            if existing.is_some() {
                return Ok(SpecOutcome::SkippedExisting);
            }
        }

        let Some(vectors) = self.source_vectors(ctx, track_ids, &spec.source_namespace)? else {
            return Ok(SpecOutcome::SkippedMissingSource);
        };
        let vector = Self::derive_vector(&vectors, spec.aggregation)?;
        let upsert = EntityEmbeddingUpsert {
            entity_type: "album".to_string(),
            entity_id: album_id.to_string(),
            namespace: spec.target_namespace.clone(),
            vector,
            dtype: "float32".to_string(),
            metadata: Self::metadata(spec, track_ids, audio_uris),
            model: Self::model(spec),
        };
        ctx.catalog_store
            .upsert_entity_embedding(&upsert)
            .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;
        Ok(SpecOutcome::Stored)
    }

    fn has_all_targets(&self, ctx: &JobContext, album_id: &str) -> Result<bool, JobError> {
        for spec in &self.settings.specs {
            let existing = ctx
                .catalog_store
                .get_entity_embedding("album", album_id, &spec.target_namespace, false)
                .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;
            if existing.is_none() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn execute_inner(
        &self,
        ctx: &JobContext,
        params: AlbumEmbeddingSyncParams,
    ) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(ctx.server_store.clone(), self.id());
        let started_at = Instant::now();
        let max_albums = params
            .max_albums
            .unwrap_or(self.settings.max_albums_per_run)
            .max(1);
        let force = params.force.unwrap_or(false);
        let target_namespaces = self.target_namespaces();

        audit.log_started(Some(json!({
            "max_albums": max_albums,
            "force": force,
            "target_namespaces": target_namespaces,
        })));

        let album_selection_limit = if force { max_albums } else { usize::MAX };
        let albums = ctx
            .catalog_store
            .list_album_tracklists(album_selection_limit)
            .map_err(|e| JobError::ExecutionFailed(e.to_string()))?;

        let mut albums_considered = 0usize;
        let mut albums_complete_local = 0usize;
        let mut albums_skipped_incomplete = 0usize;
        let mut embeddings_stored = 0usize;
        let mut specs_skipped_existing = 0usize;
        let mut specs_skipped_missing_source = 0usize;
        let mut failures = 0usize;

        for album in albums {
            if ctx.is_cancelled() {
                audit.log_failed("Cancelled", None);
                return Err(JobError::Cancelled);
            }
            if !force && self.has_all_targets(ctx, &album.album_id)? {
                continue;
            }
            if albums_considered >= max_albums {
                break;
            }
            albums_considered += 1;

            let Some(local_tracks) = self.complete_local_tracks(&album) else {
                albums_skipped_incomplete += 1;
                continue;
            };
            albums_complete_local += 1;
            let track_ids = local_tracks
                .iter()
                .map(|(track_id, _)| track_id.clone())
                .collect::<Vec<_>>();
            let audio_uris = local_tracks
                .iter()
                .map(|(_, audio_uri)| audio_uri.clone())
                .collect::<Vec<_>>();

            for spec in &self.settings.specs {
                if ctx.is_cancelled() {
                    audit.log_failed("Cancelled", None);
                    return Err(JobError::Cancelled);
                }

                match self.process_spec(ctx, &album.album_id, &track_ids, &audio_uris, spec, force)
                {
                    Ok(SpecOutcome::Stored) => embeddings_stored += 1,
                    Ok(SpecOutcome::SkippedExisting) => specs_skipped_existing += 1,
                    Ok(SpecOutcome::SkippedMissingSource) => specs_skipped_missing_source += 1,
                    Err(e) => {
                        failures += 1;
                        warn!(
                            "Failed to derive album embedding for album {} namespace {}: {}",
                            album.album_id, spec.target_namespace, e
                        );
                    }
                }
            }

            if albums_considered.is_multiple_of(50) {
                audit.log_progress(json!({
                    "albums_considered": albums_considered,
                    "albums_complete_local": albums_complete_local,
                    "albums_skipped_incomplete": albums_skipped_incomplete,
                    "embeddings_stored": embeddings_stored,
                    "specs_skipped_existing": specs_skipped_existing,
                    "specs_skipped_missing_source": specs_skipped_missing_source,
                    "failures": failures,
                }));
            }
        }

        let duration_ms = started_at.elapsed().as_millis() as u64;
        info!(
            "Album embedding sync completed: considered={} complete_local={} incomplete={} stored={} skipped_existing={} missing_source={} failures={} duration_ms={}",
            albums_considered,
            albums_complete_local,
            albums_skipped_incomplete,
            embeddings_stored,
            specs_skipped_existing,
            specs_skipped_missing_source,
            failures,
            duration_ms
        );
        audit.log_completed(Some(json!({
            "duration_ms": duration_ms,
            "albums_considered": albums_considered,
            "albums_complete_local": albums_complete_local,
            "albums_skipped_incomplete": albums_skipped_incomplete,
            "embeddings_stored": embeddings_stored,
            "specs_skipped_existing": specs_skipped_existing,
            "specs_skipped_missing_source": specs_skipped_missing_source,
            "failures": failures,
        })));

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpecOutcome {
    Stored,
    SkippedExisting,
    SkippedMissingSource,
}

fn percentile_sorted(values: &[f32], quantile: f32) -> f32 {
    debug_assert!(!values.is_empty());
    if values.len() == 1 {
        return values[0];
    }
    let position = (values.len() - 1) as f32 * quantile.clamp(0.0, 1.0);
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        values[lower]
    } else {
        let fraction = position - lower as f32;
        values[lower] + (values[upper] - values[lower]) * fraction
    }
}

impl BackgroundJob for AlbumEmbeddingSyncJob {
    fn id(&self) -> &'static str {
        "album_embedding_sync"
    }

    fn name(&self) -> &'static str {
        "Album Embedding Sync"
    }

    fn description(&self) -> &'static str {
        "Materialize derived album embeddings from complete local track embeddings"
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
        self.execute_inner(ctx, AlbumEmbeddingSyncParams::default())
    }

    fn execute_with_params(&self, ctx: &JobContext, params: Option<Value>) -> Result<(), JobError> {
        let params = match params {
            Some(value) => serde_json::from_value(value)
                .map_err(|e| JobError::ExecutionFailed(format!("Invalid params: {e}")))?,
            None => AlbumEmbeddingSyncParams::default(),
        };
        self.execute_inner(ctx, params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::DbRegistry;
    use crate::catalog_store::{
        Album, AlbumAvailability, AlbumType, Artist, CatalogStore, EntityEmbeddingUpsert,
        SqliteCatalogStore, Track, TrackAvailability,
    };
    use crate::server_store::SqliteServerStore;
    use crate::user::{SqliteUserStore, UserManager};
    use std::sync::{Arc, Mutex};
    use tokio_util::sync::CancellationToken;

    #[test]
    fn median_and_q25_are_computed_per_dimension() {
        let vectors = vec![
            vec![1.0, 10.0, 100.0],
            vec![3.0, 30.0, 300.0],
            vec![5.0, 50.0, 500.0],
        ];

        let median =
            AlbumEmbeddingSyncJob::derive_vector(&vectors, AlbumEmbeddingAggregation::Median)
                .unwrap();
        assert_eq!(median, vec![3.0, 30.0, 300.0]);

        let q25 = AlbumEmbeddingSyncJob::derive_vector(
            &vectors,
            AlbumEmbeddingAggregation::Quantile { quantile: 0.25 },
        )
        .unwrap();
        assert_eq!(q25, vec![2.0, 20.0, 200.0]);
    }

    #[test]
    fn median_averages_even_track_counts() {
        let vectors = vec![vec![1.0, 10.0], vec![5.0, 50.0]];
        let median =
            AlbumEmbeddingSyncJob::derive_vector(&vectors, AlbumEmbeddingAggregation::Median)
                .unwrap();
        assert_eq!(median, vec![3.0, 30.0]);
    }

    #[test]
    fn params_deserialize_manual_run_options() {
        let params: AlbumEmbeddingSyncParams =
            serde_json::from_value(json!({"max_albums": 7, "force": true})).unwrap();
        assert_eq!(params.max_albums, Some(7));
        assert_eq!(params.force, Some(true));
    }

    #[test]
    fn metadata_and_model_identify_derived_album_embedding() {
        let spec = AlbumEmbeddingDerivationSpec {
            source_namespace: "track:source.v1".to_string(),
            target_namespace: "album:target.v1".to_string(),
            aggregation: AlbumEmbeddingAggregation::Quantile { quantile: 0.25 },
        };
        let track_ids = vec!["track1".to_string(), "track2".to_string()];
        let audio_uris = vec!["a.ogg".to_string(), "b.ogg".to_string()];

        let metadata = AlbumEmbeddingSyncJob::metadata(&spec, &track_ids, &audio_uris);
        assert_eq!(metadata["derived"], true);
        assert_eq!(metadata["source_entity_type"], "track");
        assert_eq!(metadata["source_namespace"], "track:source.v1");
        assert_eq!(metadata["aggregation"], "quantile");
        assert_eq!(metadata["quantile"], 0.25);
        assert_eq!(metadata["album_track_count"], 2);
        assert_eq!(metadata["source_embedding_count"], 2);
        assert_eq!(
            metadata["coverage_basis"],
            "complete_album_tracklist_on_disk"
        );
        assert_eq!(metadata["source_track_ids"], json!(track_ids));

        let model = AlbumEmbeddingSyncJob::model(&spec);
        assert_eq!(model["id"], "pezzottify-derived-album-embeddings");
        assert_eq!(model["source_namespace"], "track:source.v1");
        assert_eq!(model["target_namespace"], "album:target.v1");
        assert_eq!(model["derivation_version"], "v1");
    }

    fn test_settings() -> AlbumEmbeddingDerivationsSettings {
        AlbumEmbeddingDerivationsSettings {
            enabled: true,
            interval_hours: 6,
            jitter_minutes: 15,
            max_albums_per_run: 100,
            specs: vec![AlbumEmbeddingDerivationSpec {
                source_namespace: "source.v1".to_string(),
                target_namespace: "album.derived.v1".to_string(),
                aggregation: AlbumEmbeddingAggregation::Median,
            }],
        }
    }

    fn create_context() -> (
        AlbumEmbeddingSyncJob,
        JobContext,
        Arc<SqliteCatalogStore>,
        tempfile::TempDir,
    ) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let registry = DbRegistry::new();
        let catalog_store = Arc::new(
            SqliteCatalogStore::new(
                temp_dir.path().join("catalog.db"),
                temp_dir.path(),
                2,
                &registry,
            )
            .unwrap(),
        );
        let user_store =
            Arc::new(SqliteUserStore::new(temp_dir.path().join("user.db"), &registry).unwrap());
        let server_store =
            Arc::new(SqliteServerStore::new(temp_dir.path().join("server.db"), &registry).unwrap());
        let user_manager = Arc::new(Mutex::new(UserManager::new(
            catalog_store.clone(),
            user_store.clone(),
        )));
        let ctx = JobContext::new(
            CancellationToken::new(),
            catalog_store.clone(),
            user_store,
            server_store,
            user_manager,
        );
        let job = AlbumEmbeddingSyncJob::new(test_settings(), temp_dir.path().to_path_buf());
        (job, ctx, catalog_store, temp_dir)
    }

    fn seed_album(store: &SqliteCatalogStore, album_id: &str, audio_uris: &[Option<&str>]) {
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
                    id: album_id.to_string(),
                    name: format!("Album {album_id}"),
                    album_type: AlbumType::Album,
                    label: None,
                    release_date: Some("2024".to_string()),
                    release_date_precision: Some("year".to_string()),
                    external_id_upc: None,
                    popularity: 0,
                    album_availability: AlbumAvailability::Complete,
                },
                &["artist1".to_string()],
            )
            .unwrap();

        for (idx, audio_uri) in audio_uris.iter().enumerate() {
            store
                .create_track(
                    &Track {
                        id: format!("{album_id}_track_{idx}"),
                        name: format!("Track {idx}"),
                        album_id: album_id.to_string(),
                        disc_number: 1,
                        track_number: idx as i32 + 1,
                        duration_ms: 1000,
                        explicit: false,
                        popularity: 0,
                        language: None,
                        external_id_isrc: None,
                        audio_uri: audio_uri.map(str::to_string),
                        availability: TrackAvailability::Available,
                    },
                    &["artist1".to_string()],
                )
                .unwrap();
        }
    }

    fn write_media_file(temp_dir: &tempfile::TempDir, audio_uri: &str) {
        std::fs::write(temp_dir.path().join(audio_uri), b"audio").unwrap();
    }

    fn upsert_track_embedding(store: &SqliteCatalogStore, track_id: &str, vector: Vec<f32>) {
        store
            .upsert_entity_embedding(&EntityEmbeddingUpsert {
                entity_type: "track".to_string(),
                entity_id: track_id.to_string(),
                namespace: "source.v1".to_string(),
                vector,
                dtype: "float32".to_string(),
                metadata: json!({}),
                model: json!({}),
            })
            .unwrap();
    }

    #[test]
    fn strict_eligibility_skips_incomplete_albums_and_computes_complete_album() {
        let (job, ctx, store, temp_dir) = create_context();
        seed_album(&store, "missing_audio_uri", &[Some("a.ogg"), None]);
        seed_album(&store, "missing_disk_file", &[Some("b.ogg"), Some("c.ogg")]);
        seed_album(&store, "missing_source", &[Some("d.ogg"), Some("e.ogg")]);
        seed_album(&store, "complete", &[Some("f.ogg"), Some("g.ogg")]);

        for uri in ["a.ogg", "b.ogg", "d.ogg", "e.ogg", "f.ogg", "g.ogg"] {
            write_media_file(&temp_dir, uri);
        }
        for track_id in [
            "missing_audio_uri_track_0",
            "missing_audio_uri_track_1",
            "missing_disk_file_track_0",
            "missing_disk_file_track_1",
            "missing_source_track_0",
            "complete_track_0",
            "complete_track_1",
        ] {
            upsert_track_embedding(&store, track_id, vec![1.0, 3.0]);
        }
        upsert_track_embedding(&store, "complete_track_0", vec![1.0, 10.0]);
        upsert_track_embedding(&store, "complete_track_1", vec![5.0, 30.0]);

        job.execute(&ctx).unwrap();

        assert!(store
            .get_entity_embedding("album", "missing_audio_uri", "album.derived.v1", true)
            .unwrap()
            .is_none());
        assert!(store
            .get_entity_embedding("album", "missing_disk_file", "album.derived.v1", true)
            .unwrap()
            .is_none());
        assert!(store
            .get_entity_embedding("album", "missing_source", "album.derived.v1", true)
            .unwrap()
            .is_none());
        let stored = store
            .get_entity_embedding("album", "complete", "album.derived.v1", true)
            .unwrap()
            .unwrap();
        assert_eq!(stored.vector.unwrap(), vec![3.0, 20.0]);
        assert_eq!(stored.metadata["derived"], true);
        assert_eq!(
            stored.metadata["coverage_basis"],
            "complete_album_tracklist_on_disk"
        );
        assert_eq!(stored.model["id"], "pezzottify-derived-album-embeddings");
    }

    #[test]
    fn force_false_skips_existing_and_force_true_recomputes() {
        let (job, ctx, store, temp_dir) = create_context();
        seed_album(&store, "album1", &[Some("a.ogg"), Some("b.ogg")]);
        write_media_file(&temp_dir, "a.ogg");
        write_media_file(&temp_dir, "b.ogg");
        upsert_track_embedding(&store, "album1_track_0", vec![1.0]);
        upsert_track_embedding(&store, "album1_track_1", vec![3.0]);
        store
            .upsert_entity_embedding(&EntityEmbeddingUpsert {
                entity_type: "album".to_string(),
                entity_id: "album1".to_string(),
                namespace: "album.derived.v1".to_string(),
                vector: vec![99.0],
                dtype: "float32".to_string(),
                metadata: json!({"existing": true}),
                model: json!({}),
            })
            .unwrap();

        job.execute(&ctx).unwrap();
        let existing = store
            .get_entity_embedding("album", "album1", "album.derived.v1", true)
            .unwrap()
            .unwrap();
        assert_eq!(existing.vector.unwrap(), vec![99.0]);

        job.execute_with_params(&ctx, Some(json!({"force": true})))
            .unwrap();
        let recomputed = store
            .get_entity_embedding("album", "album1", "album.derived.v1", true)
            .unwrap()
            .unwrap();
        assert_eq!(recomputed.vector.unwrap(), vec![2.0]);
        assert_eq!(recomputed.metadata["source_embedding_count"], 2);
    }
}
