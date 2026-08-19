impl DownloadQueueStore for SqliteDownloadQueueStore {
    // === Queue Management ===

    fn enqueue(&self, item: QueueItem) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT INTO download_queue (
                id, parent_id, status, priority, content_type, content_id,
                content_name, artist_name, request_source, requested_by_user_id,
                created_at, started_at, completed_at, last_attempt_at, next_retry_at,
                retry_count, max_retries, error_type, error_message,
                bytes_downloaded, processing_duration_ms
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
            )"#,
            rusqlite::params![
                item.id,
                item.parent_id,
                item.status.as_db_str(),
                item.priority.as_i32(),
                item.content_type.as_str(),
                item.content_id,
                item.content_name,
                item.artist_name,
                item.request_source.as_str(),
                item.requested_by_user_id,
                item.created_at,
                item.started_at,
                item.completed_at,
                item.last_attempt_at,
                item.next_retry_at,
                item.retry_count,
                item.max_retries,
                item.error_type.as_ref().map(|e| e.as_str()),
                item.error_message,
                item.bytes_downloaded,
                item.processing_duration_ms,
            ],
        )?;
        Ok(())
    }

    fn get_item(&self, id: &str) -> Result<Option<QueueItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM download_queue WHERE id = ?1")?;

        let item = stmt.query_row([id], Self::row_to_queue_item).optional()?;

        Ok(item)
    }

    fn delete_item(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute("DELETE FROM download_queue WHERE id = ?1", [id])?;
        Ok(rows_affected > 0)
    }

    fn get_next_pending(&self) -> Result<Option<QueueItem>> {
        let conn = self.conn.lock().unwrap();
        // Order by:
        // 1. Priority (lower = higher priority)
        // 2. Content type: children first (tracks, images) then parents (albums)
        //    This ensures one album completes before the next starts
        // 3. Creation time (older first)
        let mut stmt = conn.prepare(
            r#"SELECT * FROM download_queue
               WHERE status = 'PENDING'
               ORDER BY
                   priority ASC,
                   CASE content_type
                       WHEN 'TRACK_AUDIO' THEN 0
                       WHEN 'ALBUM_IMAGE' THEN 1
                       WHEN 'ARTIST_IMAGE' THEN 2
                       WHEN 'ALBUM' THEN 3
                       ELSE 4
                   END ASC,
                   created_at ASC
               LIMIT 1"#,
        )?;

        let item = stmt.query_row([], Self::row_to_queue_item).optional()?;

        Ok(item)
    }

    fn list_by_user(
        &self,
        user_id: &str,
        status: Option<QueueStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>> {
        let conn = self.conn.lock().unwrap();

        let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = match status {
            Some(s) => (
                r#"SELECT * FROM download_queue
                   WHERE requested_by_user_id = ?1 AND status = ?2
                   ORDER BY created_at DESC
                   LIMIT ?3 OFFSET ?4"#
                    .to_string(),
                vec![
                    Box::new(user_id.to_string()),
                    Box::new(s.as_db_str().to_string()),
                    Box::new(limit as i64),
                    Box::new(offset as i64),
                ],
            ),
            None => (
                r#"SELECT * FROM download_queue
                   WHERE requested_by_user_id = ?1
                   ORDER BY created_at DESC
                   LIMIT ?2 OFFSET ?3"#
                    .to_string(),
                vec![
                    Box::new(user_id.to_string()),
                    Box::new(limit as i64),
                    Box::new(offset as i64),
                ],
            ),
        };

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let items = stmt
            .query_map(params_refs.as_slice(), Self::row_to_queue_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    fn list_all(
        &self,
        status: Option<QueueStatus>,
        exclude_completed: bool,
        top_level_only: bool,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>> {
        let conn = self.conn.lock().unwrap();

        // Build WHERE clause based on filters
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if let Some(s) = status {
            conditions.push(format!("status = ?{}", param_idx));
            params.push(Box::new(s.as_db_str().to_string()));
            param_idx += 1;
        }

        if exclude_completed {
            conditions.push(format!("status != ?{}", param_idx));
            params.push(Box::new(QueueStatus::Completed.as_db_str().to_string()));
            param_idx += 1;
        }

        if top_level_only {
            conditions.push("parent_id IS NULL".to_string());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            r#"SELECT * FROM download_queue
               {}
               ORDER BY priority ASC, created_at ASC
               LIMIT ?{} OFFSET ?{}"#,
            where_clause,
            param_idx,
            param_idx + 1
        );

        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let items = stmt
            .query_map(params_refs.as_slice(), Self::row_to_queue_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    fn get_queue_position(&self, id: &str) -> Result<Option<usize>> {
        let conn = self.conn.lock().unwrap();

        // First check if the item exists and is pending
        let status: Option<String> = conn
            .query_row(
                "SELECT status FROM download_queue WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;

        match status {
            None => Ok(None),                      // Item doesn't exist
            Some(s) if s != "PENDING" => Ok(None), // Not pending, no queue position
            Some(_) => {
                // Get the item's priority and created_at
                let (priority, created_at): (i32, i64) = conn.query_row(
                    "SELECT priority, created_at FROM download_queue WHERE id = ?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;

                // Count items ahead in queue (higher priority or same priority but older)
                let position: i64 = conn.query_row(
                    r#"SELECT COUNT(*) + 1 FROM download_queue
                       WHERE status = 'PENDING'
                       AND (priority < ?1 OR (priority = ?1 AND created_at < ?2))"#,
                    rusqlite::params![priority, created_at],
                    |row| row.get(0),
                )?;

                Ok(Some(position as usize))
            }
        }
    }

    // === State Transitions ===

    fn claim_for_processing(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();

        // Atomically update only if currently PENDING
        let rows_affected = conn.execute(
            r#"UPDATE download_queue
               SET status = 'IN_PROGRESS',
                   started_at = ?1,
                   last_attempt_at = ?1
               WHERE id = ?2 AND status = 'PENDING'"#,
            rusqlite::params![now, id],
        )?;

        Ok(rows_affected > 0)
    }

    fn mark_completed(&self, id: &str, bytes: u64, duration_ms: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();

        conn.execute(
            r#"UPDATE download_queue
               SET status = 'COMPLETED',
                   completed_at = ?1,
                   bytes_downloaded = ?2,
                   processing_duration_ms = ?3,
                   error_type = NULL,
                   error_message = NULL
               WHERE id = ?4"#,
            rusqlite::params![now, bytes as i64, duration_ms, id],
        )?;

        Ok(())
    }

    fn mark_retry_waiting(
        &self,
        id: &str,
        next_retry_at: i64,
        error: &DownloadError,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();

        conn.execute(
            r#"UPDATE download_queue
               SET status = 'RETRY_WAITING',
                   last_attempt_at = ?1,
                   next_retry_at = ?2,
                   retry_count = retry_count + 1,
                   error_type = ?3,
                   error_message = ?4
               WHERE id = ?5"#,
            rusqlite::params![
                now,
                next_retry_at,
                error.error_type.as_str(),
                error.message,
                id
            ],
        )?;

        Ok(())
    }

    fn mark_failed(&self, id: &str, error: &DownloadError) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();

        conn.execute(
            r#"UPDATE download_queue
               SET status = 'FAILED',
                   completed_at = ?1,
                   error_type = ?2,
                   error_message = ?3
               WHERE id = ?4"#,
            rusqlite::params![now, error.error_type.as_str(), error.message, id],
        )?;

        Ok(())
    }

    // === Parent-Child Management ===

    fn create_children(&self, parent_id: &str, children: Vec<QueueItem>) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Use a transaction to insert all children atomically
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| {
            for child in children {
                // Verify the child has the correct parent_id
                let actual_parent_id = child.parent_id.as_deref().unwrap_or("");
                if actual_parent_id != parent_id {
                    bail!(
                        "Child item {} has parent_id {:?} but expected {}",
                        child.id,
                        child.parent_id,
                        parent_id
                    );
                }

                conn.execute(
                    r#"INSERT INTO download_queue (
                        id, parent_id, status, priority, content_type, content_id,
                        content_name, artist_name, request_source, requested_by_user_id,
                        created_at, started_at, completed_at, last_attempt_at, next_retry_at,
                        retry_count, max_retries, error_type, error_message,
                        bytes_downloaded, processing_duration_ms
                    ) VALUES (
                        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                        ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
                    )"#,
                    rusqlite::params![
                        child.id,
                        child.parent_id,
                        child.status.as_db_str(),
                        child.priority.as_i32(),
                        child.content_type.as_str(),
                        child.content_id,
                        child.content_name,
                        child.artist_name,
                        child.request_source.as_str(),
                        child.requested_by_user_id,
                        child.created_at,
                        child.started_at,
                        child.completed_at,
                        child.last_attempt_at,
                        child.next_retry_at,
                        child.retry_count,
                        child.max_retries,
                        child.error_type.as_ref().map(|e| e.as_str()),
                        child.error_message,
                        child.bytes_downloaded,
                        child.processing_duration_ms,
                    ],
                )?;
            }
            Ok(())
        })();

        if result.is_ok() {
            conn.execute("COMMIT", [])?;
        } else {
            conn.execute("ROLLBACK", [])?;
        }

        result
    }

    fn get_children(&self, parent_id: &str) -> Result<Vec<QueueItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT * FROM download_queue
               WHERE parent_id = ?1
               ORDER BY created_at ASC"#,
        )?;

        let items = stmt
            .query_map([parent_id], Self::row_to_queue_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    fn delete_children(&self, parent_id: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "DELETE FROM download_queue WHERE parent_id = ?1",
            [parent_id],
        )?;
        Ok(rows_affected)
    }

    fn get_children_progress(&self, parent_id: &str) -> Result<DownloadProgress> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            r#"SELECT
                   COUNT(*) as total,
                   COALESCE(SUM(CASE WHEN status = 'COMPLETED' THEN 1 ELSE 0 END), 0) as completed,
                   COALESCE(SUM(CASE WHEN status = 'FAILED' THEN 1 ELSE 0 END), 0) as failed,
                   COALESCE(SUM(CASE WHEN status = 'PENDING' THEN 1 ELSE 0 END), 0) as pending,
                   COALESCE(SUM(CASE WHEN status = 'IN_PROGRESS' THEN 1 ELSE 0 END), 0) as in_progress
               FROM download_queue
               WHERE parent_id = ?1"#,
        )?;

        let progress = stmt.query_row([parent_id], |row| {
            Ok(DownloadProgress {
                total_children: row.get::<_, i64>("total")? as usize,
                completed: row.get::<_, i64>("completed")? as usize,
                failed: row.get::<_, i64>("failed")? as usize,
                pending: row.get::<_, i64>("pending")? as usize,
                in_progress: row.get::<_, i64>("in_progress")? as usize,
            })
        })?;

        Ok(progress)
    }

    fn check_parent_completion(&self, parent_id: &str) -> Result<Option<QueueStatus>> {
        let conn = self.conn.lock().unwrap();

        // Get status counts for all children
        let mut stmt = conn.prepare(
            r#"SELECT status, COUNT(*) as count
               FROM download_queue
               WHERE parent_id = ?1
               GROUP BY status"#,
        )?;

        let mut pending = 0i64;
        let mut in_progress = 0i64;
        let mut retry_waiting = 0i64;
        let mut completed = 0i64;
        let mut failed = 0i64;

        let rows = stmt.query_map([parent_id], |row| {
            Ok((row.get::<_, String>("status")?, row.get::<_, i64>("count")?))
        })?;

        for row in rows {
            let (status, count) = row?;
            match status.as_str() {
                "PENDING" => pending = count,
                "IN_PROGRESS" => in_progress = count,
                "RETRY_WAITING" => retry_waiting = count,
                "COMPLETED" => completed = count,
                "FAILED" => failed = count,
                _ => {}
            }
        }

        let total = pending + in_progress + retry_waiting + completed + failed;

        // No children - no completion status
        if total == 0 {
            return Ok(None);
        }

        // If any child is still in a non-terminal state, parent is not complete
        if pending > 0 || in_progress > 0 || retry_waiting > 0 {
            return Ok(None);
        }

        // All children are in terminal states (COMPLETED or FAILED)
        if failed > 0 {
            // At least one child failed
            Ok(Some(QueueStatus::Failed))
        } else {
            // All children completed
            Ok(Some(QueueStatus::Completed))
        }
    }

    fn get_user_requests(
        &self,
        user_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT * FROM download_queue
               WHERE requested_by_user_id = ?1 AND parent_id IS NULL
               ORDER BY
                   CASE status
                       WHEN 'IN_PROGRESS' THEN 0
                       WHEN 'PENDING' THEN 1
                       WHEN 'RETRY_WAITING' THEN 2
                       WHEN 'FAILED' THEN 3
                       WHEN 'COMPLETED' THEN 4
                   END,
                   created_at DESC
               LIMIT ?2 OFFSET ?3"#,
        )?;

        let items = stmt
            .query_map(
                rusqlite::params![user_id, limit as i64, offset as i64],
                Self::row_to_queue_item,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    // === Retry Handling ===

    fn get_retry_ready(&self) -> Result<Vec<QueueItem>> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();

        let mut stmt = conn.prepare(
            r#"SELECT * FROM download_queue
               WHERE status = 'RETRY_WAITING' AND next_retry_at <= ?1
               ORDER BY priority ASC, next_retry_at ASC"#,
        )?;

        let items = stmt
            .query_map([now], Self::row_to_queue_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    fn promote_retry_to_pending(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            r#"UPDATE download_queue
               SET status = 'PENDING',
                   next_retry_at = NULL
               WHERE id = ?1 AND status = 'RETRY_WAITING'"#,
            [id],
        )?;

        Ok(())
    }

    fn reset_to_pending(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let rows = conn.execute(
            r#"UPDATE download_queue
               SET status = 'PENDING',
                   next_retry_at = NULL,
                   error_type = NULL,
                   error_message = NULL,
                   retry_count = 0
               WHERE id = ?1 AND status != 'COMPLETED'"#,
            [id],
        )?;

        if rows == 0 {
            return Err(anyhow::anyhow!(
                "Item not found or already completed: {}",
                id
            ));
        }

        Ok(())
    }

    // === Duplicate/Existence Checks ===

    fn find_by_content(
        &self,
        content_type: DownloadContentType,
        content_id: &str,
    ) -> Result<Option<QueueItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT * FROM download_queue
               WHERE content_type = ?1 AND content_id = ?2
               ORDER BY created_at DESC
               LIMIT 1"#,
        )?;

        let item = stmt
            .query_row(
                rusqlite::params![content_type.as_str(), content_id],
                Self::row_to_queue_item,
            )
            .optional()?;

        Ok(item)
    }

    fn is_in_queue(&self, content_type: DownloadContentType, content_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            r#"SELECT COUNT(*) FROM download_queue
               WHERE content_type = ?1 AND content_id = ?2"#,
            rusqlite::params![content_type.as_str(), content_id],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }

    fn is_in_active_queue(
        &self,
        content_type: DownloadContentType,
        content_id: &str,
    ) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            r#"SELECT COUNT(*) FROM download_queue
               WHERE content_type = ?1 AND content_id = ?2
               AND status IN ('PENDING', 'IN_PROGRESS', 'RETRY_WAITING')"#,
            rusqlite::params![content_type.as_str(), content_id],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }

    fn find_pending_by_content(
        &self,
        content_type: DownloadContentType,
        content_id: &str,
    ) -> Result<Vec<QueueItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT * FROM download_queue
               WHERE content_type = ?1 AND content_id = ?2
               AND status IN ('PENDING', 'IN_PROGRESS', 'RETRY_WAITING')
               ORDER BY created_at ASC"#,
        )?;

        let items = stmt
            .query_map(
                rusqlite::params![content_type.as_str(), content_id],
                Self::row_to_queue_item,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(items)
    }

    // === User Rate Limiting ===

    fn get_user_stats(&self, user_id: &str) -> Result<UserLimitStatus> {
        let conn = self.conn.lock().unwrap();

        // Get daily request count from stats table
        let daily_stats = conn
            .query_row(
                r#"SELECT requests_today, last_request_date
                   FROM user_request_stats
                   WHERE user_id = ?1"#,
                [user_id],
                |row| {
                    Ok((
                        row.get::<_, i32>("requests_today")?,
                        row.get::<_, Option<String>>("last_request_date")?,
                    ))
                },
            )
            .optional()?;

        // Count active items directly from the queue (source of truth)
        let in_queue: i32 = conn.query_row(
            r#"SELECT COUNT(*) FROM download_queue
               WHERE requested_by_user_id = ?1
               AND status IN ('PENDING', 'IN_PROGRESS', 'RETRY_WAITING')"#,
            [user_id],
            |row| row.get(0),
        )?;

        // Default limits (these could be made configurable)
        const MAX_REQUESTS_PER_DAY: i32 = 8;
        const MAX_QUEUE_SIZE: i32 = 100;

        let requests_today = match daily_stats {
            Some((requests_today, last_date)) => {
                let today = Self::today_date_string();
                if last_date.as_deref() == Some(&today) {
                    requests_today
                } else {
                    0 // Reset since it's a new day
                }
            }
            None => 0,
        };

        Ok(UserLimitStatus::available(
            requests_today,
            MAX_REQUESTS_PER_DAY,
            in_queue,
            MAX_QUEUE_SIZE,
        ))
    }

    fn increment_user_requests(&self, user_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();
        let today = Self::today_date_string();

        // Insert or update - if it's a new day, reset requests_today
        conn.execute(
            r#"INSERT INTO user_request_stats (user_id, requests_today, requests_in_queue, last_request_date, last_updated_at)
               VALUES (?1, 1, 0, ?2, ?3)
               ON CONFLICT(user_id) DO UPDATE SET
                   requests_today = CASE
                       WHEN last_request_date = ?2 THEN requests_today + 1
                       ELSE 1
                   END,
                   last_request_date = ?2,
                   last_updated_at = ?3"#,
            rusqlite::params![user_id, today, now],
        )?;

        Ok(())
    }

    fn reset_daily_user_stats(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let today = Self::today_date_string();
        let now = Self::now();

        let rows_affected = conn.execute(
            r#"UPDATE user_request_stats
               SET requests_today = 0,
                   last_request_date = ?1,
                   last_updated_at = ?2
               WHERE last_request_date != ?1 OR last_request_date IS NULL"#,
            rusqlite::params![today, now],
        )?;

        Ok(rows_affected)
    }

    // === Activity Tracking ===

    fn record_activity(
        &self,
        content_type: DownloadContentType,
        bytes: u64,
        success: bool,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let hour_bucket = Self::hour_bucket();
        let now = Self::now();

        // Determine which counter to increment based on content type
        let (albums_inc, tracks_inc, images_inc, failed_inc) = match (content_type, success) {
            (DownloadContentType::Album, true) => (1, 0, 0, 0),
            (DownloadContentType::TrackAudio, true) => (0, 1, 0, 0),
            (DownloadContentType::ArtistImage, true) | (DownloadContentType::AlbumImage, true) => {
                (0, 0, 1, 0)
            }
            // Artist enrichment operations don't count towards download stats
            (DownloadContentType::ArtistRelated, true)
            | (DownloadContentType::ArtistMetadata, true) => (0, 0, 0, 0),
            (_, false) => (0, 0, 0, 1),
        };

        conn.execute(
            r#"INSERT INTO download_activity_log (
                   hour_bucket, albums_downloaded, tracks_downloaded, images_downloaded,
                   bytes_downloaded, failed_count, last_updated_at
               ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
               ON CONFLICT(hour_bucket) DO UPDATE SET
                   albums_downloaded = albums_downloaded + ?2,
                   tracks_downloaded = tracks_downloaded + ?3,
                   images_downloaded = images_downloaded + ?4,
                   bytes_downloaded = bytes_downloaded + ?5,
                   failed_count = failed_count + ?6,
                   last_updated_at = ?7"#,
            rusqlite::params![
                hour_bucket,
                albums_inc,
                tracks_inc,
                images_inc,
                bytes as i64,
                failed_inc,
                now
            ],
        )?;

        Ok(())
    }

    fn get_activity_since(&self, since: i64) -> Result<Vec<ActivityLogEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT hour_bucket, albums_downloaded, tracks_downloaded, images_downloaded,
                      bytes_downloaded, failed_count
               FROM download_activity_log
               WHERE hour_bucket >= ?1
               ORDER BY hour_bucket ASC"#,
        )?;

        let entries = stmt
            .query_map([since], |row| {
                Ok(ActivityLogEntry {
                    hour_bucket: row.get("hour_bucket")?,
                    albums_downloaded: row.get("albums_downloaded")?,
                    tracks_downloaded: row.get("tracks_downloaded")?,
                    images_downloaded: row.get("images_downloaded")?,
                    bytes_downloaded: row.get("bytes_downloaded")?,
                    failed_count: row.get("failed_count")?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(entries)
    }

    fn get_hourly_counts(&self) -> Result<HourlyCounts> {
        let conn = self.conn.lock().unwrap();
        let hour_bucket = Self::hour_bucket();

        let result = conn
            .query_row(
                r#"SELECT albums_downloaded, tracks_downloaded, images_downloaded, bytes_downloaded
                   FROM download_activity_log
                   WHERE hour_bucket = ?1"#,
                [hour_bucket],
                |row| {
                    Ok(HourlyCounts {
                        albums: row.get("albums_downloaded")?,
                        tracks: row.get("tracks_downloaded")?,
                        images: row.get("images_downloaded")?,
                        bytes: row.get("bytes_downloaded")?,
                    })
                },
            )
            .optional()?;

        Ok(result.unwrap_or_default())
    }

    fn get_daily_counts(&self) -> Result<DailyCounts> {
        let conn = self.conn.lock().unwrap();
        let day_start = Self::day_start_bucket();

        let result = conn.query_row(
            r#"SELECT
                       COALESCE(SUM(albums_downloaded), 0) as albums,
                       COALESCE(SUM(tracks_downloaded), 0) as tracks,
                       COALESCE(SUM(images_downloaded), 0) as images,
                       COALESCE(SUM(bytes_downloaded), 0) as bytes
                   FROM download_activity_log
                   WHERE hour_bucket >= ?1"#,
            [day_start],
            |row| {
                Ok(DailyCounts {
                    albums: row.get("albums")?,
                    tracks: row.get("tracks")?,
                    images: row.get("images")?,
                    bytes: row.get("bytes")?,
                })
            },
        )?;

        Ok(result)
    }

    fn get_stats_history(
        &self,
        period: StatsPeriod,
        custom_since: Option<i64>,
        custom_until: Option<i64>,
    ) -> Result<DownloadStatsHistory> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();

        // Calculate time range and grouping based on period
        let (default_since, group_seconds) = match period {
            StatsPeriod::Hourly => {
                // Last 48 hours, grouped by hour
                (now - 48 * 3600, 3600)
            }
            StatsPeriod::Daily => {
                // Last 30 days, grouped by day
                (now - 30 * 24 * 3600, 24 * 3600)
            }
            StatsPeriod::Weekly => {
                // Last 12 weeks, grouped by week
                (now - 12 * 7 * 24 * 3600, 7 * 24 * 3600)
            }
        };

        // Use custom since if provided, otherwise use period default
        let since = custom_since.unwrap_or(default_since);

        // Truncate to period boundary
        let since_bucket = (since / group_seconds) * group_seconds;

        // Build query based on whether we have an upper bound
        let entries = if let Some(until) = custom_until {
            let until_bucket = (until / group_seconds) * group_seconds + group_seconds;
            let mut stmt = conn.prepare(
                r#"SELECT
                       (hour_bucket / ?1) * ?1 as period_start,
                       COALESCE(SUM(albums_downloaded), 0) as albums,
                       COALESCE(SUM(tracks_downloaded), 0) as tracks,
                       COALESCE(SUM(images_downloaded), 0) as images,
                       COALESCE(SUM(bytes_downloaded), 0) as bytes,
                       COALESCE(SUM(failed_count), 0) as failures
                   FROM download_activity_log
                   WHERE hour_bucket >= ?2 AND hour_bucket < ?3
                   GROUP BY period_start
                   ORDER BY period_start ASC"#,
            )?;
            let rows = stmt.query_map(
                rusqlite::params![group_seconds, since_bucket, until_bucket],
                |row| {
                    Ok(StatsHistoryEntry {
                        period_start: row.get("period_start")?,
                        albums: row.get("albums")?,
                        tracks: row.get("tracks")?,
                        images: row.get("images")?,
                        bytes: row.get("bytes")?,
                        failures: row.get("failures")?,
                    })
                },
            )?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            let mut stmt = conn.prepare(
                r#"SELECT
                       (hour_bucket / ?1) * ?1 as period_start,
                       COALESCE(SUM(albums_downloaded), 0) as albums,
                       COALESCE(SUM(tracks_downloaded), 0) as tracks,
                       COALESCE(SUM(images_downloaded), 0) as images,
                       COALESCE(SUM(bytes_downloaded), 0) as bytes,
                       COALESCE(SUM(failed_count), 0) as failures
                   FROM download_activity_log
                   WHERE hour_bucket >= ?2
                   GROUP BY period_start
                   ORDER BY period_start ASC"#,
            )?;
            let rows = stmt.query_map(rusqlite::params![group_seconds, since_bucket], |row| {
                Ok(StatsHistoryEntry {
                    period_start: row.get("period_start")?,
                    albums: row.get("albums")?,
                    tracks: row.get("tracks")?,
                    images: row.get("images")?,
                    bytes: row.get("bytes")?,
                    failures: row.get("failures")?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        Ok(DownloadStatsHistory::new(period, entries))
    }

    // === Statistics ===

    fn get_queue_stats(&self) -> Result<QueueStats> {
        let conn = self.conn.lock().unwrap();
        let day_start = Self::day_start_bucket();

        let stats = conn.query_row(
            r#"SELECT
                   COALESCE(SUM(CASE WHEN status = 'PENDING' THEN 1 ELSE 0 END), 0) as pending,
                   COALESCE(SUM(CASE WHEN status = 'IN_PROGRESS' THEN 1 ELSE 0 END), 0) as in_progress,
                   COALESCE(SUM(CASE WHEN status = 'RETRY_WAITING' THEN 1 ELSE 0 END), 0) as retry_waiting,
                   COALESCE(SUM(CASE WHEN status = 'COMPLETED' AND completed_at >= ?1 THEN 1 ELSE 0 END), 0) as completed_today,
                   COALESCE(SUM(CASE WHEN status = 'FAILED' AND completed_at >= ?1 THEN 1 ELSE 0 END), 0) as failed_today
               FROM download_queue"#,
            [day_start],
            |row| {
                Ok(QueueStats {
                    pending: row.get("pending")?,
                    in_progress: row.get("in_progress")?,
                    retry_waiting: row.get("retry_waiting")?,
                    completed_today: row.get("completed_today")?,
                    failed_today: row.get("failed_today")?,
                })
            },
        )?;

        Ok(stats)
    }

    fn get_failed_items(&self, limit: usize, offset: usize) -> Result<Vec<QueueItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT * FROM download_queue
               WHERE status = 'FAILED'
               ORDER BY completed_at DESC
               LIMIT ?1 OFFSET ?2"#,
        )?;

        let items = stmt
            .query_map(
                rusqlite::params![limit as i64, offset as i64],
                Self::row_to_queue_item,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    fn get_stale_in_progress(&self, stale_threshold_secs: i64) -> Result<Vec<QueueItem>> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();
        let threshold = now - stale_threshold_secs;

        let mut stmt = conn.prepare(
            r#"SELECT * FROM download_queue
               WHERE status = 'IN_PROGRESS' AND started_at < ?1
               ORDER BY started_at ASC"#,
        )?;

        let items = stmt
            .query_map([threshold], Self::row_to_queue_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    // === Audit Logging ===

    fn log_audit_event(&self, event: AuditLogEntry) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            r#"INSERT INTO download_audit_log (
                   timestamp, event_type, queue_item_id, content_type, content_id,
                   user_id, request_source, details
               ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            rusqlite::params![
                event.timestamp,
                event.event_type.as_str(),
                event.queue_item_id,
                event.content_type.as_ref().map(|ct| ct.as_str()),
                event.content_id,
                event.user_id,
                event.request_source.as_ref().map(|rs| rs.as_str()),
                event.details.as_ref().map(|d| d.to_string()),
            ],
        )?;

        Ok(())
    }

    fn get_audit_log(&self, filter: AuditLogFilter) -> Result<(Vec<AuditLogEntry>, usize)> {
        let conn = self.conn.lock().unwrap();

        // Build WHERE clauses dynamically
        let mut conditions: Vec<String> = vec![];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(ref queue_item_id) = filter.queue_item_id {
            conditions.push(format!("queue_item_id = ?{}", params.len() + 1));
            params.push(Box::new(queue_item_id.clone()));
        }

        if let Some(ref user_id) = filter.user_id {
            conditions.push(format!("user_id = ?{}", params.len() + 1));
            params.push(Box::new(user_id.clone()));
        }

        if let Some(ref content_type) = filter.content_type {
            conditions.push(format!("content_type = ?{}", params.len() + 1));
            params.push(Box::new(content_type.as_str().to_string()));
        }

        if let Some(ref content_id) = filter.content_id {
            conditions.push(format!("content_id = ?{}", params.len() + 1));
            params.push(Box::new(content_id.clone()));
        }

        if let Some(since) = filter.since {
            conditions.push(format!("timestamp >= ?{}", params.len() + 1));
            params.push(Box::new(since));
        }

        if let Some(until) = filter.until {
            conditions.push(format!("timestamp <= ?{}", params.len() + 1));
            params.push(Box::new(until));
        }

        if let Some(ref event_types) = filter.event_types {
            if !event_types.is_empty() {
                let placeholders: Vec<String> = event_types
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("?{}", params.len() + i + 1))
                    .collect();
                conditions.push(format!("event_type IN ({})", placeholders.join(", ")));
                for et in event_types {
                    params.push(Box::new(et.as_str().to_string()));
                }
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Get total count first
        let count_sql = format!("SELECT COUNT(*) FROM download_audit_log {}", where_clause);
        let total: usize = {
            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            conn.query_row(&count_sql, params_refs.as_slice(), |row| {
                row.get::<_, i64>(0)
            })? as usize
        };

        // Now get the actual rows with pagination
        let select_sql = format!(
            "SELECT * FROM download_audit_log {} ORDER BY timestamp DESC LIMIT ?{} OFFSET ?{}",
            where_clause,
            params.len() + 1,
            params.len() + 2
        );

        params.push(Box::new(filter.limit as i64));
        params.push(Box::new(filter.offset as i64));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&select_sql)?;
        let entries = stmt
            .query_map(params_refs.as_slice(), Self::row_to_audit_entry)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok((entries, total))
    }

    fn get_audit_for_item(&self, queue_item_id: &str) -> Result<Vec<AuditLogEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT * FROM download_audit_log
               WHERE queue_item_id = ?1
               ORDER BY timestamp ASC"#,
        )?;

        let entries = stmt
            .query_map([queue_item_id], Self::row_to_audit_entry)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(entries)
    }

    fn get_audit_for_user(
        &self,
        user_id: &str,
        since: Option<i64>,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<AuditLogEntry>, usize)> {
        let conn = self.conn.lock().unwrap();

        // Build conditions
        let mut conditions = vec!["user_id = ?1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(user_id.to_string())];

        if let Some(s) = since {
            conditions.push(format!("timestamp >= ?{}", params.len() + 1));
            params.push(Box::new(s));
        }

        if let Some(u) = until {
            conditions.push(format!("timestamp <= ?{}", params.len() + 1));
            params.push(Box::new(u));
        }

        let where_clause = format!("WHERE {}", conditions.join(" AND "));

        // Get total count
        let count_sql = format!("SELECT COUNT(*) FROM download_audit_log {}", where_clause);
        let total: usize = {
            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            conn.query_row(&count_sql, params_refs.as_slice(), |row| {
                row.get::<_, i64>(0)
            })? as usize
        };

        // Get rows with pagination
        let select_sql = format!(
            "SELECT * FROM download_audit_log {} ORDER BY timestamp DESC LIMIT ?{} OFFSET ?{}",
            where_clause,
            params.len() + 1,
            params.len() + 2
        );

        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&select_sql)?;
        let entries = stmt
            .query_map(params_refs.as_slice(), Self::row_to_audit_entry)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok((entries, total))
    }

    fn cleanup_old_audit_entries(&self, older_than: i64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();

        let rows_deleted = conn.execute(
            "DELETE FROM download_audit_log WHERE timestamp < ?1",
            [older_than],
        )?;

        Ok(rows_deleted)
    }
}

include!("queue_store_tests.rs");
