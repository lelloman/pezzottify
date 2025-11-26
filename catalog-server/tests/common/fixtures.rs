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
use tempfile::TempDir;

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

    // Audio files will be created per-album in write_album_json

    // Copy images (without extension - ID is the full filename)
    for i in 1..=3 {
        fs::write(
            dir.path().join(format!("images/image-{}", i)),
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
            (TRACK_1_ID, TRACK_1_TITLE, "track_1.mp3", 120000),
            (TRACK_2_ID, TRACK_2_TITLE, "track_2.mp3", 180000),
            (TRACK_3_ID, TRACK_3_TITLE, "track_3.mp3", 150000),
        ],
    )?;

    write_album_json(
        &dir,
        ALBUM_2_ID,
        ALBUM_2_TITLE,
        ARTIST_2_ID,
        IMAGE_2_ID,
        &[
            (TRACK_4_ID, TRACK_4_TITLE, "track_4.mp3", 200000),
            (TRACK_5_ID, TRACK_5_TITLE, "track_5.mp3", 160000),
        ],
    )?;

    Ok(dir)
}

/// Writes an artist JSON file to the catalog
/// ID should be the catalog ID with prefix (e.g., "R1"), which will be stripped for the filename and JSON
fn write_artist_json(
    dir: &TempDir,
    id: &str,
    name: &str,
    image_id: &str,
) -> Result<()> {
    // Strip the "R" prefix from the ID for the JSON and filename
    let json_id = id.strip_prefix("R").unwrap_or(id);

    let artist = json!({
        "id": json_id,
        "name": name,
        "genre": [],
        "portraits": [],
        "activity_periods": [],
        "related": [],
        "portrait_group": []
    });

    fs::write(
        dir.path().join(format!("artists/artist_{}.json", json_id)),
        serde_json::to_string_pretty(&artist)?,
    )?;

    Ok(())
}

/// Writes an album JSON file to the catalog
/// IDs should be catalog IDs with prefixes (e.g., "A1", "R1", "T1"), which will be stripped for JSON
fn write_album_json(
    dir: &TempDir,
    id: &str,
    title: &str,
    artist_id: &str,
    _image_id: &str,
    tracks: &[(&str, &str, &str, u64)],
) -> Result<()> {
    // Strip the "A" prefix from album ID and "R" from artist ID for the JSON
    let json_album_id = id.strip_prefix("A").unwrap_or(id);
    let json_artist_id = artist_id.strip_prefix("R").unwrap_or(artist_id);

    // Create album directory
    let album_dir = dir.path().join(format!("albums/album_{}", json_album_id));
    fs::create_dir_all(&album_dir)?;

    // Collect track IDs for the disc
    let track_ids: Vec<String> = tracks
        .iter()
        .map(|(id, _, _, _)| {
            // Strip the "T" prefix from track ID
            id.strip_prefix("T").unwrap_or(id).to_string()
        })
        .collect();

    // Write individual track JSON files and audio files
    for (track_id, track_title, file_name, duration_ms) in tracks {
        let json_track_id = track_id.strip_prefix("T").unwrap_or(track_id);

        // Write audio file
        fs::write(album_dir.join(file_name), TEST_AUDIO_BYTES)?;

        // Write track JSON file
        let track_json = json!({
            "id": json_track_id,
            "name": track_title,
            "album_id": json_album_id,
            "artists_ids": [json_artist_id],
            "number": 1,
            "disc_number": 1,
            "duration": duration_ms / 1000,  // Convert ms to seconds
            "is_explicit": false,
            "files": {
                "MP3_320": file_name
            },
            "alternatives": [],
            "tags": [],
            "earliest_live_timestamp": 0,
            "has_lyrics": false,
            "language_of_performance": [],
            "original_title": "",
            "version_title": "",
            "artists_with_role": []
        });

        fs::write(
            album_dir.join(format!("track_{}.json", json_track_id)),
            serde_json::to_string_pretty(&track_json)?,
        )?;
    }

    // Write album JSON with discs structure
    let album = json!({
        "id": json_album_id,
        "name": title,
        "album_type": "ALBUM",
        "artists_ids": [json_artist_id],
        "label": "",
        "date": 0,
        "genres": [],
        "covers": [],
        "discs": [{
            "number": 1,
            "name": "",
            "tracks": track_ids
        }],
        "related": [],
        "cover_group": [],
        "original_title": "",
        "version_title": "",
        "type_str": ""
    });

    fs::write(
        album_dir.join(format!("album_{}.json", json_album_id)),
        serde_json::to_string_pretty(&album)?,
    )?;

    Ok(())
}

/// Creates a temporary test database with test users
pub fn create_test_db_with_users() -> Result<(TempDir, String)> {
    // Create temp directory and DB file path
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    {
        let store = SqliteUserStore::new(&db_path)?;

        // Create regular test user
        let user_id = create_user_with_password_and_role(&store, TEST_USER, TEST_PASS, UserRole::Regular)?;
        eprintln!("Created test user {} with id {}", TEST_USER, user_id);

        // Create admin test user
        let admin_id = create_user_with_password_and_role(&store, ADMIN_USER, ADMIN_PASS, UserRole::Admin)?;
        eprintln!("Created admin user {} with id {}", ADMIN_USER, admin_id);

        // Store is dropped here, ensuring connection is closed
    }

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
