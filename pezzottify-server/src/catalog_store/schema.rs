//! SQLite schema definitions for the Spotify catalog database.
//!
//! This module defines the database schema matching the Spotify metadata dump.
//! Primary keys are integer rowids with unique text Spotify IDs for lookups.
//! Images are stored as URLs to Spotify CDN (lazy download on first access).

use crate::sqlite_column;
use crate::sqlite_persistence::{
    Column, ForeignKey, ForeignKeyOnChange, SqlType, Table, VersionedSchema,
};

// =============================================================================
// Core Tables - Spotify Schema
// =============================================================================

/// Artists table - stores artist metadata
const ARTISTS_TABLE: Table = Table {
    name: "artists",
    columns: &[
        sqlite_column!("rowid", &SqlType::Integer, is_primary_key = true),
        sqlite_column!("id", &SqlType::Text, non_null = true), // Spotify base62 ID
        sqlite_column!("name", &SqlType::Text, non_null = true),
        sqlite_column!("followers_total", &SqlType::Integer, non_null = true),
        sqlite_column!("popularity", &SqlType::Integer, non_null = true),
        sqlite_column!(
            "artist_available",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("0")
        ), // 1 if artist has at least one available track
        sqlite_column!("mbid", &SqlType::Text), // MusicBrainz ID (nullable)
        sqlite_column!(
            "mbid_lookup_status",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("0")
        ), // 0=not attempted, 1=found, 2=not found, 3=related fetched
    ],
    indices: &[
        ("idx_artists_id", "id"),
        ("idx_artists_available", "artist_available"),
        (
            "idx_artists_mbid_nonnull",
            "mbid WHERE mbid IS NOT NULL",
        ),
        (
            "idx_artists_need_mbid",
            "artist_available DESC, popularity DESC WHERE mbid_lookup_status = 0",
        ),
        (
            "idx_artists_need_related",
            "artist_available DESC, popularity DESC WHERE mbid_lookup_status = 1 AND mbid IS NOT NULL",
        ),
    ],
    unique_constraints: &[&["id"]],
};

/// Albums table - stores album metadata
const ALBUMS_TABLE: Table = Table {
    name: "albums",
    columns: &[
        sqlite_column!("rowid", &SqlType::Integer, is_primary_key = true),
        sqlite_column!("id", &SqlType::Text, non_null = true), // Spotify base62 ID
        sqlite_column!("name", &SqlType::Text, non_null = true),
        sqlite_column!("album_type", &SqlType::Text, non_null = true), // 'album', 'single', 'compilation'
        sqlite_column!("external_id_upc", &SqlType::Text),
        sqlite_column!("external_id_amgid", &SqlType::Text),
        sqlite_column!("label", &SqlType::Text, non_null = true),
        sqlite_column!("popularity", &SqlType::Integer, non_null = true),
        sqlite_column!("release_date", &SqlType::Text, non_null = true), // '2023-05-15', '2023-05', '2023'
        sqlite_column!("release_date_precision", &SqlType::Text, non_null = true), // 'day', 'month', 'year'
        sqlite_column!(
            "album_availability",
            &SqlType::Text,
            non_null = true,
            default_value = Some("'missing'")
        ), // 'complete', 'partial', 'missing'
        // Duration fingerprint columns for album matching
        sqlite_column!("track_count", &SqlType::Integer), // Pre-computed track count
        sqlite_column!("total_duration_ms", &SqlType::Integer), // Pre-computed total duration
    ],
    indices: &[
        ("idx_albums_id", "id"),
        ("idx_albums_upc", "external_id_upc"),
        ("idx_albums_availability", "album_availability"),
        (
            "idx_albums_complete_id",
            "id WHERE album_availability = 'complete'",
        ),
        ("idx_album_fingerprint", "track_count, total_duration_ms"),
    ],
    unique_constraints: &[&["id"]],
};

/// Tracks table - stores track metadata
const TRACKS_TABLE: Table = Table {
    name: "tracks",
    columns: &[
        sqlite_column!("rowid", &SqlType::Integer, is_primary_key = true),
        sqlite_column!("id", &SqlType::Text, non_null = true), // Spotify base62 ID
        sqlite_column!("name", &SqlType::Text, non_null = true),
        sqlite_column!("album_rowid", &SqlType::Integer, non_null = true),
        sqlite_column!("track_number", &SqlType::Integer, non_null = true),
        sqlite_column!("external_id_isrc", &SqlType::Text),
        sqlite_column!("popularity", &SqlType::Integer, non_null = true),
        sqlite_column!("disc_number", &SqlType::Integer, non_null = true),
        sqlite_column!("duration_ms", &SqlType::Integer, non_null = true),
        sqlite_column!("explicit", &SqlType::Integer, non_null = true),
        sqlite_column!("language", &SqlType::Text), // ISO 639-1 or 'zxx' for instrumental
        sqlite_column!("audio_uri", &SqlType::Text),
        sqlite_column!(
            "track_available",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("0")
        ), // 1 if audio file exists
    ],
    indices: &[
        ("idx_tracks_id", "id"),
        ("idx_tracks_album", "album_rowid"),
        ("idx_tracks_isrc", "external_id_isrc"),
        ("idx_tracks_available", "track_available"),
    ],
    unique_constraints: &[&["id"]],
};

// =============================================================================
// Junction Tables - Spotify Schema
// =============================================================================

