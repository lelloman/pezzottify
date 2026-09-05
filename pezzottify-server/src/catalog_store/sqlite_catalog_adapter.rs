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
        self.get_track(id)
            .map(|track| track.map(|track| serde_json::to_value(track).unwrap()))
    }
    fn get_track(&self, id: &str) -> Result<Option<Track>> {
        let connection = self.get_read_conn();
        let connection = connection.lock().unwrap();
        Self::get_track_inner(&connection, id)
    }
    fn media_root(&self) -> PathBuf {
        self.media_base_path.clone()
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
        appears_on: bool,
    ) -> Result<Option<ArtistDiscography>> {
        SqliteCatalogStore::get_discography(self, id, limit, offset, sort, appears_on)
    }

    fn get_album_image_url(&self, album_id: &str) -> Result<Option<ImageUrl>> {
        SqliteCatalogStore::get_album_image_url(self, album_id)
    }

    fn get_artist_image_url(&self, artist_id: &str) -> Result<Option<ImageUrl>> {
        SqliteCatalogStore::get_artist_image_url(self, artist_id)
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

    fn get_catalog_cardinality_stats(&self) -> Result<Option<CatalogCardinalityStats>> {
        SqliteCatalogStore::get_catalog_cardinality_stats(self)
    }

    fn rebuild_catalog_cardinality_stats(
        &self,
        is_cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<CatalogCardinalityStats> {
        SqliteCatalogStore::rebuild_catalog_cardinality_stats(self, is_cancelled)
    }

    fn refresh_availability_and_stats(&self) -> Result<AvailabilityRefreshResult> {
        self.refresh_availability_and_stats_with_cancel(&|| false)
    }

    fn refresh_availability_and_stats_with_cancel(
        &self,
        is_cancelled: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<AvailabilityRefreshResult> {
        self.apply_media_observations(&[], is_cancelled)
    }

    fn apply_media_observations(
        &self,
        observations: &[(String, Option<String>, bool)],
        is_cancelled: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<AvailabilityRefreshResult> {
        const BATCH_SIZE: i64 = 1000;
        let refresh_started = Instant::now();
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<AvailabilityRefreshResult> {
            if is_cancelled() {
                anyhow::bail!("cancelled");
            }
            let mut tracks_updated = 0usize;
            let mut albums_updated = 0usize;
            let mut artists_updated = 0usize;
            let mut track_updates = Vec::new();
            let mut album_updates = Vec::new();
            let mut artist_updates = Vec::new();

            for (id, expected_uri, available) in observations {
                if is_cancelled() {
                    anyhow::bail!("cancelled");
                }
                let changed = conn.execute("UPDATE tracks SET track_available = ?1 WHERE id = ?2 AND audio_uri IS ?3 AND track_available != ?1", params![*available as i32, id, expected_uri])?;
                if changed > 0 {
                    tracks_updated += changed;
                    track_updates.push(AvailabilityItemUpdate {
                        id: id.clone(),
                        available: *available,
                    });
                }
            }

            // Album and artist availability are derived from track flags by the
            // normal mutation paths. Recompute them only when filesystem
            // verification actually repaired a track. With no repairs, these
            // full-table derivations cannot produce a different result and only
            // evict useful database and filesystem cache.
            let derived_started = Instant::now();
            if tracks_updated > 0 {
                let mut last_album_rowid = 0i64;
                loop {
                    if is_cancelled() {
                        anyhow::bail!("cancelled");
                    }

                    let mut albums_stmt = conn.prepare_cached(
                    "SELECT a.rowid, a.id,
                        CASE
                            WHEN COALESCE(t.available_tracks, 0) = 0 THEN 'missing'
                            WHEN COALESCE(t.available_tracks, 0) = COALESCE(t.total_tracks, 0)
                                 AND COALESCE(t.total_tracks, 0) > 0 THEN 'complete'
                            ELSE 'partial'
                        END AS computed_availability
                 FROM albums a
                 LEFT JOIN (
                     SELECT album_rowid,
                            COUNT(*) AS total_tracks,
                            COALESCE(SUM(CASE WHEN track_available = 1 THEN 1 ELSE 0 END), 0) AS available_tracks
                     FROM tracks
                     GROUP BY album_rowid
                 ) t ON t.album_rowid = a.rowid
                 WHERE a.rowid > ?1
                   AND a.album_availability !=
                        CASE
                            WHEN COALESCE(t.available_tracks, 0) = 0 THEN 'missing'
                            WHEN COALESCE(t.available_tracks, 0) = COALESCE(t.total_tracks, 0)
                                 AND COALESCE(t.total_tracks, 0) > 0 THEN 'complete'
                            ELSE 'partial'
                        END
                 ORDER BY a.rowid
                 LIMIT ?2",
                )?;

                    let mut rows = albums_stmt.query(params![last_album_rowid, BATCH_SIZE])?;
                    let mut pending_updates: Vec<(i64, String, String)> = Vec::new();
                    let mut batch_last_rowid = last_album_rowid;
                    while let Some(row) = rows.next()? {
                        if is_cancelled() {
                            anyhow::bail!("cancelled");
                        }
                        let album_rowid: i64 = row.get(0)?;
                        let album_id: String = row.get(1)?;
                        let computed_availability: String = row.get(2)?;
                        pending_updates.push((album_rowid, album_id, computed_availability));
                        batch_last_rowid = album_rowid;
                    }
                    drop(rows);
                    drop(albums_stmt);

                    if batch_last_rowid == last_album_rowid {
                        break;
                    }

                    for (album_rowid, album_id, computed_availability) in pending_updates {
                        if is_cancelled() {
                            anyhow::bail!("cancelled");
                        }
                        conn.execute(
                            "UPDATE albums SET album_availability = ?1 WHERE rowid = ?2",
                            params![computed_availability, album_rowid],
                        )?;
                        albums_updated += 1;
                        album_updates.push(AvailabilityItemUpdate {
                            id: album_id,
                            available: computed_availability != "missing",
                        });
                    }
                    last_album_rowid = batch_last_rowid;
                }

                // Recompute artist availability from credited available tracks.
                let mut last_artist_rowid = 0i64;
                loop {
                    if is_cancelled() {
                        anyhow::bail!("cancelled");
                    }

                    let mut artists_stmt = conn.prepare_cached(
                        "SELECT a.rowid, a.id,
                        CASE WHEN EXISTS (
                            SELECT 1
                            FROM track_artists ta
                            JOIN tracks t ON t.rowid = ta.track_rowid
                            WHERE ta.artist_rowid = a.rowid
                              AND t.track_available = 1
                        ) THEN 1 ELSE 0 END AS computed_available
                 FROM artists a
                 WHERE a.rowid > ?1
                   AND a.artist_available !=
                        CASE WHEN EXISTS (
                            SELECT 1
                            FROM track_artists ta
                            JOIN tracks t ON t.rowid = ta.track_rowid
                            WHERE ta.artist_rowid = a.rowid
                              AND t.track_available = 1
                        ) THEN 1 ELSE 0 END
                 ORDER BY a.rowid
                 LIMIT ?2",
                    )?;

                    let mut rows = artists_stmt.query(params![last_artist_rowid, BATCH_SIZE])?;
                    let mut pending_updates: Vec<(i64, String, i32)> = Vec::new();
                    let mut batch_last_rowid = last_artist_rowid;
                    while let Some(row) = rows.next()? {
                        if is_cancelled() {
                            anyhow::bail!("cancelled");
                        }
                        let artist_rowid: i64 = row.get(0)?;
                        let artist_id: String = row.get(1)?;
                        let computed_available: i32 = row.get(2)?;
                        pending_updates.push((artist_rowid, artist_id, computed_available));
                        batch_last_rowid = artist_rowid;
                    }
                    drop(rows);
                    drop(artists_stmt);

                    if batch_last_rowid == last_artist_rowid {
                        break;
                    }

                    for (artist_rowid, artist_id, computed_available) in pending_updates {
                        if is_cancelled() {
                            anyhow::bail!("cancelled");
                        }
                        conn.execute(
                            "UPDATE artists SET artist_available = ?1 WHERE rowid = ?2",
                            params![computed_available, artist_rowid],
                        )?;
                        artists_updated += 1;
                        artist_updates.push(AvailabilityItemUpdate {
                            id: artist_id,
                            available: computed_available == 1,
                        });
                    }
                    last_artist_rowid = batch_last_rowid;
                }
            }
            info!(
                elapsed_ms = derived_started.elapsed().as_millis() as u64,
                skipped = tracks_updated == 0,
                albums_updated,
                artists_updated,
                "Catalog availability derived-state reconciliation completed"
            );

            // Compute aggregate stats.
            if is_cancelled() {
                anyhow::bail!("cancelled");
            }
            let stats_started = Instant::now();
            let stats: Option<(i64, i64, i64, i64, i64, i64)> = conn
                .query_row(
                    "SELECT
                         artists_count,
                         (SELECT COUNT(*) FROM artists INDEXED BY idx_artists_available
                          WHERE artist_available = 1),
                         albums_count,
                         (SELECT COUNT(*) FROM albums INDEXED BY idx_albums_availability
                          WHERE album_availability = 'complete') +
                         (SELECT COUNT(*) FROM albums INDEXED BY idx_albums_availability
                          WHERE album_availability = 'partial'),
                         tracks_count,
                         (SELECT COUNT(*) FROM tracks INDEXED BY idx_tracks_available
                          WHERE track_available = 1)
                     FROM catalog_stats
                     WHERE id = 1 AND is_valid = 1",
                    [],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get(1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            r.get(5)?,
                        ))
                    },
                )
                .optional()?;
            let (
                artists_total,
                artists_available,
                albums_total,
                albums_available,
                tracks_total,
                tracks_available,
            ) = stats.context(
                "catalog cardinality stats are not initialized; run catalog_cardinality_stats",
            )?;
            info!(
                elapsed_ms = stats_started.elapsed().as_millis() as u64,
                "Catalog availability aggregate statistics completed"
            );

            let tracks_total = tracks_total.max(0) as usize;
            let tracks_available = tracks_available.max(0) as usize;
            let albums_total = albums_total.max(0) as usize;
            let albums_available = albums_available.max(0) as usize;
            let artists_total = artists_total.max(0) as usize;
            let artists_available = artists_available.max(0) as usize;

            Ok(AvailabilityRefreshResult {
                stats: CatalogAvailabilityStats {
                    artists: AvailabilityCount {
                        total: artists_total,
                        available: artists_available,
                        unavailable: artists_total.saturating_sub(artists_available),
                    },
                    albums: AvailabilityCount {
                        total: albums_total,
                        available: albums_available,
                        unavailable: albums_total.saturating_sub(albums_available),
                    },
                    tracks: AvailabilityCount {
                        total: tracks_total,
                        available: tracks_available,
                        unavailable: tracks_total.saturating_sub(tracks_available),
                    },
                },
                repaired: AvailabilityRepairSummary {
                    tracks_updated,
                    albums_updated,
                    artists_updated,
                },
                track_updates,
                album_updates,
                artist_updates,
            })
        })();

        match result {
            Ok(stats) => {
                conn.execute("COMMIT", [])?;
                info!(
                    elapsed_ms = refresh_started.elapsed().as_millis() as u64,
                    "Catalog availability refresh committed"
                );
                Ok(stats)
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    fn get_searchable_content(&self) -> Result<Vec<SearchableItem>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        info!("Indexing all catalog content");
        let mut items = Vec::new();

        let mut artist_stmt = conn.prepare(
            "SELECT a.id, a.name, a.artist_available,
                    COALESCE((SELECT group_concat(ag.genre, char(31))
                              FROM artist_genres ag WHERE ag.artist_rowid = a.rowid), '')
             FROM artists a ORDER BY a.popularity DESC",
        )?;
        let artist_iter = artist_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)? != 0,
                row.get::<_, String>(3)?,
            ))
        })?;

        for result in artist_iter {
            let (id, name, is_available, genres) = result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Artist,
                additional_text: genres
                    .split('\u{1f}')
                    .filter(|genre| !genre.is_empty())
                    .map(|genre| format!("extra:{genre}"))
                    .collect(),
                is_available,
            });
        }
        info!("Loaded {} artists for indexing", items.len());

        let mut album_stmt = conn.prepare(
            "SELECT al.id, al.name, al.album_availability,
                    COALESCE((SELECT group_concat(ar.name, char(31))
                              FROM artist_albums aa
                              JOIN artists ar ON ar.rowid = aa.artist_rowid
                              WHERE aa.album_rowid = al.rowid), '')
             FROM albums al ORDER BY al.popularity DESC",
        )?;
        let album_iter = album_stmt.query_map([], |row| {
            let availability: String = row.get(2)?;
            // Album is available if it has at least some content (complete or partial)
            let is_available = availability != "missing";
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                is_available,
                row.get::<_, String>(3)?,
            ))
        })?;

        let album_start = items.len();
        for result in album_iter {
            let (id, name, is_available, artists) = result?;
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Album,
                additional_text: artists
                    .split('\u{1f}')
                    .filter(|artist| !artist.is_empty())
                    .map(|artist| format!("artist:{artist}"))
                    .collect(),
                is_available,
            });
        }
        info!("Loaded {} albums for indexing", items.len() - album_start);

        let mut track_stmt = conn.prepare(
            "SELECT t.id, t.name, t.track_available, al.name,
                    COALESCE((SELECT group_concat(ar.name, char(31))
                              FROM track_artists ta
                              JOIN artists ar ON ar.rowid = ta.artist_rowid
                              WHERE ta.track_rowid = t.rowid), '')
             FROM tracks t JOIN albums al ON al.rowid = t.album_rowid
             ORDER BY t.popularity DESC",
        )?;
        let track_iter = track_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)? != 0,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;

        let track_start = items.len();
        for result in track_iter {
            let (id, name, is_available, album, artists) = result?;
            let mut additional_text: Vec<String> = artists
                .split('\u{1f}')
                .filter(|artist| !artist.is_empty())
                .map(|artist| format!("artist:{artist}"))
                .collect();
            additional_text.push(format!("album:{album}"));
            items.push(SearchableItem {
                id,
                name,
                content_type: SearchableContentType::Track,
                additional_text,
                is_available,
            });
        }
        info!("Loaded {} tracks for indexing", items.len() - track_start);

        info!("Total searchable items: {}", items.len());
        Ok(items)
    }

    fn get_searchable_content_page(
        &self,
        content_type: SearchableContentType,
        after_rowid: i64,
        limit: usize,
    ) -> Result<Vec<(i64, SearchableItem)>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);

        match content_type {
            SearchableContentType::Artist => {
                let mut stmt = conn.prepare(
                    "SELECT a.rowid, a.id, a.name, a.artist_available,
                            COALESCE((SELECT group_concat(ag.genre, char(31))
                                      FROM artist_genres ag WHERE ag.artist_rowid = a.rowid), '')
                     FROM artists a WHERE a.rowid > ?1 ORDER BY a.rowid LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![after_rowid, limit], |row| {
                    let genres: String = row.get(4)?;
                    Ok((
                        row.get(0)?,
                        SearchableItem {
                            id: row.get(1)?,
                            name: row.get(2)?,
                            content_type,
                            additional_text: genres
                                .split('\u{1f}')
                                .filter(|genre| !genre.is_empty())
                                .map(|genre| format!("extra:{genre}"))
                                .collect(),
                            is_available: row.get::<_, i32>(3)? != 0,
                        },
                    ))
                })?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
            SearchableContentType::Album => {
                let mut stmt = conn.prepare(
                    "SELECT al.rowid, al.id, al.name, al.album_availability,
                            COALESCE((SELECT group_concat(ar.name, char(31))
                                      FROM artist_albums aa
                                      JOIN artists ar ON ar.rowid = aa.artist_rowid
                                      WHERE aa.album_rowid = al.rowid), '')
                     FROM albums al WHERE al.rowid > ?1 ORDER BY al.rowid LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![after_rowid, limit], |row| {
                    let availability: String = row.get(3)?;
                    let artists: String = row.get(4)?;
                    Ok((
                        row.get(0)?,
                        SearchableItem {
                            id: row.get(1)?,
                            name: row.get(2)?,
                            content_type,
                            additional_text: artists
                                .split('\u{1f}')
                                .filter(|artist| !artist.is_empty())
                                .map(|artist| format!("artist:{artist}"))
                                .collect(),
                            is_available: availability != "missing",
                        },
                    ))
                })?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
            SearchableContentType::Track => {
                let mut stmt = conn.prepare(
                    "SELECT t.rowid, t.id, t.name, t.track_available, al.name,
                            COALESCE((SELECT group_concat(ar.name, char(31))
                                      FROM track_artists ta
                                      JOIN artists ar ON ar.rowid = ta.artist_rowid
                                      WHERE ta.track_rowid = t.rowid), '')
                     FROM tracks t JOIN albums al ON al.rowid = t.album_rowid
                     WHERE t.rowid > ?1 ORDER BY t.rowid LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![after_rowid, limit], |row| {
                    let album: String = row.get(4)?;
                    let artists: String = row.get(5)?;
                    let mut additional_text: Vec<String> = artists
                        .split('\u{1f}')
                        .filter(|artist| !artist.is_empty())
                        .map(|artist| format!("artist:{artist}"))
                        .collect();
                    additional_text.push(format!("album:{album}"));
                    Ok((
                        row.get(0)?,
                        SearchableItem {
                            id: row.get(1)?,
                            name: row.get(2)?,
                            content_type,
                            additional_text,
                            is_available: row.get::<_, i32>(3)? != 0,
                        },
                    ))
                })?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
        }
    }

    fn get_available_searchable_content_page(
        &self,
        content_type: SearchableContentType,
        after_rowid: i64,
        limit: usize,
    ) -> Result<Vec<(i64, SearchableItem)>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);

        match content_type {
            SearchableContentType::Artist => {
                let mut stmt = conn.prepare(
                    "SELECT a.rowid, a.id, a.name,
                            COALESCE((SELECT group_concat(ag.genre, char(31))
                                      FROM artist_genres ag WHERE ag.artist_rowid = a.rowid), '')
                     FROM artists a INDEXED BY idx_artists_available
                     WHERE a.artist_available = 1 AND a.rowid > ?1
                     ORDER BY a.rowid LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![after_rowid, limit], |row| {
                    let genres: String = row.get(3)?;
                    Ok((
                        row.get(0)?,
                        SearchableItem {
                            id: row.get(1)?,
                            name: row.get(2)?,
                            content_type,
                            additional_text: genres
                                .split('\u{1f}')
                                .filter(|genre| !genre.is_empty())
                                .map(|genre| format!("extra:{genre}"))
                                .collect(),
                            is_available: true,
                        },
                    ))
                })?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
            SearchableContentType::Album => {
                let mut stmt = conn.prepare(
                    "SELECT al.rowid, al.id, al.name,
                            COALESCE((SELECT group_concat(ar.name, char(31))
                                      FROM artist_albums aa
                                      JOIN artists ar ON ar.rowid = aa.artist_rowid
                                      WHERE aa.album_rowid = al.rowid), '')
                     FROM albums al INDEXED BY idx_albums_availability
                     WHERE al.album_availability IN ('complete', 'partial') AND al.rowid > ?1
                     ORDER BY al.rowid LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![after_rowid, limit], |row| {
                    let artists: String = row.get(3)?;
                    Ok((
                        row.get(0)?,
                        SearchableItem {
                            id: row.get(1)?,
                            name: row.get(2)?,
                            content_type,
                            additional_text: artists
                                .split('\u{1f}')
                                .filter(|artist| !artist.is_empty())
                                .map(|artist| format!("artist:{artist}"))
                                .collect(),
                            is_available: true,
                        },
                    ))
                })?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
            SearchableContentType::Track => {
                let mut stmt = conn.prepare(
                    "SELECT t.rowid, t.id, t.name, al.name,
                            COALESCE((SELECT group_concat(ar.name, char(31))
                                      FROM track_artists ta
                                      JOIN artists ar ON ar.rowid = ta.artist_rowid
                                      WHERE ta.track_rowid = t.rowid), '')
                     FROM tracks t INDEXED BY idx_tracks_available
                     JOIN albums al ON al.rowid = t.album_rowid
                     WHERE t.track_available = 1 AND t.rowid > ?1
                     ORDER BY t.rowid LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![after_rowid, limit], |row| {
                    let album: String = row.get(3)?;
                    let artists: String = row.get(4)?;
                    let mut additional_text: Vec<String> = artists
                        .split('\u{1f}')
                        .filter(|artist| !artist.is_empty())
                        .map(|artist| format!("artist:{artist}"))
                        .collect();
                    additional_text.push(format!("album:{album}"));
                    Ok((
                        row.get(0)?,
                        SearchableItem {
                            id: row.get(1)?,
                            name: row.get(2)?,
                            content_type,
                            additional_text,
                            is_available: true,
                        },
                    ))
                })?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
        }
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

    fn list_available_track_ids_with_audio_uri(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(String, String)>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, audio_uri FROM tracks \
             WHERE track_available = 1 AND audio_uri IS NOT NULL \
             LIMIT ?1 OFFSET ?2",
        )?;
        let pairs = stmt
            .query_map(params![limit as i64, offset as i64], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<Result<Vec<(String, String)>, _>>()?;
        Ok(pairs)
    }

    fn list_available_tracks_missing_embeddings(
        &self,
        namespaces: &[String],
        limit: usize,
    ) -> Result<Vec<(String, String)>> {
        if namespaces.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let missing_predicates = namespaces
            .iter()
            .map(|_| {
                "NOT EXISTS (
                    SELECT 1 FROM entity_embeddings e
                    WHERE e.entity_type = 'track'
                      AND e.entity_id = t.id
                      AND e.namespace = ?
                )"
            })
            .collect::<Vec<_>>()
            .join(" OR ");
        let sql = format!(
            "SELECT t.id, t.audio_uri
             FROM tracks t
             WHERE t.track_available = 1
               AND t.audio_uri IS NOT NULL
               AND ({missing_predicates})
             ORDER BY t.id
             LIMIT ?"
        );

        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let mut stmt = conn.prepare(&sql)?;
        let mut params = namespaces
            .iter()
            .map(|namespace| Value::Text(namespace.clone()))
            .collect::<Vec<_>>();
        params.push(Value::Integer(limit as i64));
        let pairs = stmt
            .query_map(params_from_iter(params), |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<Vec<(String, String)>, _>>()?;
        Ok(pairs)
    }

    fn get_track_embedding_coverage(
        &self,
        namespaces: &[String],
    ) -> Result<super::TrackEmbeddingCoverage> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let available_tracks: usize = conn.query_row(
            "SELECT COUNT(*)
             FROM tracks
             WHERE track_available = 1
               AND audio_uri IS NOT NULL",
            [],
            |r| r.get(0),
        )?;

        if namespaces.is_empty() {
            return Ok(super::TrackEmbeddingCoverage {
                available_tracks,
                fully_embedded_tracks: available_tracks,
                tracks_missing_any_embedding: 0,
                namespaces: Vec::new(),
            });
        }

        let mut namespace_stats = Vec::with_capacity(namespaces.len());
        for namespace in namespaces {
            let embedded_tracks: usize = conn.query_row(
                "SELECT COUNT(DISTINCT e.entity_id)
                 FROM entity_embeddings e
                 JOIN tracks t ON t.id = e.entity_id
                 WHERE e.entity_type = 'track'
                   AND e.namespace = ?1
                   AND t.track_available = 1
                   AND t.audio_uri IS NOT NULL",
                params![namespace],
                |r| r.get(0),
            )?;
            namespace_stats.push(super::TrackEmbeddingNamespaceCoverage {
                namespace: namespace.clone(),
                embedded_tracks,
                missing_tracks: available_tracks.saturating_sub(embedded_tracks),
            });
        }

        let missing_predicates = namespaces
            .iter()
            .map(|_| {
                "NOT EXISTS (
                    SELECT 1 FROM entity_embeddings e
                    WHERE e.entity_type = 'track'
                      AND e.entity_id = t.id
                      AND e.namespace = ?
                )"
            })
            .collect::<Vec<_>>()
            .join(" OR ");
        let fully_sql = format!(
            "SELECT COUNT(*)
             FROM tracks t
             WHERE t.track_available = 1
               AND t.audio_uri IS NOT NULL
               AND NOT ({missing_predicates})"
        );
        let params = namespaces
            .iter()
            .map(|namespace| Value::Text(namespace.clone()))
            .collect::<Vec<_>>();
        let fully_embedded_tracks: usize =
            conn.query_row(&fully_sql, params_from_iter(params), |r| r.get(0))?;

        Ok(super::TrackEmbeddingCoverage {
            available_tracks,
            fully_embedded_tracks,
            tracks_missing_any_embedding: available_tracks.saturating_sub(fully_embedded_tracks),
            namespaces: namespace_stats,
        })
    }

    fn list_complete_album_tracklists_page(
        &self,
        after_album_rowid: Option<i64>,
        limit: usize,
    ) -> Result<Vec<AlbumTracklist>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        // Keep the store boundary safe even if a caller supplies an accidental
        // unbounded value. Jobs can choose smaller pages but never larger ones.
        let limit = limit.min(MAX_ALBUM_TRACKLIST_PAGE_SIZE) as i64;

        let (sql, query_params) = if let Some(after_album_rowid) = after_album_rowid {
            (
                "WITH candidates AS MATERIALIZED (
                    SELECT rowid, id
                    FROM albums INDEXED BY idx_albums_availability
                    WHERE album_availability = 'complete'
                      AND rowid > ?1
                      AND EXISTS (
                          SELECT 1 FROM tracks t2 WHERE t2.album_rowid = albums.rowid
                      )
                    ORDER BY rowid
                    LIMIT ?2
                 )
                 SELECT c.rowid, c.id, t.id, t.audio_uri
                 FROM candidates c
                 JOIN tracks t ON t.album_rowid = c.rowid
                 ORDER BY c.rowid, t.disc_number, t.track_number, t.id",
                vec![Value::Integer(after_album_rowid), Value::Integer(limit)],
            )
        } else {
            (
                "WITH candidates AS MATERIALIZED (
                    SELECT rowid, id
                    FROM albums INDEXED BY idx_albums_availability
                    WHERE album_availability = 'complete'
                      AND EXISTS (
                          SELECT 1 FROM tracks t2 WHERE t2.album_rowid = albums.rowid
                      )
                    ORDER BY rowid
                    LIMIT ?1
                 )
                 SELECT c.rowid, c.id, t.id, t.audio_uri
                 FROM candidates c
                 JOIN tracks t ON t.album_rowid = c.rowid
                 ORDER BY c.rowid, t.disc_number, t.track_number, t.id",
                vec![Value::Integer(limit)],
            )
        };

        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let mut stmt = conn.prepare(sql)?;

        let mut rows = stmt.query(params_from_iter(query_params))?;
        let mut albums = Vec::<AlbumTracklist>::new();
        while let Some(row) = rows.next()? {
            let album_rowid: i64 = row.get(0)?;
            let album_id: String = row.get(1)?;
            let track = AlbumTrackRef {
                track_id: row.get(2)?,
                audio_uri: row.get(3)?,
            };
            if let Some(album) = albums.last_mut() {
                if album.album_id == album_id {
                    album.tracks.push(track);
                    continue;
                }
            }
            albums.push(AlbumTracklist {
                album_rowid,
                album_id,
                tracks: vec![track],
            });
        }
        Ok(albums)
    }

    fn get_album_embedding_coverage(
        &self,
        namespaces: &[String],
        _media_path: &Path,
    ) -> Result<super::AlbumEmbeddingCoverage> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let complete_local_albums: usize = conn.query_row(
            "SELECT COUNT(*)
             FROM albums
             WHERE album_availability = 'complete'",
            [],
            |r| r.get(0),
        )?;

        if namespaces.is_empty() || complete_local_albums == 0 {
            return Ok(super::AlbumEmbeddingCoverage {
                complete_local_albums,
                namespaces: namespaces
                    .iter()
                    .map(|namespace| super::AlbumEmbeddingNamespaceCoverage {
                        namespace: namespace.clone(),
                        embedded_albums: 0,
                        missing_albums: complete_local_albums,
                    })
                    .collect(),
            });
        }

        let sql = "SELECT COUNT(DISTINCT entity_id)
             FROM entity_embeddings e
             JOIN albums a ON a.id = e.entity_id
             WHERE e.entity_type = 'album'
               AND e.namespace = ?1
               AND a.album_availability = 'complete'";

        let mut namespace_stats = Vec::with_capacity(namespaces.len());
        for namespace in namespaces {
            let embedded_albums: usize = conn.query_row(sql, params![namespace], |r| r.get(0))?;
            namespace_stats.push(super::AlbumEmbeddingNamespaceCoverage {
                namespace: namespace.clone(),
                embedded_albums,
                missing_albums: complete_local_albums.saturating_sub(embedded_albums),
            });
        }

        Ok(super::AlbumEmbeddingCoverage {
            complete_local_albums,
            namespaces: namespace_stats,
        })
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
                return Err(CatalogMutationError::AlreadyExists {
                    entity: "Artist",
                    id: artist.id.clone(),
                }
                .into());
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
                    return Err(CatalogMutationError::NotFound {
                        entity: "Artist",
                        id: artist.id.clone(),
                    }
                    .into());
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
                return Err(CatalogMutationError::AlreadyExists {
                    entity: "Album",
                    id: album.id.clone(),
                }
                .into());
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
                    album.release_date.as_deref().unwrap_or(""),
                    album.release_date_precision.as_deref().unwrap_or(""),
                    album.album_availability.to_db_str(),
                ],
            )?;

            let album_rowid: i64 = conn.query_row(
                "SELECT rowid FROM albums WHERE id = ?1",
                params![&album.id],
                |r| r.get(0),
            )?;

            for (idx, artist_id) in artist_ids.iter().enumerate() {
                let artist_rowid: i64 = match conn.query_row(
                    "SELECT rowid FROM artists WHERE id = ?1",
                    params![artist_id],
                    |r| r.get(0),
                ) {
                    Ok(rowid) => rowid,
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        return Err(CatalogMutationError::InvalidReference {
                            entity: "Artist",
                            id: artist_id.clone(),
                        }
                        .into());
                    }
                    Err(error) => return Err(error.into()),
                };

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

    fn update_album_metadata(
        &self,
        album_id: &str,
        metadata: &AlbumMetadataUpdate,
        artist_ids: Option<&[String]>,
    ) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            let album_rowid: i64 = match conn.query_row(
                "SELECT rowid FROM albums WHERE id = ?1",
                params![album_id],
                |r| r.get(0),
            ) {
                Ok(rowid) => rowid,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(CatalogMutationError::NotFound {
                        entity: "Album",
                        id: album_id.to_owned(),
                    }
                    .into());
                }
                Err(e) => return Err(e.into()),
            };

            conn.execute(
                "UPDATE albums SET name = ?1, album_type = ?2, external_id_upc = ?3, label = ?4,
                 popularity = ?5, release_date = ?6, release_date_precision = ?7 WHERE rowid = ?8",
                params![
                    &metadata.name,
                    metadata.album_type.to_db_str(),
                    &metadata.external_id_upc,
                    metadata.label.as_deref().unwrap_or(""),
                    metadata.popularity,
                    metadata.release_date.as_deref().unwrap_or(""),
                    metadata.release_date_precision.as_deref().unwrap_or(""),
                    album_rowid,
                ],
            )?;

            if let Some(artist_ids) = artist_ids {
                conn.execute(
                    "DELETE FROM artist_albums WHERE album_rowid = ?1 AND is_appears_on = 0",
                    params![album_rowid],
                )?;

                for (idx, artist_id) in artist_ids.iter().enumerate() {
                    let artist_rowid: i64 = match conn.query_row(
                        "SELECT rowid FROM artists WHERE id = ?1",
                        params![artist_id],
                        |r| r.get(0),
                    ) {
                        Ok(rowid) => rowid,
                        Err(rusqlite::Error::QueryReturnedNoRows) => {
                            return Err(CatalogMutationError::InvalidReference {
                                entity: "Artist",
                                id: artist_id.clone(),
                            }
                            .into());
                        }
                        Err(error) => return Err(error.into()),
                    };

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
                return Err(CatalogMutationError::AlreadyExists {
                    entity: "Track",
                    id: track.id.clone(),
                }
                .into());
            }

            let album_rowid: i64 = match conn.query_row(
                "SELECT rowid FROM albums WHERE id = ?1",
                params![&track.album_id],
                |r| r.get(0),
            ) {
                Ok(rowid) => rowid,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(CatalogMutationError::InvalidReference {
                        entity: "Album",
                        id: track.album_id.clone(),
                    }
                    .into());
                }
                Err(error) => return Err(error.into()),
            };

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
                let artist_rowid: i64 = match conn.query_row(
                    "SELECT rowid FROM artists WHERE id = ?1",
                    params![artist_id],
                    |r| r.get(0),
                ) {
                    Ok(rowid) => rowid,
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        return Err(CatalogMutationError::InvalidReference {
                            entity: "Artist",
                            id: artist_id.clone(),
                        }
                        .into());
                    }
                    Err(error) => return Err(error.into()),
                };

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

    fn update_track_metadata(
        &self,
        track_id: &str,
        metadata: &TrackMetadataUpdate,
        artist_ids: Option<&[String]>,
    ) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            let track_rowid: i64 = match conn.query_row(
                "SELECT rowid FROM tracks WHERE id = ?1",
                params![track_id],
                |r| r.get(0),
            ) {
                Ok(rowid) => rowid,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(CatalogMutationError::NotFound {
                        entity: "Track",
                        id: track_id.to_owned(),
                    }
                    .into());
                }
                Err(e) => return Err(e.into()),
            };

            let album_rowid: i64 = match conn.query_row(
                "SELECT rowid FROM albums WHERE id = ?1",
                params![&metadata.album_id],
                |r| r.get(0),
            ) {
                Ok(rowid) => rowid,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(CatalogMutationError::InvalidReference {
                        entity: "Album",
                        id: metadata.album_id.clone(),
                    }
                    .into());
                }
                Err(error) => return Err(error.into()),
            };

            conn.execute(
                "UPDATE tracks SET name = ?1, album_rowid = ?2, track_number = ?3, external_id_isrc = ?4,
                 popularity = ?5, disc_number = ?6, duration_ms = ?7, explicit = ?8, language = ?9 WHERE rowid = ?10",
                params![
                    &metadata.name,
                    album_rowid,
                    metadata.track_number,
                    &metadata.external_id_isrc,
                    metadata.popularity,
                    metadata.disc_number,
                    metadata.duration_ms,
                    if metadata.explicit { 1 } else { 0 },
                    &metadata.language,
                    track_rowid,
                ],
            )?;

            if let Some(artist_ids) = artist_ids {
                conn.execute(
                    "DELETE FROM track_artists WHERE track_rowid = ?1",
                    params![track_rowid],
                )?;

                for artist_id in artist_ids {
                    let artist_rowid: i64 = match conn.query_row(
                        "SELECT rowid FROM artists WHERE id = ?1",
                        params![artist_id],
                        |r| r.get(0),
                    ) {
                        Ok(rowid) => rowid,
                        Err(rusqlite::Error::QueryReturnedNoRows) => {
                            return Err(CatalogMutationError::InvalidReference {
                                entity: "Artist",
                                id: artist_id.clone(),
                            }
                            .into());
                        }
                        Err(error) => return Err(error.into()),
                    };

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

    fn media_presence_page(
        &self,
        after: i64,
        limit: usize,
    ) -> Result<Vec<(i64, String, Option<String>)>> {
        let connection = self.get_read_conn();
        let connection = connection.lock().unwrap();
        let mut statement = connection.prepare_cached("SELECT rowid,id,audio_uri FROM tracks INDEXED BY idx_tracks_available WHERE track_available=1 AND rowid>?1 ORDER BY rowid LIMIT ?2")?;
        let rows = statement
            .query_map(params![after, limit.min(1000) as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    fn compare_exchange_audio(
        &self,
        id: &str,
        expected: Option<&str>,
        new: Option<&str>,
    ) -> Result<bool> {
        let mut conn = self.write_conn.lock().unwrap();
        let tx = conn.transaction()?;
        let changed = tx.execute(
            "UPDATE tracks SET audio_uri=?1, track_available=?2 WHERE id=?3 AND audio_uri IS ?4",
            params![new, new.is_some() as i32, id, expected],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        tx.execute("UPDATE albums SET album_availability = CASE WHEN (SELECT COUNT(*) FROM tracks WHERE album_rowid=albums.rowid AND track_available=1)=0 THEN 'missing' WHEN (SELECT COUNT(*) FROM tracks WHERE album_rowid=albums.rowid AND track_available=0)=0 THEN 'complete' ELSE 'partial' END WHERE rowid=(SELECT album_rowid FROM tracks WHERE id=?1)", params![id])?;
        tx.execute("UPDATE artists SET artist_available=EXISTS(SELECT 1 FROM artist_albums aa JOIN albums a ON aa.album_rowid=a.rowid WHERE aa.artist_rowid=artists.rowid AND lower(a.album_availability)!='missing') WHERE rowid IN (SELECT aa.artist_rowid FROM artist_albums aa JOIN tracks t ON t.album_rowid=aa.album_rowid WHERE t.id=?1)", params![id])?;
        tx.commit()?;
        Ok(true)
    }

    fn set_track_audio_uri(&self, track_id: &str, audio_uri: &str) -> Result<()> {
        crate::media::local::normalized_media_identifier(audio_uri)?;

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

    fn clear_track_audio_uri(&self, track_id: &str) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let rows_affected = conn.execute(
            "UPDATE tracks SET audio_uri = NULL, track_available = 0 WHERE id = ?1",
            params![track_id],
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

    fn upsert_entity_embedding(
        &self,
        embedding: &EntityEmbeddingUpsert,
    ) -> Result<EntityEmbedding> {
        if embedding.vector.is_empty() {
            return Err(anyhow!("embedding vector cannot be empty"));
        }
        if embedding.dtype != "float32" {
            return Err(anyhow!(
                "unsupported embedding dtype '{}'; only float32 is currently supported",
                embedding.dtype
            ));
        }

        let conn = self.write_conn.lock().unwrap();
        let now = Utc::now().timestamp();
        let vector_blob = Self::encode_f32_vector(&embedding.vector);
        let vector_norm = Self::vector_norm(&embedding.vector);
        let metadata_json = serde_json::to_string(&embedding.metadata)?;
        let model_json = serde_json::to_string(&embedding.model)?;

        conn.execute(
            "INSERT INTO entity_embeddings (
                entity_type, entity_id, namespace, dim, dtype, vector_blob, vector_norm,
                metadata_json, model_json, created_at, updated_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
             ON CONFLICT(entity_type, entity_id, namespace) DO UPDATE SET
                dim = excluded.dim,
                dtype = excluded.dtype,
                vector_blob = excluded.vector_blob,
                vector_norm = excluded.vector_norm,
                metadata_json = excluded.metadata_json,
                model_json = excluded.model_json,
                updated_at = excluded.updated_at",
            params![
                embedding.entity_type,
                embedding.entity_id,
                embedding.namespace,
                embedding.vector.len() as i64,
                embedding.dtype,
                vector_blob,
                vector_norm,
                metadata_json,
                model_json,
                now,
            ],
        )?;

        self.get_entity_embedding(
            &embedding.entity_type,
            &embedding.entity_id,
            &embedding.namespace,
            true,
        )?
        .ok_or_else(|| anyhow!("failed to read back upserted embedding"))
    }

    fn get_entity_embedding(
        &self,
        entity_type: &str,
        entity_id: &str,
        namespace: &str,
        include_vector: bool,
    ) -> Result<Option<EntityEmbedding>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        conn.query_row(
            "SELECT entity_type, entity_id, namespace, dim, dtype, vector_blob, vector_norm,
                    metadata_json, model_json, created_at, updated_at
             FROM entity_embeddings
             WHERE entity_type = ?1 AND entity_id = ?2 AND namespace = ?3",
            params![entity_type, entity_id, namespace],
            |row| Self::row_to_embedding(row, include_vector),
        )
        .optional()
        .map_err(Into::into)
    }

    fn list_entity_embeddings(
        &self,
        entity_type: &str,
        entity_id: &str,
        include_vector: bool,
    ) -> Result<Vec<EntityEmbedding>> {
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT entity_type, entity_id, namespace, dim, dtype, vector_blob, vector_norm,
                    metadata_json, model_json, created_at, updated_at
             FROM entity_embeddings
             WHERE entity_type = ?1 AND entity_id = ?2
             ORDER BY namespace",
        )?;
        let rows = stmt.query_map(params![entity_type, entity_id], |row| {
            Self::row_to_embedding(row, include_vector)
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn delete_entity_embedding(
        &self,
        entity_type: &str,
        entity_id: &str,
        namespace: &str,
    ) -> Result<bool> {
        let conn = self.write_conn.lock().unwrap();
        let changed = conn.execute(
            "DELETE FROM entity_embeddings
             WHERE entity_type = ?1 AND entity_id = ?2 AND namespace = ?3",
            params![entity_type, entity_id, namespace],
        )?;
        Ok(changed > 0)
    }

    fn search_entity_embeddings(
        &self,
        namespace: &str,
        query: &[f32],
        entity_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<EntityEmbeddingSearchResult>> {
        if query.is_empty() {
            return Err(anyhow!("query vector cannot be empty"));
        }
        let query_norm = Self::vector_norm(query);
        if query_norm <= f64::EPSILON {
            return Err(anyhow!("query vector norm is zero"));
        }
        let limit = limit.max(1);
        let read_conn = self.get_read_conn();
        let conn = read_conn.lock().unwrap();

        let mut results = Vec::new();
        let mut push_row = |row: &rusqlite::Row| -> Result<()> {
            let dim: i64 = row.get("dim")?;
            if dim as usize != query.len() {
                return Ok(());
            }
            let dtype: String = row.get("dtype")?;
            if dtype != "float32" {
                return Ok(());
            }
            let blob: Vec<u8> = row.get("vector_blob")?;
            let vector = Self::decode_f32_vector(&blob)?;
            let vector_norm: f64 = row.get("vector_norm")?;
            if vector_norm <= f64::EPSILON {
                return Ok(());
            }
            let score = f64::from(Self::dot_product(query, &vector)) / (query_norm * vector_norm);
            let metadata_json: String = row.get("metadata_json")?;
            let model_json: String = row.get("model_json")?;
            results.push(EntityEmbeddingSearchResult {
                entity_type: row.get("entity_type")?,
                entity_id: row.get("entity_id")?,
                namespace: row.get("namespace")?,
                score: score as f32,
                dim: dim as usize,
                dtype,
                vector_norm,
                metadata: serde_json::from_str(&metadata_json)
                    .unwrap_or(serde_json::Value::Object(Default::default())),
                model: serde_json::from_str(&model_json)
                    .unwrap_or(serde_json::Value::Object(Default::default())),
                updated_at: row.get("updated_at")?,
            });
            Ok(())
        };

        if let Some(entity_type) = entity_type {
            let mut stmt = conn.prepare_cached(
                "SELECT entity_type, entity_id, namespace, dim, dtype, vector_blob, vector_norm,
                        metadata_json, model_json, updated_at
                 FROM entity_embeddings
                 WHERE namespace = ?1 AND entity_type = ?2",
            )?;
            let mut rows = stmt.query(params![namespace, entity_type])?;
            while let Some(row) = rows.next()? {
                push_row(row)?;
            }
        } else {
            let mut stmt = conn.prepare_cached(
                "SELECT entity_type, entity_id, namespace, dim, dtype, vector_blob, vector_norm,
                        metadata_json, model_json, updated_at
                 FROM entity_embeddings
                 WHERE namespace = ?1",
            )?;
            let mut rows = stmt.query(params![namespace])?;
            while let Some(row) = rows.next()? {
                push_row(row)?;
            }
        }
        results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
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

    fn get_available_album_track_ids(&self, album_id: &str) -> Result<Vec<String>> {
        let conn = self.get_read_conn();
        let conn = conn.lock().unwrap();

        let mut stmt = conn.prepare_cached(
            "SELECT t.id
             FROM tracks t
             INNER JOIN albums a ON t.album_rowid = a.rowid
             WHERE a.id = ?1 AND t.track_available = 1
             ORDER BY t.disc_number, t.track_number, t.id",
        )?;

        let track_ids = stmt
            .query_map(params![album_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(track_ids)
    }

    fn get_artist_top_track_ids(&self, artist_id: &str, limit: usize) -> Result<Vec<String>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let conn = self.get_read_conn();
        let conn = conn.lock().unwrap();

        let mut stmt = conn.prepare_cached(
            "SELECT DISTINCT t.id, t.popularity
             FROM tracks t
             INNER JOIN track_artists ta ON t.rowid = ta.track_rowid
             INNER JOIN artists a ON ta.artist_rowid = a.rowid
             WHERE a.id = ?1 AND t.track_available = 1
             ORDER BY t.popularity DESC, t.id
             LIMIT ?2",
        )?;

        let track_ids = stmt
            .query_map(params![artist_id, limit as i64], |row| {
                row.get::<_, String>(0)
            })?
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

    fn get_artist_mbid(&self, artist_id: &str) -> Result<Option<String>> {
        SqliteCatalogStore::get_artist_mbid(self, artist_id)
    }

    fn set_artist_mbid(&self, artist_id: &str, mbid: &str) -> Result<()> {
        SqliteCatalogStore::set_artist_mbid(self, artist_id, mbid)
    }

    fn mark_artist_mbid_not_found(&self, artist_id: &str) -> Result<()> {
        SqliteCatalogStore::mark_artist_mbid_not_found(self, artist_id)
    }

    fn record_artist_mbid_failure(&self, artist_rowid: i64, error: &str) -> Result<()> {
        SqliteCatalogStore::record_artist_mbid_failure(self, artist_rowid, error)
    }

    fn record_artist_related_failure(&self, artist_rowid: i64, error: &str) -> Result<()> {
        SqliteCatalogStore::record_artist_related_failure(self, artist_rowid, error)
    }

    fn release_artist_enrichment_claims(&self) -> Result<()> {
        SqliteCatalogStore::release_artist_enrichment_claims(self)
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

    fn get_artist_rowids_by_mbids(&self, mbids: &[String]) -> Result<Vec<(String, i64)>> {
        SqliteCatalogStore::get_artist_rowids_by_mbids(self, mbids)
    }
}

include!("store_tests.rs");
