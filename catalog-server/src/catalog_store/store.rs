//! SQLite-backed catalog store implementation.
//!
//! This module provides the `SqliteCatalogStore` which stores and retrieves
//! catalog metadata from a SQLite database, with media files remaining on
//! the filesystem.

use super::models::*;
use super::schema::CATALOG_VERSIONED_SCHEMAS;
use crate::sqlite_persistence::BASE_DB_VERSION;
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::info;

/// SQLite-backed catalog store.
///
/// Stores catalog metadata (artists, albums, tracks, images) in SQLite,
/// with media files remaining on the filesystem referenced by relative URIs.
#[derive(Clone)]
pub struct SqliteCatalogStore {
    conn: Arc<Mutex<Connection>>,
    media_base_path: PathBuf,
}

impl SqliteCatalogStore {
    /// Create a new SqliteCatalogStore.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    /// * `media_base_path` - Base path for resolving relative media URIs
    pub fn new<P: AsRef<Path>, M: AsRef<Path>>(db_path: P, media_base_path: M) -> Result<Self> {
        let mut conn = if db_path.as_ref().exists() {
            Connection::open_with_flags(
                &db_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                    | rusqlite::OpenFlags::SQLITE_OPEN_URI
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?
        } else {
            let conn = Connection::open(&db_path)?;
            conn.execute("PRAGMA foreign_keys = ON;", [])?;
            CATALOG_VERSIONED_SCHEMAS.last().unwrap().create(&conn)?;
            conn
        };

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON;", [])?;

        // Read the database version
        let db_version = conn
            .query_row("PRAGMA user_version;", [], |row| row.get::<usize, i64>(0))
            .context("Failed to read database version")?
            - BASE_DB_VERSION as i64;

        if db_version < 0 {
            bail!(
                "Database version {} is too old, does not contain base db version {}",
                db_version,
                BASE_DB_VERSION
            );
        }
        let version = db_version as usize;

        if version >= CATALOG_VERSIONED_SCHEMAS.len() {
            bail!("Database version {} is too new", version);
        } else {
            CATALOG_VERSIONED_SCHEMAS
                .get(version)
                .context("Failed to get schema")?
                .validate(&conn)?;
        }

        Self::migrate_if_needed(&mut conn, version)?;

        Ok(SqliteCatalogStore {
            conn: Arc::new(Mutex::new(conn)),
            media_base_path: media_base_path.as_ref().to_path_buf(),
        })
    }

    fn migrate_if_needed(conn: &mut Connection, version: usize) -> Result<()> {
        let tx = conn.transaction()?;
        let mut latest_from = version;
        for schema in CATALOG_VERSIONED_SCHEMAS.iter().skip(version + 1) {
            if let Some(migration_fn) = schema.migration {
                info!(
                    "Migrating catalog db from version {} to {}",
                    latest_from, schema.version
                );
                migration_fn(&tx)?;
                latest_from = schema.version;
            }
        }
        tx.execute(
            &format!("PRAGMA user_version = {}", BASE_DB_VERSION + latest_from),
            [],
        )?;

        tx.commit()?;
        Ok(())
    }

    // =========================================================================
    // URI Resolution
    // =========================================================================

    /// Resolve a relative audio URI to an absolute filesystem path.
    pub fn resolve_audio_uri(&self, track: &Track) -> PathBuf {
        self.media_base_path.join(&track.audio_uri)
    }

    /// Resolve a relative image URI to an absolute filesystem path.
    pub fn resolve_image_uri(&self, image: &Image) -> PathBuf {
        self.media_base_path.join(&image.uri)
    }

    // =========================================================================
    // Read Operations - Core Entities
    // =========================================================================

    /// Get an artist by ID.
    pub fn get_artist(&self, id: &str) -> Result<Option<Artist>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, genres, activity_periods FROM artists WHERE id = ?1",
        )?;