/// Track <-> Artist relationship with role
const TRACK_ARTISTS_TABLE: Table = Table {
    name: "track_artists",
    columns: &[
        sqlite_column!("track_rowid", &SqlType::Integer, non_null = true),
        sqlite_column!(
            "artist_rowid",
            &SqlType::Integer,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "artists",
                foreign_column: "rowid",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("role", &SqlType::Integer), // 0=main, 1=featured, 2=composer, 3=remixer, 4=conductor, 5=orchestra
    ],
    indices: &[
        ("idx_track_artists_track", "track_rowid"),
        ("idx_track_artists_artist", "artist_rowid"),
    ],
    unique_constraints: &[],
};

/// Artist <-> Album relationship
const ARTIST_ALBUMS_TABLE: Table = Table {
    name: "artist_albums",
    columns: &[
        sqlite_column!("artist_rowid", &SqlType::Integer, non_null = true),
        sqlite_column!("album_rowid", &SqlType::Integer, non_null = true),
        sqlite_column!("is_appears_on", &SqlType::Integer, non_null = true),
        sqlite_column!("is_implicit_appears_on", &SqlType::Integer, non_null = true),
        sqlite_column!("index_in_album", &SqlType::Integer),
    ],
    indices: &[
        ("idx_artist_albums_artist", "artist_rowid"),
        ("idx_artist_albums_album", "album_rowid"),
    ],
    unique_constraints: &[&["artist_rowid", "album_rowid", "is_appears_on"]],
};

/// Artist <-> Genre relationship
const ARTIST_GENRES_TABLE: Table = Table {
    name: "artist_genres",
    columns: &[
        sqlite_column!("artist_rowid", &SqlType::Integer, non_null = true),
        sqlite_column!("genre", &SqlType::Text, non_null = true),
    ],
    indices: &[("idx_artist_genres_artist", "artist_rowid")],
    unique_constraints: &[],
};

// =============================================================================
// Related Artists Table
// =============================================================================

/// Related artists junction table (populated by enrichment job)
const RELATED_ARTISTS_TABLE: Table = Table {
    name: "related_artists",
    columns: &[
        sqlite_column!("artist_rowid", &SqlType::Integer, non_null = true),
        sqlite_column!("related_artist_rowid", &SqlType::Integer, non_null = true),
        sqlite_column!("match_score", &SqlType::Real, non_null = true),
    ],
    indices: &[
        ("idx_related_artists_artist", "artist_rowid"),
        ("idx_related_artists_related", "related_artist_rowid"),
    ],
    unique_constraints: &[&["artist_rowid", "related_artist_rowid"]],
};

/// Durable work queue for MusicBrainz and related-artist enrichment.
///
/// Rows are admitted lazily in bounded batches for existing catalogs, while
/// newly inserted artists are enqueued by a trigger. This avoids a multi-million
/// row backfill during schema migration.
const ARTIST_ENRICHMENT_QUEUE_TABLE: Table = Table {
    name: "artist_enrichment_queue",
    columns: &[
        sqlite_column!("artist_rowid", &SqlType::Integer, non_null = true),
        sqlite_column!("phase", &SqlType::Text, non_null = true),
        sqlite_column!("status", &SqlType::Text, non_null = true),
        sqlite_column!(
            "attempt_count",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("0")
        ),
        sqlite_column!("next_attempt_at", &SqlType::Integer),
        sqlite_column!("last_attempt_at", &SqlType::Integer),
        sqlite_column!("last_error", &SqlType::Text),
        sqlite_column!("priority", &SqlType::Integer, non_null = true),
        sqlite_column!("created_at", &SqlType::Integer, non_null = true),
        sqlite_column!("updated_at", &SqlType::Integer, non_null = true),
    ],
    indices: &[(
        "idx_artist_enrichment_queue_claim",
        "phase, status, next_attempt_at, priority DESC, artist_rowid",
    )],
    unique_constraints: &[&["artist_rowid", "phase"]],
};

/// O(1) catalog cardinalities maintained transactionally by insert/delete triggers.
const CATALOG_STATS_TABLE: Table = Table {
    name: "catalog_stats",
    columns: &[
        sqlite_column!("id", &SqlType::Integer, is_primary_key = true),
        sqlite_column!("artists_count", &SqlType::Integer),
        sqlite_column!("albums_count", &SqlType::Integer),
        sqlite_column!("tracks_count", &SqlType::Integer),
        sqlite_column!(
            "is_valid",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("0")
        ),
        sqlite_column!(
            "mutation_version",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("0")
        ),
        sqlite_column!("updated_at", &SqlType::Integer, non_null = true),
    ],
    indices: &[],
    unique_constraints: &[],
};

pub(crate) fn create_catalog_stats_triggers(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS trg_catalog_stats_artists_insert
         AFTER INSERT ON artists BEGIN
             UPDATE catalog_stats SET
                 artists_count = CASE WHEN is_valid = 1 THEN artists_count + 1 ELSE artists_count END,
                 mutation_version = mutation_version + 1,
                 updated_at = CAST(strftime('%s', 'now') AS INTEGER)
             WHERE id = 1;
         END;
         CREATE TRIGGER IF NOT EXISTS trg_catalog_stats_artists_delete
         AFTER DELETE ON artists BEGIN
             UPDATE catalog_stats SET
                 artists_count = CASE WHEN is_valid = 1 THEN MAX(artists_count - 1, 0) ELSE artists_count END,
                 mutation_version = mutation_version + 1,
                 updated_at = CAST(strftime('%s', 'now') AS INTEGER)
             WHERE id = 1;
         END;
         CREATE TRIGGER IF NOT EXISTS trg_catalog_stats_albums_insert
         AFTER INSERT ON albums BEGIN
             UPDATE catalog_stats SET
                 albums_count = CASE WHEN is_valid = 1 THEN albums_count + 1 ELSE albums_count END,
                 mutation_version = mutation_version + 1,
                 updated_at = CAST(strftime('%s', 'now') AS INTEGER)
             WHERE id = 1;
         END;
         CREATE TRIGGER IF NOT EXISTS trg_catalog_stats_albums_delete
         AFTER DELETE ON albums BEGIN
             UPDATE catalog_stats SET
                 albums_count = CASE WHEN is_valid = 1 THEN MAX(albums_count - 1, 0) ELSE albums_count END,
                 mutation_version = mutation_version + 1,
                 updated_at = CAST(strftime('%s', 'now') AS INTEGER)
             WHERE id = 1;
         END;
         CREATE TRIGGER IF NOT EXISTS trg_catalog_stats_tracks_insert
         AFTER INSERT ON tracks BEGIN
             UPDATE catalog_stats SET
                 tracks_count = CASE WHEN is_valid = 1 THEN tracks_count + 1 ELSE tracks_count END,
                 mutation_version = mutation_version + 1,
                 updated_at = CAST(strftime('%s', 'now') AS INTEGER)
             WHERE id = 1;
         END;
         CREATE TRIGGER IF NOT EXISTS trg_catalog_stats_tracks_delete
         AFTER DELETE ON tracks BEGIN
             UPDATE catalog_stats SET
                 tracks_count = CASE WHEN is_valid = 1 THEN MAX(tracks_count - 1, 0) ELSE tracks_count END,
                 mutation_version = mutation_version + 1,
                 updated_at = CAST(strftime('%s', 'now') AS INTEGER)
             WHERE id = 1;
         END;",
    )
}

