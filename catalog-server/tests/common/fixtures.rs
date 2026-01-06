//! Test fixture creation for catalog and database
//!
//! NOTE: This module needs to be rewritten for the Spotify schema.
//! The Spotify catalog is read-only, so we can't use the old insert methods.
//! Tests need to use direct SQL inserts or pre-populated test databases.

use super::constants::*;
use anyhow::Result;
use pezzottify_catalog_server::catalog_store::SqliteCatalogStore;
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
/// NOTE: This uses direct SQL inserts because the Spotify schema is read-only.
pub fn create_test_catalog() -> Result<(TempDir, PathBuf, PathBuf)> {
    let dir = TempDir::new()?;

    // Create media directory structure for audio files and images
    let media_path = dir.path().join("media");
    fs::create_dir_all(media_path.join("albums/A1"))?;
    fs::create_dir_all(media_path.join("albums/A2"))?;
    fs::create_dir_all(media_path.join("images"))?;

    // Copy images
    fs::write(media_path.join("images/image-1"), TEST_IMAGE_BYTES)?;
    fs::write(media_path.join("images/image-2"), TEST_IMAGE_BYTES)?;
    fs::write(media_path.join("images/image-3"), TEST_IMAGE_BYTES)?;

    // Write audio files
    fs::write(media_path.join("albums/A1/T1.mp3"), TEST_AUDIO_BYTES)?;
    fs::write(media_path.join("albums/A1/T2.mp3"), TEST_AUDIO_BYTES)?;
    fs::write(media_path.join("albums/A1/T3.mp3"), TEST_AUDIO_BYTES)?;
    fs::write(media_path.join("albums/A2/T4.mp3"), TEST_AUDIO_BYTES)?;
    fs::write(media_path.join("albums/A2/T5.mp3"), TEST_AUDIO_BYTES)?;

    // Create SQLite catalog database
    let catalog_db_path = dir.path().join("catalog.db");

    // Initialize the store (creates schema)
    let _store = SqliteCatalogStore::new(&catalog_db_path, &media_path)?;

    // Use direct SQL to insert test data since the catalog is read-only through the API
    let conn = Connection::open(&catalog_db_path)?;

    // Insert artists
    conn.execute(
        "INSERT INTO artists (id, name, followers_total, popularity) VALUES (?1, ?2, 0, 50)",
        [ARTIST_1_ID, ARTIST_1_NAME],
    )?;
    conn.execute(
        "INSERT INTO artists (id, name, followers_total, popularity) VALUES (?1, ?2, 0, 50)",
        [ARTIST_2_ID, ARTIST_2_NAME],
    )?;

    // Insert albums
    conn.execute(
        "INSERT INTO albums (id, name, album_type, popularity) VALUES (?1, ?2, 0, 50)",
        [ALBUM_1_ID, ALBUM_1_TITLE],
    )?;
    conn.execute(
        "INSERT INTO albums (id, name, album_type, popularity) VALUES (?1, ?2, 0, 50)",
        [ALBUM_2_ID, ALBUM_2_TITLE],
    )?;

    // Link artists to albums
    conn.execute(
        "INSERT INTO artist_albums (artist_id, album_id) VALUES (?1, ?2)",
        [ARTIST_1_ID, ALBUM_1_ID],
    )?;
    conn.execute(
        "INSERT INTO artist_albums (artist_id, album_id) VALUES (?1, ?2)",
        [ARTIST_2_ID, ALBUM_2_ID],
    )?;

    // Insert tracks for album 1
    let tracks_album1 = [
        (TRACK_1_ID, TRACK_1_TITLE, 240000i64), // duration_ms
        (TRACK_2_ID, TRACK_2_TITLE, 180000),
        (TRACK_3_ID, TRACK_3_TITLE, 210000),
    ];
    for (i, (id, name, duration_ms)) in tracks_album1.iter().enumerate() {
        conn.execute(
            "INSERT INTO tracks (id, name, album_id, disc_number, track_number, duration_ms, explicit, popularity)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, 0, 50)",
            rusqlite::params![id, name, ALBUM_1_ID, (i + 1) as i32, duration_ms],
        )?;
        // Link track to artist
        conn.execute(
            "INSERT INTO track_artists (track_id, artist_id, artist_role) VALUES (?1, ?2, 0)",
            [*id, ARTIST_1_ID],
        )?;
    }

    // Insert tracks for album 2
    let tracks_album2 = [
        (TRACK_4_ID, TRACK_4_TITLE, 200000i64),
        (TRACK_5_ID, TRACK_5_TITLE, 160000),
    ];
    for (i, (id, name, duration_ms)) in tracks_album2.iter().enumerate() {
        conn.execute(
            "INSERT INTO tracks (id, name, album_id, disc_number, track_number, duration_ms, explicit, popularity)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, 0, 50)",
            rusqlite::params![id, name, ALBUM_2_ID, (i + 1) as i32, duration_ms],
        )?;
        // Link track to artist
        conn.execute(
            "INSERT INTO track_artists (track_id, artist_id, artist_role) VALUES (?1, ?2, 0)",
            [*id, ARTIST_2_ID],
        )?;
    }

    // Insert album images
    conn.execute(
        "INSERT INTO album_images (album_id, url, width, height) VALUES (?1, ?2, 300, 300)",
        [ALBUM_1_ID, "https://example.com/image-1.jpg"],
    )?;
    conn.execute(
        "INSERT INTO album_images (album_id, url, width, height) VALUES (?1, ?2, 300, 300)",
        [ALBUM_2_ID, "https://example.com/image-2.jpg"],
    )?;

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
