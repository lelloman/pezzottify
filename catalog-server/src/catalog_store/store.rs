//! SQLite-backed catalog store implementation for Spotify schema.
//!
//! This module provides the `SqliteCatalogStore` which reads catalog metadata
//! from the Spotify metadata database dump.

use super::models::*;
use super::trait_def::{CatalogStore, SearchableContentType, SearchableItem};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::info;

/// SQLite-backed catalog store for Spotify metadata.
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
    /// * `media_base_path` - Base path for resolving media file paths
    pub fn new<P: AsRef<Path>, M: AsRef<Path>>(db_path: P, media_base_path: M) -> Result<Self> {
        let conn = Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .context("Failed to open catalog database")?;

        // Log some stats
        let artist_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM artists", [], |r| r.get(0))
            .unwrap_or(0);
        let album_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM albums", [], |r| r.get(0))
            .unwrap_or(0);
        let track_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .unwrap_or(0);

        info!(
            "Opened Spotify catalog: {} artists, {} albums, {} tracks",
            artist_count, album_count, track_count
        );

        Ok(SqliteCatalogStore {
            conn: Arc::new(Mutex::new(conn)),
            media_base_path: media_base_path.as_ref().to_path_buf(),
        })
    }

    // =========================================================================
    // Internal Helper Methods
    // =========================================================================

    /// Get artist rowid from Spotify ID.
    fn get_artist_rowid(conn: &Connection, id: &str) -> Result<Option<i64>> {
        match conn.query_row("SELECT rowid FROM artists WHERE id = ?1", params![id], |r| {
            r.get(0)
        }) {
            Ok(rowid) => Ok(Some(rowid)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get album rowid from Spotify ID.
    fn get_album_rowid(conn: &Connection, id: &str) -> Result<Option<i64>> {
        match conn.query_row("SELECT rowid FROM albums WHERE id = ?1", params![id], |r| {
            r.get(0)
        }) {
            Ok(rowid) => Ok(Some(rowid)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get track rowid from Spotify ID.
    fn get_track_rowid(conn: &Connection, id: &str) -> Result<Option<i64>> {
        match conn.query_row("SELECT rowid FROM tracks WHERE id = ?1", params![id], |r| {
            r.get(0)
        }) {
            Ok(rowid) => Ok(Some(rowid)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get genres for an artist by rowid.
    fn get_artist_genres(conn: &Connection, artist_rowid: i64) -> Result<Vec<String>> {
        let mut stmt = conn.prepare_cached(
            "SELECT genre FROM artist_genres WHERE artist_rowid = ?1",
        )?;
        let genres = stmt
            .query_map(params![artist_rowid], |r| r.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(genres)
    }

    /// Parse an Artist from a row (id, name, followers_total, popularity).
    fn parse_artist_row(
        row: &rusqlite::Row,
        genres: Vec<String>,
    ) -> rusqlite::Result<Artist> {
        Ok(Artist {
            id: row.get(0)?,
            name: row.get(1)?,
            genres,
            followers_total: row.get(2)?,
            popularity: row.get(3)?,
        })
    }

    /// Parse an Album from a row.
    fn parse_album_row(row: &rusqlite::Row) -> rusqlite::Result<Album> {
        let album_type_str: String = row.get(2)?;
        let label: String = row.get(5)?;

        Ok(Album {
            id: row.get(0)?,
            name: row.get(1)?,
            album_type: AlbumType::from_db_str(&album_type_str),
            label: if label.is_empty() { None } else { Some(label) },
            release_date: row.get(7)?,
            release_date_precision: row.get(8)?,
            external_id_upc: row.get(3)?,
            popularity: row.get(6)?,
        })
    }

    /// Parse a Track from a row.
    fn parse_track_row(row: &rusqlite::Row, album_id: String) -> rusqlite::Result<Track> {
        let explicit: i32 = row.get(8)?;

        Ok(Track {
            id: row.get(0)?,
            name: row.get(1)?,
            album_id,
            disc_number: row.get(6)?,
            track_number: row.get(3)?,
            duration_ms: row.get(7)?,
            explicit: explicit != 0,
            popularity: row.get(5)?,
            language: row.get(9)?,
            external_id_isrc: row.get(4)?,
        })
    }

    // =========================================================================
    // Read Operations - Core Entities
    // =========================================================================

    /// Get an artist by ID.
    pub fn get_artist(&self, id: &str) -> Result<Option<Artist>> {
        let conn = self.conn.lock().unwrap();

        let rowid = match Self::get_artist_rowid(&conn, id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        let genres = Self::get_artist_genres(&conn, rowid)?;

        let mut stmt = conn.prepare_cached(
            "SELECT id, name, followers_total, popularity FROM artists WHERE rowid = ?1",
        )?;

        match stmt.query_row(params![rowid], |row| Self::parse_artist_row(row, genres.clone())) {
            Ok(artist) => Ok(Some(artist)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get an album by ID.
    pub fn get_album(&self, id: &str) -> Result<Option<Album>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare_cached(
            "SELECT id, name, album_type, external_id_upc, external_id_amgid,
                    label, popularity, release_date, release_date_precision
             FROM albums WHERE id = ?1",
        )?;

        match stmt.query_row(params![id], Self::parse_album_row) {
            Ok(album) => Ok(Some(album)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get a track by ID (internal helper that takes conn reference).
    fn get_track_inner(conn: &Connection, id: &str) -> Result<Option<Track>> {
        // First get the track with album_rowid
        let mut stmt = conn.prepare_cached(
            "SELECT id, name, album_rowid, track_number, external_id_isrc,
                    popularity, disc_number, duration_ms, explicit, language
             FROM tracks WHERE id = ?1",
        )?;

        let row_result = stmt.query_row(params![id], |row| {
            let album_rowid: i64 = row.get(2)?;
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, album_rowid,
                row.get::<_, i32>(3)?, row.get::<_, Option<String>>(4)?,
                row.get::<_, i32>(5)?, row.get::<_, i32>(6)?, row.get::<_, i64>(7)?,
                row.get::<_, i32>(8)?, row.get::<_, Option<String>>(9)?))
        });

        let (track_id, name, album_rowid, track_number, isrc, popularity, disc_number, duration_ms, explicit, language) = match row_result {
            Ok(data) => data,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        // Get album ID from rowid
        let album_id: String = conn.query_row(
            "SELECT id FROM albums WHERE rowid = ?1",
            params![album_rowid],
            |r| r.get(0),
        )?;

        Ok(Some(Track {
            id: track_id,
            name,
            album_id,
            disc_number,
            track_number,
            duration_ms,
            explicit: explicit != 0,
            popularity,
            language,
            external_id_isrc: isrc,
        }))
    }

    // =========================================================================
    // Read Operations - Resolved/Composite Types
    // =========================================================================

    /// Get a fully resolved artist.
    pub fn get_resolved_artist(&self, id: &str) -> Result<Option<ResolvedArtist>> {
        let artist = match self.get_artist(id)? {
            Some(a) => a,
            None => return Ok(None),
        };

        // No related artists in Spotify schema
        Ok(Some(ResolvedArtist {
            artist,
            related_artists: vec![],
        }))
    }

    /// Get a fully resolved album with tracks and artists.
    pub fn get_resolved_album(&self, id: &str) -> Result<Option<ResolvedAlbum>> {
        let conn = self.conn.lock().unwrap();

        let album_rowid = match Self::get_album_rowid(&conn, id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        // Get album
        let mut album_stmt = conn.prepare_cached(
            "SELECT id, name, album_type, external_id_upc, external_id_amgid,
                    label, popularity, release_date, release_date_precision
             FROM albums WHERE rowid = ?1",
        )?;
        let album = album_stmt.query_row(params![album_rowid], Self::parse_album_row)?;

        // Get album artists
        let mut artists_stmt = conn.prepare_cached(
            "SELECT a.id, a.name, a.followers_total, a.popularity, a.rowid
             FROM artists a
             INNER JOIN artist_albums aa ON a.rowid = aa.artist_rowid
             WHERE aa.album_rowid = ?1 AND aa.is_appears_on = 0
             ORDER BY aa.index_in_album",
        )?;
        let artists: Vec<Artist> = artists_stmt
            .query_map(params![album_rowid], |row| {
                let artist_rowid: i64 = row.get(4)?;
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?, row.get::<_, i32>(3)?, artist_rowid))
            })?
            .filter_map(|r| r.ok())
            .map(|(id, name, followers, popularity, artist_rowid)| {
                let genres = Self::get_artist_genres(&conn, artist_rowid).unwrap_or_default();
                Artist { id, name, genres, followers_total: followers, popularity }
            })
            .collect();

        // Get tracks grouped by disc
        let mut tracks_stmt = conn.prepare_cached(
            "SELECT id, name, album_rowid, track_number, external_id_isrc,
                    popularity, disc_number, duration_ms, explicit, language
             FROM tracks WHERE album_rowid = ?1
             ORDER BY disc_number, track_number",
        )?;

        let tracks: Vec<Track> = tracks_stmt
            .query_map(params![album_rowid], |row| {
                let explicit: i32 = row.get(8)?;
                Ok(Track {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    album_id: album.id.clone(),
                    disc_number: row.get(6)?,
                    track_number: row.get(3)?,
                    duration_ms: row.get(7)?,
                    explicit: explicit != 0,
                    popularity: row.get(5)?,
                    language: row.get(9)?,
                    external_id_isrc: row.get(4)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Group tracks by disc
        let mut disc_map: HashMap<i32, Vec<Track>> = HashMap::new();
        for track in tracks {
            disc_map.entry(track.disc_number).or_default().push(track);
        }

        let mut discs: Vec<Disc> = disc_map
            .into_iter()
            .map(|(number, tracks)| Disc { number, tracks })
            .collect();
        discs.sort_by_key(|d| d.number);

        Ok(Some(ResolvedAlbum {
            album,
            artists,
            discs,
        }))
    }

    /// Get a fully resolved track with album and artists.
    pub fn get_resolved_track(&self, id: &str) -> Result<Option<ResolvedTrack>> {
        let conn = self.conn.lock().unwrap();

        let track_rowid = match Self::get_track_rowid(&conn, id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        // Get track with album info
        let mut track_stmt = conn.prepare_cached(
            "SELECT t.id, t.name, t.album_rowid, t.track_number, t.external_id_isrc,
                    t.popularity, t.disc_number, t.duration_ms, t.explicit, t.language,
                    a.id as album_id
             FROM tracks t
             INNER JOIN albums a ON t.album_rowid = a.rowid
             WHERE t.rowid = ?1",
        )?;

        let (track, album_id): (Track, String) = track_stmt.query_row(params![track_rowid], |row| {
            let explicit: i32 = row.get(8)?;
            let album_id: String = row.get(10)?;
            Ok((
                Track {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    album_id: album_id.clone(),
                    disc_number: row.get(6)?,
                    track_number: row.get(3)?,
                    duration_ms: row.get(7)?,
                    explicit: explicit != 0,
                    popularity: row.get(5)?,
                    language: row.get(9)?,
                    external_id_isrc: row.get(4)?,
                },
                album_id,
            ))
        })?;

        // Get album
        let mut album_stmt = conn.prepare_cached(
            "SELECT id, name, album_type, external_id_upc, external_id_amgid,
                    label, popularity, release_date, release_date_precision
             FROM albums WHERE id = ?1",
        )?;
        let album = album_stmt.query_row(params![album_id], Self::parse_album_row)?;

        // Get track artists
        let mut artists_stmt = conn.prepare_cached(
            "SELECT a.id, a.name, a.followers_total, a.popularity, a.rowid, ta.role
             FROM artists a
             INNER JOIN track_artists ta ON a.rowid = ta.artist_rowid
             WHERE ta.track_rowid = ?1
             ORDER BY ta.role, a.popularity DESC",
        )?;

        let artists: Vec<TrackArtist> = artists_stmt
            .query_map(params![track_rowid], |row| {
                let artist_rowid: i64 = row.get(4)?;
                let role: Option<i32> = row.get(5)?;
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?, row.get::<_, i32>(3)?, artist_rowid,
                    role.unwrap_or(0)))
            })?
            .filter_map(|r| r.ok())
            .map(|(id, name, followers, popularity, artist_rowid, role)| {
                let genres = Self::get_artist_genres(&conn, artist_rowid).unwrap_or_default();
                TrackArtist {
                    artist: Artist { id, name, genres, followers_total: followers, popularity },
                    role: ArtistRole::from_db_int(role),
                }
            })
            .collect();

        Ok(Some(ResolvedTrack {
            track,
            album,
            artists,
        }))
    }

    /// Get artist's discography.
    pub fn get_discography(&self, id: &str) -> Result<Option<ArtistDiscography>> {
        let conn = self.conn.lock().unwrap();

        let artist_rowid = match Self::get_artist_rowid(&conn, id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        // Albums where artist is primary (is_appears_on = 0)
        let mut albums_stmt = conn.prepare_cached(
            "SELECT a.id, a.name, a.album_type, a.external_id_upc, a.external_id_amgid,
                    a.label, a.popularity, a.release_date, a.release_date_precision
             FROM albums a
             INNER JOIN artist_albums aa ON a.rowid = aa.album_rowid
             WHERE aa.artist_rowid = ?1 AND aa.is_appears_on = 0
             ORDER BY a.release_date DESC",
        )?;

        let albums: Vec<Album> = albums_stmt
            .query_map(params![artist_rowid], Self::parse_album_row)?
            .filter_map(|r| r.ok())
            .collect();

        // Albums where artist appears on (is_appears_on = 1)
        let mut features_stmt = conn.prepare_cached(
            "SELECT a.id, a.name, a.album_type, a.external_id_upc, a.external_id_amgid,
                    a.label, a.popularity, a.release_date, a.release_date_precision
             FROM albums a
             INNER JOIN artist_albums aa ON a.rowid = aa.album_rowid
             WHERE aa.artist_rowid = ?1 AND aa.is_appears_on = 1
             ORDER BY a.release_date DESC",
        )?;

        let features: Vec<Album> = features_stmt
            .query_map(params![artist_rowid], Self::parse_album_row)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(Some(ArtistDiscography { albums, features }))
    }

    // =========================================================================
    // Image URL Retrieval
    // =========================================================================

    /// Get the largest image URL for an album.
    pub fn get_album_image_url(&self, album_id: &str) -> Result<Option<ImageUrl>> {
        let conn = self.conn.lock().unwrap();

        let album_rowid = match Self::get_album_rowid(&conn, album_id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        let mut stmt = conn.prepare_cached(
            "SELECT url, width, height FROM album_images
             WHERE album_rowid = ?1
             ORDER BY width DESC LIMIT 1",
        )?;

        match stmt.query_row(params![album_rowid], |row| {
            Ok(ImageUrl {
                url: row.get(0)?,
                width: row.get(1)?,
                height: row.get(2)?,
            })
        }) {
            Ok(img) => Ok(Some(img)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get the largest image URL for an artist.
    pub fn get_artist_image_url(&self, artist_id: &str) -> Result<Option<ImageUrl>> {
        let conn = self.conn.lock().unwrap();

        let artist_rowid = match Self::get_artist_rowid(&conn, artist_id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        let mut stmt = conn.prepare_cached(
            "SELECT url, width, height FROM artist_images
             WHERE artist_rowid = ?1
             ORDER BY width DESC LIMIT 1",
        )?;

        match stmt.query_row(params![artist_rowid], |row| {
            Ok(ImageUrl {
                url: row.get(0)?,
                width: row.get(1)?,
                height: row.get(2)?,
            })
        }) {
            Ok(img) => Ok(Some(img)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    // =========================================================================
    // Counts
    // =========================================================================

    /// Get the number of artists.
    pub fn get_artists_count(&self) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM artists", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    /// Get the number of albums.
    pub fn get_albums_count(&self) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM albums", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    /// Get the number of tracks.
    pub fn get_tracks_count(&self) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }
}

// =============================================================================
// CatalogStore Trait Implementation
// =============================================================================

impl CatalogStore for SqliteCatalogStore {
    fn get_artist_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.get_artist(id)
            .map(|opt| opt.map(|a| serde_json::to_value(a).unwrap()))
    }

    fn get_album_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.get_album(id)
            .map(|opt| opt.map(|a| serde_json::to_value(a).unwrap()))
    }

    fn get_track_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        Self::get_track_inner(&conn, id)
            .map(|opt| opt.map(|t| serde_json::to_value(t).unwrap()))
    }

    fn get_track(&self, id: &str) -> Result<Option<Track>> {
        let conn = self.conn.lock().unwrap();
        Self::get_track_inner(&conn, id)
    }

    fn get_resolved_artist_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.get_resolved_artist(id)
            .map(|opt| opt.map(|a| serde_json::to_value(a).unwrap()))
    }

    fn get_resolved_album_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.get_resolved_album(id)
            .map(|opt| opt.map(|a| serde_json::to_value(a).unwrap()))
    }

    fn get_resolved_track_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.get_resolved_track(id)
            .map(|opt| opt.map(|t| serde_json::to_value(t).unwrap()))
    }

    fn get_resolved_artist(&self, id: &str) -> Result<Option<ResolvedArtist>> {
        SqliteCatalogStore::get_resolved_artist(self, id)
    }

    fn get_resolved_album(&self, id: &str) -> Result<Option<ResolvedAlbum>> {
        SqliteCatalogStore::get_resolved_album(self, id)
    }

    fn get_resolved_track(&self, id: &str) -> Result<Option<ResolvedTrack>> {
        SqliteCatalogStore::get_resolved_track(self, id)
    }

    fn get_artist_discography_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.get_discography(id)
            .map(|opt| opt.map(|d| serde_json::to_value(d).unwrap()))
    }

    fn get_discography(&self, id: &str) -> Result<Option<ArtistDiscography>> {
        SqliteCatalogStore::get_discography(self, id)
    }

    fn get_album_image_url(&self, album_id: &str) -> Result<Option<ImageUrl>> {
        SqliteCatalogStore::get_album_image_url(self, album_id)
    }

    fn get_artist_image_url(&self, artist_id: &str) -> Result<Option<ImageUrl>> {
        SqliteCatalogStore::get_artist_image_url(self, artist_id)
    }

    fn get_image_path(&self, id: &str) -> PathBuf {
        // Images are stored as {media_base_path}/images/{id}.jpg
        self.media_base_path.join("images").join(format!("{}.jpg", id))
    }

    fn get_track_audio_path(&self, track_id: &str) -> Option<PathBuf> {
        // Audio files are stored as {media_base_path}/audio/{track_id}.ogg
        let path = self.media_base_path.join("audio").join(format!("{}.ogg", track_id));
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    fn get_track_album_id(&self, track_id: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT a.id FROM tracks t
             INNER JOIN albums a ON t.album_rowid = a.rowid
             WHERE t.id = ?1",
            params![track_id],
            |r| r.get(0),
        )
        .ok()
    }

    fn get_artists_count(&self) -> usize {
        SqliteCatalogStore::get_artists_count(self)
    }

    fn get_albums_count(&self) -> usize {
        SqliteCatalogStore::get_albums_count(self)
    }

    fn get_tracks_count(&self) -> usize {
        SqliteCatalogStore::get_tracks_count(self)
    }

    fn get_searchable_content(&self) -> Result<Vec<SearchableItem>> {
        let conn = self.conn.lock().unwrap();
        let mut items = Vec::new();

        // Index top 5% of each category by popularity
        const TOP_PERCENTAGE: f64 = 0.05;

        // Calculate dynamic limits based on table sizes
        let artist_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM artists", [], |r| r.get(0))
            .unwrap_or(0);
        let album_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM albums", [], |r| r.get(0))
            .unwrap_or(0);
        let track_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .unwrap_or(0);

        let artist_limit = ((artist_count as f64 * TOP_PERCENTAGE) as i64).max(10000);
        let album_limit = ((album_count as f64 * TOP_PERCENTAGE) as i64).max(10000);
        let track_limit = ((track_count as f64 * TOP_PERCENTAGE) as i64).max(10000);

        info!(
            "Indexing top {:.0}%: {} artists, {} albums, {} tracks",
            TOP_PERCENTAGE * 100.0,
            artist_limit,
            album_limit,
            track_limit
        );

        // Artists (skip genres for speed - they can be added later if needed)
        let mut artist_stmt = conn.prepare(
            "SELECT id, name FROM artists ORDER BY popularity DESC LIMIT ?",
        )?;
        let artist_iter = artist_stmt.query_map([artist_limit], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        for result in artist_iter {
            let (id, name) = result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            });
        }

        // Albums
        let mut album_stmt = conn.prepare(
            "SELECT id, name FROM albums ORDER BY popularity DESC LIMIT ?",
        )?;
        let album_iter = album_stmt.query_map([album_limit], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        for result in album_iter {
            let (id, name) = result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Album,
                additional_text: vec![],
            });
        }

        // Tracks
        let mut track_stmt = conn.prepare(
            "SELECT id, name FROM tracks ORDER BY popularity DESC LIMIT ?",
        )?;
        let track_iter = track_stmt.query_map([track_limit], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        for result in track_iter {
            let (id, name) = result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Track,
                additional_text: vec![],
            });
        }

        Ok(items)
    }

    fn list_all_track_ids(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM tracks")?;
        let ids = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests would go here but require access to the actual Spotify database
}