pub(crate) fn initialize_empty_catalog_stats(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO catalog_stats
         (id, artists_count, albums_count, tracks_count, is_valid, mutation_version, updated_at)
         VALUES (1, 0, 0, 0, 1, 0, CAST(strftime('%s', 'now') AS INTEGER))",
        [],
    )?;
    Ok(())
}

pub(crate) fn create_artist_enrichment_enqueue_trigger(
    conn: &rusqlite::Connection,
) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS trg_artists_enqueue_mbid
         AFTER INSERT ON artists
         WHEN NEW.mbid_lookup_status = 0
         BEGIN
             INSERT OR IGNORE INTO artist_enrichment_queue
                 (artist_rowid, phase, status, attempt_count, next_attempt_at,
                  priority, created_at, updated_at)
             VALUES
                 (NEW.rowid, 'mbid', 'queued', 0,
                  CAST(strftime('%s', 'now') AS INTEGER),
                  NEW.artist_available * 1000 + NEW.popularity,
                  CAST(strftime('%s', 'now') AS INTEGER),
                  CAST(strftime('%s', 'now') AS INTEGER));
         END;

         CREATE TRIGGER IF NOT EXISTS trg_artists_requeue_mbid
         AFTER UPDATE OF mbid_lookup_status ON artists
         WHEN NEW.mbid_lookup_status = 0 AND OLD.mbid_lookup_status != 0
         BEGIN
             INSERT INTO artist_enrichment_queue
                 (artist_rowid, phase, status, attempt_count, next_attempt_at,
                  last_attempt_at, last_error, priority, created_at, updated_at)
             VALUES
                 (NEW.rowid, 'mbid', 'queued', 0,
                  CAST(strftime('%s', 'now') AS INTEGER), NULL, NULL,
                  NEW.artist_available * 1000 + NEW.popularity,
                  CAST(strftime('%s', 'now') AS INTEGER),
                  CAST(strftime('%s', 'now') AS INTEGER))
             ON CONFLICT(artist_rowid, phase) DO UPDATE SET
                 status = 'queued',
                 attempt_count = 0,
                 next_attempt_at = excluded.next_attempt_at,
                 last_attempt_at = NULL,
                 last_error = NULL,
                 priority = excluded.priority,
                 updated_at = excluded.updated_at;
             DELETE FROM artist_enrichment_queue
             WHERE artist_rowid = NEW.rowid AND phase = 'related';
         END;",
    )
}

// =============================================================================
// Generic Embeddings
// =============================================================================

/// Generic vector embeddings for any catalog entity.
const ENTITY_EMBEDDINGS_TABLE: Table = Table {
    name: "entity_embeddings",
    columns: &[
        sqlite_column!("entity_type", &SqlType::Text, non_null = true),
        sqlite_column!("entity_id", &SqlType::Text, non_null = true),
        sqlite_column!("namespace", &SqlType::Text, non_null = true),
        sqlite_column!("dim", &SqlType::Integer, non_null = true),
        sqlite_column!("dtype", &SqlType::Text, non_null = true),
        sqlite_column!("vector_blob", &SqlType::Blob, non_null = true),
        sqlite_column!("vector_norm", &SqlType::Real, non_null = true),
        sqlite_column!(
            "metadata_json",
            &SqlType::Text,
            non_null = true,
            default_value = Some("'{}'")
        ),
        sqlite_column!(
            "model_json",
            &SqlType::Text,
            non_null = true,
            default_value = Some("'{}'")
        ),
        sqlite_column!("created_at", &SqlType::Integer, non_null = true),
        sqlite_column!("updated_at", &SqlType::Integer, non_null = true),
    ],
    indices: &[
        ("idx_entity_embeddings_entity", "entity_type, entity_id"),
        ("idx_entity_embeddings_namespace", "namespace"),
        (
            "idx_entity_embeddings_lookup",
            "namespace, entity_type, entity_id",
        ),
    ],
    unique_constraints: &[&["entity_type", "entity_id", "namespace"]],
};

