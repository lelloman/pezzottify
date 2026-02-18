//! SQLite-backed enrichment store implementation.

use super::models::{AlbumEnrichment, ArtistEnrichment, AudioFeatures, EnrichmentStats};
use super::schema::ENRICHMENT_VERSIONED_SCHEMAS;
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
        return Ok(());
    }

    let mut current_version = if db_version < BASE_DB_VERSION as i64 {
        0
    } else {
        (db_version - BASE_DB_VERSION as i64) as usize
    };

    if current_version >= latest_version {
        return Ok(());
    }

    let tx = conn.transaction()?;
    for schema in ENRICHMENT_VERSIONED_SCHEMAS.iter().skip(current_version + 1) {
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
    Ok(())
}

impl SqliteEnrichmentStore {
    /// Create a new SqliteEnrichmentStore.
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
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

        write_conn
            .pragma_update(None, "journal_mode", "WAL")
            .context("Failed to set WAL mode on enrichment write connection")?;

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
        let mut stmt = conn.prepare_cached(
            "SELECT 1 FROM audio_features WHERE track_id = ?1",
        )?;

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
        let mut stmt =
            conn.prepare_cached("SELECT 1 FROM album_enrichment WHERE album_id = ?1")?;

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
        let store = SqliteEnrichmentStore::new(&db_path).unwrap();
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
        assert!(store.get_artist_enrichment("nonexistent").unwrap().is_none());
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
}
