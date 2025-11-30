//! SQLite schema definitions for the catalog database.
//!
//! This module defines the database schema for storing music catalog metadata.
//! Audio and image files remain on the filesystem, referenced by relative URIs.

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
        sqlite_column!("disc_number", &SqlType::Integer, non_null = true, default_value = Some("1")),
        sqlite_column!("track_number", &SqlType::Integer, non_null = true),
        sqlite_column!("duration_secs", &SqlType::Integer),
        sqlite_column!("is_explicit", &SqlType::Integer, non_null = true, default_value = Some("0")),
        sqlite_column!("audio_uri", &SqlType::Text, non_null = true), // Relative path
        sqlite_column!("format", &SqlType::Text, non_null = true),    // 'MP3_320', 'FLAC', etc.
        sqlite_column!("tags", &SqlType::Text),                       // JSON array
        sqlite_column!("has_lyrics", &SqlType::Integer, non_null = true, default_value = Some("0")),
        sqlite_column!("languages", &SqlType::Text), // JSON array
        sqlite_column!("original_title", &SqlType::Text),
        sqlite_column!("version_title", &SqlType::Text),
    ],
    indices: &[
        ("idx_tracks_album", "album_id"),
        ("idx_tracks_disc_track", "album_id, disc_number, track_number"),
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
// Versioned Schema Definition
// =============================================================================

/// All versioned schemas for the catalog database.
///
/// The catalog database uses a separate version namespace from the user database.
/// Initial version (0) contains all core tables and relationship tables.
/// Version 1 adds display_image_id to artists and albums.
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
            "INSERT INTO artists (id, name) VALUES ('R1', 'Test Artist')",
            [],
        )
        .unwrap();

        // Insert an album
        conn.execute(
            "INSERT INTO albums (id, name, album_type) VALUES ('A1', 'Test Album', 'ALBUM')",
            [],
        )
        .unwrap();

        // Insert album-artist relationship
        conn.execute(
            "INSERT INTO album_artists (album_id, artist_id, position) VALUES ('A1', 'R1', 0)",
            [],
        )
        .unwrap();

        // Insert a track
        conn.execute(
            "INSERT INTO tracks (id, name, album_id, track_number, audio_uri, format) VALUES ('T1', 'Test Track', 'A1', 1, 'albums/A1/track_T1.mp3', 'MP3_320')",
            [],
        )
        .unwrap();

        // Verify track exists
        let track_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tracks WHERE album_id = 'A1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(track_count, 1);

        // Delete the album
        conn.execute("DELETE FROM albums WHERE id = 'A1'", [])
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
            "INSERT INTO artists (id, name) VALUES ('R1', 'Artist 1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO artists (id, name) VALUES ('R2', 'Artist 2')",
            [],
        )
        .unwrap();

        // Insert related artists relationship
        conn.execute(
            "INSERT INTO related_artists (artist_id, related_artist_id) VALUES ('R1', 'R2')",
            [],
        )
        .unwrap();

        // Insert an image
        conn.execute(
            "INSERT INTO images (id, uri, size, width, height) VALUES ('I1', 'images/test.jpg', 'DEFAULT', 300, 300)",
            [],
        )
        .unwrap();

        // Insert artist-image relationship
        conn.execute(
            "INSERT INTO artist_images (artist_id, image_id, image_type, position) VALUES ('R1', 'I1', 'portrait', 0)",
            [],
        )
        .unwrap();

        // Delete artist R1
        conn.execute("DELETE FROM artists WHERE id = 'R1'", [])
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
            "INSERT INTO artists (id, name) VALUES ('R1', 'Artist')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO albums (id, name, album_type) VALUES ('A1', 'Album', 'ALBUM')",
            [],
        )
        .unwrap();

        // Insert album-artist relationship
        conn.execute(
            "INSERT INTO album_artists (album_id, artist_id, position) VALUES ('A1', 'R1', 0)",
            [],
        )
        .unwrap();

        // Try to insert duplicate - should fail
        let result = conn.execute(
            "INSERT INTO album_artists (album_id, artist_id, position) VALUES ('A1', 'R1', 1)",
            [],
        );
        assert!(result.is_err());
    }
}