// =============================================================================
// Image Tables - Spotify CDN URLs
// =============================================================================

/// Album images (URLs to Spotify CDN)
const ALBUM_IMAGES_TABLE: Table = Table {
    name: "album_images",
    columns: &[
        sqlite_column!("album_rowid", &SqlType::Integer, non_null = true),
        sqlite_column!("width", &SqlType::Integer, non_null = true),
        sqlite_column!("height", &SqlType::Integer, non_null = true),
        sqlite_column!("url", &SqlType::Text, non_null = true),
    ],
    indices: &[("idx_album_images_album", "album_rowid")],
    unique_constraints: &[],
};

/// Artist images (URLs to Spotify CDN)
const ARTIST_IMAGES_TABLE: Table = Table {
    name: "artist_images",
    columns: &[
        sqlite_column!("artist_rowid", &SqlType::Integer, non_null = true),
        sqlite_column!("width", &SqlType::Integer, non_null = true),
        sqlite_column!("height", &SqlType::Integer, non_null = true),
        sqlite_column!("url", &SqlType::Text, non_null = true),
    ],
    indices: &[("idx_artist_images_artist", "artist_rowid")],
    unique_constraints: &[],
};

// =============================================================================
// Versioned Schema Definition
// =============================================================================

/// Spotify catalog schema.
///
/// This schema matches the Spotify metadata dump structure.
pub const CATALOG_VERSIONED_SCHEMAS: &[VersionedSchema] = &[
    VersionedSchema {
        version: 0,
        tables: &[
            ARTISTS_TABLE,
            ALBUMS_TABLE,
            TRACKS_TABLE,
            TRACK_ARTISTS_TABLE,
            ARTIST_ALBUMS_TABLE,
            ARTIST_GENRES_TABLE,
            ALBUM_IMAGES_TABLE,
            ARTIST_IMAGES_TABLE,
        ],
        migration: None,
    },
    VersionedSchema {
        version: 1,
        tables: &[
            ARTISTS_TABLE,
            ALBUMS_TABLE,
            TRACKS_TABLE,
            TRACK_ARTISTS_TABLE,
            ARTIST_ALBUMS_TABLE,
            ARTIST_GENRES_TABLE,
            ALBUM_IMAGES_TABLE,
            ARTIST_IMAGES_TABLE,
        ],
        migration: Some(|tx: &rusqlite::Connection| {
            tx.execute(
                "ALTER TABLE albums ADD COLUMN album_availability TEXT NOT NULL DEFAULT 'missing'",
                [],
            )?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_albums_availability ON albums(album_availability)",
                [],
            )?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 2,
        tables: &[
            ARTISTS_TABLE,
            ALBUMS_TABLE,
            TRACKS_TABLE,
            TRACK_ARTISTS_TABLE,
            ARTIST_ALBUMS_TABLE,
            ARTIST_GENRES_TABLE,
            ALBUM_IMAGES_TABLE,
            ARTIST_IMAGES_TABLE,
        ],
        migration: Some(|tx: &rusqlite::Connection| {
            // Add track_available column (default 0 = unavailable)
            tx.execute(
                "ALTER TABLE tracks ADD COLUMN track_available INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_tracks_available ON tracks(track_available)",
                [],
            )?;
            // Add artist_available column (default 0 = unavailable)
            tx.execute(
                "ALTER TABLE artists ADD COLUMN artist_available INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_artists_available ON artists(artist_available)",
                [],
            )?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 3,
        tables: &[
            ARTISTS_TABLE,
            ALBUMS_TABLE,
            TRACKS_TABLE,
            TRACK_ARTISTS_TABLE,
            ARTIST_ALBUMS_TABLE,
            ARTIST_GENRES_TABLE,
            ALBUM_IMAGES_TABLE,
            ARTIST_IMAGES_TABLE,
        ],
        migration: Some(|tx: &rusqlite::Connection| {
            // Delete duplicate artist_albums entries, keeping the one with lowest rowid
            tx.execute(
                "DELETE FROM artist_albums
                 WHERE rowid NOT IN (
                     SELECT MIN(rowid)
                     FROM artist_albums
                     GROUP BY artist_rowid, album_rowid, is_appears_on
                 )",
                [],
            )?;
            // Create unique index to prevent future duplicates
            tx.execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_artist_albums_unique
                 ON artist_albums(artist_rowid, album_rowid, is_appears_on)",
                [],
            )?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 4,
        tables: &[
            ARTISTS_TABLE,
            ALBUMS_TABLE,
            TRACKS_TABLE,
            TRACK_ARTISTS_TABLE,
            ARTIST_ALBUMS_TABLE,
            ARTIST_GENRES_TABLE,
            ALBUM_IMAGES_TABLE,
            ARTIST_IMAGES_TABLE,
        ],
        migration: Some(|tx: &rusqlite::Connection| {
            // Add duration fingerprint columns for album matching
            tx.execute("ALTER TABLE albums ADD COLUMN track_count INTEGER", [])?;
            tx.execute(
                "ALTER TABLE albums ADD COLUMN total_duration_ms INTEGER",
                [],
            )?;

            // Populate the columns from existing track data
            tx.execute(
                "UPDATE albums SET
                    track_count = (SELECT COUNT(*) FROM tracks WHERE tracks.album_rowid = albums.rowid),
                    total_duration_ms = (SELECT COALESCE(SUM(duration_ms), 0) FROM tracks WHERE tracks.album_rowid = albums.rowid)",
                [],
            )?;

            // Create composite index for efficient fingerprint queries
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_album_fingerprint ON albums(track_count, total_duration_ms)",
                [],
            )?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 5,
        tables: &[
            ARTISTS_TABLE,
            ALBUMS_TABLE,
            TRACKS_TABLE,
            TRACK_ARTISTS_TABLE,
            ARTIST_ALBUMS_TABLE,
            ARTIST_GENRES_TABLE,
            ALBUM_IMAGES_TABLE,
            ARTIST_IMAGES_TABLE,
            RELATED_ARTISTS_TABLE,
        ],
        migration: Some(|tx: &rusqlite::Connection| {
            // Add MusicBrainz ID and lookup status columns to artists
            tx.execute("ALTER TABLE artists ADD COLUMN mbid TEXT", [])?;
            tx.execute(
                "ALTER TABLE artists ADD COLUMN mbid_lookup_status INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
            // Status: 0=not attempted, 1=mbid found, 2=mbid not found, 3=related artists fetched

            // Create related_artists junction table
            tx.execute(
                "CREATE TABLE IF NOT EXISTS related_artists (
                    artist_rowid INTEGER NOT NULL REFERENCES artists(rowid) ON DELETE CASCADE,
                    related_artist_rowid INTEGER NOT NULL,
                    match_score REAL NOT NULL,
                    UNIQUE(artist_rowid, related_artist_rowid)
                )",
                [],
            )?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_related_artists_artist ON related_artists(artist_rowid)",
                [],
            )?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_related_artists_related ON related_artists(related_artist_rowid)",
                [],
            )?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 6,
        tables: &[
            ARTISTS_TABLE,
            ALBUMS_TABLE,
            TRACKS_TABLE,
            TRACK_ARTISTS_TABLE,
            ARTIST_ALBUMS_TABLE,
            ARTIST_GENRES_TABLE,
            ALBUM_IMAGES_TABLE,
            ARTIST_IMAGES_TABLE,
            RELATED_ARTISTS_TABLE,
            ENTITY_EMBEDDINGS_TABLE,
        ],
        migration: Some(|tx: &rusqlite::Connection| {
            tx.execute(
                "CREATE TABLE IF NOT EXISTS entity_embeddings (
                    entity_type TEXT NOT NULL,
                    entity_id TEXT NOT NULL,
                    namespace TEXT NOT NULL,
                    dim INTEGER NOT NULL,
                    dtype TEXT NOT NULL,
                    vector_blob BLOB NOT NULL,
                    vector_norm REAL NOT NULL,
                    metadata_json TEXT NOT NULL DEFAULT '{}',
                    model_json TEXT NOT NULL DEFAULT '{}',
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    UNIQUE(entity_type, entity_id, namespace)
                )",
                [],
            )?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_entity_embeddings_entity
                 ON entity_embeddings(entity_type, entity_id)",
                [],
            )?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_entity_embeddings_namespace
                 ON entity_embeddings(namespace)",
                [],
            )?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_entity_embeddings_lookup
                 ON entity_embeddings(namespace, entity_type, entity_id)",
                [],
            )?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 7,
        tables: &[
            ARTISTS_TABLE,
            ALBUMS_TABLE,
            TRACKS_TABLE,
            TRACK_ARTISTS_TABLE,
            ARTIST_ALBUMS_TABLE,
            ARTIST_GENRES_TABLE,
            ALBUM_IMAGES_TABLE,
            ARTIST_IMAGES_TABLE,
            RELATED_ARTISTS_TABLE,
            ENTITY_EMBEDDINGS_TABLE,
        ],
        migration: Some(|tx: &rusqlite::Connection| {
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_artists_mbid_nonnull
                 ON artists(mbid) WHERE mbid IS NOT NULL",
                [],
            )?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_artists_need_mbid
                 ON artists(artist_available DESC, popularity DESC)
                 WHERE mbid_lookup_status = 0",
                [],
            )?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_artists_need_related
                 ON artists(artist_available DESC, popularity DESC)
                 WHERE mbid_lookup_status = 1 AND mbid IS NOT NULL",
                [],
            )?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 8,
        tables: &[
            ARTISTS_TABLE,
            ALBUMS_TABLE,
            TRACKS_TABLE,
            TRACK_ARTISTS_TABLE,
            ARTIST_ALBUMS_TABLE,
            ARTIST_GENRES_TABLE,
            ALBUM_IMAGES_TABLE,
            ARTIST_IMAGES_TABLE,
            RELATED_ARTISTS_TABLE,
            ENTITY_EMBEDDINGS_TABLE,
            ARTIST_ENRICHMENT_QUEUE_TABLE,
        ],
        migration: Some(|tx: &rusqlite::Connection| {
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS artist_enrichment_queue (
                    artist_rowid INTEGER NOT NULL,
                    phase TEXT NOT NULL,
                    status TEXT NOT NULL,
                    attempt_count INTEGER NOT NULL DEFAULT 0,
                    next_attempt_at INTEGER,
                    last_attempt_at INTEGER,
                    last_error TEXT,
                    priority INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    UNIQUE(artist_rowid, phase)
                );
                CREATE INDEX IF NOT EXISTS idx_artist_enrichment_queue_claim
                    ON artist_enrichment_queue(
                        phase, status, next_attempt_at, priority DESC, artist_rowid
                    );",
            )?;
            create_artist_enrichment_enqueue_trigger(tx)?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 9,
        tables: &[
            ARTISTS_TABLE,
            ALBUMS_TABLE,
            TRACKS_TABLE,
            TRACK_ARTISTS_TABLE,
            ARTIST_ALBUMS_TABLE,
            ARTIST_GENRES_TABLE,
            ALBUM_IMAGES_TABLE,
            ARTIST_IMAGES_TABLE,
            RELATED_ARTISTS_TABLE,
            ENTITY_EMBEDDINGS_TABLE,
            ARTIST_ENRICHMENT_QUEUE_TABLE,
            CATALOG_STATS_TABLE,
        ],
        migration: Some(|tx: &rusqlite::Connection| {
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS catalog_stats (
                    id INTEGER PRIMARY KEY,
                    artists_count INTEGER,
                    albums_count INTEGER,
                    tracks_count INTEGER,
                    is_valid INTEGER NOT NULL DEFAULT 0,
                    mutation_version INTEGER NOT NULL DEFAULT 0,
                    updated_at INTEGER NOT NULL
                );
                INSERT OR IGNORE INTO catalog_stats
                    (id, artists_count, albums_count, tracks_count, is_valid,
                     mutation_version, updated_at)
                VALUES
                    (1, NULL, NULL, NULL, 0, 0,
                     CAST(strftime('%s', 'now') AS INTEGER));",
            )?;
            create_catalog_stats_triggers(tx)?;
            Ok(())
        }),
    },
    VersionedSchema {
        version: 10,
        tables: &[
            ARTISTS_TABLE,
            ALBUMS_TABLE,
            TRACKS_TABLE,
            TRACK_ARTISTS_TABLE,
            ARTIST_ALBUMS_TABLE,
            ARTIST_GENRES_TABLE,
            ALBUM_IMAGES_TABLE,
            ARTIST_IMAGES_TABLE,
            RELATED_ARTISTS_TABLE,
            ENTITY_EMBEDDINGS_TABLE,
            ARTIST_ENRICHMENT_QUEUE_TABLE,
            CATALOG_STATS_TABLE,
        ],
        migration: Some(|tx: &rusqlite::Connection| {
            tx.execute(
                "CREATE INDEX IF NOT EXISTS idx_albums_complete_id
                 ON albums(id) WHERE album_availability = 'complete'",
                [],
            )?;
            Ok(())
        }),
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_schema_creates_successfully() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &CATALOG_VERSIONED_SCHEMAS[0];
        schema.create(&conn).unwrap();
        schema.validate(&conn).unwrap();
    }

    #[test]
    fn test_latest_schema_creates_entity_embeddings() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = CATALOG_VERSIONED_SCHEMAS.last().unwrap();
        schema.create(&conn).unwrap();
        schema.validate(&conn).unwrap();

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='entity_embeddings'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1);
    }

    #[test]
    fn test_latest_schema_creates_album_pagination_index() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = CATALOG_VERSIONED_SCHEMAS.last().unwrap();
        schema.create(&conn).unwrap();

        let index_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_albums_complete_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(index_sql.contains("ON albums(id) WHERE album_availability = 'complete'"));
    }

    #[test]
    fn test_latest_schema_creates_related_artist_enrichment_indexes() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = CATALOG_VERSIONED_SCHEMAS.last().unwrap();
        schema.create(&conn).unwrap();

        for index_name in [
            "idx_artists_mbid_nonnull",
            "idx_artists_need_mbid",
            "idx_artists_need_related",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    [index_name],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing index {index_name}");
        }

        let candidate_plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN
                 SELECT id, rowid FROM artists
                 WHERE mbid_lookup_status = 0
                 ORDER BY artist_available DESC, popularity DESC
                 LIMIT 200",
                [],
                |row| row.get(3),
            )
            .unwrap();
        assert!(candidate_plan.contains("idx_artists_need_mbid"));

        let related_candidate_plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN
                 SELECT id, mbid, rowid FROM artists
                 WHERE mbid_lookup_status = 1 AND mbid IS NOT NULL
                 ORDER BY artist_available DESC, popularity DESC
                 LIMIT 200",
                [],
                |row| row.get(3),
            )
            .unwrap();
        assert!(related_candidate_plan.contains("idx_artists_need_related"));

        let mbid_plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN SELECT rowid FROM artists WHERE mbid = ?1",
                ["test-mbid"],
                |row| row.get(3),
            )
            .unwrap();
        assert!(mbid_plan.contains("idx_artists_mbid_nonnull"));
    }

    #[test]
    fn test_latest_schema_creates_durable_artist_enrichment_queue() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = CATALOG_VERSIONED_SCHEMAS.last().unwrap();
        schema.create(&conn).unwrap();
        create_artist_enrichment_enqueue_trigger(&conn).unwrap();

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'artist_enrichment_queue'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1);

        conn.execute(
            "INSERT INTO artists (id, name, followers_total, popularity, artist_available)
             VALUES ('new-artist', 'New Artist', 0, 77, 1)",
            [],
        )
        .unwrap();
        let queued: (String, String, i64) = conn
            .query_row(
                "SELECT phase, status, priority FROM artist_enrichment_queue",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(queued, ("mbid".to_string(), "queued".to_string(), 1077));

        conn.execute(
            "UPDATE artist_enrichment_queue
             SET status = 'permanent_failure', attempt_count = 8
             WHERE artist_rowid = (SELECT rowid FROM artists WHERE id = 'new-artist')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO artist_enrichment_queue
             (artist_rowid, phase, status, attempt_count, priority, created_at, updated_at)
             SELECT rowid, 'related', 'completed', 1, 1077, 0, 0
             FROM artists WHERE id = 'new-artist'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE artists SET mbid_lookup_status = 2 WHERE id = 'new-artist'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE artists SET mbid_lookup_status = 0 WHERE id = 'new-artist'",
            [],
        )
        .unwrap();
        let reset: (String, i64) = conn
            .query_row(
                "SELECT status, attempt_count FROM artist_enrichment_queue",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(reset, ("queued".to_string(), 0));
        let phase_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM artist_enrichment_queue", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(
            phase_count, 1,
            "reset must discard stale related phase state"
        );

        let claim_plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN
                 SELECT artist_rowid FROM artist_enrichment_queue
                 WHERE phase = 'mbid' AND status = 'queued'
                   AND next_attempt_at <= 9999999999
                 ORDER BY next_attempt_at ASC, priority DESC, artist_rowid ASC LIMIT 100",
                [],
                |row| row.get(3),
            )
            .unwrap();
        assert!(claim_plan.contains("idx_artist_enrichment_queue_claim"));
    }

    #[test]
    fn test_latest_schema_catalog_stats_triggers_track_cardinality_changes() {
        let conn = Connection::open_in_memory().unwrap();
        CATALOG_VERSIONED_SCHEMAS
            .last()
            .unwrap()
            .create(&conn)
            .unwrap();
        initialize_empty_catalog_stats(&conn).unwrap();
        create_catalog_stats_triggers(&conn).unwrap();

        conn.execute(
            "INSERT INTO artists (id, name, followers_total, popularity)
             VALUES ('artist', 'Artist', 0, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO albums
             (id, name, album_type, label, popularity, release_date, release_date_precision)
             VALUES ('album', 'Album', 'album', '', 0, '2026', 'year')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks
             (id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit)
             VALUES ('track', 'Track', (SELECT rowid FROM albums WHERE id = 'album'),
                     1, 0, 1, 1000, 0)",
            [],
        )
        .unwrap();

        let counts: (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT artists_count, albums_count, tracks_count, mutation_version
                 FROM catalog_stats WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 1, 1, 3));

        conn.execute("DELETE FROM tracks WHERE id = 'track'", [])
            .unwrap();
        conn.execute("DELETE FROM albums WHERE id = 'album'", [])
            .unwrap();
        conn.execute("DELETE FROM artists WHERE id = 'artist'", [])
            .unwrap();
        let counts_after_delete: (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT artists_count, albums_count, tracks_count, mutation_version
                 FROM catalog_stats WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(counts_after_delete, (0, 0, 0, 6));
    }

    #[test]
    fn test_insert_artist_and_genres() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &CATALOG_VERSIONED_SCHEMAS[0];
        schema.create(&conn).unwrap();

        // Insert an artist
        conn.execute(
            "INSERT INTO artists (id, name, followers_total, popularity) VALUES ('0TnOYISbd1XYRBk9myaseg', 'Pitbull', 25000000, 82)",
            [],
        )
        .unwrap();

        // Get the rowid
        let artist_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM artists WHERE id = '0TnOYISbd1XYRBk9myaseg'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        // Insert genres
        conn.execute(
            "INSERT INTO artist_genres (artist_rowid, genre) VALUES (?, 'dance pop')",
            [artist_rowid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO artist_genres (artist_rowid, genre) VALUES (?, 'miami hip hop')",
            [artist_rowid],
        )
        .unwrap();

        // Query genres
        let mut stmt = conn
            .prepare("SELECT genre FROM artist_genres WHERE artist_rowid = ?")
            .unwrap();
        let genres: Vec<String> = stmt
            .query_map([artist_rowid], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(genres.len(), 2);
        assert!(genres.contains(&"dance pop".to_string()));
        assert!(genres.contains(&"miami hip hop".to_string()));
    }

    #[test]
    fn test_insert_album_with_images() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &CATALOG_VERSIONED_SCHEMAS[0];
        schema.create(&conn).unwrap();

        // Insert an album
        conn.execute(
            "INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision)
             VALUES ('4aawyAB9vmqN3uQ7FjRGTy', 'Global Warming', 'album', 'RCA Records', 75, '2012-11-16', 'day')",
            [],
        )
        .unwrap();

        let album_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM albums WHERE id = '4aawyAB9vmqN3uQ7FjRGTy'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        // Insert images (different sizes)
        conn.execute(
            "INSERT INTO album_images (album_rowid, width, height, url) VALUES (?, 640, 640, 'https://i.scdn.co/image/ab67616d0000b273...')",
            [album_rowid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO album_images (album_rowid, width, height, url) VALUES (?, 300, 300, 'https://i.scdn.co/image/ab67616d00001e02...')",
            [album_rowid],
        )
        .unwrap();

        // Query largest image
        let largest_url: String = conn
            .query_row(
                "SELECT url FROM album_images WHERE album_rowid = ? ORDER BY width DESC LIMIT 1",
                [album_rowid],
                |r| r.get(0),
            )
            .unwrap();

        assert!(largest_url.contains("scdn.co"));
    }

    #[test]
    fn test_track_artists_with_roles() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &CATALOG_VERSIONED_SCHEMAS[0];
        schema.create(&conn).unwrap();

        // Insert artists
        conn.execute(
            "INSERT INTO artists (id, name, followers_total, popularity) VALUES ('artist1', 'Main Artist', 1000, 50)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO artists (id, name, followers_total, popularity) VALUES ('artist2', 'Featured Artist', 500, 30)",
            [],
        )
        .unwrap();

        // Insert album and track
        conn.execute(
            "INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision)
             VALUES ('album1', 'Test Album', 'album', 'Test Label', 50, '2023', 'year')",
            [],
        )
        .unwrap();
        let album_rowid: i64 = conn
            .query_row("SELECT rowid FROM albums WHERE id = 'album1'", [], |r| {
                r.get(0)
            })
            .unwrap();

        conn.execute(
            "INSERT INTO tracks (id, name, album_rowid, track_number, popularity, disc_number, duration_ms, explicit)
             VALUES ('track1', 'Test Track', ?, 1, 50, 1, 210000, 0)",
            [album_rowid],
        )
        .unwrap();
        let track_rowid: i64 = conn
            .query_row("SELECT rowid FROM tracks WHERE id = 'track1'", [], |r| {
                r.get(0)
            })
            .unwrap();

        // Get artist rowids
        let artist1_rowid: i64 = conn
            .query_row("SELECT rowid FROM artists WHERE id = 'artist1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let artist2_rowid: i64 = conn
            .query_row("SELECT rowid FROM artists WHERE id = 'artist2'", [], |r| {
                r.get(0)
            })
            .unwrap();

        // Insert track-artist relationships
        conn.execute(
            "INSERT INTO track_artists (track_rowid, artist_rowid, role) VALUES (?, ?, 0)", // main artist
            [track_rowid, artist1_rowid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO track_artists (track_rowid, artist_rowid, role) VALUES (?, ?, 1)", // featured
            [track_rowid, artist2_rowid],
        )
        .unwrap();

        // Query artists for track
        let mut stmt = conn
            .prepare(
                "SELECT a.name, ta.role FROM track_artists ta
                 JOIN artists a ON a.rowid = ta.artist_rowid
                 WHERE ta.track_rowid = ?
                 ORDER BY ta.role",
            )
            .unwrap();
        let artists: Vec<(String, i32)> = stmt
            .query_map([track_rowid], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(artists.len(), 2);
        assert_eq!(artists[0], ("Main Artist".to_string(), 0));
        assert_eq!(artists[1], ("Featured Artist".to_string(), 1));
    }

    #[test]
    fn test_artist_albums_relationship() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &CATALOG_VERSIONED_SCHEMAS[0];
        schema.create(&conn).unwrap();

        // Insert artist
        conn.execute(
            "INSERT INTO artists (id, name, followers_total, popularity) VALUES ('artist1', 'Test Artist', 1000, 50)",
            [],
        )
        .unwrap();
        let artist_rowid: i64 = conn
            .query_row("SELECT rowid FROM artists WHERE id = 'artist1'", [], |r| {
                r.get(0)
            })
            .unwrap();

        // Insert albums
        conn.execute(
            "INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision)
             VALUES ('album1', 'Own Album', 'album', 'Label', 50, '2023', 'year')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision)
             VALUES ('album2', 'Appears On', 'compilation', 'Label', 30, '2022', 'year')",
            [],
        )
        .unwrap();
        let album1_rowid: i64 = conn
            .query_row("SELECT rowid FROM albums WHERE id = 'album1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let album2_rowid: i64 = conn
            .query_row("SELECT rowid FROM albums WHERE id = 'album2'", [], |r| {
                r.get(0)
            })
            .unwrap();

        // Insert artist-album relationships
        conn.execute(
            "INSERT INTO artist_albums (artist_rowid, album_rowid, is_appears_on, is_implicit_appears_on, index_in_album)
             VALUES (?, ?, 0, 0, 0)", // primary artist
            [artist_rowid, album1_rowid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO artist_albums (artist_rowid, album_rowid, is_appears_on, is_implicit_appears_on, index_in_album)
             VALUES (?, ?, 1, 0, NULL)", // appears on
            [artist_rowid, album2_rowid],
        )
        .unwrap();

        // Query own albums
        let own_albums: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM artist_albums WHERE artist_rowid = ? AND is_appears_on = 0",
                [artist_rowid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(own_albums, 1);

        // Query appears-on albums
        let appears_on: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM artist_albums WHERE artist_rowid = ? AND is_appears_on = 1",
                [artist_rowid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(appears_on, 1);
    }
}