        match stmt.query_row(params![id], |row| {
            let genres_json: Option<String> = row.get(2)?;
            let activity_periods_json: Option<String> = row.get(3)?;

            Ok(Artist {
                id: row.get(0)?,
                name: row.get(1)?,
                genres: genres_json
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
                activity_periods: activity_periods_json
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
            })
        }) {
            Ok(artist) => Ok(Some(artist)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get an album by ID.
    pub fn get_album(&self, id: &str) -> Result<Option<Album>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, album_type, label, release_date, genres, original_title, version_title
             FROM albums WHERE id = ?1",
        )?;

        match stmt.query_row(params![id], |row| {
            let album_type_str: String = row.get(2)?;
            let genres_json: Option<String> = row.get(5)?;

            Ok(Album {
                id: row.get(0)?,
                name: row.get(1)?,
                album_type: AlbumType::from_db_str(&album_type_str),
                label: row.get(3)?,
                release_date: row.get(4)?,
                genres: genres_json
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
                original_title: row.get(6)?,
                version_title: row.get(7)?,
            })
        }) {
            Ok(album) => Ok(Some(album)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get a track by ID.
    pub fn get_track(&self, id: &str) -> Result<Option<Track>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, album_id, disc_number, track_number, duration_secs, is_explicit,
                    audio_uri, format, tags, has_lyrics, languages, original_title, version_title
             FROM tracks WHERE id = ?1",
        )?;

        match stmt.query_row(params![id], |row| {
            let format_str: String = row.get(8)?;
            let tags_json: Option<String> = row.get(9)?;
            let languages_json: Option<String> = row.get(11)?;
            let is_explicit: i32 = row.get(6)?;
            let has_lyrics: i32 = row.get(10)?;

            Ok(Track {
                id: row.get(0)?,
                name: row.get(1)?,
                album_id: row.get(2)?,
                disc_number: row.get(3)?,
                track_number: row.get(4)?,
                duration_secs: row.get(5)?,
                is_explicit: is_explicit != 0,
                audio_uri: row.get(7)?,
                format: Format::from_db_str(&format_str),
                tags: tags_json
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
                has_lyrics: has_lyrics != 0,
                languages: languages_json
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
                original_title: row.get(12)?,
                version_title: row.get(13)?,
            })
        }) {
            Ok(track) => Ok(Some(track)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get an image by ID.
    pub fn get_image(&self, id: &str) -> Result<Option<Image>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, uri, size, width, height FROM images WHERE id = ?1",
        )?;

        match stmt.query_row(params![id], |row| {
            let size_str: String = row.get(2)?;
            let width: i32 = row.get(3)?;
            let height: i32 = row.get(4)?;

            Ok(Image {
                id: row.get(0)?,
                uri: row.get(1)?,
                size: ImageSize::from_db_str(&size_str),
                width: width as u16,
                height: height as u16,
            })
        }) {
            Ok(image) => Ok(Some(image)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    // =========================================================================
    // Read Operations - Resolved/Composite Types
    // =========================================================================

    /// Get a fully resolved artist with images and related artists.
    pub fn get_resolved_artist(&self, id: &str) -> Result<Option<ResolvedArtist>> {
        let artist = match self.get_artist(id)? {
            Some(a) => a,
            None => return Ok(None),
        };

        let images = self.get_artist_images(id)?;
        let related_artists = self.get_related_artists(id)?;

        Ok(Some(ResolvedArtist {
            artist,
            images,
            related_artists,
        }))
    }

    /// Get artist's discography.
    pub fn get_artist_discography(&self, id: &str) -> Result<Option<ArtistDiscography>> {
        // First check if artist exists
        if self.get_artist(id)?.is_none() {
            return Ok(None);
        }

        let conn = self.conn.lock().unwrap();

        // Albums where artist is primary (album_artists)
        let mut stmt = conn.prepare(
            "SELECT DISTINCT a.id, a.name, a.album_type, a.label, a.release_date,
                    a.genres, a.original_title, a.version_title
             FROM albums a
             INNER JOIN album_artists aa ON a.id = aa.album_id
             WHERE aa.artist_id = ?1
             ORDER BY a.release_date DESC",
        )?;

        let albums: Vec<Album> = stmt
            .query_map(params![id], |row| {
                let album_type_str: String = row.get(2)?;
                let genres_json: Option<String> = row.get(5)?;

                Ok(Album {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    album_type: AlbumType::from_db_str(&album_type_str),
                    label: row.get(3)?,
                    release_date: row.get(4)?,
                    genres: genres_json
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                    original_title: row.get(6)?,
                    version_title: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Albums where artist is featured on tracks (but not primary album artist)
        let mut stmt = conn.prepare(
            "SELECT DISTINCT a.id, a.name, a.album_type, a.label, a.release_date,
                    a.genres, a.original_title, a.version_title
             FROM albums a
             INNER JOIN tracks t ON a.id = t.album_id
             INNER JOIN track_artists ta ON t.id = ta.track_id
             WHERE ta.artist_id = ?1
               AND a.id NOT IN (
                   SELECT album_id FROM album_artists WHERE artist_id = ?1
               )
             ORDER BY a.release_date DESC",
        )?;

        let features: Vec<Album> = stmt
            .query_map(params![id], |row| {
                let album_type_str: String = row.get(2)?;
                let genres_json: Option<String> = row.get(5)?;

                Ok(Album {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    album_type: AlbumType::from_db_str(&album_type_str),
                    label: row.get(3)?,
                    release_date: row.get(4)?,
                    genres: genres_json
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                    original_title: row.get(6)?,
                    version_title: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(ArtistDiscography { albums, features }))
    }

    /// Get a fully resolved album with tracks, artists, and images.
    pub fn get_resolved_album(&self, id: &str) -> Result<Option<ResolvedAlbum>> {
        let album = match self.get_album(id)? {
            Some(a) => a,
            None => return Ok(None),
        };

        let artists = self.get_album_artists(id)?;
        let discs = self.get_album_discs(id)?;
        let images = self.get_album_images(id)?;

        Ok(Some(ResolvedAlbum {
            album,
            artists,
            discs,
            images,
        }))
    }

    /// Get a fully resolved track with artists and album info.
    pub fn get_resolved_track(&self, id: &str) -> Result<Option<ResolvedTrack>> {
        let track = match self.get_track(id)? {
            Some(t) => t,
            None => return Ok(None),
        };

        let album = self
            .get_album(&track.album_id)?
            .context("Track references non-existent album")?;

        let artists = self.get_track_artists(id)?;

        Ok(Some(ResolvedTrack {
            track,
            album,
            artists,
        }))
    }

    // =========================================================================
    // Read Operations - Relationships
    // =========================================================================

    /// Get images for an artist.
    fn get_artist_images(&self, artist_id: &str) -> Result<Vec<Image>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.uri, i.size, i.width, i.height
             FROM images i
             INNER JOIN artist_images ai ON i.id = ai.image_id
             WHERE ai.artist_id = ?1
             ORDER BY ai.position",
        )?;

        let images: Vec<Image> = stmt
            .query_map(params![artist_id], |row| {
                let size_str: String = row.get(2)?;
                let width: i32 = row.get(3)?;
                let height: i32 = row.get(4)?;

                Ok(Image {
                    id: row.get(0)?,
                    uri: row.get(1)?,
                    size: ImageSize::from_db_str(&size_str),
                    width: width as u16,
                    height: height as u16,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// Get related artists for an artist.
    fn get_related_artists(&self, artist_id: &str) -> Result<Vec<Artist>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT a.id, a.name, a.genres, a.activity_periods
             FROM artists a
             INNER JOIN related_artists ra ON a.id = ra.related_artist_id
             WHERE ra.artist_id = ?1",
        )?;

        let artists: Vec<Artist> = stmt
            .query_map(params![artist_id], |row| {
                let genres_json: Option<String> = row.get(2)?;
                let activity_periods_json: Option<String> = row.get(3)?;

                Ok(Artist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    genres: genres_json
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                    activity_periods: activity_periods_json
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(artists)
    }

    /// Get artists for an album.
    fn get_album_artists(&self, album_id: &str) -> Result<Vec<Artist>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT a.id, a.name, a.genres, a.activity_periods
             FROM artists a
             INNER JOIN album_artists aa ON a.id = aa.artist_id
             WHERE aa.album_id = ?1
             ORDER BY aa.position",
        )?;

        let artists: Vec<Artist> = stmt
            .query_map(params![album_id], |row| {
                let genres_json: Option<String> = row.get(2)?;
                let activity_periods_json: Option<String> = row.get(3)?;

                Ok(Artist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    genres: genres_json
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                    activity_periods: activity_periods_json
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(artists)
    }

    /// Get images for an album.
    fn get_album_images(&self, album_id: &str) -> Result<Vec<Image>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.uri, i.size, i.width, i.height
             FROM images i
             INNER JOIN album_images ai ON i.id = ai.image_id
             WHERE ai.album_id = ?1
             ORDER BY ai.position",
        )?;

        let images: Vec<Image> = stmt
            .query_map(params![album_id], |row| {
                let size_str: String = row.get(2)?;
                let width: i32 = row.get(3)?;
                let height: i32 = row.get(4)?;

                Ok(Image {
                    id: row.get(0)?,
                    uri: row.get(1)?,
                    size: ImageSize::from_db_str(&size_str),
                    width: width as u16,
                    height: height as u16,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(images)
    }

    /// Get tracks for an album grouped by disc.
    fn get_album_discs(&self, album_id: &str) -> Result<Vec<Disc>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, album_id, disc_number, track_number, duration_secs, is_explicit,
                    audio_uri, format, tags, has_lyrics, languages, original_title, version_title
             FROM tracks
             WHERE album_id = ?1
             ORDER BY disc_number, track_number",
        )?;

        let tracks: Vec<Track> = stmt
            .query_map(params![album_id], |row| {
                let format_str: String = row.get(8)?;
                let tags_json: Option<String> = row.get(9)?;
                let languages_json: Option<String> = row.get(11)?;
                let is_explicit: i32 = row.get(6)?;
                let has_lyrics: i32 = row.get(10)?;

                Ok(Track {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    album_id: row.get(2)?,
                    disc_number: row.get(3)?,
                    track_number: row.get(4)?,
                    duration_secs: row.get(5)?,
                    is_explicit: is_explicit != 0,
                    audio_uri: row.get(7)?,
                    format: Format::from_db_str(&format_str),
                    tags: tags_json
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                    has_lyrics: has_lyrics != 0,
                    languages: languages_json
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                    original_title: row.get(12)?,
                    version_title: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Group tracks by disc number
        let mut disc_map: HashMap<i32, Vec<Track>> = HashMap::new();
        for track in tracks {
            disc_map.entry(track.disc_number).or_default().push(track);
        }

        let mut discs: Vec<Disc> = disc_map
            .into_iter()
            .map(|(number, tracks)| Disc {
                number,
                name: None, // Could be extended to support disc names
                tracks,
            })
            .collect();

        discs.sort_by_key(|d| d.number);
        Ok(discs)
    }

    /// Get artists for a track with their roles.
    fn get_track_artists(&self, track_id: &str) -> Result<Vec<TrackArtist>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT a.id, a.name, a.genres, a.activity_periods, ta.role
             FROM artists a
             INNER JOIN track_artists ta ON a.id = ta.artist_id
             WHERE ta.track_id = ?1
             ORDER BY ta.position",
        )?;

        let track_artists: Vec<TrackArtist> = stmt
            .query_map(params![track_id], |row| {
                let genres_json: Option<String> = row.get(2)?;
                let activity_periods_json: Option<String> = row.get(3)?;
                let role_str: String = row.get(4)?;

                Ok(TrackArtist {
                    artist: Artist {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        genres: genres_json
                            .and_then(|s| serde_json::from_str(&s).ok())
                            .unwrap_or_default(),
                        activity_periods: activity_periods_json
                            .and_then(|s| serde_json::from_str(&s).ok())
                            .unwrap_or_default(),
                    },
                    role: ArtistRole::from_db_str(&role_str),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(track_artists)
    }

    // =========================================================================
    // Listing Operations
    // =========================================================================

    /// Get all artist IDs.
    pub fn list_artist_ids(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM artists")?;
        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// Get all album IDs.
    pub fn list_album_ids(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM albums")?;
        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// Get all track IDs.
    pub fn list_track_ids(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM tracks")?;
        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// Get counts of all entities.
    pub fn get_counts(&self) -> Result<(usize, usize, usize, usize)> {
        let conn = self.conn.lock().unwrap();
        let artists: i64 = conn.query_row("SELECT COUNT(*) FROM artists", [], |r| r.get(0))?;
        let albums: i64 = conn.query_row("SELECT COUNT(*) FROM albums", [], |r| r.get(0))?;
        let tracks: i64 = conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))?;
        let images: i64 = conn.query_row("SELECT COUNT(*) FROM images", [], |r| r.get(0))?;
        Ok((
            artists as usize,
            albums as usize,
            tracks as usize,
            images as usize,
        ))
    }

    // =========================================================================
    // Write Operations - Core Entities
    // =========================================================================

    /// Insert an artist.
    pub fn insert_artist(&self, artist: &Artist) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let genres_json = serde_json::to_string(&artist.genres)?;
        let activity_periods_json = serde_json::to_string(&artist.activity_periods)?;

        conn.execute(
            "INSERT INTO artists (id, name, genres, activity_periods) VALUES (?1, ?2, ?3, ?4)",
            params![artist.id, artist.name, genres_json, activity_periods_json],
        )?;
        Ok(())
    }

    /// Insert an album.
    pub fn insert_album(&self, album: &Album) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let genres_json = serde_json::to_string(&album.genres)?;

        conn.execute(
            "INSERT INTO albums (id, name, album_type, label, release_date, genres, original_title, version_title)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                album.id,
                album.name,
                album.album_type.to_db_str(),
                album.label,
                album.release_date,
                genres_json,
                album.original_title,
                album.version_title,
            ],
        )?;
        Ok(())
    }

    /// Insert a track.
    pub fn insert_track(&self, track: &Track) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tags_json = serde_json::to_string(&track.tags)?;
        let languages_json = serde_json::to_string(&track.languages)?;

        conn.execute(
            "INSERT INTO tracks (id, name, album_id, disc_number, track_number, duration_secs,
                    is_explicit, audio_uri, format, tags, has_lyrics, languages, original_title, version_title)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                track.id,
                track.name,
                track.album_id,
                track.disc_number,
                track.track_number,
                track.duration_secs,
                track.is_explicit as i32,
                track.audio_uri,
                track.format.to_db_str(),
                tags_json,
                track.has_lyrics as i32,
                languages_json,
                track.original_title,
                track.version_title,
            ],
        )?;
        Ok(())
    }

    /// Insert an image.
    pub fn insert_image(&self, image: &Image) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO images (id, uri, size, width, height) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                image.id,
                image.uri,
                image.size.to_db_str(),
                image.width as i32,
                image.height as i32,
            ],
        )?;
        Ok(())
    }

    // =========================================================================
    // Write Operations - Relationships
    // =========================================================================

    /// Add an artist to an album.
    pub fn add_album_artist(&self, album_id: &str, artist_id: &str, position: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO album_artists (album_id, artist_id, position) VALUES (?1, ?2, ?3)",
            params![album_id, artist_id, position],
        )?;
        Ok(())
    }

    /// Add an artist to a track with a role.
    pub fn add_track_artist(
        &self,
        track_id: &str,
        artist_id: &str,
        role: &ArtistRole,
        position: i32,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO track_artists (track_id, artist_id, role, position) VALUES (?1, ?2, ?3, ?4)",
            params![track_id, artist_id, role.to_db_str(), position],
        )?;
        Ok(())
    }

    /// Add a related artist.
    pub fn add_related_artist(&self, artist_id: &str, related_artist_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO related_artists (artist_id, related_artist_id) VALUES (?1, ?2)",
            params![artist_id, related_artist_id],
        )?;
        Ok(())
    }

    /// Add an image to an artist.
    pub fn add_artist_image(
        &self,
        artist_id: &str,
        image_id: &str,
        image_type: &ImageType,
        position: i32,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO artist_images (artist_id, image_id, image_type, position) VALUES (?1, ?2, ?3, ?4)",
            params![artist_id, image_id, image_type.to_db_str(), position],
        )?;
        Ok(())
    }

    /// Add an image to an album.
    pub fn add_album_image(
        &self,
        album_id: &str,
        image_id: &str,
        image_type: &ImageType,
        position: i32,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO album_images (album_id, image_id, image_type, position) VALUES (?1, ?2, ?3, ?4)",
            params![album_id, image_id, image_type.to_db_str(), position],
        )?;
        Ok(())
    }

    // =========================================================================
    // Batch Import Operations
    // =========================================================================

    /// Begin a transaction for batch import operations.
    /// Returns a transaction guard that must be committed.
    pub fn begin_import(&self) -> Result<ImportTransaction<'_>> {
        let conn = self.conn.lock().unwrap();
        conn.execute("BEGIN TRANSACTION", [])?;
        Ok(ImportTransaction { store: self })
    }
}

/// Transaction guard for batch import operations.
pub struct ImportTransaction<'a> {
    store: &'a SqliteCatalogStore,
}

impl<'a> ImportTransaction<'a> {
    /// Commit the import transaction.
    pub fn commit(self) -> Result<()> {
        let conn = self.store.conn.lock().unwrap();
        conn.execute("COMMIT", [])?;
        Ok(())
    }

    /// Rollback the import transaction.
    pub fn rollback(self) -> Result<()> {
        let conn = self.store.conn.lock().unwrap();
        conn.execute("ROLLBACK", [])?;
        Ok(())
    }
}

// =========================================================================
// CatalogStore trait implementation
// =========================================================================

use super::trait_def::{CatalogStore, SearchableContentType, SearchableItem};

impl CatalogStore for SqliteCatalogStore {
    fn get_artist_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        match self.get_resolved_artist(id)? {
            Some(artist) => Ok(Some(serde_json::to_value(artist)?)),
            None => Ok(None),
        }
    }

    fn get_album_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        match self.get_album(id)? {
            Some(album) => Ok(Some(serde_json::to_value(album)?)),
            None => Ok(None),
        }
    }

    fn get_track_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        match self.get_track(id)? {
            Some(track) => Ok(Some(serde_json::to_value(track)?)),
            None => Ok(None),
        }
    }

    fn get_resolved_album_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        match self.get_resolved_album(id)? {
            Some(album) => Ok(Some(serde_json::to_value(album)?)),
            None => Ok(None),
        }
    }

    fn get_resolved_track_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        match self.get_resolved_track(id)? {
            Some(track) => Ok(Some(serde_json::to_value(track)?)),
            None => Ok(None),
        }
    }

    fn get_artist_discography_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        match self.get_artist_discography(id)? {
            Some(discography) => Ok(Some(serde_json::to_value(discography)?)),
            None => Ok(None),
        }
    }

    fn get_image_path(&self, id: &str) -> PathBuf {
        // For SQLite store, we need to look up the image URI and resolve it
        match self.get_image(id) {
            Ok(Some(image)) => self.resolve_image_uri(&image),
            _ => self.media_base_path.join("images").join(id),
        }
    }

    fn get_track_audio_path(&self, track_id: &str) -> Option<PathBuf> {
        self.get_track(track_id)
            .ok()
            .flatten()
            .map(|track| self.resolve_audio_uri(&track))
    }

    fn get_track_album_id(&self, track_id: &str) -> Option<String> {
        self.get_track(track_id)
            .ok()
            .flatten()
            .map(|track| track.album_id)
    }

    fn get_artists_count(&self) -> usize {
        self.get_counts().map(|(a, _, _, _)| a).unwrap_or(0)
    }

    fn get_albums_count(&self) -> usize {
        self.get_counts().map(|(_, a, _, _)| a).unwrap_or(0)
    }

    fn get_tracks_count(&self) -> usize {
        self.get_counts().map(|(_, _, t, _)| t).unwrap_or(0)
    }

    fn get_searchable_content(&self) -> Result<Vec<SearchableItem>> {
        let mut items = Vec::new();

        // Get all artists
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, genres FROM artists")?;
        let artists = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let genres_json: Option<String> = row.get(2)?;
            let genres: Vec<String> = genres_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            Ok((id, name, genres))
        })?;

        for artist_result in artists {
            let (id, name, genres) = artist_result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Artist,
                additional_text: genres,
            });
        }
        drop(stmt);

        // Get all albums
        let mut stmt = conn.prepare("SELECT id, name, genres FROM albums")?;
        let albums = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let genres_json: Option<String> = row.get(2)?;
            let genres: Vec<String> = genres_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            Ok((id, name, genres))
        })?;

        for album_result in albums {
            let (id, name, genres) = album_result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Album,
                additional_text: genres,
            });
        }
        drop(stmt);

        // Get all tracks
        let mut stmt = conn.prepare("SELECT id, name, tags FROM tracks")?;
        let tracks = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let tags_json: Option<String> = row.get(2)?;
            let tags: Vec<String> = tags_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            Ok((id, name, tags))
        })?;

        for track_result in tracks {
            let (id, name, tags) = track_result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Track,
                additional_text: tags,
            });
        }

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_store() -> (SqliteCatalogStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_catalog.db");
        let store = SqliteCatalogStore::new(&db_path, temp_dir.path()).unwrap();
        (store, temp_dir)
    }

    fn insert_test_artist(store: &SqliteCatalogStore, id: &str, name: &str) {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO artists (id, name, genres, activity_periods) VALUES (?1, ?2, ?3, ?4)",
            params![id, name, r#"["rock","metal"]"#, r#"[{"Decade":1990}]"#],
        )
        .unwrap();
    }

    fn insert_test_album(store: &SqliteCatalogStore, id: &str, name: &str) {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO albums (id, name, album_type, genres) VALUES (?1, ?2, ?3, ?4)",
            params![id, name, "ALBUM", r#"["rock"]"#],
        )
        .unwrap();
    }

    fn insert_test_track(store: &SqliteCatalogStore, id: &str, name: &str, album_id: &str) {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO tracks (id, name, album_id, track_number, audio_uri, format)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name, album_id, 1, "albums/A1/track.mp3", "MP3_320"],
        )
        .unwrap();
    }

    fn insert_test_image(store: &SqliteCatalogStore, id: &str, uri: &str) {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO images (id, uri, size, width, height) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, uri, "DEFAULT", 300, 300],
        )
        .unwrap();
    }

    #[test]
    fn test_create_and_open_store() {
        let (store, _temp_dir) = create_test_store();
        let counts = store.get_counts().unwrap();
        assert_eq!(counts, (0, 0, 0, 0));
    }

    #[test]
    fn test_get_artist() {
        let (store, _temp_dir) = create_test_store();
        insert_test_artist(&store, "R1", "Test Artist");

        let artist = store.get_artist("R1").unwrap().unwrap();
        assert_eq!(artist.id, "R1");
        assert_eq!(artist.name, "Test Artist");
        assert_eq!(artist.genres, vec!["rock", "metal"]);
        assert_eq!(artist.activity_periods.len(), 1);

        // Non-existent artist
        assert!(store.get_artist("R999").unwrap().is_none());
    }

    #[test]
    fn test_get_album() {
        let (store, _temp_dir) = create_test_store();
        insert_test_album(&store, "A1", "Test Album");

        let album = store.get_album("A1").unwrap().unwrap();
        assert_eq!(album.id, "A1");
        assert_eq!(album.name, "Test Album");
        assert_eq!(album.album_type, AlbumType::Album);
        assert_eq!(album.genres, vec!["rock"]);

        // Non-existent album
        assert!(store.get_album("A999").unwrap().is_none());
    }

    #[test]
    fn test_get_track() {
        let (store, _temp_dir) = create_test_store();
        insert_test_album(&store, "A1", "Test Album");
        insert_test_track(&store, "T1", "Test Track", "A1");

        let track = store.get_track("T1").unwrap().unwrap();
        assert_eq!(track.id, "T1");
        assert_eq!(track.name, "Test Track");
        assert_eq!(track.album_id, "A1");
        assert_eq!(track.format, Format::Mp3_320);

        // Non-existent track
        assert!(store.get_track("T999").unwrap().is_none());
    }

    #[test]
    fn test_get_image() {
        let (store, _temp_dir) = create_test_store();
        insert_test_image(&store, "I1", "images/test.jpg");

        let image = store.get_image("I1").unwrap().unwrap();
        assert_eq!(image.id, "I1");
        assert_eq!(image.uri, "images/test.jpg");
        assert_eq!(image.size, ImageSize::Default);
        assert_eq!(image.width, 300);
        assert_eq!(image.height, 300);

        // Non-existent image
        assert!(store.get_image("I999").unwrap().is_none());
    }

    #[test]
    fn test_get_resolved_artist() {
        let (store, _temp_dir) = create_test_store();
        insert_test_artist(&store, "R1", "Artist 1");
        insert_test_artist(&store, "R2", "Artist 2");
        insert_test_image(&store, "I1", "images/portrait.jpg");

        // Add relationships
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO related_artists (artist_id, related_artist_id) VALUES (?1, ?2)",
                params!["R1", "R2"],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO artist_images (artist_id, image_id, image_type, position)
                 VALUES (?1, ?2, ?3, ?4)",
                params!["R1", "I1", "portrait", 0],
            )
            .unwrap();
        }

        let resolved = store.get_resolved_artist("R1").unwrap().unwrap();
        assert_eq!(resolved.artist.name, "Artist 1");
        assert_eq!(resolved.images.len(), 1);
        assert_eq!(resolved.related_artists.len(), 1);
        assert_eq!(resolved.related_artists[0].name, "Artist 2");
    }

    #[test]
    fn test_get_resolved_album() {
        let (store, _temp_dir) = create_test_store();
        insert_test_artist(&store, "R1", "Artist 1");
        insert_test_album(&store, "A1", "Album 1");
        insert_test_track(&store, "T1", "Track 1", "A1");
        insert_test_track(&store, "T2", "Track 2", "A1");
        insert_test_image(&store, "I1", "images/cover.jpg");

        // Add relationships
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO album_artists (album_id, artist_id, position) VALUES (?1, ?2, ?3)",
                params!["A1", "R1", 0],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO album_images (album_id, image_id, image_type, position)
                 VALUES (?1, ?2, ?3, ?4)",
                params!["A1", "I1", "cover", 0],
            )
            .unwrap();
        }

        let resolved = store.get_resolved_album("A1").unwrap().unwrap();
        assert_eq!(resolved.album.name, "Album 1");
        assert_eq!(resolved.artists.len(), 1);
        assert_eq!(resolved.artists[0].name, "Artist 1");
        assert_eq!(resolved.discs.len(), 1);
        assert_eq!(resolved.discs[0].tracks.len(), 2);
        assert_eq!(resolved.images.len(), 1);
    }

    #[test]
    fn test_get_resolved_track() {
        let (store, _temp_dir) = create_test_store();
        insert_test_artist(&store, "R1", "Artist 1");
        insert_test_album(&store, "A1", "Album 1");
        insert_test_track(&store, "T1", "Track 1", "A1");

        // Add track artist relationship
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO track_artists (track_id, artist_id, role, position)
                 VALUES (?1, ?2, ?3, ?4)",
                params!["T1", "R1", "MAIN_ARTIST", 0],
            )
            .unwrap();
        }

        let resolved = store.get_resolved_track("T1").unwrap().unwrap();
        assert_eq!(resolved.track.name, "Track 1");
        assert_eq!(resolved.album.name, "Album 1");
        assert_eq!(resolved.artists.len(), 1);
        assert_eq!(resolved.artists[0].artist.name, "Artist 1");
        assert_eq!(resolved.artists[0].role, ArtistRole::MainArtist);
    }

    #[test]
    fn test_get_artist_discography() {
        let (store, _temp_dir) = create_test_store();
        insert_test_artist(&store, "R1", "Artist 1");
        insert_test_artist(&store, "R2", "Artist 2");
        insert_test_album(&store, "A1", "Album 1");
        insert_test_album(&store, "A2", "Album 2");
        insert_test_track(&store, "T1", "Track 1", "A2");

        // R1 is album artist on A1
        // R1 is featured on a track in A2 (owned by R2)
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO album_artists (album_id, artist_id, position) VALUES (?1, ?2, ?3)",
                params!["A1", "R1", 0],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO album_artists (album_id, artist_id, position) VALUES (?1, ?2, ?3)",
                params!["A2", "R2", 0],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO track_artists (track_id, artist_id, role, position)
                 VALUES (?1, ?2, ?3, ?4)",
                params!["T1", "R1", "FEATURED_ARTIST", 0],
            )
            .unwrap();
        }

        let discography = store.get_artist_discography("R1").unwrap().unwrap();
        assert_eq!(discography.albums.len(), 1);
        assert_eq!(discography.albums[0].name, "Album 1");
        assert_eq!(discography.features.len(), 1);
        assert_eq!(discography.features[0].name, "Album 2");
    }

    #[test]
    fn test_uri_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_catalog.db");
        let media_base = PathBuf::from("/mnt/music");
        let store = SqliteCatalogStore::new(&db_path, &media_base).unwrap();

        let track = Track {
            id: "T1".to_string(),
            name: "Test".to_string(),
            album_id: "A1".to_string(),
            disc_number: 1,
            track_number: 1,
            duration_secs: None,
            is_explicit: false,
            audio_uri: "albums/A1/track.mp3".to_string(),
            format: Format::Mp3_320,
            tags: vec![],
            has_lyrics: false,
            languages: vec![],
            original_title: None,
            version_title: None,
        };

        let resolved = store.resolve_audio_uri(&track);
        assert_eq!(resolved, PathBuf::from("/mnt/music/albums/A1/track.mp3"));
    }

    #[test]
    fn test_list_operations() {
        let (store, _temp_dir) = create_test_store();
        insert_test_artist(&store, "R1", "Artist 1");
        insert_test_artist(&store, "R2", "Artist 2");
        insert_test_album(&store, "A1", "Album 1");
        insert_test_track(&store, "T1", "Track 1", "A1");

        let artist_ids = store.list_artist_ids().unwrap();
        assert_eq!(artist_ids.len(), 2);

        let album_ids = store.list_album_ids().unwrap();
        assert_eq!(album_ids.len(), 1);

        let track_ids = store.list_track_ids().unwrap();
        assert_eq!(track_ids.len(), 1);

        let counts = store.get_counts().unwrap();
        assert_eq!(counts, (2, 1, 1, 0));
    }
}
