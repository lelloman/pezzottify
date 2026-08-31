impl UserListeningStore for SqliteUserStore {
    fn record_listening_event(&self, event: ListeningEvent) -> Result<(usize, bool)> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        // If session_id is provided, check if it exists and belongs to this user
        // This enables clients to send progress updates and final data for the same session
        // while preventing one user from overwriting another user's session
        //
        // Returns (id, created) where:
        // - id: the row id of the event
        // - created: true only if this is a NEW finalized event (for metrics counting)
        //   This prevents double-counting when clients retry or update sessions
        let (id, created) = if let Some(ref session_id) = event.session_id {
            // Check if session exists and whether it was already finalized
            let existing: Option<(usize, usize, bool, String, u64)> = conn
                .query_row(
                    &format!(
                        "SELECT id, user_id, ended_at IS NOT NULL, track_id, started_at
                         FROM {} WHERE session_id = ?1",
                        LISTENING_EVENTS_TABLE_V_6.name
                    ),
                    params![session_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)? as usize,
                            row.get::<_, i64>(1)? as usize,
                            row.get(2)?,
                            row.get(3)?,
                            row.get::<_, i64>(4)? as u64,
                        ))
                    },
                )
                .optional()?;

            match existing {
                Some((id, existing_uid, _, _, _)) if existing_uid != event.user_id => {
                    // Session belongs to a different user - ignore this event
                    record_db_query("record_listening_event", start.elapsed());
                    return Ok((id, false));
                }
                Some((id, _, true, _, _)) => {
                    // Finalized events are immutable. Retries are idempotent and cannot
                    // rewrite trusted aggregate inputs.
                    (id, false)
                }
                Some((id, _, false, existing_track_id, existing_started_at))
                    if existing_track_id != event.track_id
                        || existing_started_at != event.started_at =>
                {
                    // An idempotency key cannot be repurposed for another playback.
                    (id, false)
                }
                Some((id, _, false, _, _)) => {
                    // Same user updating an in-progress session. Keep the stable row ID.
                    conn.execute(
                        &format!(
                            "UPDATE {} SET ended_at = ?1, duration_seconds = ?2,
                             track_duration_seconds = ?3, completed = ?4, seek_count = ?5,
                             pause_count = ?6, playback_context = ?7, client_type = ?8, date = ?9
                             WHERE id = ?10",
                            LISTENING_EVENTS_TABLE_V_6.name
                        ),
                        params![
                            event.ended_at.map(|t| t as i64),
                            event.duration_seconds as i64,
                            event.track_duration_seconds as i64,
                            if event.completed { 1 } else { 0 },
                            event.seek_count as i64,
                            event.pause_count as i64,
                            event.playback_context,
                            event.client_type,
                            event.date as i64,
                            id as i64,
                        ],
                    )?;
                    let created = event.ended_at.is_some();
                    (id, created)
                }
                None => {
                    // New session - insert
                    conn.execute(
                        &format!(
                            "INSERT INTO {} (user_id, track_id, session_id, started_at, ended_at,
                             duration_seconds, track_duration_seconds, completed, seek_count, pause_count,
                             playback_context, client_type, date)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                            LISTENING_EVENTS_TABLE_V_6.name
                        ),
                        params![
                            event.user_id,
                            event.track_id,
                            session_id,
                            event.started_at as i64,
                            event.ended_at.map(|t| t as i64),
                            event.duration_seconds as i64,
                            event.track_duration_seconds as i64,
                            if event.completed { 1 } else { 0 },
                            event.seek_count as i64,
                            event.pause_count as i64,
                            event.playback_context,
                            event.client_type,
                            event.date as i64,
                        ],
                    )?;
                    let id = conn.last_insert_rowid() as usize;
                    // New session, created = true only if finalized
                    let created = event.ended_at.is_some();
                    (id, created)
                }
            }
        } else {
            // No session_id, always insert as new event
            conn.execute(
                &format!(
                    "INSERT INTO {} (user_id, track_id, session_id, started_at, ended_at,
                     duration_seconds, track_duration_seconds, completed, seek_count, pause_count,
                     playback_context, client_type, date)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    LISTENING_EVENTS_TABLE_V_6.name
                ),
                params![
                    event.user_id,
                    event.track_id,
                    event.session_id,
                    event.started_at as i64,
                    event.ended_at.map(|t| t as i64),
                    event.duration_seconds as i64,
                    event.track_duration_seconds as i64,
                    if event.completed { 1 } else { 0 },
                    event.seek_count as i64,
                    event.pause_count as i64,
                    event.playback_context,
                    event.client_type,
                    event.date as i64,
                ],
            )?;
            let id = conn.last_insert_rowid() as usize;
            // No session_id means always new, created = true only if finalized
            let created = event.ended_at.is_some();
            (id, created)
        };

        record_db_query("record_listening_event", start.elapsed());
        Ok((id, created))
    }

    fn get_user_listening_events(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ListeningEvent>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let limit_val = limit.unwrap_or(50).min(500) as i64;
        let offset_val = offset.unwrap_or(0) as i64;

        let mut stmt = conn.prepare(&format!(
            "SELECT id, user_id, track_id, session_id, started_at, ended_at,
             duration_seconds, track_duration_seconds, completed, seek_count, pause_count,
             playback_context, client_type, date
             FROM {} WHERE user_id = ?1 AND date >= ?2 AND date <= ?3
             ORDER BY started_at DESC
             LIMIT ?4 OFFSET ?5",
            LISTENING_EVENTS_TABLE_V_6.name
        ))?;

        let events = stmt
            .query_map(
                params![user_id, start_date, end_date, limit_val, offset_val],
                |row| {
                    Ok(ListeningEvent {
                        id: Some(row.get::<_, i64>(0)? as usize),
                        user_id: row.get::<_, i64>(1)? as usize,
                        track_id: row.get(2)?,
                        session_id: row.get(3)?,
                        started_at: row.get::<_, i64>(4)? as u64,
                        ended_at: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                        duration_seconds: row.get::<_, i64>(6)? as u32,
                        track_duration_seconds: row.get::<_, i64>(7)? as u32,
                        completed: row.get::<_, i64>(8)? != 0,
                        seek_count: row.get::<_, i64>(9)? as u32,
                        pause_count: row.get::<_, i64>(10)? as u32,
                        playback_context: row.get(11)?,
                        client_type: row.get(12)?,
                        date: row.get::<_, i64>(13)? as u32,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_user_listening_events", start.elapsed());
        Ok(events)
    }

    fn get_user_listening_summary(
        &self,
        user_id: usize,
        start_date: u32,
        end_date: u32,
    ) -> Result<ListeningSummary> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let summary = conn.query_row(
            &format!(
                "SELECT
                    COUNT(*) as total_plays,
                    COALESCE(SUM(duration_seconds), 0) as total_duration_seconds,
                    COALESCE(SUM(completed), 0) as completed_plays,
                    COUNT(DISTINCT track_id) as unique_tracks
                 FROM {} WHERE user_id = ?1 AND date >= ?2 AND date <= ?3
                 AND ended_at IS NOT NULL",
                LISTENING_EVENTS_TABLE_V_6.name
            ),
            params![user_id, start_date, end_date],
            |row| {
                Ok(ListeningSummary {
                    user_id: Some(user_id),
                    total_plays: row.get::<_, i64>(0)? as u64,
                    total_duration_seconds: row.get::<_, i64>(1)? as u64,
                    completed_plays: row.get::<_, i64>(2)? as u64,
                    unique_tracks: row.get::<_, i64>(3)? as u64,
                })
            },
        )?;

        record_db_query("get_user_listening_summary", start.elapsed());
        Ok(summary)
    }

    fn get_user_listening_history(
        &self,
        user_id: usize,
        limit: usize,
    ) -> Result<Vec<UserListeningHistoryEntry>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(&format!(
            "SELECT
                track_id,
                MAX(started_at) as last_played_at,
                COUNT(*) as play_count,
                SUM(duration_seconds) as total_duration_seconds
             FROM {} WHERE user_id = ?1 AND ended_at IS NOT NULL
             GROUP BY track_id
             ORDER BY last_played_at DESC
             LIMIT ?2",
            LISTENING_EVENTS_TABLE_V_6.name
        ))?;

        let entries = stmt
            .query_map(params![user_id, limit as i64], |row| {
                Ok(UserListeningHistoryEntry {
                    track_id: row.get(0)?,
                    last_played_at: row.get::<_, i64>(1)? as u64,
                    play_count: row.get::<_, i64>(2)? as u64,
                    total_duration_seconds: row.get::<_, i64>(3)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_user_listening_history", start.elapsed());
        Ok(entries)
    }

    fn get_track_listening_stats(
        &self,
        track_id: &str,
        start_date: u32,
        end_date: u32,
    ) -> Result<TrackListeningStats> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let stats = conn.query_row(
            &format!(
                "SELECT
                    COUNT(*) as play_count,
                    COALESCE(SUM(duration_seconds), 0) as total_duration_seconds,
                    COALESCE(SUM(completed), 0) as completed_count,
                    COUNT(DISTINCT user_id) as unique_listeners
                 FROM {} WHERE track_id = ?1 AND date >= ?2 AND date <= ?3
                 AND ended_at IS NOT NULL",
                LISTENING_EVENTS_TABLE_V_6.name
            ),
            params![track_id, start_date, end_date],
            |row| {
                Ok(TrackListeningStats {
                    track_id: track_id.to_string(),
                    play_count: row.get::<_, i64>(0)? as u64,
                    total_duration_seconds: row.get::<_, i64>(1)? as u64,
                    completed_count: row.get::<_, i64>(2)? as u64,
                    unique_listeners: row.get::<_, i64>(3)? as u64,
                })
            },
        )?;

        record_db_query("get_track_listening_stats", start.elapsed());
        Ok(stats)
    }

    fn get_track_listening_seconds_since(&self, track_id: &str, since: i64) -> Result<u64> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();
        let seconds: i64 = conn.query_row(
            &format!(
                "SELECT COALESCE(SUM(duration_seconds), 0)
                 FROM {} WHERE track_id = ?1 AND started_at >= ?2",
                LISTENING_EVENTS_TABLE_V_6.name
            ),
            params![track_id, since],
            |row| row.get(0),
        )?;
        record_db_query("get_track_listening_seconds_since", start.elapsed());
        Ok(seconds.max(0) as u64)
    }

    fn get_daily_listening_stats(
        &self,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<DailyListeningStats>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(&format!(
            "SELECT
                date,
                COUNT(*) as total_plays,
                COALESCE(SUM(duration_seconds), 0) as total_duration_seconds,
                COALESCE(SUM(completed), 0) as completed_plays,
                COUNT(DISTINCT user_id) as unique_users,
                COUNT(DISTINCT track_id) as unique_tracks
             FROM {} WHERE date >= ?1 AND date <= ?2 AND ended_at IS NOT NULL
             GROUP BY date
             ORDER BY date DESC",
            LISTENING_EVENTS_TABLE_V_6.name
        ))?;

        let stats = stmt
            .query_map(params![start_date, end_date], |row| {
                Ok(DailyListeningStats {
                    date: row.get::<_, i64>(0)? as u32,
                    total_plays: row.get::<_, i64>(1)? as u64,
                    total_duration_seconds: row.get::<_, i64>(2)? as u64,
                    completed_plays: row.get::<_, i64>(3)? as u64,
                    unique_users: row.get::<_, i64>(4)? as u64,
                    unique_tracks: row.get::<_, i64>(5)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_daily_listening_stats", start.elapsed());
        Ok(stats)
    }

    fn get_top_tracks(
        &self,
        start_date: u32,
        end_date: u32,
        limit: usize,
    ) -> Result<Vec<TrackListeningStats>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(&format!(
            "SELECT
                track_id,
                COUNT(*) as play_count,
                COALESCE(SUM(duration_seconds), 0) as total_duration_seconds,
                COALESCE(SUM(completed), 0) as completed_count,
                COUNT(DISTINCT user_id) as unique_listeners
             FROM {} WHERE date >= ?1 AND date <= ?2 AND ended_at IS NOT NULL
             GROUP BY track_id
             ORDER BY play_count DESC
             LIMIT ?3",
            LISTENING_EVENTS_TABLE_V_6.name
        ))?;

        let stats = stmt
            .query_map(params![start_date, end_date, limit as i64], |row| {
                Ok(TrackListeningStats {
                    track_id: row.get(0)?,
                    play_count: row.get::<_, i64>(1)? as u64,
                    total_duration_seconds: row.get::<_, i64>(2)? as u64,
                    completed_count: row.get::<_, i64>(3)? as u64,
                    unique_listeners: row.get::<_, i64>(4)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_top_tracks", start.elapsed());
        Ok(stats)
    }

    fn get_all_track_play_counts(
        &self,
        start_date: u32,
        end_date: u32,
    ) -> Result<Vec<TrackPlayCount>> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(&format!(
            "SELECT track_id, COUNT(*) as play_count
             FROM {} WHERE date >= ?1 AND date <= ?2 AND ended_at IS NOT NULL
             GROUP BY track_id",
            LISTENING_EVENTS_TABLE_V_6.name
        ))?;

        let counts = stmt
            .query_map(params![start_date, end_date], |row| {
                Ok(TrackPlayCount {
                    track_id: row.get(0)?,
                    play_count: row.get::<_, i64>(1)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        record_db_query("get_all_track_play_counts", start.elapsed());
        Ok(counts)
    }

    fn prune_listening_events(&self, older_than_days: u32) -> Result<usize> {
        let start = Instant::now();
        let conn = self.conn.lock().unwrap();

        // Calculate the cutoff date in YYYYMMDD format
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cutoff_secs = now - (older_than_days as u64 * 24 * 60 * 60);

        // Convert to YYYYMMDD format
        let cutoff_date = {
            let datetime = chrono::DateTime::from_timestamp(cutoff_secs as i64, 0)
                .unwrap_or_else(chrono::Utc::now);
            datetime
                .format("%Y%m%d")
                .to_string()
                .parse::<u32>()
                .unwrap_or(0)
        };

        let deleted = conn.execute(
            &format!(
                "DELETE FROM {} WHERE date < ?1",
                LISTENING_EVENTS_TABLE_V_6.name
            ),
            params![cutoff_date],
        )?;

        record_db_query("prune_listening_events", start.elapsed());
        Ok(deleted)
    }
}
