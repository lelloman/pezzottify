//! SQLite-backed catalog store implementation for Spotify schema.
//!
//! This module provides the `SqliteCatalogStore` which reads catalog metadata
//! from the Spotify metadata database dump.

use super::models::*;
use super::schema::CATALOG_VERSIONED_SCHEMAS;
use super::trait_def::{CatalogStore, SearchableContentType, SearchableItem};
use crate::sqlite_persistence::BASE_DB_VERSION;
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

fn migrate_if_needed(conn: &mut Connection) -> Result<()> {
    let db_version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    let latest_version = CATALOG_VERSIONED_SCHEMAS.len() - 1;
    let latest_schema = &CATALOG_VERSIONED_SCHEMAS[latest_version];

    // Check if this is a brand new database (no tables exist)
    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if table_count == 0 {
        // Brand new database - create the latest schema directly
        info!("Creating catalog db schema at version {}", latest_version);
        latest_schema.create(conn)?;
        return Ok(());
    }

    // Handle legacy databases that don't have versioned schema yet (user_version = 0)
    // These should be treated as version 0 and need migration
    let mut current_version = if db_version < BASE_DB_VERSION as i64 {
        // Legacy database - check which columns exist to determine effective version
        let has_album_availability = conn
            .query_row(
                "SELECT 1 FROM pragma_table_info('albums') WHERE name = 'album_availability'",
                [],
                |r| r.get::<_, i32>(0),
            )
            .ok()
            == Some(1);

        if has_album_availability {
            1 // Has v1 columns, treat as v1
        } else {
            0 // Legacy database at v0
        }
    } else {
        (db_version - BASE_DB_VERSION as i64) as usize
    };

    if current_version >= latest_version {
        return Ok(());
    }

    let tx = conn.transaction()?;
    for schema in CATALOG_VERSIONED_SCHEMAS.iter().skip(current_version + 1) {
        if let Some(migration_fn) = schema.migration {
            info!(
                "Migrating catalog db from version {} to {}",
                current_version, schema.version
            );
            migration_fn(&tx)?;
            current_version = schema.version;
        }
    }
    tx.pragma_update(None, "user_version", BASE_DB_VERSION + current_version)?;

    tx.commit()?;
    let _ = conn.query_row(
        "PRAGMA wal_checkpoint(TRUNCATE)",
        [],
        |_: &rusqlite::Row| Ok(()),
    );
    Ok(())
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

        let mut write_conn = Connection::open_with_flags(
            db_path_ref,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .context("Failed to open catalog database")?;

        migrate_if_needed(&mut write_conn)?;

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
            available: row.get::<_, i32>(4)? != 0,
        })
    }

    /// Parse an Album from a row.
    fn parse_album_row(row: &rusqlite::Row) -> rusqlite::Result<Album> {
        let album_type_str: String = row.get(2)?;
        let label: String = row.get(5)?;
        let availability_str: String = row.get(9)?;

        Ok(Album {
            id: row.get(0)?,
            name: row.get(1)?,
            album_type: AlbumType::from_db_str(&album_type_str),
            label: if label.is_empty() { None } else { Some(label) },
            release_date: row.get(7)?,
            release_date_precision: row.get(8)?,
            external_id_upc: row.get(3)?,
            popularity: row.get(6)?,
            album_availability: AlbumAvailability::from_db_str(&availability_str),
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
            "SELECT id, name, followers_total, popularity, artist_available FROM artists WHERE rowid = ?1",
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
                    label, popularity, release_date, release_date_precision, album_availability
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

        let related_artists = self.get_related_artists(id).unwrap_or_default();

        Ok(Some(ResolvedArtist {
            artist,
            related_artists,
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
                    label, popularity, release_date, release_date_precision, album_availability
             FROM albums WHERE rowid = ?1",
        )?;
        let album = album_stmt.query_row(params![album_rowid], Self::parse_album_row)?;

        let mut artists_stmt = conn.prepare_cached(
            "SELECT a.id, a.name, a.followers_total, a.popularity, a.rowid, a.artist_available
             FROM artists a
             INNER JOIN artist_albums aa ON a.rowid = aa.artist_rowid
             WHERE aa.album_rowid = ?1 AND aa.is_appears_on = 0
             ORDER BY aa.index_in_album",
        )?;
        let artists: Vec<Artist> = artists_stmt
            .query_map(params![album_rowid], |row| {
                let artist_rowid: i64 = row.get(4)?;
                let available: i32 = row.get(5)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i32>(3)?,
                    artist_rowid,
                    available != 0,
                ))
            })?
            .filter_map(|r| r.ok())
            .map(
                |(id, name, followers, popularity, artist_rowid, available)| {
                    let genres = Self::get_artist_genres(&conn, artist_rowid).unwrap_or_default();
                    Artist {
                        id,
                        name,
                        genres,
                        followers_total: followers,
                        popularity,
                        available,
                    }
                },
            )
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
                    label, popularity, release_date, release_date_precision, album_availability
             FROM albums WHERE id = ?1",
        )?;
        let album = album_stmt.query_row(params![album_id], Self::parse_album_row)?;

        let mut artists_stmt = conn.prepare_cached(
            "SELECT a.id, a.name, a.followers_total, a.popularity, a.rowid, ta.role, a.artist_available
             FROM artists a
             INNER JOIN track_artists ta ON a.rowid = ta.artist_rowid
             WHERE ta.track_rowid = ?1
             ORDER BY ta.role, a.popularity DESC",
        )?;

        let artists: Vec<TrackArtist> = artists_stmt
            .query_map(params![track_rowid], |row| {
                let artist_rowid: i64 = row.get(4)?;
                let role: Option<i32> = row.get(5)?;
                let available: i32 = row.get(6)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i32>(3)?,
                    artist_rowid,
                    role.unwrap_or(0),
                    available != 0,
                ))
            })?
            .filter_map(|r| r.ok())
            .map(
                |(id, name, followers, popularity, artist_rowid, role, available)| {
                    let genres = Self::get_artist_genres(&conn, artist_rowid).unwrap_or_default();
                    TrackArtist {
                        artist: Artist {
                            id,
                            name,
                            genres,
                            followers_total: followers,
                            popularity,
                            available,
                        },
                        role: ArtistRole::from_db_int(role),
                    }
                },
            )
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
            "SELECT COUNT(*) FROM artist_albums aa
             INNER JOIN albums a ON a.rowid = aa.album_rowid
             WHERE aa.artist_rowid = ?1 AND aa.is_appears_on = 0 AND a.album_type != 'single'",
            params![artist_rowid],
            |row| row.get::<_, i64>(0),
        )? as usize;

        let availability_order = "CASE a.album_availability
            WHEN 'complete' THEN 1
            WHEN 'partial' THEN 2
            WHEN 'missing' THEN 3
        END";

        let order_clause = match sort {
            DiscographySort::Popularity => format!(
                "{}, a.popularity DESC, a.release_date DESC",
                availability_order
            ),
            DiscographySort::ReleaseDate => format!(
                "{}, a.release_date DESC, a.popularity DESC",
                availability_order
            ),
        };

        let query = format!(
            "SELECT a.id, a.name, a.album_type, a.external_id_upc, a.external_id_amgid,
                    a.label, a.popularity, a.release_date, a.release_date_precision, a.album_availability
             FROM albums a
             INNER JOIN artist_albums aa ON a.rowid = aa.album_rowid
             WHERE aa.artist_rowid = ?1 AND aa.is_appears_on = 0 AND a.album_type != 'single'
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

    // =========================================================================
    // Related Artists Enrichment
    // =========================================================================

    /// Get artists needing MusicBrainz ID lookup (status = 0).
    pub fn get_artists_needing_mbid(&self, limit: usize) -> Result<Vec<(String, i64)>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        let mut stmt = conn.prepare_cached(
            "SELECT id, rowid FROM artists WHERE mbid_lookup_status = 0 ORDER BY artist_available DESC, popularity DESC LIMIT ?1",
        )?;

        let results = stmt
            .query_map(params![limit as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Get artists needing related artists fetch (status = 1, has mbid).
    pub fn get_artists_needing_related(&self, limit: usize) -> Result<Vec<(String, String, i64)>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        let mut stmt = conn.prepare_cached(
            "SELECT id, mbid, rowid FROM artists WHERE mbid_lookup_status = 1 AND mbid IS NOT NULL LIMIT ?1",
        )?;

        let results = stmt
            .query_map(params![limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Set MusicBrainz ID for an artist, marking status = 1.
    pub fn set_artist_mbid(&self, artist_id: &str, mbid: &str) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute(
            "UPDATE artists SET mbid = ?1, mbid_lookup_status = 1 WHERE id = ?2",
            params![mbid, artist_id],
        )?;
        Ok(())
    }

    /// Mark artist mbid as not found (status = 2).
    pub fn mark_artist_mbid_not_found(&self, artist_id: &str) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute(
            "UPDATE artists SET mbid_lookup_status = 2 WHERE id = ?1",
            params![artist_id],
        )?;
        Ok(())
    }

    /// Store related artists and mark status = 3.
    pub fn set_related_artists(&self, artist_rowid: i64, related: &[(i64, f64)]) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            // Clear existing relationships
            conn.execute(
                "DELETE FROM related_artists WHERE artist_rowid = ?1",
                params![artist_rowid],
            )?;

            // Insert new relationships
            for (related_rowid, score) in related {
                conn.execute(
                    "INSERT OR IGNORE INTO related_artists (artist_rowid, related_artist_rowid, match_score) VALUES (?1, ?2, ?3)",
                    params![artist_rowid, related_rowid, score],
                )?;
            }

            // Mark as done
            conn.execute(
                "UPDATE artists SET mbid_lookup_status = 3 WHERE rowid = ?1",
                params![artist_rowid],
            )?;

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

    /// Get related artists for an artist, ordered by match score descending.
    pub fn get_related_artists(&self, artist_id: &str) -> Result<Vec<Artist>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        let artist_rowid = match Self::get_artist_rowid(&conn, artist_id)? {
            Some(r) => r,
            None => return Ok(vec![]),
        };

        let mut stmt = conn.prepare_cached(
            "SELECT a.id, a.name, a.followers_total, a.popularity, a.rowid, a.artist_available
             FROM artists a
             INNER JOIN related_artists ra ON a.rowid = ra.related_artist_rowid
             WHERE ra.artist_rowid = ?1
             ORDER BY ra.match_score DESC",
        )?;

        let artists: Vec<Artist> = stmt
            .query_map(params![artist_rowid], |row| {
                let artist_rowid: i64 = row.get(4)?;
                let available: i32 = row.get(5)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i32>(3)?,
                    artist_rowid,
                    available != 0,
                ))
            })?
            .filter_map(|r| r.ok())
            .map(
                |(id, name, followers, popularity, artist_rowid, available)| {
                    let genres = Self::get_artist_genres(&conn, artist_rowid).unwrap_or_default();
                    Artist {
                        id,
                        name,
                        genres,
                        followers_total: followers,
                        popularity,
                        available,
                    }
                },
            )
            .collect();

        Ok(artists)
    }

    /// Look up artist rowid by MusicBrainz ID.
    pub fn get_artist_rowid_by_mbid(&self, mbid: &str) -> Result<Option<i64>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        match conn.query_row(
            "SELECT rowid FROM artists WHERE mbid = ?1",
            params![mbid],
            |r| r.get(0),
        ) {
            Ok(rowid) => Ok(Some(rowid)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
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

        let mut artist_stmt = conn
            .prepare("SELECT id, name, artist_available FROM artists ORDER BY popularity DESC")?;
        let artist_iter = artist_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)? != 0,
            ))
        })?;

        for result in artist_iter {
            let (id, name, is_available) = result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available,
            });
        }
        info!("Loaded {} artists for indexing", items.len());

        let mut album_stmt = conn
            .prepare("SELECT id, name, album_availability FROM albums ORDER BY popularity DESC")?;
        let album_iter = album_stmt.query_map([], |row| {
            let availability: String = row.get(2)?;
            // Album is available if it has at least some content (complete or partial)
            let is_available = availability != "missing";
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                is_available,
            ))
        })?;

        let album_start = items.len();
        for result in album_iter {
            let (id, name, is_available) = result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Album,
                additional_text: vec![],
                is_available,
            });
        }
        info!("Loaded {} albums for indexing", items.len() - album_start);

        let mut track_stmt =
            conn.prepare("SELECT id, name, track_available FROM tracks ORDER BY popularity DESC")?;
        let track_iter = track_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)? != 0,
            ))
        })?;

        let track_start = items.len();
        for result in track_iter {
            let (id, name, is_available) = result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Track,
                additional_text: vec![],
                is_available,
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
                conn.execute(
                    "DELETE FROM related_artists WHERE artist_rowid = ?1 OR related_artist_rowid = ?1",
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
                "INSERT INTO albums (id, name, album_type, external_id_upc, label, popularity, release_date, release_date_precision, album_availability)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    &album.id,
                    &album.name,
                    album.album_type.to_db_str(),
                    &album.external_id_upc,
                    album.label.as_deref().unwrap_or(""),
                    album.popularity,
                    &album.release_date,
                    &album.release_date_precision,
                    album.album_availability.to_db_str(),
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
                 popularity = ?5, release_date = ?6, release_date_precision = ?7, album_availability = ?8 WHERE rowid = ?9",
                params![
                    &album.name,
                    album.album_type.to_db_str(),
                    &album.external_id_upc,
                    album.label.as_deref().unwrap_or(""),
                    album.popularity,
                    &album.release_date,
                    &album.release_date_precision,
                    album.album_availability.to_db_str(),
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

    fn set_track_audio_uri(&self, track_id: &str, audio_uri: &str) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let rows_affected = conn.execute(
            "UPDATE tracks SET audio_uri = ?1, track_available = 1 WHERE id = ?2",
            params![audio_uri, track_id],
        )?;

        if rows_affected == 0 {
            anyhow::bail!("Track with id '{}' not found", track_id);
        }

        Ok(())
    }

    fn recompute_album_availability(&self, album_id: &str) -> Result<AlbumAvailability> {
        let conn = self.write_conn.lock().unwrap();

        // Get album rowid
        let album_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM albums WHERE id = ?1",
                params![album_id],
                |r| r.get(0),
            )
            .context(format!("Album '{}' not found", album_id))?;

        // Count total tracks and available tracks
        // Use COALESCE for the SUM since it returns NULL when no rows match the condition
        let (total_tracks, available_tracks): (i32, i32) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN track_available = 1 THEN 1 ELSE 0 END), 0) FROM tracks WHERE album_rowid = ?1",
            params![album_rowid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        // Determine availability
        let availability = if available_tracks == 0 {
            AlbumAvailability::Missing
        } else if available_tracks == total_tracks {
            AlbumAvailability::Complete
        } else {
            AlbumAvailability::Partial
        };

        // Update album
        conn.execute(
            "UPDATE albums SET album_availability = ?1 WHERE rowid = ?2",
            params![availability.to_db_str(), album_rowid],
        )?;

        Ok(availability)
    }

    fn recompute_artist_availability(&self, artist_id: &str) -> Result<bool> {
        let conn = self.write_conn.lock().unwrap();

        // Get artist rowid
        let artist_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM artists WHERE id = ?1",
                params![artist_id],
                |r| r.get(0),
            )
            .context(format!("Artist '{}' not found", artist_id))?;

        // Check if artist has any non-Missing albums
        let has_available_album: bool = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM albums a
                    JOIN artist_albums aa ON a.rowid = aa.album_rowid
                    WHERE aa.artist_rowid = ?1
                      AND a.album_availability != 'MISSING'
                )",
                params![artist_rowid],
                |r| r.get(0),
            )
            .unwrap_or(false);

        // Update artist availability
        conn.execute(
            "UPDATE artists SET artist_available = ?1 WHERE rowid = ?2",
            params![has_available_album as i32, artist_rowid],
        )?;

        Ok(has_available_album)
    }

    fn get_album_artist_ids(&self, album_id: &str) -> Result<Vec<String>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        // Get album rowid
        let album_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM albums WHERE id = ?1",
                params![album_id],
                |r| r.get(0),
            )
            .context(format!("Album '{}' not found", album_id))?;

        // Get primary artist IDs (not appears_on)
        let mut stmt = conn.prepare_cached(
            "SELECT ar.id FROM artists ar
             JOIN artist_albums aa ON ar.rowid = aa.artist_rowid
             WHERE aa.album_rowid = ?1 AND aa.is_appears_on = 0
             ORDER BY aa.index_in_album ASC",
        )?;

        let artist_ids: Vec<String> = stmt
            .query_map(params![album_rowid], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(artist_ids)
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

    fn get_genres_with_counts(&self) -> Result<Vec<GenreInfo>> {
        let conn = self.get_read_conn();
        let conn = conn.lock().unwrap();

        let mut stmt = conn.prepare_cached(
            "SELECT ag.genre, COUNT(DISTINCT t.rowid) as track_count
             FROM artist_genres ag
             JOIN track_artists ta ON ta.artist_rowid = ag.artist_rowid
             JOIN tracks t ON t.rowid = ta.track_rowid
             WHERE t.track_available = 1
             GROUP BY ag.genre
             HAVING track_count > 0
             ORDER BY track_count DESC",
        )?;

        let genres = stmt
            .query_map([], |row| {
                Ok(GenreInfo {
                    name: row.get(0)?,
                    track_count: row.get::<_, i64>(1)? as usize,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(genres)
    }

    fn get_tracks_by_genre(
        &self,
        genre: &str,
        limit: usize,
        offset: usize,
    ) -> Result<GenreTracksResult> {
        let conn = self.get_read_conn();
        let conn = conn.lock().unwrap();

        // Get total count using EXISTS (much faster than JOIN with DISTINCT)
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tracks t
             WHERE t.track_available = 1
               AND EXISTS (
                 SELECT 1 FROM track_artists ta
                 JOIN artist_genres ag ON ta.artist_rowid = ag.artist_rowid
                 WHERE ta.track_rowid = t.rowid AND ag.genre = ?1
               )",
            params![genre],
            |row| row.get(0),
        )?;

        // Get paginated track IDs using EXISTS (much faster than JOIN with DISTINCT)
        let mut stmt = conn.prepare_cached(
            "SELECT t.id FROM tracks t
             WHERE t.track_available = 1
               AND EXISTS (
                 SELECT 1 FROM track_artists ta
                 JOIN artist_genres ag ON ta.artist_rowid = ag.artist_rowid
                 WHERE ta.track_rowid = t.rowid AND ag.genre = ?1
               )
             ORDER BY t.popularity DESC
             LIMIT ?2 OFFSET ?3",
        )?;

        let track_ids = stmt
            .query_map(params![genre, limit as i64, offset as i64], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let total = total as usize;
        let has_more = offset + track_ids.len() < total;

        Ok(GenreTracksResult {
            track_ids,
            total,
            has_more,
        })
    }

    fn get_random_tracks_by_genre(&self, genre: &str, limit: usize) -> Result<Vec<String>> {
        let conn = self.get_read_conn();
        let conn = conn.lock().unwrap();

        // Use EXISTS for much faster performance than JOIN with DISTINCT
        let mut stmt = conn.prepare_cached(
            "SELECT t.id FROM tracks t
             WHERE t.track_available = 1
               AND EXISTS (
                 SELECT 1 FROM track_artists ta
                 JOIN artist_genres ag ON ta.artist_rowid = ag.artist_rowid
                 WHERE ta.track_rowid = t.rowid AND ag.genre = ?1
               )
             ORDER BY RANDOM()
             LIMIT ?2",
        )?;

        let track_ids = stmt
            .query_map(params![genre, limit as i64], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(track_ids)
    }

    fn find_albums_by_fingerprint(
        &self,
        track_count: i32,
        total_duration_ms: i64,
    ) -> Result<Vec<AlbumFingerprintCandidate>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        // Phase 1: Filter by track count and total duration (±0.1% tolerance)
        let min_duration = total_duration_ms * 999 / 1000;
        let max_duration = total_duration_ms * 1001 / 1000;

        // Query albums with matching fingerprint and get their track durations
        let mut stmt = conn.prepare_cached(
            "SELECT a.rowid, a.id, a.name, a.release_date
             FROM albums a
             WHERE a.track_count = ?1
               AND a.total_duration_ms BETWEEN ?2 AND ?3",
        )?;

        let album_rows: Vec<(i64, String, String, Option<String>)> = stmt
            .query_map(params![track_count, min_duration, max_duration], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        // For each candidate, get the track durations and primary artist
        let mut candidates = Vec::with_capacity(album_rows.len());

        for (album_rowid, album_id, album_name, release_date) in album_rows {
            // Get primary artist name
            let artist_name: String = conn
                .query_row(
                    "SELECT ar.name FROM artists ar
                     JOIN artist_albums aa ON ar.rowid = aa.artist_rowid
                     WHERE aa.album_rowid = ?1 AND aa.is_appears_on = 0
                     ORDER BY aa.index_in_album ASC
                     LIMIT 1",
                    params![album_rowid],
                    |r| r.get(0),
                )
                .unwrap_or_else(|_| "Unknown Artist".to_string());

            // Get track durations ordered by disc and track number
            let mut duration_stmt = conn.prepare_cached(
                "SELECT duration_ms FROM tracks
                 WHERE album_rowid = ?1
                 ORDER BY disc_number ASC, track_number ASC",
            )?;

            let track_durations: Vec<i64> = duration_stmt
                .query_map(params![album_rowid], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();

            let total_duration: i64 = track_durations.iter().sum();

            candidates.push(AlbumFingerprintCandidate {
                id: album_id,
                name: album_name,
                artist_name,
                release_date,
                track_count,
                total_duration_ms: total_duration,
                track_durations,
            });
        }

        Ok(candidates)
    }

    fn get_album_track_durations(&self, album_id: &str) -> Result<Vec<i64>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        let album_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM albums WHERE id = ?1",
                params![album_id],
                |r| r.get(0),
            )
            .context(format!("Album '{}' not found", album_id))?;

        let mut stmt = conn.prepare_cached(
            "SELECT duration_ms FROM tracks
             WHERE album_rowid = ?1
             ORDER BY disc_number ASC, track_number ASC",
        )?;

        let durations: Vec<i64> = stmt
            .query_map(params![album_rowid], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(durations)
    }

    fn update_album_fingerprint(&self, album_id: &str) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();

        // Get album rowid
        let album_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM albums WHERE id = ?1",
                params![album_id],
                |r| r.get(0),
            )
            .context(format!("Album '{}' not found", album_id))?;

        // Compute track_count and total_duration_ms from tracks
        let (track_count, total_duration_ms): (i32, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(duration_ms), 0) FROM tracks WHERE album_rowid = ?1",
            params![album_rowid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        // Update the album fingerprint columns
        conn.execute(
            "UPDATE albums SET track_count = ?1, total_duration_ms = ?2 WHERE rowid = ?3",
            params![track_count, total_duration_ms, album_rowid],
        )?;

        Ok(())
    }

    fn get_artists_needing_mbid(&self, limit: usize) -> Result<Vec<(String, i64)>> {
        SqliteCatalogStore::get_artists_needing_mbid(self, limit)
    }

    fn get_artists_needing_related(&self, limit: usize) -> Result<Vec<(String, String, i64)>> {
        SqliteCatalogStore::get_artists_needing_related(self, limit)
    }

    fn set_artist_mbid(&self, artist_id: &str, mbid: &str) -> Result<()> {
        SqliteCatalogStore::set_artist_mbid(self, artist_id, mbid)
    }

    fn mark_artist_mbid_not_found(&self, artist_id: &str) -> Result<()> {
        SqliteCatalogStore::mark_artist_mbid_not_found(self, artist_id)
    }

    fn set_related_artists(&self, artist_rowid: i64, related: &[(i64, f64)]) -> Result<()> {
        SqliteCatalogStore::set_related_artists(self, artist_rowid, related)
    }

    fn get_related_artists(&self, artist_id: &str) -> Result<Vec<Artist>> {
        SqliteCatalogStore::get_related_artists(self, artist_id)
    }

    fn get_artist_rowid_by_mbid(&self, mbid: &str) -> Result<Option<i64>> {
        SqliteCatalogStore::get_artist_rowid_by_mbid(self, mbid)
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
