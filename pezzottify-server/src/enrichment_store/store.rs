//! SQLite-backed enrichment store implementation.

use super::models::{
    AlbumEnrichment, AlbumEnrichmentV1, ArtistEnrichment, ArtistEnrichmentV1, AudioFeatures,
    EnrichmentQueueItemV1, EnrichmentStats, EntityAliasV1, EntityContributorV1,
    EntityEnrichmentStatusV1, EntityEvidenceV1, EntityExternalIdV1, EntityRelationV1,
    EntitySourceV1, EntityTagV1, TrackEnrichmentV1,
};
use super::schema::{create_enrichment_v1_schema, ENRICHMENT_VERSIONED_SCHEMAS};
use super::trait_def::EnrichmentStore;
use crate::sqlite_persistence::BASE_DB_VERSION;
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

/// SQLite-backed enrichment store.
#[derive(Clone)]
pub struct SqliteEnrichmentStore {
    read_conn: Arc<Mutex<Connection>>,
    write_conn: Arc<Mutex<Connection>>,
}

fn migrate_if_needed(conn: &mut Connection) -> Result<()> {
    let db_version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    let latest_version = ENRICHMENT_VERSIONED_SCHEMAS.len() - 1;
    let latest_schema = &ENRICHMENT_VERSIONED_SCHEMAS[latest_version];

    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if table_count == 0 {
        info!(
            "Creating enrichment db schema at version {}",
            latest_version
        );
        latest_schema.create(conn)?;
        create_enrichment_v1_schema(conn)?;
        return Ok(());
    }

    let mut current_version = if db_version < BASE_DB_VERSION as i64 {
        0
    } else {
        (db_version - BASE_DB_VERSION as i64) as usize
    };

    if current_version >= latest_version {
        create_enrichment_v1_schema(conn)?;
        return Ok(());
    }

    let tx = conn.transaction()?;
    for schema in ENRICHMENT_VERSIONED_SCHEMAS
        .iter()
        .skip(current_version + 1)
    {
        if let Some(migration_fn) = schema.migration {
            info!(
                "Migrating enrichment db from version {} to {}",
                current_version, schema.version
            );
            migration_fn(&tx)?;
            current_version = schema.version;
        }
    }
    tx.pragma_update(None, "user_version", BASE_DB_VERSION + current_version)?;
    tx.commit()?;
    create_enrichment_v1_schema(conn)?;
    Ok(())
}

