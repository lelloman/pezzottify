//! SQLite schema definitions for the enrichment database.

use crate::sqlite_column;
use crate::sqlite_persistence::{Column, SqlType, Table, VersionedSchema};
use anyhow::Result;
use rusqlite::Connection;

/// Track audio features table (populated by audio analysis background job).
const AUDIO_FEATURES_TABLE: Table = Table {
    name: "audio_features",
    columns: &[
        sqlite_column!("track_id", &SqlType::Text, is_primary_key = true),
        // Rhythm
        sqlite_column!("bpm", &SqlType::Real, non_null = true),
        sqlite_column!("danceability", &SqlType::Real, non_null = true),
        // Tonal
        sqlite_column!("key", &SqlType::Text, non_null = true),
        sqlite_column!("chords_key", &SqlType::Text, non_null = true),
        sqlite_column!("chords_scale", &SqlType::Text, non_null = true),
        sqlite_column!("chords_changes_rate", &SqlType::Real, non_null = true),
        // Loudness
        sqlite_column!("loudness", &SqlType::Real, non_null = true),
        sqlite_column!("average_loudness", &SqlType::Real, non_null = true),
        sqlite_column!("dynamic_complexity", &SqlType::Real, non_null = true),
        // Timbre
        sqlite_column!("spectral_complexity", &SqlType::Real, non_null = true),
        // Classifiers (SVM)
        sqlite_column!("vocal_instrumental", &SqlType::Real, non_null = true),
        sqlite_column!("valence", &SqlType::Real, non_null = true),
        // Metadata
        sqlite_column!("analyzed_at", &SqlType::Integer, non_null = true),
        sqlite_column!("analyzer_version", &SqlType::Text, non_null = true),
    ],
    indices: &[],
    unique_constraints: &[],
};

/// Artist canonical enrichment table (populated by external agent).
const ARTIST_ENRICHMENT_TABLE: Table = Table {
    name: "artist_enrichment",
    columns: &[
        sqlite_column!("artist_id", &SqlType::Text, is_primary_key = true),
        sqlite_column!("entity_type", &SqlType::Text),
        sqlite_column!("nationalities", &SqlType::Text), // JSON array
        sqlite_column!("decades_active", &SqlType::Text), // JSON array
        sqlite_column!("is_composer", &SqlType::Integer),
        sqlite_column!("is_producer", &SqlType::Integer),
        sqlite_column!("instruments", &SqlType::Text), // JSON array
        sqlite_column!("gender", &SqlType::Text),
        sqlite_column!("vocal_type", &SqlType::Text),
        sqlite_column!("primary_language", &SqlType::Text),
        sqlite_column!("enriched_at", &SqlType::Integer, non_null = true),
        sqlite_column!("source", &SqlType::Text, non_null = true),
    ],
    indices: &[],
    unique_constraints: &[],
};

/// Album canonical enrichment table (populated by external agent).
const ALBUM_ENRICHMENT_TABLE: Table = Table {
    name: "album_enrichment",
    columns: &[
        sqlite_column!("album_id", &SqlType::Text, is_primary_key = true),
        sqlite_column!("is_live", &SqlType::Integer),
        sqlite_column!("is_compilation", &SqlType::Integer),
        sqlite_column!("is_soundtrack", &SqlType::Integer),
        sqlite_column!("is_concept_album", &SqlType::Integer),
        sqlite_column!("is_remix_album", &SqlType::Integer),
        sqlite_column!("primary_language", &SqlType::Text),
        sqlite_column!("production_era", &SqlType::Text),
        sqlite_column!("enriched_at", &SqlType::Integer, non_null = true),
        sqlite_column!("source", &SqlType::Text, non_null = true),
    ],
    indices: &[],
    unique_constraints: &[],
};

pub const ENRICHMENT_VERSIONED_SCHEMAS: &[VersionedSchema] = &[VersionedSchema {
    version: 0,
    tables: &[
        AUDIO_FEATURES_TABLE,
        ARTIST_ENRICHMENT_TABLE,
        ALBUM_ENRICHMENT_TABLE,
    ],
    migration: None,
}];

