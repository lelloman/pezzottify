//! Test fixture creation for catalog and database
//!
//! This module creates temporary test catalogs and databases.
//! When catalog/database schemas change, update only this file.

use super::constants::*;
use anyhow::Result;
use pezzottify_catalog_server::catalog_store::{
    Album, AlbumType, Artist, ArtistRole, Format, ImageSize, SqliteCatalogStore, Track,
};
use pezzottify_catalog_server::user::auth::PezzottifyHasher;
use pezzottify_catalog_server::user::{
    SqliteUserStore, UserAuthCredentials, UserAuthCredentialsStore, UserRole, UserStore,
    UsernamePasswordCredentials,
};
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
    let store = SqliteCatalogStore::new(&catalog_db_path, &media_path)?;

    // Insert artists (use IDs from constants: R1, R2)
    let artist1 = Artist {
        id: ARTIST_1_ID.to_string(),
        name: ARTIST_1_NAME.to_string(),
        genres: vec![],
        activity_periods: vec![],
    };
    let artist2 = Artist {
        id: ARTIST_2_ID.to_string(),
        name: ARTIST_2_NAME.to_string(),
        genres: vec![],
        activity_periods: vec![],
    };
    store.insert_artist(&artist1)?;
    store.insert_artist(&artist2)?;

    // Insert albums (use IDs from constants: A1, A2)
    let album1 = Album {
        id: ALBUM_1_ID.to_string(),
        name: ALBUM_1_TITLE.to_string(),
        album_type: AlbumType::Album,
        label: None,
        release_date: None,
        genres: vec![],
        original_title: None,
        version_title: None,
    };
    let album2 = Album {
        id: ALBUM_2_ID.to_string(),
        name: ALBUM_2_TITLE.to_string(),
        album_type: AlbumType::Album,
        label: None,
        release_date: None,
        genres: vec![],
        original_title: None,
        version_title: None,
    };
    store.insert_album(&album1)?;
    store.insert_album(&album2)?;

    // Link albums to artists
    store.add_album_artist(ALBUM_1_ID, ARTIST_1_ID, 0)?;
    store.add_album_artist(ALBUM_2_ID, ARTIST_2_ID, 0)?;

    // Insert tracks for album 1 (use IDs from constants: T1, T2, T3)
    let tracks_album1 = [
        (TRACK_1_ID, TRACK_1_TITLE, "albums/A1/T1.mp3", 120),
        (TRACK_2_ID, TRACK_2_TITLE, "albums/A1/T2.mp3", 180),
        (TRACK_3_ID, TRACK_3_TITLE, "albums/A1/T3.mp3", 150),
    ];
    for (i, (id, name, audio_uri, duration)) in tracks_album1.iter().enumerate() {
        let track = Track {
            id: id.to_string(),
            name: name.to_string(),
            album_id: ALBUM_1_ID.to_string(),
            disc_number: 1,
            track_number: (i + 1) as i32,
            duration_secs: Some(*duration),
            is_explicit: false,
            audio_uri: audio_uri.to_string(),
            format: Format::Mp3_320,
            tags: vec![],
            has_lyrics: false,
            languages: vec![],
            original_title: None,
            version_title: None,
        };
        store.insert_track(&track)?;
        store.add_track_artist(id, ARTIST_1_ID, &ArtistRole::MainArtist, 0)?;
    }

    // Insert tracks for album 2 (use IDs from constants: T4, T5)
    let tracks_album2 = [
        (TRACK_4_ID, TRACK_4_TITLE, "albums/A2/T4.mp3", 200),
        (TRACK_5_ID, TRACK_5_TITLE, "albums/A2/T5.mp3", 160),
    ];
    for (i, (id, name, audio_uri, duration)) in tracks_album2.iter().enumerate() {
        let track = Track {
            id: id.to_string(),
            name: name.to_string(),
            album_id: ALBUM_2_ID.to_string(),
            disc_number: 1,
            track_number: (i + 1) as i32,
            duration_secs: Some(*duration),
            is_explicit: false,
            audio_uri: audio_uri.to_string(),
            format: Format::Mp3_320,
            tags: vec![],
            has_lyrics: false,
            languages: vec![],
            original_title: None,
            version_title: None,
        };
        store.insert_track(&track)?;
        store.add_track_artist(id, ARTIST_2_ID, &ArtistRole::MainArtist, 0)?;
    }

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