impl SqliteEnrichmentStore {
    /// Create a new SqliteEnrichmentStore.
    pub fn new<P: AsRef<Path>>(
        db_path: P,
        db_registry: &crate::backup::DbRegistry,
    ) -> Result<Self> {
        let db_path_ref = db_path.as_ref();

        let mut write_conn = Connection::open_with_flags(
            db_path_ref,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .context("Failed to open enrichment database")?;

        migrate_if_needed(&mut write_conn)?;

        db_registry.register(db_path_ref.to_path_buf(), &write_conn)?;

        let read_conn = Connection::open_with_flags(
            db_path_ref,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .context("Failed to open enrichment database for reading")?;

        read_conn
            .pragma_update(None, "journal_mode", "WAL")
            .context("Failed to set WAL mode on enrichment read connection")?;

        let stats = Self::count_rows(&read_conn)?;
        info!(
            "Enrichment store ready: {} tracks analyzed, {} artists enriched, {} albums enriched",
            stats.tracks_analyzed, stats.artists_enriched, stats.albums_enriched
        );

        Ok(Self {
            read_conn: Arc::new(Mutex::new(read_conn)),
            write_conn: Arc::new(Mutex::new(write_conn)),
        })
    }

    fn claim_enrichment_queue_batch_matching(
        &self,
        limit: usize,
        entity_types: &[String],
    ) -> Result<Vec<EnrichmentQueueItemV1>> {
        let now = now_unix();
        let conn = self.write_conn.lock().unwrap();
        let mut sql = String::from(
            "SELECT id, entity_type, entity_id, status, priority, reason, stage, attempts,
                    created_at, updated_at, next_attempt_at, started_at, completed_at, last_error
             FROM enrichment_queue_v1
             WHERE status = 'queued' AND (next_attempt_at IS NULL OR next_attempt_at <= ?)",
        );
        let valid_types = entity_types
            .iter()
            .filter(|entity_type| valid_entity_type(entity_type))
            .cloned()
            .collect::<Vec<_>>();
        if !valid_types.is_empty() {
            let placeholders = std::iter::repeat_n("?", valid_types.len())
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(" AND entity_type IN (");
            sql.push_str(&placeholders);
            sql.push(')');
        }
        sql.push_str(" ORDER BY priority DESC, updated_at ASC LIMIT ?");

        let mut values: Vec<rusqlite::types::Value> = Vec::with_capacity(valid_types.len() + 2);
        values.push(now.into());
        for entity_type in &valid_types {
            values.push(entity_type.clone().into());
        }
        values.push((limit as i64).into());

        let mut stmt = conn.prepare(&sql)?;
        let items = stmt
            .query_map(
                rusqlite::params_from_iter(values.iter()),
                queue_item_from_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        for item in &items {
            conn.execute(
                "UPDATE enrichment_queue_v1 SET status = 'running', stage = 'running', attempts = attempts + 1,
                        started_at = ?1, updated_at = ?1, last_error = NULL WHERE id = ?2",
                params![now, item.id],
            )?;
        }
        Ok(items
            .into_iter()
            .map(|mut item| {
                item.status = "running".to_string();
                item.stage = Some("running".to_string());
                item.attempts += 1;
                item.started_at = Some(now);
                item.updated_at = now;
                item.last_error = None;
                item
            })
            .collect())
    }

    fn count_rows(conn: &Connection) -> Result<EnrichmentStats> {
        let tracks_analyzed: usize =
            conn.query_row("SELECT COUNT(*) FROM audio_features", [], |r| r.get(0))?;
        let artists_enriched: usize =
            conn.query_row("SELECT COUNT(*) FROM artist_enrichment", [], |r| r.get(0))?;
        let albums_enriched: usize =
            conn.query_row("SELECT COUNT(*) FROM album_enrichment", [], |r| r.get(0))?;
        Ok(EnrichmentStats {
            tracks_analyzed,
            artists_enriched,
            albums_enriched,
        })
    }
}

// Helper: serialize Option<Vec<String>> to JSON or NULL
fn json_array_or_null(v: &Option<Vec<String>>) -> Option<String> {
    v.as_ref().map(|arr| serde_json::to_string(arr).unwrap())
}

// Helper: deserialize JSON array or NULL to Option<Vec<String>>
fn parse_json_array(s: Option<String>) -> Option<Vec<String>> {
    s.and_then(|json| {
        serde_json::from_str(&json).unwrap_or_else(|e| {
            warn!("Malformed JSON array in enrichment db: {}: {}", json, e);
            None
        })
    })
}

// Helper: Option<bool> to Option<i32>
fn bool_to_int(v: &Option<bool>) -> Option<i32> {
    v.map(|b| if b { 1 } else { 0 })
}

// Helper: Option<i32> to Option<bool>
fn int_to_bool(v: Option<i32>) -> Option<bool> {
    v.map(|i| i != 0)
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn valid_entity_type(entity_type: &str) -> bool {
    matches!(entity_type, "artist" | "album" | "track")
}

fn table_for_entity_type(entity_type: &str) -> Result<(&'static str, &'static str)> {
    match entity_type {
        "artist" => Ok(("artist_enrichment_v1", "artist_id")),
        "album" => Ok(("album_enrichment_v1", "album_id")),
        "track" => Ok(("track_enrichment_v1", "track_id")),
        other => anyhow::bail!("invalid enrichment entity type: {}", other),
    }
}

fn optional_json_to_string(value: &Option<serde_json::Value>) -> Result<Option<String>> {
    value
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .context("failed to serialize enrichment evidence JSON")
}

fn parse_optional_json(value: Option<String>) -> Option<serde_json::Value> {
    value.and_then(|s| match serde_json::from_str(&s) {
        Ok(v) => Some(v),
        Err(e) => {
            warn!("Malformed JSON in enrichment db: {}: {}", s, e);
            None
        }
    })
}

fn queue_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EnrichmentQueueItemV1> {
    Ok(EnrichmentQueueItemV1 {
        id: row.get(0)?,
        entity_type: row.get(1)?,
        entity_id: row.get(2)?,
        status: row.get(3)?,
        priority: row.get(4)?,
        reason: row.get(5)?,
        stage: row.get(6)?,
        attempts: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        next_attempt_at: row.get(10)?,
        started_at: row.get(11)?,
        completed_at: row.get(12)?,
        last_error: row.get(13)?,
    })
}

impl EnrichmentStore for SqliteEnrichmentStore {
    fn get_audio_features(&self, track_id: &str) -> Result<Option<AudioFeatures>> {
        let conn = self.read_conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT track_id, bpm, danceability, key, chords_key, chords_scale,
                    chords_changes_rate, loudness, average_loudness, dynamic_complexity,
                    spectral_complexity, vocal_instrumental, valence, analyzed_at, analyzer_version
             FROM audio_features WHERE track_id = ?1",
        )?;
        let result = stmt
            .query_row(params![track_id], |row| {
                Ok(AudioFeatures {
                    track_id: row.get(0)?,
                    bpm: row.get(1)?,
                    danceability: row.get(2)?,
                    key: row.get(3)?,
                    chords_key: row.get(4)?,
                    chords_scale: row.get(5)?,
                    chords_changes_rate: row.get(6)?,
                    loudness: row.get(7)?,
                    average_loudness: row.get(8)?,
                    dynamic_complexity: row.get(9)?,
                    spectral_complexity: row.get(10)?,
                    vocal_instrumental: row.get(11)?,
                    valence: row.get(12)?,
                    analyzed_at: row.get(13)?,
                    analyzer_version: row.get(14)?,
                })
            })
            .optional()?;
        Ok(result)
    }

    fn upsert_audio_features(&self, features: &AudioFeatures) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO audio_features
             (track_id, bpm, danceability, key, chords_key, chords_scale, chords_changes_rate,
              loudness, average_loudness, dynamic_complexity, spectral_complexity,
              vocal_instrumental, valence, analyzed_at, analyzer_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                features.track_id,
                features.bpm,
                features.danceability,
                features.key,
                features.chords_key,
                features.chords_scale,
                features.chords_changes_rate,
                features.loudness,
                features.average_loudness,
                features.dynamic_complexity,
                features.spectral_complexity,
                features.vocal_instrumental,
                features.valence,
                features.analyzed_at,
                features.analyzer_version,
            ],
        )?;
        Ok(())
    }

    fn upsert_audio_features_batch(&self, features: &[AudioFeatures]) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO audio_features
                 (track_id, bpm, danceability, key, chords_key, chords_scale, chords_changes_rate,
                  loudness, average_loudness, dynamic_complexity, spectral_complexity,
                  vocal_instrumental, valence, analyzed_at, analyzer_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            )?;
            for f in features {
                stmt.execute(params![
                    f.track_id,
                    f.bpm,
                    f.danceability,
                    f.key,
                    f.chords_key,
                    f.chords_scale,
                    f.chords_changes_rate,
                    f.loudness,
                    f.average_loudness,
                    f.dynamic_complexity,
                    f.spectral_complexity,
                    f.vocal_instrumental,
                    f.valence,
                    f.analyzed_at,
                    f.analyzer_version,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn get_tracks_needing_analysis(
        &self,
        catalog_track_ids: &[String],
        limit: usize,
    ) -> Result<Vec<String>> {
        let conn = self.read_conn.lock().unwrap();

        // Check each candidate against the enrichment DB
        let mut stmt = conn.prepare_cached("SELECT 1 FROM audio_features WHERE track_id = ?1")?;

        let mut result = Vec::with_capacity(limit.min(catalog_track_ids.len()));
        for id in catalog_track_ids {
            if result.len() >= limit {
                break;
            }
            let exists: bool = stmt
                .query_row(params![id], |_| Ok(()))
                .optional()?
                .is_some();
            if !exists {
                result.push(id.clone());
            }
        }

        Ok(result)
    }

    fn get_artist_enrichment(&self, artist_id: &str) -> Result<Option<ArtistEnrichment>> {
        let conn = self.read_conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT artist_id, entity_type, nationalities, decades_active, is_composer,
                    is_producer, instruments, gender, vocal_type, primary_language,
                    enriched_at, source
             FROM artist_enrichment WHERE artist_id = ?1",
        )?;
        let result = stmt
            .query_row(params![artist_id], |row| {
                Ok(ArtistEnrichment {
                    artist_id: row.get(0)?,
                    entity_type: row.get(1)?,
                    nationalities: parse_json_array(row.get(2)?),
                    decades_active: parse_json_array(row.get(3)?),
                    is_composer: int_to_bool(row.get(4)?),
                    is_producer: int_to_bool(row.get(5)?),
                    instruments: parse_json_array(row.get(6)?),
                    gender: row.get(7)?,
                    vocal_type: row.get(8)?,
                    primary_language: row.get(9)?,
                    enriched_at: row.get(10)?,
                    source: row.get(11)?,
                })
            })
            .optional()?;
        Ok(result)
    }

    fn upsert_artist_enrichment(&self, enrichment: &ArtistEnrichment) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO artist_enrichment
             (artist_id, entity_type, nationalities, decades_active, is_composer,
              is_producer, instruments, gender, vocal_type, primary_language,
              enriched_at, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                enrichment.artist_id,
                enrichment.entity_type,
                json_array_or_null(&enrichment.nationalities),
                json_array_or_null(&enrichment.decades_active),
                bool_to_int(&enrichment.is_composer),
                bool_to_int(&enrichment.is_producer),
                json_array_or_null(&enrichment.instruments),
                enrichment.gender,
                enrichment.vocal_type,
                enrichment.primary_language,
                enrichment.enriched_at,
                enrichment.source,
            ],
        )?;
        Ok(())
    }

    fn get_artists_needing_enrichment(
        &self,
        catalog_artist_ids: &[String],
        limit: usize,
    ) -> Result<Vec<String>> {
        let conn = self.read_conn.lock().unwrap();
        let mut stmt =
            conn.prepare_cached("SELECT 1 FROM artist_enrichment WHERE artist_id = ?1")?;

        let mut result = Vec::with_capacity(limit.min(catalog_artist_ids.len()));
        for id in catalog_artist_ids {
            if result.len() >= limit {
                break;
            }
            let exists: bool = stmt
                .query_row(params![id], |_| Ok(()))
                .optional()?
                .is_some();
            if !exists {
                result.push(id.clone());
            }
        }

        Ok(result)
    }

    fn get_album_enrichment(&self, album_id: &str) -> Result<Option<AlbumEnrichment>> {
        let conn = self.read_conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT album_id, is_live, is_compilation, is_soundtrack, is_concept_album,
                    is_remix_album, primary_language, production_era, enriched_at, source
             FROM album_enrichment WHERE album_id = ?1",
        )?;
        let result = stmt
            .query_row(params![album_id], |row| {
                Ok(AlbumEnrichment {
                    album_id: row.get(0)?,
                    is_live: int_to_bool(row.get(1)?),
                    is_compilation: int_to_bool(row.get(2)?),
                    is_soundtrack: int_to_bool(row.get(3)?),
                    is_concept_album: int_to_bool(row.get(4)?),
                    is_remix_album: int_to_bool(row.get(5)?),
                    primary_language: row.get(6)?,
                    production_era: row.get(7)?,
                    enriched_at: row.get(8)?,
                    source: row.get(9)?,
                })
            })
            .optional()?;
        Ok(result)
    }

