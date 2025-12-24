//! SQLite schema definitions for the catalog database.
//!
//! This module defines the database schema for storing music catalog metadata.
//! Audio and image files remain on the filesystem, referenced by relative URIs.
#![allow(dead_code)]

use crate::sqlite_column;
use crate::sqlite_persistence::{
    Column, ForeignKey, ForeignKeyOnChange, SqlType, Table, VersionedSchema,
};

// =============================================================================
// Core Tables - Version 0
// =============================================================================

/// Artists table - stores artist metadata
const ARTISTS_TABLE_V0: Table = Table {
    name: "artists",
    columns: &[
        sqlite_column!("id", &SqlType::Text, is_primary_key = true),
        sqlite_column!("name", &SqlType::Text, non_null = true),
        sqlite_column!("genres", &SqlType::Text), // JSON array: ["rock", "metal"]
        sqlite_column!("activity_periods", &SqlType::Text), // JSON array of ActivityPeriod
    ],
    indices: &[],
    unique_constraints: &[],
};

/// Albums table - stores album metadata
const ALBUMS_TABLE_V0: Table = Table {
    name: "albums",
    columns: &[
        sqlite_column!("id", &SqlType::Text, is_primary_key = true),
        sqlite_column!("name", &SqlType::Text, non_null = true),
        sqlite_column!("album_type", &SqlType::Text, non_null = true), // 'ALBUM', 'SINGLE', 'EP', etc.
        sqlite_column!("label", &SqlType::Text),
        sqlite_column!("release_date", &SqlType::Integer), // Unix timestamp
        sqlite_column!("genres", &SqlType::Text),          // JSON array
        sqlite_column!("original_title", &SqlType::Text),
        sqlite_column!("version_title", &SqlType::Text),
    ],
    indices: &[],
    unique_constraints: &[],
};

/// Tracks table - stores track metadata with foreign key to albums
const TRACKS_TABLE_V0: Table = Table {
    name: "tracks",
    columns: &[
        sqlite_column!("id", &SqlType::Text, is_primary_key = true),
        sqlite_column!("name", &SqlType::Text, non_null = true),
        sqlite_column!(
            "album_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "albums",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!(
            "disc_number",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("1")
        ),
        sqlite_column!("track_number", &SqlType::Integer, non_null = true),
        sqlite_column!("duration_secs", &SqlType::Integer),
        sqlite_column!(
            "is_explicit",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("0")
        ),
        sqlite_column!("audio_uri", &SqlType::Text, non_null = true), // Relative path
        sqlite_column!("format", &SqlType::Text, non_null = true),    // 'MP3_320', 'FLAC', etc.
        sqlite_column!("tags", &SqlType::Text),                       // JSON array
        sqlite_column!(
            "has_lyrics",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("0")
        ),
        sqlite_column!("languages", &SqlType::Text), // JSON array
        sqlite_column!("original_title", &SqlType::Text),
        sqlite_column!("version_title", &SqlType::Text),
    ],
    indices: &[
        ("idx_tracks_album", "album_id"),
        (
            "idx_tracks_disc_track",
            "album_id, disc_number, track_number",
        ),
    ],
    unique_constraints: &[],
};

/// Images table - stores image metadata
const IMAGES_TABLE_V0: Table = Table {
    name: "images",
    columns: &[
        sqlite_column!("id", &SqlType::Text, is_primary_key = true),
        sqlite_column!("uri", &SqlType::Text, non_null = true), // Relative path
        sqlite_column!("size", &SqlType::Text, non_null = true), // 'DEFAULT', 'SMALL', 'LARGE', 'XLARGE'
        sqlite_column!("width", &SqlType::Integer, non_null = true),
        sqlite_column!("height", &SqlType::Integer, non_null = true),
    ],
    indices: &[],
    unique_constraints: &[],
};

// =============================================================================
// Relationship Tables - Version 0
// =============================================================================

