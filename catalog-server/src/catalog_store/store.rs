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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tracing::info;

/// SQLite-backed catalog store for Spotify metadata.
#[derive(Clone)]
pub struct SqliteCatalogStore {
    read_pool: Vec<Arc<Mutex<Connection>>>,
    write_conn: Arc<Mutex<Connection>>,
    media_base_path: PathBuf,
    read_index: Arc<AtomicUsize>,
}

impl SqliteCatalogStore {
    /// Create a new SqliteCatalogStore.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    /// * `media_base_path` - Base path for resolving media file paths
    /// * `read_pool_size` - Number of connections for concurrent read operations (default: 4)
    pub fn new<P: AsRef<Path>, M: AsRef<Path>>(
        db_path: P,
        media_base_path: M,
        read_pool_size: usize,
    ) -> Result<Self> {
        let db_path_ref = db_path.as_ref();

        let write_conn = Connection::open_with_flags(
            db_path_ref,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .context("Failed to open catalog database")?;

        write_conn.pragma_update(None, "journal_mode", "WAL")?;

        let artist_count: i64 = write_conn
            .query_row("SELECT COUNT(*) FROM artists", [], |r| r.get(0))
            .unwrap_or(0);
        let album_count: i64 = write_conn
            .query_row("SELECT COUNT(*) FROM albums", [], |r| r.get(0))
            .unwrap_or(0);
        let track_count: i64 = write_conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .unwrap_or(0);

        info!(
            "Opened Spotify catalog: {} artists, {} albums, {} tracks",
            artist_count, album_count, track_count
        );

        let mut read_pool = Vec::with_capacity(read_pool_size);
        for _ in 0..read_pool_size {
            let read_conn = Connection::open_with_flags(
                db_path_ref,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_URI
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            read_conn.pragma_update(None, "journal_mode", "WAL")?;
            read_pool.push(Arc::new(Mutex::new(read_conn)));
        }

        Ok(SqliteCatalogStore {
            write_conn: Arc::new(Mutex::new(write_conn)),
            read_pool,
            media_base_path: media_base_path.as_ref().to_path_buf(),
            read_index: Arc::new(AtomicUsize::new(0)),
        })
    }

    fn get_read_conn(&self) -> Arc<Mutex<Connection>> {
        let index = self.read_index.fetch_add(1, Ordering::SeqCst) % self.read_pool.len();
        self.read_pool[index].clone()
    }

    /// Compute track availability from an already-fetched audio_uri.
    ///
    /// This avoids acquiring another database connection, preventing deadlocks
    /// when called from within methods that already hold a connection.
    fn availability_from_audio_uri(&self, audio_uri: &Option<String>) -> TrackAvailability {
        match audio_uri {
            Some(uri) if !uri.is_empty() => {
                let path = self.media_base_path.join(uri);
                if path.exists() {
                    TrackAvailability::Available
                } else {
                    TrackAvailability::Unavailable
                }
            }
            _ => TrackAvailability::Unavailable,
        }
    }

    // =========================================================================
    // Internal Helper Methods
    // =========================================================================

    /// Get artist rowid from Spotify ID.
    fn get_artist_rowid(conn: &Connection, id: &str) -> Result<Option<i64>> {
        match conn.query_row(
            "SELECT rowid FROM artists WHERE id = ?1",
            params![id],
            |r| r.get(0),
        ) {
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
        let mut stmt =
            conn.prepare_cached("SELECT genre FROM artist_genres WHERE artist_rowid = ?1")?;
        let genres = stmt
            .query_map(params![artist_rowid], |r| r.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(genres)
    }

    /// Parse an Artist from a row (id, name, followers_total, popularity).
    fn parse_artist_row(row: &rusqlite::Row, genres: Vec<String>) -> rusqlite::Result<Artist> {
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

    // =========================================================================
    // Read Operations - Core Entities
    // =========================================================================

    /// Get an artist by ID.
    pub fn get_artist(&self, id: &str) -> Result<Option<Artist>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        let rowid = match Self::get_artist_rowid(&conn, id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        let genres = Self::get_artist_genres(&conn, rowid)?;

        let mut stmt = conn.prepare_cached(
            "SELECT id, name, followers_total, popularity FROM artists WHERE rowid = ?1",
        )?;

        match stmt.query_row(params![rowid], |row| {
            Self::parse_artist_row(row, genres.clone())
        }) {
            Ok(artist) => Ok(Some(artist)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get an album by ID.
    pub fn get_album(&self, id: &str) -> Result<Option<Album>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

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
        let mut stmt = conn.prepare_cached(
            "SELECT id, name, album_rowid, track_number, external_id_isrc,
                    popularity, disc_number, duration_ms, explicit, language, audio_uri
             FROM tracks WHERE id = ?1",
        )?;

        let row_result = stmt.query_row(params![id], |row| {
            let album_rowid: i64 = row.get(2)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                album_rowid,
                row.get::<_, i32>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i32>(5)?,
                row.get::<_, i32>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i32>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
            ))
        });

        let (
            track_id,
            name,
            album_rowid,
            track_number,
            isrc,
            popularity,
            disc_number,
            duration_ms,
            explicit,
            language,
            audio_uri,
        ) = match row_result {
            Ok(data) => data,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

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
            audio_uri,
            availability: TrackAvailability::default(),
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

        Ok(Some(ResolvedArtist {
            artist,
            related_artists: vec![],
        }))
    }

    /// Get a fully resolved album with tracks and artists.
    pub fn get_resolved_album(&self, id: &str) -> Result<Option<ResolvedAlbum>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        let album_rowid = match Self::get_album_rowid(&conn, id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        let mut album_stmt = conn.prepare_cached(
            "SELECT id, name, album_type, external_id_upc, external_id_amgid,
                    label, popularity, release_date, release_date_precision
             FROM albums WHERE rowid = ?1",
        )?;
        let album = album_stmt.query_row(params![album_rowid], Self::parse_album_row)?;

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
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i32>(3)?,
                    artist_rowid,
                ))
            })?
            .filter_map(|r| r.ok())
            .map(|(id, name, followers, popularity, artist_rowid)| {
                let genres = Self::get_artist_genres(&conn, artist_rowid).unwrap_or_default();
                Artist {
                    id,
                    name,
                    genres,
                    followers_total: followers,
                    popularity,
                }
            })
            .collect();

        let mut tracks_stmt = conn.prepare_cached(
            "SELECT id, name, album_rowid, track_number, external_id_isrc,
                    popularity, disc_number, duration_ms, explicit, language, audio_uri
             FROM tracks WHERE album_rowid = ?1
             ORDER BY disc_number, track_number",
        )?;

        let tracks: Vec<Track> = tracks_stmt
            .query_map(params![album_rowid], |row| {
                let explicit: i32 = row.get(8)?;
                let audio_uri: Option<String> = row.get(10)?;
                Ok((
                    Track {
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
                        audio_uri: audio_uri.clone(),
                        availability: TrackAvailability::default(),
                    },
                    audio_uri,
                ))
            })?
            .filter_map(|r| r.ok())
            .map(|(mut t, audio_uri)| {
                // Compute availability using already-fetched audio_uri to avoid
                // acquiring another connection (which would cause deadlocks)
                t.availability = self.availability_from_audio_uri(&audio_uri);
                t
            })
            .collect();

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
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        let track_rowid = match Self::get_track_rowid(&conn, id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        let mut track_stmt = conn.prepare_cached(
            "SELECT t.id, t.name, t.album_rowid, t.track_number, t.external_id_isrc,
                    t.popularity, t.disc_number, t.duration_ms, t.explicit, t.language,
                    a.id as album_id, t.audio_uri
             FROM tracks t
             INNER JOIN albums a ON t.album_rowid = a.rowid
             WHERE t.rowid = ?1",
        )?;

        let (mut track, album_id, audio_uri): (Track, String, Option<String>) = track_stmt
            .query_row(params![track_rowid], |row| {
                let explicit: i32 = row.get(8)?;
                let album_id: String = row.get(10)?;
                let audio_uri: Option<String> = row.get(11)?;
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
                        audio_uri: audio_uri.clone(),
                        availability: TrackAvailability::default(),
                    },
                    album_id,
                    audio_uri,
                ))
            })?;

        // Compute availability using already-fetched audio_uri to avoid
        // acquiring another connection (which would cause deadlocks)
        track.availability = self.availability_from_audio_uri(&audio_uri);

        let mut album_stmt = conn.prepare_cached(
            "SELECT id, name, album_type, external_id_upc, external_id_amgid,
                    label, popularity, release_date, release_date_precision
             FROM albums WHERE id = ?1",
        )?;
        let album = album_stmt.query_row(params![album_id], Self::parse_album_row)?;

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
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i32>(3)?,
                    artist_rowid,
                    role.unwrap_or(0),
                ))
            })?
            .filter_map(|r| r.ok())
            .map(|(id, name, followers, popularity, artist_rowid, role)| {
                let genres = Self::get_artist_genres(&conn, artist_rowid).unwrap_or_default();
                TrackArtist {
                    artist: Artist {
                        id,
                        name,
                        genres,
                        followers_total: followers,
                        popularity,
                    },
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

    /// Get artist's discography with pagination.
    pub fn get_discography(
        &self,
        id: &str,
        limit: usize,
        offset: usize,
        sort: DiscographySort,
    ) -> Result<Option<ArtistDiscography>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        let artist_rowid = match Self::get_artist_rowid(&conn, id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        let total: usize = conn.query_row(
            "SELECT COUNT(*) FROM artist_albums WHERE artist_rowid = ?1 AND is_appears_on = 0",
            params![artist_rowid],
            |row| row.get::<_, i64>(0),
        )? as usize;

        let order_clause = match sort {
            DiscographySort::Popularity => "a.popularity DESC, a.release_date DESC",
            DiscographySort::ReleaseDate => "a.release_date DESC, a.popularity DESC",
        };

        let query = format!(
            "SELECT a.id, a.name, a.album_type, a.external_id_upc, a.external_id_amgid,
                    a.label, a.popularity, a.release_date, a.release_date_precision
             FROM albums a
             INNER JOIN artist_albums aa ON a.rowid = aa.album_rowid
             WHERE aa.artist_rowid = ?1 AND aa.is_appears_on = 0
             ORDER BY {}
             LIMIT ?2 OFFSET ?3",
            order_clause
        );

        let mut albums_stmt = conn.prepare_cached(&query)?;

        let albums: Vec<Album> = albums_stmt
            .query_map(
                params![artist_rowid, limit as i64, offset as i64],
                Self::parse_album_row,
            )?
            .filter_map(|r| r.ok())
            .collect();

        let has_more = offset + albums.len() < total;

        Ok(Some(ArtistDiscography {
            albums,
            total,
            has_more,
        }))
    }

    // =========================================================================
    // Image URL Retrieval
    // =========================================================================

    /// Get the largest image URL for an album.
    pub fn get_album_image_url(&self, album_id: &str) -> Result<Option<ImageUrl>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

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
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

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
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM artists", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    /// Get the number of albums.
    pub fn get_albums_count(&self) -> usize {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM albums", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    /// Get the number of tracks.
    pub fn get_tracks_count(&self) -> usize {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
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
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        Self::get_track_inner(&conn, id).map(|opt| {
            opt.map(|mut t| {
                // Compute availability using already-fetched audio_uri to avoid
                // acquiring another connection (which would cause deadlocks)
                t.availability = self.availability_from_audio_uri(&t.audio_uri);
                serde_json::to_value(t).unwrap()
            })
        })
    }

    fn get_track(&self, id: &str) -> Result<Option<Track>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        Self::get_track_inner(&conn, id).map(|opt| {
            opt.map(|mut t| {
                // Compute availability using already-fetched audio_uri to avoid
                // acquiring another connection (which would cause deadlocks)
                t.availability = self.availability_from_audio_uri(&t.audio_uri);
                t
            })
        })
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

    fn get_discography(
        &self,
        id: &str,
        limit: usize,
        offset: usize,
        sort: DiscographySort,
    ) -> Result<Option<ArtistDiscography>> {
        SqliteCatalogStore::get_discography(self, id, limit, offset, sort)
    }

    fn get_album_image_url(&self, album_id: &str) -> Result<Option<ImageUrl>> {
        SqliteCatalogStore::get_album_image_url(self, album_id)
    }

    fn get_artist_image_url(&self, artist_id: &str) -> Result<Option<ImageUrl>> {
        SqliteCatalogStore::get_artist_image_url(self, artist_id)
    }

    fn get_image_path(&self, id: &str) -> PathBuf {
        self.media_base_path
            .join("images")
            .join(format!("{}.jpg", id))
    }

    fn get_track_audio_path(&self, track_id: &str) -> Option<PathBuf> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let audio_uri: Option<String> = conn
            .query_row(
                "SELECT audio_uri FROM tracks WHERE id = ?1",
                params![track_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();

        audio_uri.map(|uri| self.media_base_path.join(uri))
    }

    fn get_track_album_id(&self, track_id: &str) -> Option<String> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
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
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        let artist_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM artists", [], |r| r.get(0))
            .unwrap_or(0);
        let album_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM albums", [], |r| r.get(0))
            .unwrap_or(0);
        let track_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .unwrap_or(0);

        let total = artist_count + album_count + track_count;
        info!(
            "Indexing all content: {} artists, {} albums, {} tracks ({} total)",
            artist_count, album_count, track_count, total
        );

        let mut items = Vec::with_capacity(total as usize);

        let mut artist_stmt =
            conn.prepare("SELECT id, name FROM artists ORDER BY popularity DESC")?;
        let artist_iter = artist_stmt.query_map([], |row| {
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
        info!("Loaded {} artists for indexing", items.len());

        let mut album_stmt =
            conn.prepare("SELECT id, name FROM albums ORDER BY popularity DESC")?;
        let album_iter = album_stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let album_start = items.len();
        for result in album_iter {
            let (id, name) = result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Album,
                additional_text: vec![],
            });
        }
        info!("Loaded {} albums for indexing", items.len() - album_start);

        let mut track_stmt =
            conn.prepare("SELECT id, name FROM tracks ORDER BY popularity DESC")?;
        let track_iter = track_stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let track_start = items.len();
        for result in track_iter {
            let (id, name) = result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Track,
                additional_text: vec![],
            });
        }
        info!("Loaded {} tracks for indexing", items.len() - track_start);

        info!("Total searchable items: {}", items.len());
        Ok(items)
    }

    fn list_all_track_ids(&self) -> Result<Vec<String>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM tracks")?;
        let ids = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(ids)
    }

    // =========================================================================
    // CRUD Operations (with transactions)
    // =========================================================================

    fn create_artist(&self, artist: &Artist) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM artists WHERE id = ?1)",
                params![&artist.id],
                |r| r.get(0),
            )?;
            if exists {
                anyhow::bail!("Artist with id '{}' already exists", artist.id);
            }

            conn.execute(
                "INSERT INTO artists (id, name, followers_total, popularity) VALUES (?1, ?2, ?3, ?4)",
                params![
                    &artist.id,
                    &artist.name,
                    artist.followers_total,
                    artist.popularity
                ],
            )?;

            let artist_rowid: i64 = conn.query_row(
                "SELECT rowid FROM artists WHERE id = ?1",
                params![&artist.id],
                |r| r.get(0),
            )?;

            for genre in &artist.genres {
                conn.execute(
                    "INSERT INTO artist_genres (artist_rowid, genre) VALUES (?1, ?2)",
                    params![artist_rowid, genre],
                )?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    fn update_artist(&self, artist: &Artist) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            let artist_rowid: i64 = match conn.query_row(
                "SELECT rowid FROM artists WHERE id = ?1",
                params![&artist.id],
                |r| r.get(0),
            ) {
                Ok(rowid) => rowid,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    anyhow::bail!("Artist with id '{}' not found", artist.id);
                }
                Err(e) => return Err(e.into()),
            };

            conn.execute(
                "UPDATE artists SET name = ?1, followers_total = ?2, popularity = ?3 WHERE rowid = ?4",
                params![
                    &artist.name,
                    artist.followers_total,
                    artist.popularity,
                    artist_rowid
                ],
            )?;

            conn.execute(
                "DELETE FROM artist_genres WHERE artist_rowid = ?1",
                params![artist_rowid],
            )?;
            for genre in &artist.genres {
                conn.execute(
                    "INSERT INTO artist_genres (artist_rowid, genre) VALUES (?1, ?2)",
                    params![artist_rowid, genre],
                )?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    fn delete_artist(&self, id: &str) -> Result<bool> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<bool> {
            let artist_rowid: Option<i64> = match conn.query_row(
                "SELECT rowid FROM artists WHERE id = ?1",
                params![id],
                |r| r.get(0),
            ) {
                Ok(rowid) => Some(rowid),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(e.into()),
            };

            if let Some(rowid) = artist_rowid {
                conn.execute(
                    "DELETE FROM artist_genres WHERE artist_rowid = ?1",
                    params![rowid],
                )?;
                conn.execute(
                    "DELETE FROM artist_albums WHERE artist_rowid = ?1",
                    params![rowid],
                )?;
                conn.execute(
                    "DELETE FROM track_artists WHERE artist_rowid = ?1",
                    params![rowid],
                )?;
                conn.execute(
                    "DELETE FROM artist_images WHERE artist_rowid = ?1",
                    params![rowid],
                )?;
                conn.execute("DELETE FROM artists WHERE rowid = ?1", params![rowid])?;
                Ok(true)
            } else {
                Ok(false)
            }
        })();

        match result {
            Ok(deleted) => {
                conn.execute("COMMIT", [])?;
                Ok(deleted)
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    fn create_album(&self, album: &Album, artist_ids: &[String]) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM albums WHERE id = ?1)",
                params![&album.id],
                |r| r.get(0),
            )?;
            if exists {
                anyhow::bail!("Album with id '{}' already exists", album.id);
            }

            conn.execute(
                "INSERT INTO albums (id, name, album_type, external_id_upc, label, popularity, release_date, release_date_precision)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    &album.id,
                    &album.name,
                    album.album_type.to_db_str(),
                    &album.external_id_upc,
                    album.label.as_deref().unwrap_or(""),
                    album.popularity,
                    &album.release_date,
                    &album.release_date_precision,
                ],
            )?;

            let album_rowid: i64 = conn.query_row(
                "SELECT rowid FROM albums WHERE id = ?1",
                params![&album.id],
                |r| r.get(0),
            )?;

            for (idx, artist_id) in artist_ids.iter().enumerate() {
                let artist_rowid: i64 = conn
                    .query_row(
                        "SELECT rowid FROM artists WHERE id = ?1",
                        params![artist_id],
                        |r| r.get(0),
                    )
                    .context(format!("Artist '{}' not found", artist_id))?;

                conn.execute(
                    "INSERT INTO artist_albums (artist_rowid, album_rowid, is_appears_on, is_implicit_appears_on, index_in_album)
                     VALUES (?1, ?2, 0, 0, ?3)",
                    params![artist_rowid, album_rowid, idx as i32],
                )?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    fn update_album(&self, album: &Album, artist_ids: Option<&[String]>) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            let album_rowid: i64 = match conn.query_row(
                "SELECT rowid FROM albums WHERE id = ?1",
                params![&album.id],
                |r| r.get(0),
            ) {
                Ok(rowid) => rowid,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    anyhow::bail!("Album with id '{}' not found", album.id);
                }
                Err(e) => return Err(e.into()),
            };

            conn.execute(
                "UPDATE albums SET name = ?1, album_type = ?2, external_id_upc = ?3, label = ?4,
                 popularity = ?5, release_date = ?6, release_date_precision = ?7 WHERE rowid = ?8",
                params![
                    &album.name,
                    album.album_type.to_db_str(),
                    &album.external_id_upc,
                    album.label.as_deref().unwrap_or(""),
                    album.popularity,
                    &album.release_date,
                    &album.release_date_precision,
                    album_rowid,
                ],
            )?;

            if let Some(artist_ids) = artist_ids {
                conn.execute(
                    "DELETE FROM artist_albums WHERE album_rowid = ?1 AND is_appears_on = 0",
                    params![album_rowid],
                )?;

                for (idx, artist_id) in artist_ids.iter().enumerate() {
                    let artist_rowid: i64 = conn
                        .query_row(
                            "SELECT rowid FROM artists WHERE id = ?1",
                            params![artist_id],
                            |r| r.get(0),
                        )
                        .context(format!("Artist '{}' not found", artist_id))?;

                    conn.execute(
                        "INSERT INTO artist_albums (artist_rowid, album_rowid, is_appears_on, is_implicit_appears_on, index_in_album)
                         VALUES (?1, ?2, 0, 0, ?3)",
                        params![artist_rowid, album_rowid, idx as i32],
                    )?;
                }
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    fn delete_album(&self, id: &str) -> Result<bool> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<bool> {
            let album_rowid: Option<i64> =
                match conn.query_row("SELECT rowid FROM albums WHERE id = ?1", params![id], |r| {
                    r.get(0)
                }) {
                    Ok(rowid) => Some(rowid),
                    Err(rusqlite::Error::QueryReturnedNoRows) => None,
                    Err(e) => return Err(e.into()),
                };

            if let Some(rowid) = album_rowid {
                conn.execute(
                    "DELETE FROM track_artists WHERE track_rowid IN (SELECT rowid FROM tracks WHERE album_rowid = ?1)",
                    params![rowid],
                )?;
                conn.execute("DELETE FROM tracks WHERE album_rowid = ?1", params![rowid])?;
                conn.execute(
                    "DELETE FROM artist_albums WHERE album_rowid = ?1",
                    params![rowid],
                )?;
                conn.execute(
                    "DELETE FROM album_images WHERE album_rowid = ?1",
                    params![rowid],
                )?;
                conn.execute("DELETE FROM albums WHERE rowid = ?1", params![rowid])?;
                Ok(true)
            } else {
                Ok(false)
            }
        })();

        match result {
            Ok(deleted) => {
                conn.execute("COMMIT", [])?;
                Ok(deleted)
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    fn create_track(&self, track: &Track, artist_ids: &[String]) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM tracks WHERE id = ?1)",
                params![&track.id],
                |r| r.get(0),
            )?;
            if exists {
                anyhow::bail!("Track with id '{}' already exists", track.id);
            }

            let album_rowid: i64 = conn
                .query_row(
                    "SELECT rowid FROM albums WHERE id = ?1",
                    params![&track.album_id],
                    |r| r.get(0),
                )
                .context(format!("Album '{}' not found", track.album_id))?;

            conn.execute(
                "INSERT INTO tracks (id, name, album_rowid, track_number, external_id_isrc, popularity,
                 disc_number, duration_ms, explicit, language, audio_uri) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    &track.id,
                    &track.name,
                    album_rowid,
                    track.track_number,
                    &track.external_id_isrc,
                    track.popularity,
                    track.disc_number,
                    track.duration_ms,
                    if track.explicit { 1 } else { 0 },
                    &track.language,
                    &track.audio_uri,
                ],
            )?;

            let track_rowid: i64 = conn.query_row(
                "SELECT rowid FROM tracks WHERE id = ?1",
                params![&track.id],
                |r| r.get(0),
            )?;

            for artist_id in artist_ids {
                let artist_rowid: i64 = conn
                    .query_row(
                        "SELECT rowid FROM artists WHERE id = ?1",
                        params![artist_id],
                        |r| r.get(0),
                    )
                    .context(format!("Artist '{}' not found", artist_id))?;

                conn.execute(
                    "INSERT INTO track_artists (track_rowid, artist_rowid, role) VALUES (?1, ?2, 0)",
                    params![track_rowid, artist_rowid],
                )?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    fn update_track(&self, track: &Track, artist_ids: Option<&[String]>) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            let track_rowid: i64 = match conn.query_row(
                "SELECT rowid FROM tracks WHERE id = ?1",
                params![&track.id],
                |r| r.get(0),
            ) {
                Ok(rowid) => rowid,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    anyhow::bail!("Track with id '{}' not found", track.id);
                }
                Err(e) => return Err(e.into()),
            };

            let album_rowid: i64 = conn
                .query_row(
                    "SELECT rowid FROM albums WHERE id = ?1",
                    params![&track.album_id],
                    |r| r.get(0),
                )
                .context(format!("Album '{}' not found", track.album_id))?;

            conn.execute(
                "UPDATE tracks SET name = ?1, album_rowid = ?2, track_number = ?3, external_id_isrc = ?4,
                 popularity = ?5, disc_number = ?6, duration_ms = ?7, explicit = ?8, language = ?9, audio_uri = ?10 WHERE rowid = ?11",
                params![
                    &track.name,
                    album_rowid,
                    track.track_number,
                    &track.external_id_isrc,
                    track.popularity,
                    track.disc_number,
                    track.duration_ms,
                    if track.explicit { 1 } else { 0 },
                    &track.language,
                    &track.audio_uri,
                    track_rowid,
                ],
            )?;

            if let Some(artist_ids) = artist_ids {
                conn.execute(
                    "DELETE FROM track_artists WHERE track_rowid = ?1",
                    params![track_rowid],
                )?;

                for artist_id in artist_ids {
                    let artist_rowid: i64 = conn
                        .query_row(
                            "SELECT rowid FROM artists WHERE id = ?1",
                            params![artist_id],
                            |r| r.get(0),
                        )
                        .context(format!("Artist '{}' not found", artist_id))?;

                    conn.execute(
                        "INSERT INTO track_artists (track_rowid, artist_rowid, role) VALUES (?1, ?2, 0)",
                        params![track_rowid, artist_rowid],
                    )?;
                }
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    fn delete_track(&self, id: &str) -> Result<bool> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<bool> {
            let track_rowid: Option<i64> =
                match conn.query_row("SELECT rowid FROM tracks WHERE id = ?1", params![id], |r| {
                    r.get(0)
                }) {
                    Ok(rowid) => Some(rowid),
                    Err(rusqlite::Error::QueryReturnedNoRows) => None,
                    Err(e) => return Err(e.into()),
                };

            if let Some(rowid) = track_rowid {
                conn.execute(
                    "DELETE FROM track_artists WHERE track_rowid = ?1",
                    params![rowid],
                )?;
                conn.execute("DELETE FROM tracks WHERE rowid = ?1", params![rowid])?;
                Ok(true)
            } else {
                Ok(false)
            }
        })();

        match result {
            Ok(deleted) => {
                conn.execute("COMMIT", [])?;
                Ok(deleted)
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    fn get_items_popularity(
        &self,
        items: &[(String, SearchableContentType)],
    ) -> Result<HashMap<(String, SearchableContentType), i32>> {
        if items.is_empty() {
            return Ok(HashMap::new());
        }

        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let mut result = HashMap::new();

        let mut artist_ids: Vec<&str> = Vec::new();
        let mut album_ids: Vec<&str> = Vec::new();
        let mut track_ids: Vec<&str> = Vec::new();

        for (id, content_type) in items {
            match content_type {
                SearchableContentType::Artist => artist_ids.push(id),
                SearchableContentType::Album => album_ids.push(id),
                SearchableContentType::Track => track_ids.push(id),
            }
        }

        if !artist_ids.is_empty() {
            let placeholders = artist_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!(
                "SELECT id, popularity FROM artists WHERE id IN ({})",
                placeholders
            );
            let mut stmt = conn.prepare(&query)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(artist_ids.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
            })?;
            for row in rows.flatten() {
                let (id, popularity) = row;
                result.insert((id, SearchableContentType::Artist), popularity);
            }
        }

        if !album_ids.is_empty() {
            let placeholders = album_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!(
                "SELECT id, popularity FROM albums WHERE id IN ({})",
                placeholders
            );
            let mut stmt = conn.prepare(&query)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(album_ids.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
            })?;
            for row in rows.flatten() {
                let (id, popularity) = row;
                result.insert((id, SearchableContentType::Album), popularity);
            }
        }

        if !track_ids.is_empty() {
            let placeholders = track_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!(
                "SELECT id, popularity FROM tracks WHERE id IN ({})",
                placeholders
            );
            let mut stmt = conn.prepare(&query)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(track_ids.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
            })?;
            for row in rows.flatten() {
                let (id, popularity) = row;
                result.insert((id, SearchableContentType::Track), popularity);
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrent_reads_no_blocking() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let store =
            SqliteCatalogStore::new(temp_dir.path().join("test.db"), temp_dir.path(), 4).unwrap();

        let handles: Vec<_> = (0..10)
            .map(|_| {
                tokio::spawn({
                    let store = store.clone();
                    async move {
                        for _ in 0..100 {
                            let _ = store.get_artists_count();
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.await.unwrap();
        }
    }
}
