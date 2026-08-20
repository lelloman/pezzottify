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
        db_registry: &crate::backup::DbRegistry,
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
        configure_connection(&write_conn)?;

        migrate_if_needed(&mut write_conn)?;

        db_registry.register(db_path_ref.to_path_buf(), &write_conn)?;

        info!("Opened Spotify catalog");

        let mut read_pool = Vec::with_capacity(read_pool_size);
        for _ in 0..read_pool_size {
            let read_conn = Connection::open_with_flags(
                db_path_ref,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_URI
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            configure_connection(&read_conn)?;
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

    fn encode_f32_vector(vector: &[f32]) -> Vec<u8> {
        let mut out = Vec::with_capacity(std::mem::size_of_val(vector));
        for value in vector {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    fn decode_f32_vector(blob: &[u8]) -> Result<Vec<f32>> {
        if !blob.len().is_multiple_of(std::mem::size_of::<f32>()) {
            return Err(anyhow!(
                "invalid float32 vector blob length: {}",
                blob.len()
            ));
        }
        Ok(blob
            .chunks_exact(std::mem::size_of::<f32>())
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect())
    }

    fn vector_norm(vector: &[f32]) -> f64 {
        vector
            .iter()
            .map(|value| f64::from(*value) * f64::from(*value))
            .sum::<f64>()
            .sqrt()
    }

    fn dot_product(left: &[f32], right: &[f32]) -> f32 {
        left.iter()
            .zip(right.iter())
            .map(|(a, b)| a * b)
            .sum::<f32>()
    }

    fn row_to_embedding(
        row: &rusqlite::Row,
        include_vector: bool,
    ) -> rusqlite::Result<EntityEmbedding> {
        let metadata_json: String = row.get("metadata_json")?;
        let model_json: String = row.get("model_json")?;
        let vector = if include_vector {
            let blob: Vec<u8> = row.get("vector_blob")?;
            Some(Self::decode_f32_vector(&blob).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    blob.len(),
                    rusqlite::types::Type::Blob,
                    err.into(),
                )
            })?)
        } else {
            None
        };
        Ok(EntityEmbedding {
            entity_type: row.get("entity_type")?,
            entity_id: row.get("entity_id")?,
            namespace: row.get("namespace")?,
            dim: row.get::<_, i64>("dim")? as usize,
            dtype: row.get("dtype")?,
            vector,
            vector_norm: row.get("vector_norm")?,
            metadata: serde_json::from_str(&metadata_json)
                .unwrap_or(serde_json::Value::Object(Default::default())),
            model: serde_json::from_str(&model_json)
                .unwrap_or(serde_json::Value::Object(Default::default())),
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    /// Compute track availability from an already-fetched audio_uri.
    ///
    /// This avoids acquiring another database connection, preventing deadlocks
    /// when called from within methods that already hold a connection.
    fn availability_from_audio_uri(&self, audio_uri: &Option<String>) -> TrackAvailability {
        match audio_uri {
            Some(uri) if open_media_file_beneath(&self.media_base_path, uri).is_ok() => {
                TrackAvailability::Available
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
        appears_on: bool,
    ) -> Result<Option<ArtistDiscography>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        let artist_rowid = match Self::get_artist_rowid(&conn, id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        let where_clause = if appears_on {
            "aa.artist_rowid = ?1 AND aa.is_appears_on = 1"
        } else {
            "aa.artist_rowid = ?1 AND aa.is_appears_on = 0 AND a.album_type != 'single'"
        };

        let total: usize = conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM artist_albums aa
                 INNER JOIN albums a ON a.rowid = aa.album_rowid
                 WHERE {}",
                where_clause
            ),
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
             WHERE {}
             ORDER BY {}
             LIMIT ?2 OFFSET ?3",
            where_clause, order_clause
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
        self.get_catalog_cardinality_stats()
            .ok()
            .flatten()
            .map(|stats| stats.artists)
            .unwrap_or(0)
    }

    /// Get the number of albums.
    pub fn get_albums_count(&self) -> usize {
        self.get_catalog_cardinality_stats()
            .ok()
            .flatten()
            .map(|stats| stats.albums)
            .unwrap_or(0)
    }

    /// Get the number of tracks.
    pub fn get_tracks_count(&self) -> usize {
        self.get_catalog_cardinality_stats()
            .ok()
            .flatten()
            .map(|stats| stats.tracks)
            .unwrap_or(0)
    }

    pub fn get_catalog_cardinality_stats(&self) -> Result<Option<CatalogCardinalityStats>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let row = conn
            .query_row(
                "SELECT artists_count, albums_count, tracks_count,
                        mutation_version, updated_at
                 FROM catalog_stats WHERE id = 1 AND is_valid = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(artists, albums, tracks, mutation_version, updated_at)| {
            Ok(CatalogCardinalityStats {
                artists: usize::try_from(artists).context("invalid persisted artist count")?,
                albums: usize::try_from(albums).context("invalid persisted album count")?,
                tracks: usize::try_from(tracks).context("invalid persisted track count")?,
                mutation_version,
                updated_at,
            })
        })
        .transpose()
    }

    fn count_table_rows_cancellable(
        &self,
        table: &str,
        covering_index: &str,
        is_cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<i64> {
        // SUM(1), rather than SQLite's optimized COUNT(*), keeps the VM progress
        // handler active while the covering index is scanned. The narrow
        // availability index avoids reading the much larger table payload pages.
        const PROGRESS_DELAY: std::time::Duration = std::time::Duration::from_millis(10);
        if is_cancelled() {
            anyhow::bail!("cancelled");
        }
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let sql = format!(
            "SELECT COALESCE(SUM(1), 0)
             FROM {table} INDEXED BY {covering_index}"
        );
        let progress_is_cancelled = is_cancelled.clone();
        let result = crate::sqlite_persistence::with_progress_handler(
            &conn,
            crate::sqlite_persistence::CANCELLATION_PROGRESS_OPS,
            move || {
                if progress_is_cancelled() {
                    true
                } else {
                    std::thread::sleep(PROGRESS_DELAY);
                    false
                }
            },
            || conn.query_row(&sql, [], |row| row.get::<_, i64>(0)),
        );
        match result {
            Ok(count) => Ok(count),
            Err(_) if is_cancelled() => anyhow::bail!("cancelled"),
            Err(error) => Err(error.into()),
        }
    }

    pub fn rebuild_catalog_cardinality_stats(
        &self,
        is_cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<CatalogCardinalityStats> {
        let expected_mutation_version = {
            let read_conn = self.get_read_conn();
            let conn = read_conn.lock().unwrap();
            conn.query_row(
                "SELECT mutation_version FROM catalog_stats WHERE id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )?
        };

        let artists = self.count_table_rows_cancellable(
            "artists",
            "idx_artists_available",
            is_cancelled.clone(),
        )?;
        let albums = self.count_table_rows_cancellable(
            "albums",
            "idx_albums_availability",
            is_cancelled.clone(),
        )?;
        let tracks = self.count_table_rows_cancellable(
            "tracks",
            "idx_tracks_available",
            is_cancelled.clone(),
        )?;
        if is_cancelled() {
            anyhow::bail!("cancelled");
        }

        let now = Utc::now().timestamp();
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;
        let result = (|| -> Result<()> {
            let changed = conn.execute(
                "UPDATE catalog_stats
                 SET artists_count = ?1, albums_count = ?2, tracks_count = ?3,
                     is_valid = 1, updated_at = ?4
                 WHERE id = 1 AND mutation_version = ?5",
                params![artists, albums, tracks, now, expected_mutation_version],
            )?;
            if changed != 1 {
                anyhow::bail!("catalog changed while counts were being rebuilt; retry the job");
            }
            Ok(())
        })();
        Self::finish_explicit_transaction(&conn, result)?;

        Ok(CatalogCardinalityStats {
            artists: usize::try_from(artists)?,
            albums: usize::try_from(albums)?,
            tracks: usize::try_from(tracks)?,
            mutation_version: expected_mutation_version,
            updated_at: now,
        })
    }

    // =========================================================================
    // Related Artists Enrichment
    // =========================================================================

    /// Get artists needing MusicBrainz ID lookup (status = 0).
    pub fn get_artists_needing_mbid(&self, limit: usize) -> Result<Vec<(String, i64)>> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;
        let result = (|| -> Result<Vec<(String, i64)>> {
            let rowids = Self::claim_artist_enrichment_batch(
                &conn,
                "mbid",
                "a.mbid_lookup_status = 0",
                limit,
            )?;
            let mut results = Vec::with_capacity(rowids.len());
            let mut stmt = conn.prepare_cached("SELECT id FROM artists WHERE rowid = ?1")?;
            for rowid in rowids {
                let id = stmt.query_row(params![rowid], |row| row.get(0))?;
                results.push((id, rowid));
            }
            Ok(results)
        })();
        Self::finish_explicit_transaction(&conn, result)
    }

    /// Get artists needing related artists fetch (status = 1, has mbid).
    pub fn get_artists_needing_related(&self, limit: usize) -> Result<Vec<(String, String, i64)>> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;
        let result = (|| -> Result<Vec<(String, String, i64)>> {
            let rowids = Self::claim_artist_enrichment_batch(
                &conn,
                "related",
                "a.mbid_lookup_status = 1 AND a.mbid IS NOT NULL",
                limit,
            )?;
            let mut results = Vec::with_capacity(rowids.len());
            let mut stmt = conn.prepare_cached("SELECT id, mbid FROM artists WHERE rowid = ?1")?;
            for rowid in rowids {
                let (id, mbid) = stmt.query_row(params![rowid], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                results.push((id, mbid, rowid));
            }
            Ok(results)
        })();
        Self::finish_explicit_transaction(&conn, result)
    }

    fn claim_artist_enrichment_batch(
        conn: &Connection,
        phase: &str,
        eligibility_sql: &str,
        limit: usize,
    ) -> Result<Vec<i64>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE artist_enrichment_queue
             SET status = 'queued', next_attempt_at = ?1, updated_at = ?1,
                 last_error = COALESCE(last_error || '; ', '') || 'claim lease expired'
             WHERE phase = ?2 AND status = 'in_progress' AND updated_at <= ?3",
            params![now, phase, now - ENRICHMENT_CLAIM_LEASE_SECS],
        )?;

        // Existing catalogs are admitted gradually. The correlated primary-key
        // lookup excludes retries and permanent failures without a table-wide
        // queue backfill during migration.
        let admission_sql = format!(
            "INSERT OR IGNORE INTO artist_enrichment_queue
                (artist_rowid, phase, status, attempt_count, next_attempt_at,
                 priority, created_at, updated_at)
             SELECT a.rowid, ?1, 'queued', 0, ?2,
                    a.artist_available * 1000 + a.popularity, ?2, ?2
             FROM artists a
             WHERE {eligibility_sql}
               AND NOT EXISTS (
                   SELECT 1 FROM artist_enrichment_queue q
                   WHERE q.artist_rowid = a.rowid AND q.phase = ?1
               )
             ORDER BY a.artist_available DESC, a.popularity DESC, a.rowid ASC
             LIMIT ?3"
        );
        conn.execute(&admission_sql, params![phase, now, limit as i64])?;

        let mut stmt = conn.prepare_cached(
            "SELECT artist_rowid
             FROM artist_enrichment_queue
             WHERE phase = ?1 AND status = 'queued' AND next_attempt_at <= ?2
             ORDER BY next_attempt_at ASC, priority DESC, artist_rowid ASC
             LIMIT ?3",
        )?;
        let rowids = stmt
            .query_map(params![phase, now, limit as i64], |row| row.get(0))?
            .collect::<Result<Vec<i64>, _>>()?;
        drop(stmt);

        for rowid in &rowids {
            conn.execute(
                "UPDATE artist_enrichment_queue
                 SET status = 'in_progress', attempt_count = attempt_count + 1,
                     last_attempt_at = ?1, updated_at = ?1, last_error = NULL
                 WHERE artist_rowid = ?2 AND phase = ?3 AND status = 'queued'",
                params![now, rowid, phase],
            )?;
        }
        Ok(rowids)
    }

    fn finish_explicit_transaction<T>(conn: &Connection, result: Result<T>) -> Result<T> {
        match result {
            Ok(value) => {
                conn.execute("COMMIT", [])?;
                Ok(value)
            }
            Err(error) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(error)
            }
        }
    }

    fn record_artist_enrichment_failure(
        &self,
        artist_rowid: i64,
        phase: &str,
        error: &str,
    ) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let now = Utc::now().timestamp();
        let attempts: i64 = conn.query_row(
            "SELECT attempt_count FROM artist_enrichment_queue
             WHERE artist_rowid = ?1 AND phase = ?2",
            params![artist_rowid, phase],
            |row| row.get(0),
        )?;

        if attempts >= ENRICHMENT_MAX_ATTEMPTS {
            conn.execute(
                "UPDATE artist_enrichment_queue
                 SET status = 'permanent_failure', next_attempt_at = NULL,
                     last_error = ?1, updated_at = ?2
                 WHERE artist_rowid = ?3 AND phase = ?4",
                params![error, now, artist_rowid, phase],
            )?;
        } else {
            let exponent = u32::try_from(attempts.saturating_sub(1))
                .unwrap_or(0)
                .min(20);
            let delay = ENRICHMENT_RETRY_BASE_SECS
                .saturating_mul(1_i64 << exponent)
                .min(ENRICHMENT_RETRY_MAX_SECS);
            // Stable per-task jitter (0-10%) avoids synchronized retries while
            // preserving deterministic behavior in tests and across restarts.
            let jitter = if delay >= 10 {
                artist_rowid.rem_euclid(delay / 10 + 1)
            } else {
                0
            };
            conn.execute(
                "UPDATE artist_enrichment_queue
                 SET status = 'queued', next_attempt_at = ?1,
                     last_error = ?2, updated_at = ?3
                 WHERE artist_rowid = ?4 AND phase = ?5",
                params![now + delay + jitter, error, now, artist_rowid, phase],
            )?;
        }
        Ok(())
    }

    pub fn record_artist_mbid_failure(&self, artist_rowid: i64, error: &str) -> Result<()> {
        self.record_artist_enrichment_failure(artist_rowid, "mbid", error)
    }

    pub fn record_artist_related_failure(&self, artist_rowid: i64, error: &str) -> Result<()> {
        self.record_artist_enrichment_failure(artist_rowid, "related", error)
    }

    pub fn release_artist_enrichment_claims(&self) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE artist_enrichment_queue
             SET status = 'queued', next_attempt_at = ?1, updated_at = ?1,
                 last_error = 'claim released after cancellation'
             WHERE status = 'in_progress'",
            params![now],
        )?;
        Ok(())
    }

    /// Get MusicBrainz ID for an artist.
    pub fn get_artist_mbid(&self, artist_id: &str) -> Result<Option<String>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let mbid = conn
            .query_row(
                "SELECT mbid FROM artists WHERE id = ?1",
                params![artist_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(mbid)
    }

    /// Set MusicBrainz ID for an artist, marking status = 1.
    pub fn set_artist_mbid(&self, artist_id: &str, mbid: &str) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;
        let result = (|| -> Result<()> {
            let now = Utc::now().timestamp();
            conn.execute(
                "UPDATE artists SET mbid = ?1, mbid_lookup_status = 1 WHERE id = ?2",
                params![mbid, artist_id],
            )?;
            conn.execute(
                "UPDATE artist_enrichment_queue
                 SET status = 'completed', next_attempt_at = NULL,
                     last_error = NULL, updated_at = ?1
                 WHERE artist_rowid = (SELECT rowid FROM artists WHERE id = ?2)
                   AND phase = 'mbid'",
                params![now, artist_id],
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO artist_enrichment_queue
                    (artist_rowid, phase, status, attempt_count, next_attempt_at,
                     priority, created_at, updated_at)
                 SELECT rowid, 'related', 'queued', 0, ?1,
                        artist_available * 1000 + popularity, ?1, ?1
                 FROM artists WHERE id = ?2",
                params![now, artist_id],
            )?;
            Ok(())
        })();
        Self::finish_explicit_transaction(&conn, result)
    }

    /// Mark artist mbid as not found (status = 2).
    pub fn mark_artist_mbid_not_found(&self, artist_id: &str) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;
        let result = (|| -> Result<()> {
            let now = Utc::now().timestamp();
            conn.execute(
                "UPDATE artists SET mbid_lookup_status = 2 WHERE id = ?1",
                params![artist_id],
            )?;
            conn.execute(
                "UPDATE artist_enrichment_queue
                 SET status = 'completed', next_attempt_at = NULL,
                     last_error = 'MusicBrainz ID not found', updated_at = ?1
                 WHERE artist_rowid = (SELECT rowid FROM artists WHERE id = ?2)
                   AND phase = 'mbid'",
                params![now, artist_id],
            )?;
            Ok(())
        })();
        Self::finish_explicit_transaction(&conn, result)
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

            conn.execute(
                "UPDATE artist_enrichment_queue
                 SET status = 'completed', next_attempt_at = NULL,
                     last_error = NULL, updated_at = ?1
                 WHERE artist_rowid = ?2 AND phase = 'related'",
                params![Utc::now().timestamp(), artist_rowid],
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

    /// Look up artist rowids for a batch of MusicBrainz IDs.
    pub fn get_artist_rowids_by_mbids(&self, mbids: &[String]) -> Result<Vec<(String, i64)>> {
        if mbids.is_empty() {
            return Ok(Vec::new());
        }

        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let placeholders = mbids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("SELECT mbid, rowid FROM artists WHERE mbid IN ({placeholders})");
        let mut stmt = conn.prepare(&sql)?;
        let resolved = stmt
            .query_map(params_from_iter(mbids.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(resolved)
    }
}

// =============================================================================
// CatalogStore Trait Implementation
// =============================================================================
