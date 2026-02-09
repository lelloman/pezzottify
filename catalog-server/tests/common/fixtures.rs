//! Test fixture creation for catalog and database
//!
//! NOTE: This module uses the Spotify schema with rowid-based foreign keys.
//! The catalog store opens databases in read-only mode, so we create and
//! populate the database first, then open it with SqliteCatalogStore.

use super::constants::*;
use anyhow::Result;
use pezzottify_catalog_server::catalog_store::CATALOG_VERSIONED_SCHEMAS;
use pezzottify_catalog_server::user::auth::PezzottifyHasher;
use pezzottify_catalog_server::user::{
    SqliteUserStore, UserAuthCredentials, UserAuthCredentialsStore, UserRole, UserStore,
    UsernamePasswordCredentials,
};
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use tempfile::TempDir;

/// Test audio file embedded at compile time
const TEST_AUDIO_BYTES: &[u8] = include_bytes!("../fixtures/test-audio.mp3");

/// Test image file embedded at compile time
const TEST_IMAGE_BYTES: &[u8] = include_bytes!("../fixtures/test-image.jpg");

/// Creates a temporary test catalog with 2 artists, 2 albums, 5 tracks
/// Returns (temp_dir, catalog_db_path, media_path)
///
/// NOTE: This creates the schema first, populates data, then the SqliteCatalogStore
/// can open it in read-only mode.
pub fn create_test_catalog() -> Result<(TempDir, PathBuf, PathBuf)> {
    let dir = TempDir::new()?;

    // Create media directory structure for audio files and images
    let media_path = dir.path().join("media");
    fs::create_dir_all(media_path.join("audio"))?;
    fs::create_dir_all(media_path.join("images"))?;

    // Copy images (using album IDs as filenames - the image endpoint now takes item IDs)
    fs::write(
        media_path.join(format!("images/{}.jpg", ALBUM_1_ID)),
        TEST_IMAGE_BYTES,
    )?;
    fs::write(
        media_path.join(format!("images/{}.jpg", ALBUM_2_ID)),
        TEST_IMAGE_BYTES,
    )?;

    // Write audio files (using track IDs)
    fs::write(
        media_path.join(format!("audio/{}.ogg", TRACK_1_ID)),
        TEST_AUDIO_BYTES,
    )?;
    fs::write(
        media_path.join(format!("audio/{}.ogg", TRACK_2_ID)),
        TEST_AUDIO_BYTES,
    )?;
    fs::write(
        media_path.join(format!("audio/{}.ogg", TRACK_3_ID)),
        TEST_AUDIO_BYTES,
    )?;
    fs::write(
        media_path.join(format!("audio/{}.ogg", TRACK_4_ID)),
        TEST_AUDIO_BYTES,
    )?;
    fs::write(
        media_path.join(format!("audio/{}.ogg", TRACK_5_ID)),
        TEST_AUDIO_BYTES,
    )?;

    // Create SQLite catalog database with schema
    let catalog_db_path = dir.path().join("catalog.db");
    let conn = Connection::open(&catalog_db_path)?;

    // Create schema (use latest version)
    let latest_schema = &CATALOG_VERSIONED_SCHEMAS[CATALOG_VERSIONED_SCHEMAS.len() - 1];
    latest_schema.create(&conn)?;

    // Insert artists
    conn.execute(
        "INSERT INTO artists (id, name, followers_total, popularity) VALUES (?1, ?2, 0, 50)",
        [ARTIST_1_ID, ARTIST_1_NAME],
    )?;
    conn.execute(
        "INSERT INTO artists (id, name, followers_total, popularity) VALUES (?1, ?2, 0, 50)",
        [ARTIST_2_ID, ARTIST_2_NAME],
    )?;

    // Get artist rowids
    let artist1_rowid: i64 = conn.query_row(
        "SELECT rowid FROM artists WHERE id = ?1",
        [ARTIST_1_ID],
        |r| r.get(0),
    )?;
    let artist2_rowid: i64 = conn.query_row(
        "SELECT rowid FROM artists WHERE id = ?1",
        [ARTIST_2_ID],
        |r| r.get(0),
    )?;

    // Insert albums (with all required fields)
    conn.execute(
        "INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision, album_availability)
         VALUES (?1, ?2, 'album', '', 50, '2023', 'year', 'complete')",
        [ALBUM_1_ID, ALBUM_1_TITLE],
    )?;
    conn.execute(
        "INSERT INTO albums (id, name, album_type, label, popularity, release_date, release_date_precision, album_availability)
         VALUES (?1, ?2, 'album', '', 50, '2023', 'year', 'complete')",
        [ALBUM_2_ID, ALBUM_2_TITLE],
    )?;

    // Get album rowids
    let album1_rowid: i64 = conn.query_row(
        "SELECT rowid FROM albums WHERE id = ?1",
        [ALBUM_1_ID],
        |r| r.get(0),
    )?;
    let album2_rowid: i64 = conn.query_row(
        "SELECT rowid FROM albums WHERE id = ?1",
        [ALBUM_2_ID],
        |r| r.get(0),
    )?;

    // Link artists to albums (using rowids)
    conn.execute(
        "INSERT INTO artist_albums (artist_rowid, album_rowid, is_appears_on, is_implicit_appears_on, index_in_album)
         VALUES (?1, ?2, 0, 0, 0)",
        [artist1_rowid, album1_rowid],
    )?;
    conn.execute(
        "INSERT INTO artist_albums (artist_rowid, album_rowid, is_appears_on, is_implicit_appears_on, index_in_album)
         VALUES (?1, ?2, 0, 0, 0)",
        [artist2_rowid, album2_rowid],
    )?;

    // Insert tracks for album 1
    let tracks_album1 = [
        (TRACK_1_ID, TRACK_1_TITLE, 240000i64), // duration_ms
        (TRACK_2_ID, TRACK_2_TITLE, 180000),
        (TRACK_3_ID, TRACK_3_TITLE, 210000),
    ];
    for (i, (id, name, duration_ms)) in tracks_album1.iter().enumerate() {
        conn.execute(
            "INSERT INTO tracks (id, name, album_rowid, disc_number, track_number, duration_ms, explicit, popularity, audio_uri)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, 0, 50, ?6)",
            rusqlite::params![id, name, album1_rowid, (i + 1) as i32, duration_ms, format!("audio/{}.ogg", id)],
        )?;

        // Get track rowid
        let track_rowid: i64 =
            conn.query_row("SELECT rowid FROM tracks WHERE id = ?1", [*id], |r| {
                r.get(0)
            })?;

        // Link track to artist (using rowids)
        conn.execute(
            "INSERT INTO track_artists (track_rowid, artist_rowid, role) VALUES (?1, ?2, 0)",
            [track_rowid, artist1_rowid],
        )?;
    }

    // Insert tracks for album 2
    let tracks_album2 = [
        (TRACK_4_ID, TRACK_4_TITLE, 200000i64),
        (TRACK_5_ID, TRACK_5_TITLE, 160000),
    ];
    for (i, (id, name, duration_ms)) in tracks_album2.iter().enumerate() {
        conn.execute(
            "INSERT INTO tracks (id, name, album_rowid, disc_number, track_number, duration_ms, explicit, popularity, audio_uri)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, 0, 50, ?6)",
            rusqlite::params![id, name, album2_rowid, (i + 1) as i32, duration_ms, format!("audio/{}.ogg", id)],
        )?;

        // Get track rowid
        let track_rowid: i64 =
            conn.query_row("SELECT rowid FROM tracks WHERE id = ?1", [*id], |r| {
                r.get(0)
            })?;

        // Link track to artist (using rowids)
        conn.execute(
            "INSERT INTO track_artists (track_rowid, artist_rowid, role) VALUES (?1, ?2, 0)",
            [track_rowid, artist2_rowid],
        )?;
    }

    // Insert album images (using rowids)
    conn.execute(
        "INSERT INTO album_images (album_rowid, url, width, height) VALUES (?1, ?2, 300, 300)",
        rusqlite::params![album1_rowid, "https://example.com/image-1.jpg"],
    )?;
    conn.execute(
        "INSERT INTO album_images (album_rowid, url, width, height) VALUES (?1, ?2, 300, 300)",
        rusqlite::params![album2_rowid, "https://example.com/image-2.jpg"],
    )?;

    // Close the connection before opening in read-only mode
    drop(conn);

    Ok((dir, catalog_db_path, media_path))
}