/// Create the explicit v1 metadata enrichment schema.
///
/// These tables are intentionally named with a `_v1` suffix because they are
/// the queryable storage API for generated metadata. Legacy `artist_enrichment`
/// and `album_enrichment` remain untouched for compatibility.
pub fn create_enrichment_v1_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS artist_enrichment_v1 (
            artist_id TEXT PRIMARY KEY,
            kind TEXT,
            birth_date TEXT,
            death_date TEXT,
            foundation_date TEXT,
            dissolution_date TEXT,
            origin_place TEXT,
            origin_country TEXT,
            primary_language TEXT,
            is_person INTEGER,
            is_group INTEGER,
            is_composer INTEGER,
            is_performer INTEGER,
            is_conductor INTEGER,
            is_producer INTEGER,
            confidence REAL,
            summary TEXT,
            bio TEXT,
            enriched_at INTEGER NOT NULL,
            last_verified_at INTEGER,
            source_status TEXT
        );

        CREATE TABLE IF NOT EXISTS album_enrichment_v1 (
            album_id TEXT PRIMARY KEY,
            album_kind TEXT,
            original_release_date TEXT,
            recording_start_date TEXT,
            recording_end_date TEXT,
            release_country TEXT,
            label TEXT,
            catalog_number TEXT,
            is_live INTEGER,
            is_compilation INTEGER,
            is_soundtrack INTEGER,
            is_concept_album INTEGER,
            is_remix_album INTEGER,
            is_archival INTEGER,
            confidence REAL,
            summary TEXT,
            notes TEXT,
            enriched_at INTEGER NOT NULL,
            last_verified_at INTEGER,
            source_status TEXT
        );

        CREATE TABLE IF NOT EXISTS track_enrichment_v1 (
            track_id TEXT PRIMARY KEY,
            track_kind TEXT,
            work_title TEXT,
            composition_date TEXT,
            recording_date TEXT,
            language TEXT,
            is_instrumental INTEGER,
            is_live INTEGER,
            is_cover INTEGER,
            is_remix INTEGER,
            is_remaster INTEGER,
            is_arrangement INTEGER,
            movement_number INTEGER,
            movement_title TEXT,
            key_signature TEXT,
            opus_number TEXT,
            catalog_number TEXT,
            form TEXT,
            confidence REAL,
            summary TEXT,
            notes TEXT,
            performance_context TEXT,
            enriched_at INTEGER NOT NULL,
            last_verified_at INTEGER,
            source_status TEXT
        );

        CREATE TABLE IF NOT EXISTS enrichment_queue_v1 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'queued',
            priority INTEGER NOT NULL DEFAULT 0,
            reason TEXT,
            stage TEXT,
            attempts INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as int)),
            updated_at INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as int)),
            next_attempt_at INTEGER,
            started_at INTEGER,
            completed_at INTEGER,
            last_error TEXT,
            UNIQUE(entity_type, entity_id)
        );

        CREATE TABLE IF NOT EXISTS entity_relations_v1 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_entity_type TEXT NOT NULL,
            source_entity_id TEXT NOT NULL,
            relation_type TEXT NOT NULL,
            target_entity_type TEXT,
            target_entity_id TEXT,
            external_target_name TEXT,
            external_target_url TEXT,
            confidence REAL,
            visible INTEGER NOT NULL DEFAULT 0,
            evidence TEXT,
            created_at INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as int))
        );

        CREATE TABLE IF NOT EXISTS entity_contributors_v1 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            contributor_name TEXT NOT NULL,
            contributor_id TEXT,
            role TEXT NOT NULL,
            confidence REAL,
            created_at INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as int))
        );

        CREATE TABLE IF NOT EXISTS entity_tags_v1 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            tag_type TEXT NOT NULL,
            tag TEXT NOT NULL,
            confidence REAL,
            source TEXT,
            created_at INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as int)),
            UNIQUE(entity_type, entity_id, tag_type, tag)
        );

        CREATE TABLE IF NOT EXISTS entity_aliases_v1 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            alias TEXT NOT NULL,
            locale TEXT,
            source TEXT,
            confidence REAL,
            created_at INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as int)),
            UNIQUE(entity_type, entity_id, alias, locale, source)
        );

        CREATE TABLE IF NOT EXISTS entity_external_ids_v1 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            external_id TEXT,
            url TEXT,
            confidence REAL,
            created_at INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as int)),
            UNIQUE(entity_type, entity_id, provider, external_id, url)
        );

        CREATE TABLE IF NOT EXISTS entity_sources_v1 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            source_name TEXT NOT NULL,
            source_url TEXT,
            retrieved_at INTEGER,
            confidence REAL,
            created_at INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as int))
        );

        CREATE TABLE IF NOT EXISTS entity_evidence_v1 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            source_name TEXT,
            source_url TEXT,
            snippet TEXT,
            raw_payload TEXT,
            created_at INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as int))
        );

        CREATE INDEX IF NOT EXISTS idx_artist_enrichment_v1_flags ON artist_enrichment_v1(is_person, is_group, is_composer, is_performer);
        CREATE INDEX IF NOT EXISTS idx_album_enrichment_v1_flags ON album_enrichment_v1(is_live, is_compilation, is_soundtrack);
        CREATE INDEX IF NOT EXISTS idx_track_enrichment_v1_flags ON track_enrichment_v1(is_instrumental, is_live, is_cover, is_remix);
        CREATE INDEX IF NOT EXISTS idx_enrichment_queue_v1_status_priority ON enrichment_queue_v1(status, priority DESC, next_attempt_at, updated_at);
        CREATE INDEX IF NOT EXISTS idx_enrichment_queue_v1_entity ON enrichment_queue_v1(entity_type, entity_id);
        CREATE INDEX IF NOT EXISTS idx_entity_relations_v1_source ON entity_relations_v1(source_entity_type, source_entity_id, relation_type);
        CREATE INDEX IF NOT EXISTS idx_entity_relations_v1_visible ON entity_relations_v1(source_entity_type, source_entity_id, visible, confidence);
        CREATE INDEX IF NOT EXISTS idx_entity_contributors_v1_entity ON entity_contributors_v1(entity_type, entity_id, role);
        CREATE INDEX IF NOT EXISTS idx_entity_tags_v1_entity ON entity_tags_v1(entity_type, entity_id, tag_type);
        CREATE INDEX IF NOT EXISTS idx_entity_aliases_v1_entity ON entity_aliases_v1(entity_type, entity_id);
        CREATE INDEX IF NOT EXISTS idx_entity_external_ids_v1_entity ON entity_external_ids_v1(entity_type, entity_id, provider);
        CREATE INDEX IF NOT EXISTS idx_entity_sources_v1_entity ON entity_sources_v1(entity_type, entity_id);
        CREATE INDEX IF NOT EXISTS idx_entity_evidence_v1_entity ON entity_evidence_v1(entity_type, entity_id);
        "#,
    )?;
    Ok(())
}
