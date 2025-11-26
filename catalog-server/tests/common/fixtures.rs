//! Test fixture creation for catalog and database
//!
//! This module creates temporary test catalogs and databases.
//! When catalog/database schemas change, update only this file.

use super::constants::*;
use anyhow::Result;
use pezzottify_catalog_server::user::{
    SqliteUserStore, UserAuthCredentials, UserAuthCredentialsStore, UserRole, UserStore,
    UsernamePasswordCredentials,
};
use pezzottify_catalog_server::user::auth::PezzottifyHasher;
use serde_json::json;
use std::fs;
use std::time::SystemTime;
use tempfile::{NamedTempFile, TempDir};

/// Test audio file embedded at compile time
const TEST_AUDIO_BYTES: &[u8] = include_bytes!("../fixtures/test-audio.mp3");

/// Test image file embedded at compile time
const TEST_IMAGE_BYTES: &[u8] = include_bytes!("../fixtures/test-image.jpg");

/// Creates a temporary test catalog with 2 artists, 2 albums, 5 tracks, 3 images
pub fn create_test_catalog() -> Result<TempDir> {
    let dir = TempDir::new()?;

    // Create directory structure
    fs::create_dir_all(dir.path().join("albums"))?;
    fs::create_dir_all(dir.path().join("artists"))?;
    fs::create_dir_all(dir.path().join("images"))?;

    // Copy audio files (all same content, different filenames)
    for i in 1..=5 {
        fs::write(
            dir.path().join(format!("albums/track-{}.mp3", i)),
            TEST_AUDIO_BYTES,
        )?;
    }

    // Copy images
    for i in 1..=3 {
        fs::write(
            dir.path().join(format!("images/image-{}.jpg", i)),
            TEST_IMAGE_BYTES,
        )?;
    }

    // Create artists
    write_artist_json(&dir, ARTIST_1_ID, ARTIST_1_NAME, IMAGE_1_ID)?;
    write_artist_json(&dir, ARTIST_2_ID, ARTIST_2_NAME, IMAGE_2_ID)?;

    // Create albums
    write_album_json(
        &dir,
        ALBUM_1_ID,
        ALBUM_1_TITLE,
        ARTIST_1_ID,
        IMAGE_1_ID,
        &[
            (TRACK_1_ID, TRACK_1_TITLE, "track-1.mp3", 120000),
            (TRACK_2_ID, TRACK_2_TITLE, "track-2.mp3", 180000),
            (TRACK_3_ID, TRACK_3_TITLE, "track-3.mp3", 150000),
        ],
    )?;

    write_album_json(
        &dir,
        ALBUM_2_ID,
        ALBUM_2_TITLE,
        ARTIST_2_ID,
        IMAGE_2_ID,
        &[
            (TRACK_4_ID, TRACK_4_TITLE, "track-4.mp3", 200000),
            (TRACK_5_ID, TRACK_5_TITLE, "track-5.mp3", 160000),
        ],
    )?;

    Ok(dir)
}

/// Writes an artist JSON file to the catalog
fn write_artist_json(
    dir: &TempDir,
    id: &str,
    name: &str,
    image_id: &str,
) -> Result<()> {
    let artist = json!({
        "id": id,
        "name": name,
        "image_id": image_id
    });

    fs::write(
        dir.path().join(format!("artists/{}.json", id)),
        serde_json::to_string_pretty(&artist)?,
    )?;

    Ok(())
}

/// Writes an album JSON file to the catalog
fn write_album_json(
    dir: &TempDir,
    id: &str,
    title: &str,
    artist_id: &str,
    image_id: &str,
    tracks: &[(&str, &str, &str, u64)],
) -> Result<()> {
    let tracks_json: Vec<_> = tracks
        .iter()
        .map(|(id, title, file, duration)| {
            json!({
                "id": id,
                "title": title,
                "file_name": file,
                "duration_ms": duration
            })
        })
        .collect();

    let album = json!({
        "id": id,
        "title": title,
        "artist_id": artist_id,
        "image_id": image_id,
        "tracks": tracks_json
    });

    fs::write(
        dir.path().join(format!("albums/{}.json", id)),
        serde_json::to_string_pretty(&album)?,
    )?;

    Ok(())
}

/// Creates a temporary test database with test users
pub fn create_test_db_with_users() -> Result<(TempDir, String)> {
    // Create temp directory and DB file path
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    let store = SqliteUserStore::new(&db_path)?;

    // Create regular test user
    let user_id = create_user_with_password_and_role(&store, TEST_USER, TEST_PASS, UserRole::Regular)?;
    eprintln!("Created test user {} with id {}", TEST_USER, user_id);

    // Create admin test user
    let admin_id = create_user_with_password_and_role(&store, ADMIN_USER, ADMIN_PASS, UserRole::Admin)?;
    eprintln!("Created admin user {} with id {}", ADMIN_USER, admin_id);

    let db_path_str = db_path.to_string_lossy().to_string();
    eprintln!("Created test database at: {}", db_path_str);

    Ok((temp_dir, db_path_str))
}

/// Helper to create a user with password and role
fn create_user_with_password_and_role(
    store: &SqliteUserStore,
    handle: &str,
    password: &str,
    role: UserRole,
) -> Result<usize> {
    // Create the user
    let user_id = store.create_user(handle)?;

    // Hash the password
    let hasher = PezzottifyHasher::Argon2;
    let salt = hasher.generate_b64_salt();
    let hash = hasher.hash(password.as_bytes(), &salt)?;

    // Create credentials
    let credentials = UserAuthCredentials {
        user_id,
        username_password: Some(UsernamePasswordCredentials {
            user_id,
            salt,
            hash,
            hasher,
            created: SystemTime::now(),
            last_tried: None,
            last_used: None,
        }),
        keys: Vec::new(), // No crypto keys for test users
    };

    // Update credentials in store
    store.update_user_auth_credentials(credentials)?;

    // Add role
    store.add_user_role(user_id, role)?;

    Ok(user_id)
}