/// Album <-> Artist relationship (many-to-many)
const ALBUM_ARTISTS_TABLE_V0: Table = Table {
    name: "album_artists",
    columns: &[
        sqlite_column!(
            "album_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "albums",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!(
            "artist_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "artists",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("position", &SqlType::Integer, non_null = true), // Ordering of artists
    ],
    indices: &[("idx_album_artists_artist", "artist_id")],
    unique_constraints: &[&["album_id", "artist_id"]],
};

/// Track <-> Artist relationship (many-to-many with role)
const TRACK_ARTISTS_TABLE_V0: Table = Table {
    name: "track_artists",
    columns: &[
        sqlite_column!(
            "track_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "tracks",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!(
            "artist_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "artists",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("role", &SqlType::Text, non_null = true), // 'MAIN_ARTIST', 'FEATURED_ARTIST', etc.
        sqlite_column!("position", &SqlType::Integer, non_null = true),
    ],
    indices: &[("idx_track_artists_artist", "artist_id")],
    unique_constraints: &[&["track_id", "artist_id", "role"]],
};

/// Artist <-> Artist relationship (related artists, many-to-many)
const RELATED_ARTISTS_TABLE_V0: Table = Table {
    name: "related_artists",
    columns: &[
        sqlite_column!(
            "artist_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "artists",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!(
            "related_artist_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "artists",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
    ],
    indices: &[],
    unique_constraints: &[&["artist_id", "related_artist_id"]],
};

/// Artist <-> Image relationship (many-to-many with type)
const ARTIST_IMAGES_TABLE_V0: Table = Table {
    name: "artist_images",
    columns: &[
        sqlite_column!(
            "artist_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "artists",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!(
            "image_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "images",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("image_type", &SqlType::Text, non_null = true), // 'portrait', 'portrait_group'
        sqlite_column!("position", &SqlType::Integer, non_null = true),
    ],
    indices: &[("idx_artist_images_artist", "artist_id")],
    unique_constraints: &[&["artist_id", "image_id", "image_type"]],
};

/// Album <-> Image relationship (many-to-many with type)
const ALBUM_IMAGES_TABLE_V0: Table = Table {
    name: "album_images",
    columns: &[
        sqlite_column!(
            "album_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "albums",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!(
            "image_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "images",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("image_type", &SqlType::Text, non_null = true), // 'cover', 'cover_group'
        sqlite_column!("position", &SqlType::Integer, non_null = true),
    ],
    indices: &[("idx_album_images_album", "album_id")],
    unique_constraints: &[&["album_id", "image_id", "image_type"]],
};

// =============================================================================
// Version 1 - Add display_image_id to artists and albums
// =============================================================================

/// Artists table with display_image_id - Version 1
const ARTISTS_TABLE_V1: Table = Table {
    name: "artists",
    columns: &[
        sqlite_column!("id", &SqlType::Text, is_primary_key = true),
        sqlite_column!("name", &SqlType::Text, non_null = true),
        sqlite_column!("genres", &SqlType::Text), // JSON array: ["rock", "metal"]
        sqlite_column!("activity_periods", &SqlType::Text), // JSON array of ActivityPeriod
        sqlite_column!(
            "display_image_id",
            &SqlType::Text,
            foreign_key = Some(&ForeignKey {
                foreign_table: "images",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::SetNull,
            })
        ),
    ],
    indices: &[],
    unique_constraints: &[],
};

/// Albums table with display_image_id - Version 1
const ALBUMS_TABLE_V1: Table = Table {
    name: "albums",
    columns: &[
        sqlite_column!("id", &SqlType::Text, is_primary_key = true),
        sqlite_column!("name", &SqlType::Text, non_null = true),
        sqlite_column!("album_type", &SqlType::Text, non_null = true), // 'ALBUM', 'SINGLE', 'EP', etc.
        sqlite_column!("label", &SqlType::Text),
        sqlite_column!("release_date", &SqlType::Integer), // Unix timestamp
        sqlite_column!("genres", &SqlType::Text),          // JSON array
        sqlite_column!("original_title", &SqlType::Text),
        sqlite_column!("version_title", &SqlType::Text),
        sqlite_column!(
            "display_image_id",
            &SqlType::Text,
            foreign_key = Some(&ForeignKey {
                foreign_table: "images",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::SetNull,
            })
        ),
    ],
    indices: &[],
    unique_constraints: &[],
};

/// Migration from version 0 to version 1: add display_image_id columns
fn migrate_v0_to_v1(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    conn.execute(
        "ALTER TABLE artists ADD COLUMN display_image_id TEXT REFERENCES images(id) ON DELETE SET NULL",
        [],
    )?;
    conn.execute(
        "ALTER TABLE albums ADD COLUMN display_image_id TEXT REFERENCES images(id) ON DELETE SET NULL",
        [],
    )?;
    Ok(())
}

// =============================================================================
// Version 2 - Add catalog changelog tables
// =============================================================================

/// Catalog batches table - groups related catalog changes
const CATALOG_BATCHES_TABLE_V2: Table = Table {
    name: "catalog_batches",
    columns: &[
        sqlite_column!("id", &SqlType::Text, is_primary_key = true),
        sqlite_column!("name", &SqlType::Text, non_null = true),
        sqlite_column!("description", &SqlType::Text),
        sqlite_column!(
            "is_open",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("1")
        ),
        sqlite_column!("created_at", &SqlType::Integer, non_null = true),
        sqlite_column!("closed_at", &SqlType::Integer),
        sqlite_column!("last_activity_at", &SqlType::Integer, non_null = true),
    ],
    indices: &[
        ("idx_batches_is_open", "is_open"),
        ("idx_batches_closed_at", "closed_at DESC"),
    ],
    unique_constraints: &[],
};

/// Catalog change log table - tracks individual entity changes
const CATALOG_CHANGE_LOG_TABLE_V2: Table = Table {
    name: "catalog_change_log",
    columns: &[
        sqlite_column!("id", &SqlType::Integer, is_primary_key = true), // AUTOINCREMENT via INTEGER PRIMARY KEY
        sqlite_column!(
            "batch_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "catalog_batches",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("entity_type", &SqlType::Text, non_null = true), // 'artist', 'album', 'track', 'image'
        sqlite_column!("entity_id", &SqlType::Text, non_null = true),
        sqlite_column!("operation", &SqlType::Text, non_null = true), // 'create', 'update', 'delete'
        sqlite_column!("field_changes", &SqlType::Text, non_null = true), // JSON
        sqlite_column!("entity_snapshot", &SqlType::Text, non_null = true), // JSON
        sqlite_column!("display_summary", &SqlType::Text),
        sqlite_column!("created_at", &SqlType::Integer, non_null = true),
    ],
    indices: &[
        ("idx_changelog_batch", "batch_id"),
        ("idx_changelog_entity", "entity_type, entity_id"),
        ("idx_changelog_created", "created_at DESC"),
    ],
    unique_constraints: &[],
};

/// Tracks table with availability - Version 3
const TRACKS_TABLE_V3: Table = Table {
    name: "tracks",
    columns: &[
        sqlite_column!("id", &SqlType::Text, is_primary_key = true),
        sqlite_column!("name", &SqlType::Text, non_null = true),
        sqlite_column!(
            "album_id",
            &SqlType::Text,
            non_null = true,
            foreign_key = Some(&ForeignKey {
                foreign_table: "albums",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!(
            "disc_number",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("1")
        ),
        sqlite_column!("track_number", &SqlType::Integer, non_null = true),
        sqlite_column!("duration_secs", &SqlType::Integer),
        sqlite_column!(
            "is_explicit",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("0")
        ),
        sqlite_column!("audio_uri", &SqlType::Text, non_null = true), // Relative path
        sqlite_column!("format", &SqlType::Text, non_null = true),    // 'MP3_320', 'FLAC', etc.
        sqlite_column!("tags", &SqlType::Text),                       // JSON array
        sqlite_column!(
            "has_lyrics",
            &SqlType::Integer,
            non_null = true,
            default_value = Some("0")
        ),
        sqlite_column!("languages", &SqlType::Text), // JSON array
        sqlite_column!("original_title", &SqlType::Text),
        sqlite_column!("version_title", &SqlType::Text),
        sqlite_column!(
            "availability",
            &SqlType::Text,
            non_null = true,
            default_value = Some("'available'")
        ), // 'available', 'unavailable', 'fetching', 'fetch_error'
    ],
    indices: &[
        ("idx_tracks_album", "album_id"),
        (
            "idx_tracks_disc_track",
            "album_id, disc_number, track_number",
        ),
        ("idx_tracks_availability", "availability"),
    ],
    unique_constraints: &[],
};

/// Migration from version 2 to version 3: add availability column to tracks
fn migrate_v2_to_v3(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    conn.execute(
        "ALTER TABLE tracks ADD COLUMN availability TEXT NOT NULL DEFAULT 'available'",
        [],
    )?;
    conn.execute(
        "CREATE INDEX idx_tracks_availability ON tracks(availability)",
        [],
    )?;
    Ok(())
}

/// Migration from version 1 to version 2: add changelog tables
fn migrate_v1_to_v2(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    // Create catalog_batches table
    conn.execute(
        "CREATE TABLE catalog_batches (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            is_open INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            closed_at INTEGER,
            last_activity_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX idx_batches_is_open ON catalog_batches(is_open)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX idx_batches_closed_at ON catalog_batches(closed_at DESC)",
        [],
    )?;

    // Create catalog_change_log table
    conn.execute(
        "CREATE TABLE catalog_change_log (
            id INTEGER PRIMARY KEY,
            batch_id TEXT NOT NULL REFERENCES catalog_batches(id) ON DELETE CASCADE,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            operation TEXT NOT NULL,
            field_changes TEXT NOT NULL,
            entity_snapshot TEXT NOT NULL,
            display_summary TEXT,
            created_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX idx_changelog_batch ON catalog_change_log(batch_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX idx_changelog_entity ON catalog_change_log(entity_type, entity_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX idx_changelog_created ON catalog_change_log(created_at DESC)",
        [],
    )?;

    Ok(())
}

// =============================================================================
// Versioned Schema Definition
// =============================================================================

/// All versioned schemas for the catalog database.
///
/// The catalog database uses a separate version namespace from the user database.
/// Initial version (0) contains all core tables and relationship tables.
/// Version 1 adds display_image_id to artists and albums.
/// Version 2 adds catalog changelog tables (catalog_batches, catalog_change_log).
/// Version 3 adds availability column to tracks for Quentin Torrentino integration.
pub const CATALOG_VERSIONED_SCHEMAS: &[VersionedSchema] = &[
    VersionedSchema {
        version: 0,
        tables: &[
            // Core tables first (order matters for foreign keys)
            ARTISTS_TABLE_V0,
            ALBUMS_TABLE_V0,
            IMAGES_TABLE_V0,
            TRACKS_TABLE_V0,
            // Relationship tables
            ALBUM_ARTISTS_TABLE_V0,
            TRACK_ARTISTS_TABLE_V0,
            RELATED_ARTISTS_TABLE_V0,
            ARTIST_IMAGES_TABLE_V0,
            ALBUM_IMAGES_TABLE_V0,
        ],
        migration: None, // Initial version has no migration
    },
    VersionedSchema {
        version: 1,
        tables: &[
            // Core tables first (order matters for foreign keys)
            ARTISTS_TABLE_V1,
            ALBUMS_TABLE_V1,
            IMAGES_TABLE_V0,
            TRACKS_TABLE_V0,
            // Relationship tables
            ALBUM_ARTISTS_TABLE_V0,
            TRACK_ARTISTS_TABLE_V0,
            RELATED_ARTISTS_TABLE_V0,
            ARTIST_IMAGES_TABLE_V0,
            ALBUM_IMAGES_TABLE_V0,
        ],
        migration: Some(migrate_v0_to_v1),
    },
    VersionedSchema {
        version: 2,
        tables: &[
            // Core tables first (order matters for foreign keys)
            ARTISTS_TABLE_V1,
            ALBUMS_TABLE_V1,
            IMAGES_TABLE_V0,
            TRACKS_TABLE_V0,
            // Relationship tables
            ALBUM_ARTISTS_TABLE_V0,
            TRACK_ARTISTS_TABLE_V0,
            RELATED_ARTISTS_TABLE_V0,
            ARTIST_IMAGES_TABLE_V0,
            ALBUM_IMAGES_TABLE_V0,
            // Changelog tables
            CATALOG_BATCHES_TABLE_V2,
            CATALOG_CHANGE_LOG_TABLE_V2,
        ],
        migration: Some(migrate_v1_to_v2),
    },
    VersionedSchema {
        version: 3,
        tables: &[
            // Core tables first (order matters for foreign keys)
            ARTISTS_TABLE_V1,
            ALBUMS_TABLE_V1,
            IMAGES_TABLE_V0,
            TRACKS_TABLE_V3,
            // Relationship tables
            ALBUM_ARTISTS_TABLE_V0,
            TRACK_ARTISTS_TABLE_V0,
            RELATED_ARTISTS_TABLE_V0,
            ARTIST_IMAGES_TABLE_V0,
            ALBUM_IMAGES_TABLE_V0,
            // Changelog tables
            CATALOG_BATCHES_TABLE_V2,
            CATALOG_CHANGE_LOG_TABLE_V2,
        ],
        migration: Some(migrate_v2_to_v3),
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
    fn test_foreign_key_cascade_on_album_delete() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
        let schema = &CATALOG_VERSIONED_SCHEMAS[0];
        schema.create(&conn).unwrap();

        // Insert an artist
        conn.execute(
            "INSERT INTO artists (id, name) VALUES ('test_artist_001', 'Test Artist')",
            [],
        )
        .unwrap();

        // Insert an album
        conn.execute(
            "INSERT INTO albums (id, name, album_type) VALUES ('test_album_001', 'Test Album', 'ALBUM')",
            [],
        )
        .unwrap();

        // Insert album-artist relationship
        conn.execute(
            "INSERT INTO album_artists (album_id, artist_id, position) VALUES ('test_album_001', 'test_artist_001', 0)",
            [],
        )
        .unwrap();

        // Insert a track
        conn.execute(
            "INSERT INTO tracks (id, name, album_id, track_number, audio_uri, format) VALUES ('test_track_001', 'Test Track', 'test_album_001', 1, 'albums/test_album_001/test_track_001.mp3', 'MP3_320')",
            [],
        )
        .unwrap();

        // Verify track exists
        let track_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tracks WHERE album_id = 'test_album_001'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(track_count, 1);

        // Delete the album
        conn.execute("DELETE FROM albums WHERE id = 'test_album_001'", [])
            .unwrap();

        // Verify track was cascade deleted
        let track_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(track_count, 0);

        // Verify album_artists was cascade deleted
        let rel_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM album_artists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rel_count, 0);

        // Artist should still exist
        let artist_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM artists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(artist_count, 1);
    }

    #[test]
    fn test_foreign_key_cascade_on_artist_delete() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
        let schema = &CATALOG_VERSIONED_SCHEMAS[0];
        schema.create(&conn).unwrap();

        // Insert artists
        conn.execute(
            "INSERT INTO artists (id, name) VALUES ('test_artist_001', 'Artist 1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO artists (id, name) VALUES ('test_artist_002', 'Artist 2')",
            [],
        )
        .unwrap();

        // Insert related artists relationship
        conn.execute(
            "INSERT INTO related_artists (artist_id, related_artist_id) VALUES ('test_artist_001', 'test_artist_002')",
            [],
        )
        .unwrap();

        // Insert an image
        conn.execute(
            "INSERT INTO images (id, uri, size, width, height) VALUES ('test_image_001', 'images/test.jpg', 'DEFAULT', 300, 300)",
            [],
        )
        .unwrap();

        // Insert artist-image relationship
        conn.execute(
            "INSERT INTO artist_images (artist_id, image_id, image_type, position) VALUES ('test_artist_001', 'test_image_001', 'portrait', 0)",
            [],
        )
        .unwrap();

        // Delete artist R1
        conn.execute("DELETE FROM artists WHERE id = 'test_artist_001'", [])
            .unwrap();

        // Verify related_artists was cascade deleted
        let rel_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM related_artists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rel_count, 0);

        // Verify artist_images was cascade deleted
        let img_rel_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM artist_images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(img_rel_count, 0);

        // Image should still exist
        let img_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(img_count, 1);

        // Artist R2 should still exist
        let artist_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM artists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(artist_count, 1);
    }

    #[test]
    fn test_unique_constraints() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
        let schema = &CATALOG_VERSIONED_SCHEMAS[0];
        schema.create(&conn).unwrap();

        // Insert artist and album
        conn.execute(
            "INSERT INTO artists (id, name) VALUES ('test_artist_001', 'Artist')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO albums (id, name, album_type) VALUES ('test_album_001', 'Album', 'ALBUM')",
            [],
        )
        .unwrap();

        // Insert album-artist relationship
        conn.execute(
            "INSERT INTO album_artists (album_id, artist_id, position) VALUES ('test_album_001', 'test_artist_001', 0)",
            [],
        )
        .unwrap();

        // Try to insert duplicate - should fail
        let result = conn.execute(
            "INSERT INTO album_artists (album_id, artist_id, position) VALUES ('test_album_001', 'test_artist_001', 1)",
            [],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_v2_schema_creates_changelog_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
        let schema = &CATALOG_VERSIONED_SCHEMAS[2];
        schema.create(&conn).unwrap();
        schema.validate(&conn).unwrap();

        // Verify catalog_batches table exists
        let batch_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='catalog_batches'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(batch_exists, 1);

        // Verify catalog_change_log table exists
        let changelog_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='catalog_change_log'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(changelog_exists, 1);
    }

    #[test]
    fn test_v2_changelog_cascade_delete_on_batch() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
        let schema = &CATALOG_VERSIONED_SCHEMAS[2];
        schema.create(&conn).unwrap();

        // Insert a batch
        conn.execute(
            "INSERT INTO catalog_batches (id, name, is_open, created_at, last_activity_at)
             VALUES ('test_batch_001', 'Test Batch', 1, 1700000000, 1700000000)",
            [],
        )
        .unwrap();

        // Insert a change log entry
        conn.execute(
            "INSERT INTO catalog_change_log (batch_id, entity_type, entity_id, operation, field_changes, entity_snapshot, created_at)
             VALUES ('test_batch_001', 'artist', 'test_artist_001', 'create', '{}', '{\"id\":\"R1\",\"name\":\"Test\"}', 1700000000)",
            [],
        )
        .unwrap();

        // Verify change exists
        let change_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM catalog_change_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(change_count, 1);

        // Delete the batch
        conn.execute(
            "DELETE FROM catalog_batches WHERE id = 'test_batch_001'",
            [],
        )
        .unwrap();

        // Verify change was cascade deleted
        let change_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM catalog_change_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(change_count, 0);
    }
}