    fn upsert_album_enrichment(&self, enrichment: &AlbumEnrichment) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO album_enrichment
             (album_id, is_live, is_compilation, is_soundtrack, is_concept_album,
              is_remix_album, primary_language, production_era, enriched_at, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                enrichment.album_id,
                bool_to_int(&enrichment.is_live),
                bool_to_int(&enrichment.is_compilation),
                bool_to_int(&enrichment.is_soundtrack),
                bool_to_int(&enrichment.is_concept_album),
                bool_to_int(&enrichment.is_remix_album),
                enrichment.primary_language,
                enrichment.production_era,
                enrichment.enriched_at,
                enrichment.source,
            ],
        )?;
        Ok(())
    }

    fn get_albums_needing_enrichment(
        &self,
        catalog_album_ids: &[String],
        limit: usize,
    ) -> Result<Vec<String>> {
        let conn = self.read_conn.lock().unwrap();
        let mut stmt = conn.prepare_cached("SELECT 1 FROM album_enrichment WHERE album_id = ?1")?;

        let mut result = Vec::with_capacity(limit.min(catalog_album_ids.len()));
        for id in catalog_album_ids {
            if result.len() >= limit {
                break;
            }
            let exists: bool = stmt
                .query_row(params![id], |_| Ok(()))
                .optional()?
                .is_some();
            if !exists {
                result.push(id.clone());
            }
        }

        Ok(result)
    }

    fn get_artist_enrichment_v1(&self, artist_id: &str) -> Result<Option<ArtistEnrichmentV1>> {
        let conn = self.read_conn.lock().unwrap();
        let result = conn
            .prepare_cached(
                "SELECT artist_id, kind, birth_date, death_date, foundation_date, dissolution_date,
                    origin_place, origin_country, primary_language, is_person, is_group,
                    is_composer, is_performer, is_conductor, is_producer, confidence, summary,
                    bio, enriched_at, last_verified_at, source_status
             FROM artist_enrichment_v1 WHERE artist_id = ?1",
            )?
            .query_row(params![artist_id], |row| {
                Ok(ArtistEnrichmentV1 {
                    artist_id: row.get(0)?,
                    kind: row.get(1)?,
                    birth_date: row.get(2)?,
                    death_date: row.get(3)?,
                    foundation_date: row.get(4)?,
                    dissolution_date: row.get(5)?,
                    origin_place: row.get(6)?,
                    origin_country: row.get(7)?,
                    primary_language: row.get(8)?,
                    is_person: int_to_bool(row.get(9)?),
                    is_group: int_to_bool(row.get(10)?),
                    is_composer: int_to_bool(row.get(11)?),
                    is_performer: int_to_bool(row.get(12)?),
                    is_conductor: int_to_bool(row.get(13)?),
                    is_producer: int_to_bool(row.get(14)?),
                    confidence: row.get(15)?,
                    summary: row.get(16)?,
                    bio: row.get(17)?,
                    enriched_at: row.get(18)?,
                    last_verified_at: row.get(19)?,
                    source_status: row.get(20)?,
                })
            })
            .optional()?;
        Ok(result)
    }

    fn upsert_artist_enrichment_v1(&self, e: &ArtistEnrichmentV1) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO artist_enrichment_v1
             (artist_id, kind, birth_date, death_date, foundation_date, dissolution_date,
              origin_place, origin_country, primary_language, is_person, is_group,
              is_composer, is_performer, is_conductor, is_producer, confidence, summary,
              bio, enriched_at, last_verified_at, source_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                e.artist_id, e.kind, e.birth_date, e.death_date, e.foundation_date,
                e.dissolution_date, e.origin_place, e.origin_country, e.primary_language,
                bool_to_int(&e.is_person), bool_to_int(&e.is_group), bool_to_int(&e.is_composer),
                bool_to_int(&e.is_performer), bool_to_int(&e.is_conductor), bool_to_int(&e.is_producer),
                e.confidence, e.summary, e.bio, e.enriched_at, e.last_verified_at, e.source_status,
            ],
        )?;
        Ok(())
    }

    fn get_album_enrichment_v1(&self, album_id: &str) -> Result<Option<AlbumEnrichmentV1>> {
        let conn = self.read_conn.lock().unwrap();
        let result = conn
            .prepare_cached(
                "SELECT album_id, album_kind, original_release_date, recording_start_date,
                    recording_end_date, release_country, label, catalog_number, is_live,
                    is_compilation, is_soundtrack, is_concept_album, is_remix_album, is_archival,
                    confidence, summary, notes, enriched_at, last_verified_at, source_status
             FROM album_enrichment_v1 WHERE album_id = ?1",
            )?
            .query_row(params![album_id], |row| {
                Ok(AlbumEnrichmentV1 {
                    album_id: row.get(0)?,
                    album_kind: row.get(1)?,
                    original_release_date: row.get(2)?,
                    recording_start_date: row.get(3)?,
                    recording_end_date: row.get(4)?,
                    release_country: row.get(5)?,
                    label: row.get(6)?,
                    catalog_number: row.get(7)?,
                    is_live: int_to_bool(row.get(8)?),
                    is_compilation: int_to_bool(row.get(9)?),
                    is_soundtrack: int_to_bool(row.get(10)?),
                    is_concept_album: int_to_bool(row.get(11)?),
                    is_remix_album: int_to_bool(row.get(12)?),
                    is_archival: int_to_bool(row.get(13)?),
                    confidence: row.get(14)?,
                    summary: row.get(15)?,
                    notes: row.get(16)?,
                    enriched_at: row.get(17)?,
                    last_verified_at: row.get(18)?,
                    source_status: row.get(19)?,
                })
            })
            .optional()?;
        Ok(result)
    }

    fn upsert_album_enrichment_v1(&self, e: &AlbumEnrichmentV1) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO album_enrichment_v1
             (album_id, album_kind, original_release_date, recording_start_date, recording_end_date,
              release_country, label, catalog_number, is_live, is_compilation, is_soundtrack,
              is_concept_album, is_remix_album, is_archival, confidence, summary, notes,
              enriched_at, last_verified_at, source_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                e.album_id, e.album_kind, e.original_release_date, e.recording_start_date,
                e.recording_end_date, e.release_country, e.label, e.catalog_number,
                bool_to_int(&e.is_live), bool_to_int(&e.is_compilation), bool_to_int(&e.is_soundtrack),
                bool_to_int(&e.is_concept_album), bool_to_int(&e.is_remix_album), bool_to_int(&e.is_archival),
                e.confidence, e.summary, e.notes, e.enriched_at, e.last_verified_at, e.source_status,
            ],
        )?;
        Ok(())
    }

    fn get_track_enrichment_v1(&self, track_id: &str) -> Result<Option<TrackEnrichmentV1>> {
        let conn = self.read_conn.lock().unwrap();
        let result = conn.prepare_cached(
            "SELECT track_id, track_kind, work_title, composition_date, recording_date, language,
                    is_instrumental, is_live, is_cover, is_remix, is_remaster, is_arrangement,
                    movement_number, movement_title, key_signature, opus_number, catalog_number,
                    form, confidence, summary, notes, performance_context, enriched_at,
                    last_verified_at, source_status
             FROM track_enrichment_v1 WHERE track_id = ?1",
        )?
        .query_row(params![track_id], |row| {
            Ok(TrackEnrichmentV1 {
                track_id: row.get(0)?,
                track_kind: row.get(1)?,
                work_title: row.get(2)?,
                composition_date: row.get(3)?,
                recording_date: row.get(4)?,
                language: row.get(5)?,
                is_instrumental: int_to_bool(row.get(6)?),
                is_live: int_to_bool(row.get(7)?),
                is_cover: int_to_bool(row.get(8)?),
                is_remix: int_to_bool(row.get(9)?),
                is_remaster: int_to_bool(row.get(10)?),
                is_arrangement: int_to_bool(row.get(11)?),
                movement_number: row.get(12)?,
                movement_title: row.get(13)?,
                key_signature: row.get(14)?,
                opus_number: row.get(15)?,
                catalog_number: row.get(16)?,
                form: row.get(17)?,
                confidence: row.get(18)?,
                summary: row.get(19)?,
                notes: row.get(20)?,
                performance_context: row.get(21)?,
                enriched_at: row.get(22)?,
                last_verified_at: row.get(23)?,
                source_status: row.get(24)?,
            })
        })
        .optional()?;
        Ok(result)
    }

    fn upsert_track_enrichment_v1(&self, e: &TrackEnrichmentV1) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO track_enrichment_v1
             (track_id, track_kind, work_title, composition_date, recording_date, language,
              is_instrumental, is_live, is_cover, is_remix, is_remaster, is_arrangement,
              movement_number, movement_title, key_signature, opus_number, catalog_number,
              form, confidence, summary, notes, performance_context, enriched_at,
              last_verified_at, source_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
            params![
                e.track_id, e.track_kind, e.work_title, e.composition_date, e.recording_date,
                e.language, bool_to_int(&e.is_instrumental), bool_to_int(&e.is_live),
                bool_to_int(&e.is_cover), bool_to_int(&e.is_remix), bool_to_int(&e.is_remaster),
                bool_to_int(&e.is_arrangement), e.movement_number, e.movement_title,
                e.key_signature, e.opus_number, e.catalog_number, e.form, e.confidence,
                e.summary, e.notes, e.performance_context, e.enriched_at, e.last_verified_at,
                e.source_status,
            ],
        )?;
        Ok(())
    }

    fn is_enrichment_missing_or_stale(
        &self,
        entity_type: &str,
        entity_id: &str,
        stale_after_secs: i64,
        now: i64,
    ) -> Result<bool> {
        let (table, id_col) = table_for_entity_type(entity_type)?;
        let conn = self.read_conn.lock().unwrap();
        let sql = format!(
            "SELECT COALESCE(last_verified_at, enriched_at), source_status FROM {} WHERE {} = ?1",
            table, id_col
        );
        let row: Option<(i64, Option<String>)> = conn
            .query_row(&sql, params![entity_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        Ok(match row {
            None => true,
            Some((_ts, Some(source_status))) if source_status == "llm_inferred" => true,
            Some((ts, _source_status)) => now.saturating_sub(ts) >= stale_after_secs,
        })
    }

    fn enqueue_enrichment_if_missing_or_stale(
        &self,
        entity_type: &str,
        entity_id: &str,
        reason: &str,
        priority: i64,
        stale_after_secs: i64,
    ) -> Result<bool> {
        if !valid_entity_type(entity_type) || entity_id.trim().is_empty() {
            return Ok(false);
        }
        let now = now_unix();
        if !self.is_enrichment_missing_or_stale(entity_type, entity_id, stale_after_secs, now)? {
            return Ok(false);
        }
        let conn = self.write_conn.lock().unwrap();
        conn.execute(
            "INSERT INTO enrichment_queue_v1
             (entity_type, entity_id, status, priority, reason, stage, attempts, created_at, updated_at, next_attempt_at, started_at, completed_at, last_error)
             VALUES (?1, ?2, 'queued', ?3, ?4, 'queued', 0, ?5, ?5, NULL, NULL, NULL, NULL)
             ON CONFLICT(entity_type, entity_id) DO UPDATE SET
                status = CASE WHEN enrichment_queue_v1.status = 'running' THEN enrichment_queue_v1.status ELSE 'queued' END,
                priority = MAX(enrichment_queue_v1.priority, excluded.priority),
                reason = excluded.reason,
                updated_at = excluded.updated_at,
                next_attempt_at = NULL,
                completed_at = NULL,
                last_error = NULL
             WHERE enrichment_queue_v1.status != 'running'",
            params![entity_type, entity_id, priority, reason, now],
        )?;
        Ok(true)
    }

    fn get_enrichment_queue_item(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Option<EnrichmentQueueItemV1>> {
        let conn = self.read_conn.lock().unwrap();
        let result = conn
            .prepare_cached(
                "SELECT id, entity_type, entity_id, status, priority, reason, stage, attempts,
                    created_at, updated_at, next_attempt_at, started_at, completed_at, last_error
             FROM enrichment_queue_v1 WHERE entity_type = ?1 AND entity_id = ?2",
            )?
            .query_row(params![entity_type, entity_id], queue_item_from_row)
            .optional()?;
        Ok(result)
    }

    fn claim_enrichment_queue_batch(&self, limit: usize) -> Result<Vec<EnrichmentQueueItemV1>> {
        self.claim_enrichment_queue_batch_matching(limit, &[])
    }

    fn claim_enrichment_queue_batch_for_types(
        &self,
        limit: usize,
        entity_types: &[String],
    ) -> Result<Vec<EnrichmentQueueItemV1>> {
        self.claim_enrichment_queue_batch_matching(limit, entity_types)
    }

    fn requeue_stale_running_enrichment_queue_items(&self, stale_after_secs: i64) -> Result<usize> {
        let now = now_unix();
        let cutoff = now.saturating_sub(stale_after_secs.max(0));
        let conn = self.write_conn.lock().unwrap();
        let count = conn.execute(
            "UPDATE enrichment_queue_v1
             SET status = 'queued',
                 stage = 'queued',
                 updated_at = ?1,
                 next_attempt_at = NULL,
                 started_at = NULL,
                 last_error = 'requeued after interrupted enrichment run'
             WHERE status = 'running'
               AND started_at IS NOT NULL
               AND started_at <= ?2",
            params![now, cutoff],
        )?;
        Ok(count)
    }

    fn complete_enrichment_queue_item(&self, id: i64) -> Result<()> {
        let now = now_unix();
        let conn = self.write_conn.lock().unwrap();
        conn.execute(
            "UPDATE enrichment_queue_v1 SET status = 'completed', stage = 'completed', completed_at = ?1, updated_at = ?1, last_error = NULL WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    fn fail_enrichment_queue_item(
        &self,
        id: i64,
        error: &str,
        retry_after_secs: Option<i64>,
    ) -> Result<()> {
        let now = now_unix();
        let (status, next_attempt_at) = match retry_after_secs {
            Some(secs) => ("queued", Some(now.saturating_add(secs))),
            None => ("failed", None),
        };
        let conn = self.write_conn.lock().unwrap();
        conn.execute(
            "UPDATE enrichment_queue_v1 SET status = ?1, stage = 'failed', updated_at = ?2, next_attempt_at = ?3, last_error = ?4 WHERE id = ?5",
            params![status, now, next_attempt_at, error, id],
        )?;
        Ok(())
    }

    fn get_entity_enrichment_status(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Option<EntityEnrichmentStatusV1>> {
        if !valid_entity_type(entity_type) {
            return Ok(None);
        }
        let (table, id_col) = table_for_entity_type(entity_type)?;
        let conn = self.read_conn.lock().unwrap();
        let enrichment_sql = format!(
            "SELECT enriched_at, source_status FROM {} WHERE {} = ?1",
            table, id_col
        );
        let enriched: Option<(i64, Option<String>)> = conn
            .query_row(&enrichment_sql, params![entity_id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        let queue = conn
            .prepare_cached(
                "SELECT id, entity_type, entity_id, status, priority, reason, stage, attempts,
                    created_at, updated_at, next_attempt_at, started_at, completed_at, last_error
             FROM enrichment_queue_v1 WHERE entity_type = ?1 AND entity_id = ?2",
            )?
            .query_row(params![entity_type, entity_id], queue_item_from_row)
            .optional()?;

        match (queue, enriched) {
            (Some(q), enriched) => Ok(Some(EntityEnrichmentStatusV1 {
                entity_type: entity_type.to_string(),
                entity_id: entity_id.to_string(),
                status: q.status,
                stage: q.stage,
                attempts: q.attempts,
                last_error: q.last_error,
                updated_at: Some(q.updated_at),
                enriched_at: enriched.as_ref().map(|v| v.0),
                source_status: enriched.and_then(|v| v.1),
            })),
            (None, Some((enriched_at, source_status))) => Ok(Some(EntityEnrichmentStatusV1 {
                entity_type: entity_type.to_string(),
                entity_id: entity_id.to_string(),
                status: "completed".to_string(),
                stage: None,
                attempts: 0,
                last_error: None,
                updated_at: None,
                enriched_at: Some(enriched_at),
                source_status,
            })),
            (None, None) => Ok(None),
        }
    }

    fn replace_entity_tags(
        &self,
        entity_type: &str,
        entity_id: &str,
        tags: &[EntityTagV1],
    ) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM entity_tags_v1 WHERE entity_type = ?1 AND entity_id = ?2",
            params![entity_type, entity_id],
        )?;
        for tag in tags {
            tx.execute(
                "INSERT INTO entity_tags_v1 (entity_type, entity_id, tag_type, tag, confidence, source) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![entity_type, entity_id, tag.tag_type, tag.tag, tag.confidence, tag.source],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn list_entity_tags(&self, entity_type: &str, entity_id: &str) -> Result<Vec<EntityTagV1>> {
        let conn = self.read_conn.lock().unwrap();
        let mut stmt = conn.prepare_cached("SELECT tag_type, tag, confidence, source FROM entity_tags_v1 WHERE entity_type = ?1 AND entity_id = ?2 ORDER BY tag_type, tag")?;
        let rows = stmt
            .query_map(params![entity_type, entity_id], |r| {
                Ok(EntityTagV1 {
                    tag_type: r.get(0)?,
                    tag: r.get(1)?,
                    confidence: r.get(2)?,
                    source: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn replace_entity_contributors(
        &self,
        entity_type: &str,
        entity_id: &str,
        contributors: &[EntityContributorV1],
    ) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM entity_contributors_v1 WHERE entity_type = ?1 AND entity_id = ?2",
            params![entity_type, entity_id],
        )?;
        for c in contributors {
            tx.execute(
                "INSERT INTO entity_contributors_v1 (entity_type, entity_id, contributor_name, contributor_id, role, confidence) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![entity_type, entity_id, c.contributor_name, c.contributor_id, c.role, c.confidence],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn list_entity_contributors(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<EntityContributorV1>> {
        let conn = self.read_conn.lock().unwrap();
        let mut stmt = conn.prepare_cached("SELECT contributor_name, contributor_id, role, confidence FROM entity_contributors_v1 WHERE entity_type = ?1 AND entity_id = ?2 ORDER BY role, contributor_name")?;
        let rows = stmt
            .query_map(params![entity_type, entity_id], |r| {
                Ok(EntityContributorV1 {
                    contributor_name: r.get(0)?,
                    contributor_id: r.get(1)?,
                    role: r.get(2)?,
                    confidence: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn replace_entity_relations(
        &self,
        entity_type: &str,
        entity_id: &str,
        relations: &[EntityRelationV1],
    ) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM entity_relations_v1 WHERE source_entity_type = ?1 AND source_entity_id = ?2", params![entity_type, entity_id])?;
        for r in relations {
            tx.execute(
                "INSERT INTO entity_relations_v1 (source_entity_type, source_entity_id, relation_type, target_entity_type, target_entity_id, external_target_name, external_target_url, confidence, visible, evidence) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![entity_type, entity_id, r.relation_type, r.target_entity_type, r.target_entity_id, r.external_target_name, r.external_target_url, r.confidence, if r.visible { 1 } else { 0 }, optional_json_to_string(&r.evidence)?],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn list_visible_entity_relations(
        &self,
        entity_type: &str,
        entity_id: &str,
        min_confidence: f64,
    ) -> Result<Vec<EntityRelationV1>> {
        let conn = self.read_conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT source_entity_type, source_entity_id, relation_type, target_entity_type, target_entity_id,
                    external_target_name, external_target_url, confidence, visible, evidence
             FROM entity_relations_v1
             WHERE source_entity_type = ?1 AND source_entity_id = ?2 AND visible = 1 AND COALESCE(confidence, 0.0) >= ?3
             ORDER BY confidence DESC",
        )?;
        let rows = stmt
            .query_map(params![entity_type, entity_id, min_confidence], |row| {
                Ok(EntityRelationV1 {
                    source_entity_type: row.get(0)?,
                    source_entity_id: row.get(1)?,
                    relation_type: row.get(2)?,
                    target_entity_type: row.get(3)?,
                    target_entity_id: row.get(4)?,
                    external_target_name: row.get(5)?,
                    external_target_url: row.get(6)?,
                    confidence: row.get(7)?,
                    visible: row.get::<_, i32>(8)? != 0,
                    evidence: parse_optional_json(row.get(9)?),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn replace_entity_sources(
        &self,
        entity_type: &str,
        entity_id: &str,
        sources: &[EntitySourceV1],
    ) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM entity_sources_v1 WHERE entity_type = ?1 AND entity_id = ?2",
            params![entity_type, entity_id],
        )?;
        for source in sources {
            tx.execute("INSERT INTO entity_sources_v1 (entity_type, entity_id, source_name, source_url, retrieved_at, confidence) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![entity_type, entity_id, source.source_name, source.source_url, source.retrieved_at, source.confidence])?;
        }
        tx.commit()?;
        Ok(())
    }

    fn replace_entity_aliases(
        &self,
        entity_type: &str,
        entity_id: &str,
        aliases: &[EntityAliasV1],
    ) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM entity_aliases_v1 WHERE entity_type = ?1 AND entity_id = ?2",
            params![entity_type, entity_id],
        )?;
        for alias in aliases {
            tx.execute("INSERT OR IGNORE INTO entity_aliases_v1 (entity_type, entity_id, alias, locale, source, confidence) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![entity_type, entity_id, alias.alias, alias.locale, alias.source, alias.confidence])?;
        }
        tx.commit()?;
        Ok(())
    }

    fn replace_entity_external_ids(
        &self,
        entity_type: &str,
        entity_id: &str,
        external_ids: &[EntityExternalIdV1],
    ) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM entity_external_ids_v1 WHERE entity_type = ?1 AND entity_id = ?2",
            params![entity_type, entity_id],
        )?;
        for ext in external_ids {
            tx.execute("INSERT OR IGNORE INTO entity_external_ids_v1 (entity_type, entity_id, provider, external_id, url, confidence) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![entity_type, entity_id, ext.provider, ext.external_id, ext.url, ext.confidence])?;
        }
        tx.commit()?;
        Ok(())
    }

    fn replace_entity_evidence(
        &self,
        entity_type: &str,
        entity_id: &str,
        evidence: &[EntityEvidenceV1],
    ) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM entity_evidence_v1 WHERE entity_type = ?1 AND entity_id = ?2",
            params![entity_type, entity_id],
        )?;
        for ev in evidence {
            tx.execute("INSERT INTO entity_evidence_v1 (entity_type, entity_id, source_name, source_url, snippet, raw_payload) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![entity_type, entity_id, ev.source_name, ev.source_url, ev.snippet, optional_json_to_string(&ev.raw_payload)?])?;
        }
        tx.commit()?;
        Ok(())
    }

    fn get_enrichment_stats(&self) -> Result<EnrichmentStats> {
        let conn = self.read_conn.lock().unwrap();
        Self::count_rows(&conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_store() -> (SqliteEnrichmentStore, TempDir) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("enrichment.db");
        let store =
            SqliteEnrichmentStore::new(&db_path, &crate::backup::DbRegistry::new()).unwrap();
        (store, tmp)
    }

    fn make_audio_features(track_id: &str) -> AudioFeatures {
        AudioFeatures {
            track_id: track_id.to_string(),
            bpm: 120.0,
            danceability: 0.7,
            key: "C major".to_string(),
            chords_key: "C".to_string(),
            chords_scale: "major".to_string(),
            chords_changes_rate: 0.15,
            loudness: -14.0,
            average_loudness: 0.42,
            dynamic_complexity: 3.5,
            spectral_complexity: 0.5,
            vocal_instrumental: 0.9,
            valence: 0.6,
            analyzed_at: 1700000000,
            analyzer_version: "essentia-2.1-test".to_string(),
        }
    }

    fn make_artist_enrichment(artist_id: &str) -> ArtistEnrichment {
        ArtistEnrichment {
            artist_id: artist_id.to_string(),
            entity_type: Some("band".to_string()),
            nationalities: Some(vec!["american".to_string(), "british".to_string()]),
            decades_active: Some(vec!["1990s".to_string(), "2000s".to_string()]),
            is_composer: Some(false),
            is_producer: Some(false),
            instruments: Some(vec!["guitar".to_string(), "vocals".to_string()]),
            gender: Some("mixed".to_string()),
            vocal_type: Some("tenor".to_string()),
            primary_language: Some("en".to_string()),
            enriched_at: 1700000000,
            source: "test-agent".to_string(),
        }
    }

    fn make_album_enrichment(album_id: &str) -> AlbumEnrichment {
        AlbumEnrichment {
            album_id: album_id.to_string(),
            is_live: Some(false),
            is_compilation: Some(false),
            is_soundtrack: Some(true),
            is_concept_album: Some(false),
            is_remix_album: Some(false),
            primary_language: Some("en".to_string()),
            production_era: Some("modern_digital".to_string()),
            enriched_at: 1700000000,
            source: "test-agent".to_string(),
        }
    }

    // =========================================================================
    // Audio Features Tests
    // =========================================================================

    #[test]
    fn test_audio_features_crud() {
        let (store, _tmp) = create_test_store();
        let features = make_audio_features("track1");

        // Insert
        store.upsert_audio_features(&features).unwrap();

        // Read
        let result = store.get_audio_features("track1").unwrap().unwrap();
        assert_eq!(result.track_id, "track1");
        assert!((result.bpm - 120.0).abs() < f64::EPSILON);
        assert_eq!(result.key, "C major");
        assert_eq!(result.chords_key, "C");
        assert_eq!(result.chords_scale, "major");
        assert!((result.chords_changes_rate - 0.15).abs() < f64::EPSILON);
        assert!((result.average_loudness - 0.42).abs() < f64::EPSILON);
        assert!((result.dynamic_complexity - 3.5).abs() < f64::EPSILON);

        // Update
        let mut updated = features.clone();
        updated.bpm = 140.0;
        updated.key = "A minor".to_string();
        store.upsert_audio_features(&updated).unwrap();

        let result = store.get_audio_features("track1").unwrap().unwrap();
        assert!((result.bpm - 140.0).abs() < f64::EPSILON);
        assert_eq!(result.key, "A minor");

        // Not found
        assert!(store.get_audio_features("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_audio_features_batch() {
        let (store, _tmp) = create_test_store();
        let features: Vec<AudioFeatures> = (0..5)
            .map(|i| make_audio_features(&format!("track{}", i)))
            .collect();

        store.upsert_audio_features_batch(&features).unwrap();

        for i in 0..5 {
            let result = store
                .get_audio_features(&format!("track{}", i))
                .unwrap()
                .unwrap();
            assert_eq!(result.track_id, format!("track{}", i));
        }
    }

    #[test]
    fn test_tracks_needing_analysis() {
        let (store, _tmp) = create_test_store();

        // Analyze 2 tracks
        store
            .upsert_audio_features(&make_audio_features("track1"))
            .unwrap();
        store
            .upsert_audio_features(&make_audio_features("track2"))
            .unwrap();

        let catalog_ids: Vec<String> = (1..=5).map(|i| format!("track{}", i)).collect();
        let needing = store.get_tracks_needing_analysis(&catalog_ids, 10).unwrap();

        assert_eq!(needing.len(), 3);
        assert!(needing.contains(&"track3".to_string()));
        assert!(needing.contains(&"track4".to_string()));
        assert!(needing.contains(&"track5".to_string()));
    }

    #[test]
    fn test_tracks_needing_analysis_with_limit() {
        let (store, _tmp) = create_test_store();

        let catalog_ids: Vec<String> = (1..=10).map(|i| format!("track{}", i)).collect();
        let needing = store.get_tracks_needing_analysis(&catalog_ids, 3).unwrap();

        assert_eq!(needing.len(), 3);
    }

    // =========================================================================
    // Artist Enrichment Tests
    // =========================================================================

    #[test]
    fn test_artist_enrichment_crud() {
        let (store, _tmp) = create_test_store();
        let enrichment = make_artist_enrichment("artist1");

        store.upsert_artist_enrichment(&enrichment).unwrap();

        let result = store.get_artist_enrichment("artist1").unwrap().unwrap();
        assert_eq!(result.artist_id, "artist1");
        assert_eq!(result.entity_type, Some("band".to_string()));
        assert_eq!(
            result.nationalities,
            Some(vec!["american".to_string(), "british".to_string()])
        );
        assert_eq!(result.is_composer, Some(false));
        assert_eq!(result.vocal_type, Some("tenor".to_string()));
        assert_eq!(result.source, "test-agent");

        // Not found
        assert!(store
            .get_artist_enrichment("nonexistent")
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_artist_enrichment_with_nulls() {
        let (store, _tmp) = create_test_store();
        let enrichment = ArtistEnrichment {
            artist_id: "artist1".to_string(),
            entity_type: None,
            nationalities: None,
            decades_active: None,
            is_composer: None,
            is_producer: None,
            instruments: None,
            gender: None,
            vocal_type: None,
            primary_language: None,
            enriched_at: 1700000000,
            source: "test".to_string(),
        };

        store.upsert_artist_enrichment(&enrichment).unwrap();

        let result = store.get_artist_enrichment("artist1").unwrap().unwrap();
        assert!(result.entity_type.is_none());
        assert!(result.nationalities.is_none());
        assert!(result.is_composer.is_none());
    }

    #[test]
    fn test_artists_needing_enrichment() {
        let (store, _tmp) = create_test_store();

        store
            .upsert_artist_enrichment(&make_artist_enrichment("artist1"))
            .unwrap();

        let catalog_ids: Vec<String> = (1..=3).map(|i| format!("artist{}", i)).collect();
        let needing = store
            .get_artists_needing_enrichment(&catalog_ids, 10)
            .unwrap();

        assert_eq!(needing.len(), 2);
        assert!(needing.contains(&"artist2".to_string()));
        assert!(needing.contains(&"artist3".to_string()));
    }

    // =========================================================================
    // Album Enrichment Tests
    // =========================================================================

    #[test]
    fn test_album_enrichment_crud() {
        let (store, _tmp) = create_test_store();
        let enrichment = make_album_enrichment("album1");

        store.upsert_album_enrichment(&enrichment).unwrap();

        let result = store.get_album_enrichment("album1").unwrap().unwrap();
        assert_eq!(result.album_id, "album1");
        assert_eq!(result.is_live, Some(false));
        assert_eq!(result.is_soundtrack, Some(true));
        assert_eq!(result.production_era, Some("modern_digital".to_string()));

        // Not found
        assert!(store.get_album_enrichment("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_albums_needing_enrichment() {
        let (store, _tmp) = create_test_store();

        store
            .upsert_album_enrichment(&make_album_enrichment("album1"))
            .unwrap();
        store
            .upsert_album_enrichment(&make_album_enrichment("album2"))
            .unwrap();

        let catalog_ids: Vec<String> = (1..=4).map(|i| format!("album{}", i)).collect();
        let needing = store
            .get_albums_needing_enrichment(&catalog_ids, 10)
            .unwrap();

        assert_eq!(needing.len(), 2);
        assert!(needing.contains(&"album3".to_string()));
        assert!(needing.contains(&"album4".to_string()));
    }

    // =========================================================================
    // Stats Tests
    // =========================================================================

    #[test]
    fn test_enrichment_stats() {
        let (store, _tmp) = create_test_store();

        let stats = store.get_enrichment_stats().unwrap();
        assert_eq!(stats.tracks_analyzed, 0);
        assert_eq!(stats.artists_enriched, 0);
        assert_eq!(stats.albums_enriched, 0);

        store
            .upsert_audio_features(&make_audio_features("t1"))
            .unwrap();
        store
            .upsert_audio_features(&make_audio_features("t2"))
            .unwrap();
        store
            .upsert_artist_enrichment(&make_artist_enrichment("a1"))
            .unwrap();
        store
            .upsert_album_enrichment(&make_album_enrichment("al1"))
            .unwrap();

        let stats = store.get_enrichment_stats().unwrap();
        assert_eq!(stats.tracks_analyzed, 2);
        assert_eq!(stats.artists_enriched, 1);
        assert_eq!(stats.albums_enriched, 1);
    }

    #[test]
    fn test_upsert_replaces_existing() {
        let (store, _tmp) = create_test_store();

        let mut enrichment = make_artist_enrichment("artist1");
        enrichment.entity_type = Some("person".to_string());
        store.upsert_artist_enrichment(&enrichment).unwrap();

        enrichment.entity_type = Some("band".to_string());
        enrichment.source = "updated-agent".to_string();
        store.upsert_artist_enrichment(&enrichment).unwrap();

        let result = store.get_artist_enrichment("artist1").unwrap().unwrap();
        assert_eq!(result.entity_type, Some("band".to_string()));
        assert_eq!(result.source, "updated-agent");

        // Should still be just 1 row
        let stats = store.get_enrichment_stats().unwrap();
        assert_eq!(stats.artists_enriched, 1);
    }

    #[test]
    fn test_v1_artist_crud_and_stale_detection() {
        let (store, _tmp) = create_test_store();
        assert!(store
            .is_enrichment_missing_or_stale("artist", "artist1", 3600, 2_000)
            .unwrap());

        let enrichment = ArtistEnrichmentV1 {
            artist_id: "artist1".to_string(),
            kind: Some("person".to_string()),
            birth_date: Some("1942-06-18".to_string()),
            death_date: None,
            foundation_date: None,
            dissolution_date: None,
            origin_place: Some("Liverpool".to_string()),
            origin_country: Some("GB".to_string()),
            primary_language: Some("en".to_string()),
            is_person: Some(true),
            is_group: Some(false),
            is_composer: Some(true),
            is_performer: Some(true),
            is_conductor: Some(false),
            is_producer: Some(false),
            confidence: Some(0.95),
            summary: Some("Test summary".to_string()),
            bio: Some("Test bio".to_string()),
            enriched_at: 1_000,
            last_verified_at: Some(1_000),
            source_status: Some("verified".to_string()),
        };
        store.upsert_artist_enrichment_v1(&enrichment).unwrap();
        assert_eq!(
            store.get_artist_enrichment_v1("artist1").unwrap(),
            Some(enrichment)
        );
        assert!(!store
            .is_enrichment_missing_or_stale("artist", "artist1", 3600, 2_000)
            .unwrap());
        assert!(store
            .is_enrichment_missing_or_stale("artist", "artist1", 3600, 5_000)
            .unwrap());

        let mut legacy_enrichment = store.get_artist_enrichment_v1("artist1").unwrap().unwrap();
        legacy_enrichment.last_verified_at = Some(2_000);
        legacy_enrichment.source_status = Some("llm_inferred".to_string());
        store
            .upsert_artist_enrichment_v1(&legacy_enrichment)
            .unwrap();
        assert!(store
            .is_enrichment_missing_or_stale("artist", "artist1", 3600, 2_001)
            .unwrap());
    }

    #[test]
    fn test_v1_queue_dedupes_and_claims() {
        let (store, _tmp) = create_test_store();
        assert!(store
            .enqueue_enrichment_if_missing_or_stale("track", "track1", "impression", 5, 3600)
            .unwrap());
        assert!(store
            .enqueue_enrichment_if_missing_or_stale("track", "track1", "listening", 20, 3600)
            .unwrap());

        let item = store
            .get_enrichment_queue_item("track", "track1")
            .unwrap()
            .unwrap();
        assert_eq!(item.status, "queued");
        assert_eq!(item.priority, 20);
        assert_eq!(item.reason, Some("listening".to_string()));

        store
            .enqueue_enrichment_if_missing_or_stale("artist", "artist1", "impression", 5, 3600)
            .unwrap();
        let artist_only = store
            .claim_enrichment_queue_batch_for_types(10, &["artist".to_string()])
            .unwrap();
        assert_eq!(artist_only.len(), 1);
        assert_eq!(artist_only[0].entity_type, "artist");
        store
            .complete_enrichment_queue_item(artist_only[0].id)
            .unwrap();

        let claimed = store.claim_enrichment_queue_batch(10).unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].status, "running");
        store.complete_enrichment_queue_item(claimed[0].id).unwrap();
        let status = store
            .get_entity_enrichment_status("track", "track1")
            .unwrap()
            .unwrap();
        assert_eq!(status.status, "completed");
    }

    #[test]
    fn test_v1_requeues_stale_running_items() {
        let (store, _tmp) = create_test_store();
        for track_id in ["track1", "track2"] {
            store
                .enqueue_enrichment_if_missing_or_stale("track", track_id, "listening", 5, 3600)
                .unwrap();
        }

        let claimed = store.claim_enrichment_queue_batch(10).unwrap();
        assert_eq!(claimed.len(), 2);

        let stale_started_at = now_unix() - 120;
        {
            let conn = store.write_conn.lock().unwrap();
            conn.execute(
                "UPDATE enrichment_queue_v1 SET started_at = ?1, updated_at = ?1 WHERE id = ?2",
                params![stale_started_at, claimed[0].id],
            )
            .unwrap();
        }

        assert_eq!(
            store
                .requeue_stale_running_enrichment_queue_items(60)
                .unwrap(),
            1
        );

        let stale_item = store
            .get_enrichment_queue_item("track", &claimed[0].entity_id)
            .unwrap()
            .unwrap();
        let fresh_item = store
            .get_enrichment_queue_item("track", &claimed[1].entity_id)
            .unwrap()
            .unwrap();
        assert_eq!(stale_item.status, "queued");
        assert_eq!(stale_item.stage, Some("queued".to_string()));
        assert_eq!(stale_item.started_at, None);
        assert_eq!(fresh_item.status, "running");
    }

    #[test]
    fn test_v1_child_replacement_and_relation_visibility() {
        let (store, _tmp) = create_test_store();
        store
            .replace_entity_tags(
                "album",
                "album1",
                &[
                    EntityTagV1 {
                        tag_type: "genre".to_string(),
                        tag: "rock".to_string(),
                        confidence: Some(0.9),
                        source: Some("test".to_string()),
                    },
                    EntityTagV1 {
                        tag_type: "mood".to_string(),
                        tag: "bright".to_string(),
                        confidence: Some(0.7),
                        source: Some("test".to_string()),
                    },
                ],
            )
            .unwrap();
        assert_eq!(store.list_entity_tags("album", "album1").unwrap().len(), 2);
        store
            .replace_entity_tags(
                "album",
                "album1",
                &[EntityTagV1 {
                    tag_type: "genre".to_string(),
                    tag: "pop".to_string(),
                    confidence: Some(0.8),
                    source: None,
                }],
            )
            .unwrap();
        let tags = store.list_entity_tags("album", "album1").unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].tag, "pop");

        store
            .replace_entity_relations(
                "artist",
                "artist1",
                &[
                    EntityRelationV1 {
                        source_entity_type: "artist".to_string(),
                        source_entity_id: "artist1".to_string(),
                        relation_type: "influenced_by".to_string(),
                        target_entity_type: Some("artist".to_string()),
                        target_entity_id: Some("artist2".to_string()),
                        external_target_name: None,
                        external_target_url: None,
                        confidence: Some(0.95),
                        visible: true,
                        evidence: Some(serde_json::json!({"snippet": "test"})),
                    },
                    EntityRelationV1 {
                        source_entity_type: "artist".to_string(),
                        source_entity_id: "artist1".to_string(),
                        relation_type: "similar_to".to_string(),
                        target_entity_type: Some("artist".to_string()),
                        target_entity_id: Some("artist3".to_string()),
                        external_target_name: None,
                        external_target_url: None,
                        confidence: Some(0.4),
                        visible: true,
                        evidence: None,
                    },
                ],
            )
            .unwrap();
        let visible = store
            .list_visible_entity_relations("artist", "artist1", 0.8)
            .unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].target_entity_id, Some("artist2".to_string()));
    }
}
