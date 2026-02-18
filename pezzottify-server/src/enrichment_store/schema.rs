//! SQLite schema definitions for the enrichment database.

use crate::sqlite_column;
use crate::sqlite_persistence::{Column, SqlType, Table, VersionedSchema};

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