/// Creates a temporary test database with test users
pub fn create_test_db_with_users() -> Result<(TempDir, String)> {
    // Create temp directory and DB file path
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    {
        let store = SqliteUserStore::new(&db_path)?;

        // Create regular test user
        let user_id =
            create_user_with_password_and_role(&store, TEST_USER, TEST_PASS, UserRole::Regular)?;
        eprintln!("Created test user {} with id {}", TEST_USER, user_id);

        // Create admin test user
        let admin_id =
            create_user_with_password_and_role(&store, ADMIN_USER, ADMIN_PASS, UserRole::Admin)?;
        eprintln!("Created admin user {} with id {}", ADMIN_USER, admin_id);
    }

    let path_str = db_path.to_string_lossy().into_owned();
    Ok((temp_dir, path_str))
}

/// Creates a user with the given credentials and role
pub fn create_user_with_password_and_role(
    store: &SqliteUserStore,
    username: &str,
    password: &str,
    role: UserRole,
) -> Result<usize> {
    // Create user
    let user_id = store.create_user(username)?;

    // Add role
    store.add_user_role(user_id, role)?;

    // Create password credentials using the hasher
    let hasher = PezzottifyHasher::Argon2;
    let salt = hasher.generate_b64_salt();
    let hash = hasher.hash(password.as_bytes(), &salt)?;

    let password_credentials = UsernamePasswordCredentials {
        user_id,
        salt,
        hash,
        hasher,
        created: SystemTime::now(),
        last_tried: None,
        last_used: None,
    };

    let credentials = UserAuthCredentials {
        user_id,
        username_password: Some(password_credentials),
        keys: vec![],
    };

    // Store credentials
    store.update_user_auth_credentials(credentials)?;

    Ok(user_id)
}

/// Creates a combined test setup with both catalog and users.
/// Returns (temp_dir, catalog_db_path, user_db_path, media_path).
#[allow(dead_code)]
pub fn create_combined_test_setup() -> Result<(TempDir, PathBuf, PathBuf, PathBuf)> {
    let (temp_dir, catalog_db_path, media_path) = create_test_catalog()?;

    // Create user DB in same temp directory
    let user_db_path = temp_dir.path().join("users.db");
    {
        let store = SqliteUserStore::new(&user_db_path)?;

        // Create test users
        create_user_with_password_and_role(&store, TEST_USER, TEST_PASS, UserRole::Regular)?;
        create_user_with_password_and_role(&store, ADMIN_USER, ADMIN_PASS, UserRole::Admin)?;
    }

    Ok((temp_dir, catalog_db_path, user_db_path, media_path))
}
