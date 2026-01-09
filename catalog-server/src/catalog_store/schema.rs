//! SQLite schema definitions for the Spotify catalog database.
//!
//! This module defines the database schema matching the Spotify metadata dump.
//! Primary keys are integer rowids with unique text Spotify IDs for lookups.
//! Images are stored as URLs to Spotify CDN (lazy download on first access).

use crate::sqlite_column;
use crate::sqlite_persistence::{Column, SqlType, Table, VersionedSchema};

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
    ],
    indices: &[("idx_artists_id", "id")],
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
    ],
    indices: &[
        ("idx_albums_id", "id"),
        ("idx_albums_upc", "external_id_upc"),
        ("idx_albums_availability", "album_availability"),
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
    ],
    indices: &[
        ("idx_tracks_id", "id"),
        ("idx_tracks_album", "album_rowid"),
        ("idx_tracks_isrc", "external_id_isrc"),
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
        sqlite_column!("artist_rowid", &SqlType::Integer, non_null = true),
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
    unique_constraints: &[],
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
pub const CATALOG_VERSIONED_SCHEMAS: &[VersionedSchema] = &[VersionedSchema {
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
}];

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
