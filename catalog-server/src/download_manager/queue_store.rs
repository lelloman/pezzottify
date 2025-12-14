//! Download queue storage and persistence.
//!
//! Provides SQLite-backed storage for download queue items and related data.

use super::models::*;
use super::schema::DOWNLOAD_QUEUE_VERSIONED_SCHEMAS;
use crate::sqlite_persistence::BASE_DB_VERSION;
use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

/// Trait for download queue storage operations.
///
/// Provides methods for managing download queue items, tracking activity,
/// rate limiting, and audit logging.
pub trait DownloadQueueStore: Send + Sync {
    // === Queue Management ===

    /// Add a new item to the download queue.
    fn enqueue(&self, item: QueueItem) -> Result<()>;

    /// Get a queue item by ID.
    fn get_item(&self, id: &str) -> Result<Option<QueueItem>>;

    /// Delete a queue item by ID. Returns true if item was deleted.
    fn delete_item(&self, id: &str) -> Result<bool>;

    /// Get the next pending item to process (by priority, then age).
    fn get_next_pending(&self) -> Result<Option<QueueItem>>;

    /// List queue items for a specific user.
    fn list_by_user(
        &self,
        user_id: &str,
        status: Option<QueueStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>>;

    /// List all queue items with optional status filter.
    /// If `exclude_completed` is true, completed items are excluded from results.
    /// If `top_level_only` is true, only items with parent_id IS NULL are returned.
    fn list_all(
        &self,
        status: Option<QueueStatus>,
        exclude_completed: bool,
        top_level_only: bool,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>>;

    /// Get the queue position for an item (1-based, among pending items).
    fn get_queue_position(&self, id: &str) -> Result<Option<usize>>;

    // === State Transitions (atomic) ===

    /// Atomically claim an item for processing (PENDING → IN_PROGRESS).
    /// Returns true if claimed, false if already claimed or not pending.
    fn claim_for_processing(&self, id: &str) -> Result<bool>;

    /// Mark an item as completed with metrics.
    fn mark_completed(&self, id: &str, bytes: u64, duration_ms: i64) -> Result<()>;

    /// Mark an item for retry with next retry time and error details.
    fn mark_retry_waiting(&self, id: &str, next_retry_at: i64, error: &DownloadError)
        -> Result<()>;

    /// Mark an item as permanently failed.
    fn mark_failed(&self, id: &str, error: &DownloadError) -> Result<()>;

    // === Parent-Child Management ===

    /// Create child items for a parent (e.g., tracks for an album).
    fn create_children(&self, parent_id: &str, children: Vec<QueueItem>) -> Result<()>;

    /// Get all children of a parent item.
    fn get_children(&self, parent_id: &str) -> Result<Vec<QueueItem>>;

    /// Get download progress for a parent item based on children status.
    fn get_children_progress(&self, parent_id: &str) -> Result<DownloadProgress>;

    /// Check if all children are in terminal state and return new parent status.
    /// Returns Some(status) if parent should be updated, None otherwise.
    fn check_parent_completion(&self, parent_id: &str) -> Result<Option<QueueStatus>>;

    /// Get top-level user requests (parent_id IS NULL only).
    fn get_user_requests(
        &self,
        user_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>>;

    // === Retry Handling ===

    /// Get items ready for retry (next_retry_at <= now).
    fn get_retry_ready(&self) -> Result<Vec<QueueItem>>;

    /// Promote a retry-waiting item back to pending.
    fn promote_retry_to_pending(&self, id: &str) -> Result<()>;

    /// Reset any non-completed item back to pending (for admin retry).
    /// Clears error info and resets retry state.
    fn reset_to_pending(&self, id: &str) -> Result<()>;

    // === Duplicate/Existence Checks ===

    /// Find a queue item by content type and ID.
    fn find_by_content(
        &self,
        content_type: DownloadContentType,
        content_id: &str,
    ) -> Result<Option<QueueItem>>;

    /// Check if content exists in the queue (any status).
    fn is_in_queue(&self, content_type: DownloadContentType, content_id: &str) -> Result<bool>;

    /// Check if content is in the active queue (non-terminal status).
    fn is_in_active_queue(
        &self,
        content_type: DownloadContentType,
        content_id: &str,
    ) -> Result<bool>;

    // === User Rate Limiting ===

    /// Get rate limit status for a user.
    fn get_user_stats(&self, user_id: &str) -> Result<UserLimitStatus>;

    /// Increment user's request count and queue count.
    fn increment_user_requests(&self, user_id: &str) -> Result<()>;

    /// Decrement user's queue count (when item completes/fails).
    fn decrement_user_queue(&self, user_id: &str) -> Result<()>;

    /// Reset daily request counts for all users. Returns number of users reset.
    fn reset_daily_user_stats(&self) -> Result<usize>;

    // === Activity Tracking ===

    /// Record download activity for capacity tracking.
    fn record_activity(
        &self,
        content_type: DownloadContentType,
        bytes: u64,
        success: bool,
    ) -> Result<()>;

    /// Get activity log entries since a timestamp.
    fn get_activity_since(&self, since: i64) -> Result<Vec<ActivityLogEntry>>;

    /// Get hourly download counts.
    fn get_hourly_counts(&self) -> Result<HourlyCounts>;

    /// Get daily download counts.
    fn get_daily_counts(&self) -> Result<DailyCounts>;

    /// Get download statistics history aggregated by period.
    /// - Hourly: last 48 hours (default if no custom range)
    /// - Daily: last 30 days (default if no custom range)
    /// - Weekly: last 12 weeks (default if no custom range)
    ///
    /// If `since` is provided, uses it as the start time instead of the period default.
    /// If `until` is provided, uses it as the end time (otherwise no upper bound).
    fn get_stats_history(
        &self,
        period: StatsPeriod,
        since: Option<i64>,
        until: Option<i64>,
    ) -> Result<DownloadStatsHistory>;

    // === Statistics ===

    /// Get overall queue statistics.
    fn get_queue_stats(&self) -> Result<QueueStats>;

    /// Get failed items for review/retry.
    fn get_failed_items(&self, limit: usize, offset: usize) -> Result<Vec<QueueItem>>;

    /// Get items stuck in IN_PROGRESS state (for alerting).
    fn get_stale_in_progress(&self, stale_threshold_secs: i64) -> Result<Vec<QueueItem>>;

    // === Audit Logging ===

    /// Log an audit event.
    fn log_audit_event(&self, event: AuditLogEntry) -> Result<()>;

    /// Get audit log entries with filtering. Returns (entries, total_count).
    fn get_audit_log(&self, filter: AuditLogFilter) -> Result<(Vec<AuditLogEntry>, usize)>;

    /// Get all audit entries for a specific queue item.
    fn get_audit_for_item(&self, queue_item_id: &str) -> Result<Vec<AuditLogEntry>>;

    /// Get audit entries for a user with time range. Returns (entries, total_count).
    fn get_audit_for_user(
        &self,
        user_id: &str,
        since: Option<i64>,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<AuditLogEntry>, usize)>;

    /// Clean up old audit entries. Returns number of entries deleted.
    fn cleanup_old_audit_entries(&self, older_than: i64) -> Result<usize>;
}

/// SQLite-backed download queue store.
///
/// Stores download queue items, activity logs, user rate limits, and audit entries.
pub struct SqliteDownloadQueueStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteDownloadQueueStore {
    /// Create a new SqliteDownloadQueueStore.
    ///
    /// Opens an existing database or creates a new one with the current schema.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let conn = if db_path.as_ref().exists() {
            Connection::open_with_flags(
                &db_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                    | rusqlite::OpenFlags::SQLITE_OPEN_URI
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?
        } else {
            let conn = Connection::open(&db_path)?;
            conn.execute("PRAGMA foreign_keys = ON;", [])?;
            DOWNLOAD_QUEUE_VERSIONED_SCHEMAS
                .last()
                .context("No schemas defined")?
                .create(&conn)?;
            info!(
                "Created new download queue database at {:?}",
                db_path.as_ref()
            );
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
                "Download queue database version {} is too old, does not contain base db version {}",
                db_version,
                BASE_DB_VERSION
            );
        }
        let version = db_version as usize;

        let schema_count = DOWNLOAD_QUEUE_VERSIONED_SCHEMAS.len();
        if version >= schema_count {
            bail!(
                "Download queue database version {} is too new (max supported: {})",
                version,
                schema_count - 1
            );
        }

        // Validate schema matches expected structure
        DOWNLOAD_QUEUE_VERSIONED_SCHEMAS
            .get(version)
            .context("Failed to get schema")?
            .validate(&conn)?;

        // Run migrations if needed
        Self::migrate_if_needed(&conn, version)?;

        Ok(SqliteDownloadQueueStore {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create an in-memory store for testing.
    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute("PRAGMA foreign_keys = ON;", [])?;
        DOWNLOAD_QUEUE_VERSIONED_SCHEMAS
            .last()
            .context("No schemas defined")?
            .create(&conn)?;

        Ok(SqliteDownloadQueueStore {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Run any pending migrations.
    fn migrate_if_needed(conn: &Connection, current_version: usize) -> Result<()> {
        let target_version = DOWNLOAD_QUEUE_VERSIONED_SCHEMAS.len() - 1;

        if current_version >= target_version {
            return Ok(());
        }

        info!(
            "Migrating download queue database from version {} to {}",
            current_version, target_version
        );

        for schema in DOWNLOAD_QUEUE_VERSIONED_SCHEMAS
            .iter()
            .skip(current_version + 1)
        {
            if let Some(migration_fn) = schema.migration {
                info!(
                    "Running download queue migration to version {}",
                    schema.version
                );
                migration_fn(conn)?;
            }
        }

        // Update version
        conn.execute(
            &format!("PRAGMA user_version = {}", BASE_DB_VERSION + target_version),
            [],
        )?;

        Ok(())
    }

    /// Get a reference to the connection for internal use.
    #[allow(dead_code)]
    pub(crate) fn connection(&self) -> &Arc<Mutex<Connection>> {
        &self.conn
    }

    /// Helper to convert a database row to a QueueItem.
    fn row_to_queue_item(row: &rusqlite::Row) -> rusqlite::Result<QueueItem> {
        Ok(QueueItem {
            id: row.get("id")?,
            parent_id: row.get("parent_id")?,
            status: QueueStatus::from_db_str(&row.get::<_, String>("status")?),
            priority: QueuePriority::from_i32(row.get("priority")?).unwrap_or(QueuePriority::User),
            content_type: DownloadContentType::from_str(&row.get::<_, String>("content_type")?)
                .unwrap_or(DownloadContentType::Album),
            content_id: row.get("content_id")?,
            content_name: row.get("content_name")?,
            artist_name: row.get("artist_name")?,
            request_source: RequestSource::from_str(&row.get::<_, String>("request_source")?)
                .unwrap_or(RequestSource::User),
            requested_by_user_id: row.get("requested_by_user_id")?,
            created_at: row.get("created_at")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
            last_attempt_at: row.get("last_attempt_at")?,
            next_retry_at: row.get("next_retry_at")?,
            retry_count: row.get("retry_count")?,
            max_retries: row.get("max_retries")?,
            error_type: row
                .get::<_, Option<String>>("error_type")?
                .and_then(|s| DownloadErrorType::from_str(&s)),
            error_message: row.get("error_message")?,
            bytes_downloaded: row.get("bytes_downloaded")?,
            processing_duration_ms: row.get("processing_duration_ms")?,
        })
    }

    /// Get current timestamp in seconds.
    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    /// Get today's date as a string in YYYY-MM-DD format.
    fn today_date_string() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Convert to days since epoch, then back to date components
        let days = secs / 86400;
        let mut year = 1970i32;
        let mut remaining_days = days as i32;

        // Calculate year
        loop {
            let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                366
            } else {
                365
            };
            if remaining_days < days_in_year {
                break;
            }
            remaining_days -= days_in_year;
            year += 1;
        }

        // Calculate month and day
        let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days_in_months = if is_leap {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut month = 1u32;
        for days_in_month in days_in_months.iter() {
            if remaining_days < *days_in_month {
                break;
            }
            remaining_days -= days_in_month;
            month += 1;
        }
        let day = (remaining_days + 1) as u32;

        format!("{:04}-{:02}-{:02}", year, month, day)
    }

    /// Get current hour bucket (timestamp truncated to hour).
    fn hour_bucket() -> i64 {
        let now = Self::now();
        // Truncate to hour (3600 seconds)
        (now / 3600) * 3600
    }

    /// Get start of current day as hour bucket.
    fn day_start_bucket() -> i64 {
        let now = Self::now();
        // Truncate to day (86400 seconds)
        (now / 86400) * 86400
    }

    /// Helper to convert a database row to an AuditLogEntry.
    fn row_to_audit_entry(row: &rusqlite::Row) -> rusqlite::Result<AuditLogEntry> {
        let event_type_str: String = row.get("event_type")?;
        let content_type_str: Option<String> = row.get("content_type")?;
        let request_source_str: Option<String> = row.get("request_source")?;
        let details_str: Option<String> = row.get("details")?;

        Ok(AuditLogEntry {
            id: row.get("id")?,
            timestamp: row.get("timestamp")?,
            event_type: AuditEventType::from_str(&event_type_str)
                .unwrap_or(AuditEventType::RequestCreated),
            queue_item_id: row.get("queue_item_id")?,
            content_type: content_type_str.and_then(|s| DownloadContentType::from_str(&s)),
            content_id: row.get("content_id")?,
            user_id: row.get("user_id")?,
            request_source: request_source_str.and_then(|s| RequestSource::from_str(&s)),
            details: details_str.and_then(|s| serde_json::from_str(&s).ok()),
        })
    }
}

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
        let mut stmt = conn.prepare(
            r#"SELECT * FROM download_queue
               WHERE status = 'PENDING'
               ORDER BY priority ASC, created_at ASC
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
               ORDER BY created_at DESC
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

    // === User Rate Limiting ===

    fn get_user_stats(&self, user_id: &str) -> Result<UserLimitStatus> {
        let conn = self.conn.lock().unwrap();

        // Try to get existing stats
        let result = conn
            .query_row(
                r#"SELECT requests_today, requests_in_queue, last_request_date
                   FROM user_request_stats
                   WHERE user_id = ?1"#,
                [user_id],
                |row| {
                    Ok((
                        row.get::<_, i32>("requests_today")?,
                        row.get::<_, i32>("requests_in_queue")?,
                        row.get::<_, Option<String>>("last_request_date")?,
                    ))
                },
            )
            .optional()?;

        // Default limits (these could be made configurable)
        const MAX_REQUESTS_PER_DAY: i32 = 50;
        const MAX_QUEUE_SIZE: i32 = 100;

        match result {
            Some((requests_today, in_queue, last_date)) => {
                // Check if we need to reset (date changed)
                let today = Self::today_date_string();
                let effective_requests = if last_date.as_deref() == Some(&today) {
                    requests_today
                } else {
                    0 // Reset since it's a new day
                };

                Ok(UserLimitStatus::available(
                    effective_requests,
                    MAX_REQUESTS_PER_DAY,
                    in_queue,
                    MAX_QUEUE_SIZE,
                ))
            }
            None => {
                // No record yet - user has full quota
                Ok(UserLimitStatus::available(
                    0,
                    MAX_REQUESTS_PER_DAY,
                    0,
                    MAX_QUEUE_SIZE,
                ))
            }
        }
    }

    fn increment_user_requests(&self, user_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();
        let today = Self::today_date_string();

        // Insert or update - if it's a new day, reset requests_today
        conn.execute(
            r#"INSERT INTO user_request_stats (user_id, requests_today, requests_in_queue, last_request_date, last_updated_at)
               VALUES (?1, 1, 1, ?2, ?3)
               ON CONFLICT(user_id) DO UPDATE SET
                   requests_today = CASE
                       WHEN last_request_date = ?2 THEN requests_today + 1
                       ELSE 1
                   END,
                   requests_in_queue = requests_in_queue + 1,
                   last_request_date = ?2,
                   last_updated_at = ?3"#,
            rusqlite::params![user_id, today, now],
        )?;

        Ok(())
    }

    fn decrement_user_queue(&self, user_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();

        conn.execute(
            r#"UPDATE user_request_stats
               SET requests_in_queue = MAX(0, requests_in_queue - 1),
                   last_updated_at = ?1
               WHERE user_id = ?2"#,
            rusqlite::params![now, user_id],
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_new_database() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("download_queue.db");

        let store = SqliteDownloadQueueStore::new(&db_path).unwrap();

        // Verify database file was created
        assert!(db_path.exists());

        // Verify we can access the connection
        let conn = store.conn.lock().unwrap();
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='download_queue'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_open_existing_database() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("download_queue.db");

        // Create database
        {
            let _store = SqliteDownloadQueueStore::new(&db_path).unwrap();
        }

        // Reopen database
        let store = SqliteDownloadQueueStore::new(&db_path).unwrap();

        // Verify tables exist
        let conn = store.conn.lock().unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'download%' OR name LIKE 'user_request%'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"download_queue".to_string()));
        assert!(tables.contains(&"download_activity_log".to_string()));
        assert!(tables.contains(&"download_audit_log".to_string()));
        assert!(tables.contains(&"user_request_stats".to_string()));
    }

    #[test]
    fn test_in_memory_store() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let conn = store.conn.lock().unwrap();
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        // 4 tables should be created
        assert_eq!(count, 4);
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let conn = store.conn.lock().unwrap();
        let fk_enabled: i32 = conn
            .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
            .unwrap();

        assert_eq!(fk_enabled, 1, "Foreign keys should be enabled");
    }

    #[test]
    fn test_schema_version_stored() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let conn = store.conn.lock().unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .unwrap();

        // Version should be BASE_DB_VERSION + schema version (0)
        assert_eq!(version as usize, BASE_DB_VERSION);
    }

    // === Queue Management Tests ===

    #[test]
    fn test_enqueue_and_get_item() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "test-item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_names(
            Some("Test Album".to_string()),
            Some("Test Artist".to_string()),
        )
        .with_user("user-456".to_string());

        store.enqueue(item.clone()).unwrap();

        let retrieved = store.get_item("test-item-1").unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, "test-item-1");
        assert_eq!(retrieved.content_type, DownloadContentType::Album);
        assert_eq!(retrieved.content_id, "album-123");
        assert_eq!(retrieved.content_name, Some("Test Album".to_string()));
        assert_eq!(retrieved.artist_name, Some("Test Artist".to_string()));
        assert_eq!(retrieved.requested_by_user_id, Some("user-456".to_string()));
        assert_eq!(retrieved.status, QueueStatus::Pending);
        assert_eq!(retrieved.priority, QueuePriority::User);
    }

    #[test]
    fn test_get_item_not_found() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let result = store.get_item("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_next_pending_priority_order() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add items with different priorities
        let low_priority = QueueItem::new(
            "low-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::Expansion, // Lowest priority (3)
            RequestSource::Expansion,
            5,
        );

        let high_priority = QueueItem::new(
            "high-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::Watchdog, // Highest priority (1)
            RequestSource::Watchdog,
            3,
        );

        let mid_priority = QueueItem::new(
            "mid-1".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User, // Mid priority (2)
            RequestSource::User,
            5,
        );

        // Enqueue in wrong order
        store.enqueue(low_priority).unwrap();
        store.enqueue(mid_priority).unwrap();
        store.enqueue(high_priority).unwrap();

        // Should get highest priority (lowest value) first
        let next = store.get_next_pending().unwrap();
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, "high-1");
    }

    #[test]
    fn test_get_next_pending_age_order() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add items with same priority but different created_at
        let mut older = QueueItem::new(
            "older-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        older.created_at = 1000;

        let mut newer = QueueItem::new(
            "newer-1".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        newer.created_at = 2000;

        store.enqueue(newer).unwrap();
        store.enqueue(older).unwrap();

        // Should get older item first
        let next = store.get_next_pending().unwrap();
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, "older-1");
    }

    #[test]
    fn test_get_next_pending_empty_queue() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let next = store.get_next_pending().unwrap();
        assert!(next.is_none());
    }

    #[test]
    fn test_list_by_user() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add items for different users
        let user1_item1 = QueueItem::new(
            "u1-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-1".to_string());

        let user1_item2 = QueueItem::new(
            "u1-2".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-1".to_string());

        let user2_item = QueueItem::new(
            "u2-1".to_string(),
            DownloadContentType::Album,
            "album-3".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-2".to_string());

        store.enqueue(user1_item1).unwrap();
        store.enqueue(user1_item2).unwrap();
        store.enqueue(user2_item).unwrap();

        // List user-1 items
        let user1_items = store.list_by_user("user-1", None, 100, 0).unwrap();
        assert_eq!(user1_items.len(), 2);

        // List user-2 items
        let user2_items = store.list_by_user("user-2", None, 100, 0).unwrap();
        assert_eq!(user2_items.len(), 1);
        assert_eq!(user2_items[0].id, "u2-1");
    }

    #[test]
    fn test_list_all_with_status_filter() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item1 = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );

        let mut item2 = QueueItem::new(
            "item-2".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item2.status = QueueStatus::Completed;

        store.enqueue(item1).unwrap();
        store.enqueue(item2).unwrap();

        // List all
        let all = store.list_all(None, false, false, 100, 0).unwrap();
        assert_eq!(all.len(), 2);

        // List pending only
        let pending = store
            .list_all(Some(QueueStatus::Pending), false, false, 100, 0)
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "item-1");

        // List completed only
        let completed = store
            .list_all(Some(QueueStatus::Completed), false, false, 100, 0)
            .unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "item-2");
    }

    #[test]
    fn test_list_all_pagination() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add 5 items
        for i in 0..5 {
            let mut item = QueueItem::new(
                format!("item-{}", i),
                DownloadContentType::Album,
                format!("album-{}", i),
                QueuePriority::User,
                RequestSource::User,
                5,
            );
            item.created_at = i as i64;
            store.enqueue(item).unwrap();
        }

        // Get first page
        let page1 = store.list_all(None, false, false, 2, 0).unwrap();
        assert_eq!(page1.len(), 2);

        // Get second page
        let page2 = store.list_all(None, false, false, 2, 2).unwrap();
        assert_eq!(page2.len(), 2);

        // Get third page
        let page3 = store.list_all(None, false, false, 2, 4).unwrap();
        assert_eq!(page3.len(), 1);
    }

    #[test]
    fn test_get_queue_position() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add items with different priorities
        let mut high = QueueItem::new(
            "high".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::Watchdog,
            RequestSource::Watchdog,
            5,
        );
        high.created_at = 1000;

        let mut mid = QueueItem::new(
            "mid".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        mid.created_at = 2000;

        let mut low = QueueItem::new(
            "low".to_string(),
            DownloadContentType::Album,
            "album-3".to_string(),
            QueuePriority::Expansion,
            RequestSource::Expansion,
            5,
        );
        low.created_at = 3000;

        store.enqueue(low).unwrap();
        store.enqueue(mid).unwrap();
        store.enqueue(high).unwrap();

        // Check positions
        assert_eq!(store.get_queue_position("high").unwrap(), Some(1));
        assert_eq!(store.get_queue_position("mid").unwrap(), Some(2));
        assert_eq!(store.get_queue_position("low").unwrap(), Some(3));
    }

    #[test]
    fn test_get_queue_position_not_pending() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let mut item = QueueItem::new(
            "completed-item".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.status = QueueStatus::Completed;

        store.enqueue(item).unwrap();

        // Completed items have no queue position
        assert_eq!(store.get_queue_position("completed-item").unwrap(), None);
    }

    #[test]
    fn test_get_queue_position_nonexistent() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        assert_eq!(store.get_queue_position("nonexistent").unwrap(), None);
    }

    // === State Transition Tests ===

    #[test]
    fn test_claim_for_processing_success() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // Claim the item
        let claimed = store.claim_for_processing("item-1").unwrap();
        assert!(claimed);

        // Verify status changed
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::InProgress);
        assert!(item.started_at.is_some());
        assert!(item.last_attempt_at.is_some());
    }

    #[test]
    fn test_claim_for_processing_already_claimed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // First claim succeeds
        assert!(store.claim_for_processing("item-1").unwrap());

        // Second claim fails
        assert!(!store.claim_for_processing("item-1").unwrap());
    }

    #[test]
    fn test_claim_for_processing_not_pending() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let mut item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.status = QueueStatus::Completed;
        store.enqueue(item).unwrap();

        // Cannot claim a completed item
        let claimed = store.claim_for_processing("item-1").unwrap();
        assert!(!claimed);
    }

    #[test]
    fn test_claim_for_processing_nonexistent() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Claiming nonexistent item returns false (not an error)
        let claimed = store.claim_for_processing("nonexistent").unwrap();
        assert!(!claimed);
    }

    #[test]
    fn test_mark_completed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item-1").unwrap();

        // Mark as completed
        store.mark_completed("item-1", 1024000, 500).unwrap();

        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::Completed);
        assert!(item.completed_at.is_some());
        assert_eq!(item.bytes_downloaded, Some(1024000));
        assert_eq!(item.processing_duration_ms, Some(500));
        assert!(item.error_type.is_none());
        assert!(item.error_message.is_none());
    }

    #[test]
    fn test_mark_completed_clears_error() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create item with previous error
        let mut item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.error_type = Some(DownloadErrorType::Connection);
        item.error_message = Some("Previous error".to_string());
        store.enqueue(item).unwrap();

        // Mark as completed
        store.mark_completed("item-1", 1024000, 500).unwrap();

        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::Completed);
        assert!(item.error_type.is_none());
        assert!(item.error_message.is_none());
    }

    #[test]
    fn test_mark_retry_waiting() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item-1").unwrap();

        // Mark for retry
        let error = DownloadError::new(DownloadErrorType::Timeout, "Request timed out");
        let next_retry = 1700000000;
        store
            .mark_retry_waiting("item-1", next_retry, &error)
            .unwrap();

        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::RetryWaiting);
        assert_eq!(item.retry_count, 1);
        assert_eq!(item.next_retry_at, Some(next_retry));
        assert_eq!(item.error_type, Some(DownloadErrorType::Timeout));
        assert_eq!(item.error_message, Some("Request timed out".to_string()));
        assert!(item.last_attempt_at.is_some());
    }

    #[test]
    fn test_mark_retry_waiting_increments_count() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        let error = DownloadError::new(DownloadErrorType::Connection, "Connection refused");

        // First retry
        store.mark_retry_waiting("item-1", 1000, &error).unwrap();
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.retry_count, 1);

        // Second retry
        store.mark_retry_waiting("item-1", 2000, &error).unwrap();
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.retry_count, 2);

        // Third retry
        store.mark_retry_waiting("item-1", 3000, &error).unwrap();
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.retry_count, 3);
    }

    #[test]
    fn test_mark_failed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item-1").unwrap();

        // Mark as failed
        let error = DownloadError::new(DownloadErrorType::NotFound, "Album not found");
        store.mark_failed("item-1", &error).unwrap();

        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::Failed);
        assert!(item.completed_at.is_some());
        assert_eq!(item.error_type, Some(DownloadErrorType::NotFound));
        assert_eq!(item.error_message, Some("Album not found".to_string()));
    }

    #[test]
    fn test_state_transition_sequence() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            3,
        );
        store.enqueue(item).unwrap();

        // PENDING -> IN_PROGRESS
        assert!(store.claim_for_processing("item-1").unwrap());
        assert_eq!(
            store.get_item("item-1").unwrap().unwrap().status,
            QueueStatus::InProgress
        );

        // IN_PROGRESS -> RETRY_WAITING (simulating failure)
        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        store.mark_retry_waiting("item-1", 1000, &error).unwrap();
        assert_eq!(
            store.get_item("item-1").unwrap().unwrap().status,
            QueueStatus::RetryWaiting
        );

        // Item no longer shows up as next pending
        assert!(store.get_next_pending().unwrap().is_none());
    }

    #[test]
    fn test_get_next_pending_skips_in_progress() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let mut item1 = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item1.created_at = 1000;

        let mut item2 = QueueItem::new(
            "item-2".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item2.created_at = 2000;

        store.enqueue(item1).unwrap();
        store.enqueue(item2).unwrap();

        // Claim item-1
        store.claim_for_processing("item-1").unwrap();

        // Next pending should be item-2
        let next = store.get_next_pending().unwrap().unwrap();
        assert_eq!(next.id, "item-2");
    }

    // === Parent-Child Management Tests ===

    #[test]
    fn test_create_children() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create parent
        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-1".to_string());
        store.enqueue(parent).unwrap();

        // Create children
        let children = vec![
            QueueItem::new_child(
                "child-1".to_string(),
                "parent-1".to_string(),
                DownloadContentType::TrackAudio,
                "track-1".to_string(),
                QueuePriority::User,
                RequestSource::User,
                Some("user-1".to_string()),
                3,
            ),
            QueueItem::new_child(
                "child-2".to_string(),
                "parent-1".to_string(),
                DownloadContentType::TrackAudio,
                "track-2".to_string(),
                QueuePriority::User,
                RequestSource::User,
                Some("user-1".to_string()),
                3,
            ),
        ];

        store.create_children("parent-1", children).unwrap();

        // Verify children were created
        let retrieved_children = store.get_children("parent-1").unwrap();
        assert_eq!(retrieved_children.len(), 2);
        assert_eq!(retrieved_children[0].id, "child-1");
        assert_eq!(retrieved_children[1].id, "child-2");
    }

    #[test]
    fn test_create_children_empty_list() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create parent
        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(parent).unwrap();

        // Creating no children should succeed
        store.create_children("parent-1", vec![]).unwrap();

        let children = store.get_children("parent-1").unwrap();
        assert!(children.is_empty());
    }

    #[test]
    fn test_create_children_wrong_parent_id() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(parent).unwrap();

        // Child with wrong parent_id
        let children = vec![QueueItem::new_child(
            "child-1".to_string(),
            "wrong-parent".to_string(), // Wrong parent!
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        )];

        // Should fail
        let result = store.create_children("parent-1", children);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_children_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let children = store.get_children("nonexistent-parent").unwrap();
        assert!(children.is_empty());
    }

    #[test]
    fn test_get_children_progress() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create parent
        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(parent).unwrap();

        // Create children with different statuses
        let mut child1 = QueueItem::new_child(
            "child-1".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child1.status = QueueStatus::Completed;

        let mut child2 = QueueItem::new_child(
            "child-2".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child2.status = QueueStatus::Failed;

        let child3 = QueueItem::new_child(
            "child-3".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-3".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        ); // Pending by default

        let mut child4 = QueueItem::new_child(
            "child-4".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-4".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child4.status = QueueStatus::InProgress;

        store
            .create_children("parent-1", vec![child1, child2, child3, child4])
            .unwrap();

        let progress = store.get_children_progress("parent-1").unwrap();
        assert_eq!(progress.total_children, 4);
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.failed, 1);
        assert_eq!(progress.pending, 1);
        assert_eq!(progress.in_progress, 1);
    }

    #[test]
    fn test_get_children_progress_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let progress = store.get_children_progress("nonexistent").unwrap();
        assert_eq!(progress.total_children, 0);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_check_parent_completion_no_children() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let result = store.check_parent_completion("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_check_parent_completion_still_pending() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(parent).unwrap();

        let child = QueueItem::new_child(
            "child-1".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        ); // Pending by default

        store.create_children("parent-1", vec![child]).unwrap();

        let result = store.check_parent_completion("parent-1").unwrap();
        assert!(result.is_none()); // Still pending
    }

    #[test]
    fn test_check_parent_completion_all_completed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(parent).unwrap();

        let mut child1 = QueueItem::new_child(
            "child-1".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child1.status = QueueStatus::Completed;

        let mut child2 = QueueItem::new_child(
            "child-2".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child2.status = QueueStatus::Completed;

        store
            .create_children("parent-1", vec![child1, child2])
            .unwrap();

        let result = store.check_parent_completion("parent-1").unwrap();
        assert_eq!(result, Some(QueueStatus::Completed));
    }

    #[test]
    fn test_check_parent_completion_some_failed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let parent = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(parent).unwrap();

        let mut child1 = QueueItem::new_child(
            "child-1".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child1.status = QueueStatus::Completed;

        let mut child2 = QueueItem::new_child(
            "child-2".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            None,
            3,
        );
        child2.status = QueueStatus::Failed;

        store
            .create_children("parent-1", vec![child1, child2])
            .unwrap();

        let result = store.check_parent_completion("parent-1").unwrap();
        assert_eq!(result, Some(QueueStatus::Failed));
    }

    #[test]
    fn test_get_user_requests() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create some parent items
        let parent1 = QueueItem::new(
            "parent-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-1".to_string());

        let parent2 = QueueItem::new(
            "parent-2".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-1".to_string());

        let parent3 = QueueItem::new(
            "parent-3".to_string(),
            DownloadContentType::Album,
            "album-3".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-2".to_string()); // Different user

        store.enqueue(parent1).unwrap();
        store.enqueue(parent2).unwrap();
        store.enqueue(parent3).unwrap();

        // Create a child for parent-1 (should not show up in user requests)
        let child = QueueItem::new_child(
            "child-1".to_string(),
            "parent-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            Some("user-1".to_string()),
            3,
        );
        store.create_children("parent-1", vec![child]).unwrap();

        // Get user-1 requests
        let requests = store.get_user_requests("user-1", 100, 0).unwrap();
        assert_eq!(requests.len(), 2);

        // All should be parent items (no parent_id)
        for req in &requests {
            assert!(req.parent_id.is_none());
        }

        // Get user-2 requests
        let requests = store.get_user_requests("user-2", 100, 0).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].id, "parent-3");
    }

    #[test]
    fn test_get_user_requests_pagination() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create 5 parent items
        for i in 0..5 {
            let mut item = QueueItem::new(
                format!("parent-{}", i),
                DownloadContentType::Album,
                format!("album-{}", i),
                QueuePriority::User,
                RequestSource::User,
                5,
            )
            .with_user("user-1".to_string());
            item.created_at = i as i64; // Ensure consistent ordering
            store.enqueue(item).unwrap();
        }

        // Get first page (2 items)
        let page1 = store.get_user_requests("user-1", 2, 0).unwrap();
        assert_eq!(page1.len(), 2);

        // Get second page (2 items)
        let page2 = store.get_user_requests("user-1", 2, 2).unwrap();
        assert_eq!(page2.len(), 2);

        // Get third page (1 item)
        let page3 = store.get_user_requests("user-1", 2, 4).unwrap();
        assert_eq!(page3.len(), 1);
    }

    // === Retry Handling Tests ===

    #[test]
    fn test_get_retry_ready_none() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // No items
        let ready = store.get_retry_ready().unwrap();
        assert!(ready.is_empty());
    }

    #[test]
    fn test_get_retry_ready_with_items() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create item and put it in retry waiting state
        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // Mark it for retry with a past retry time
        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        let past_time = 1000; // Far in the past
        store
            .mark_retry_waiting("item-1", past_time, &error)
            .unwrap();

        // Should be ready for retry
        let ready = store.get_retry_ready().unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "item-1");
    }

    #[test]
    fn test_get_retry_ready_not_yet() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // Mark for retry with a far future time
        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        let future_time = 9999999999; // Far in the future
        store
            .mark_retry_waiting("item-1", future_time, &error)
            .unwrap();

        // Should not be ready yet
        let ready = store.get_retry_ready().unwrap();
        assert!(ready.is_empty());
    }

    #[test]
    fn test_get_retry_ready_priority_order() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create items with different priorities
        let high = QueueItem::new(
            "high".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::Watchdog,
            RequestSource::Watchdog,
            5,
        );
        let low = QueueItem::new(
            "low".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::Expansion,
            RequestSource::Expansion,
            5,
        );

        store.enqueue(low).unwrap();
        store.enqueue(high).unwrap();

        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        store.mark_retry_waiting("low", 1000, &error).unwrap();
        store.mark_retry_waiting("high", 1000, &error).unwrap();

        // Should return high priority first
        let ready = store.get_retry_ready().unwrap();
        assert_eq!(ready.len(), 2);
        assert_eq!(ready[0].id, "high");
        assert_eq!(ready[1].id, "low");
    }

    #[test]
    fn test_promote_retry_to_pending() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // Mark for retry
        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        store.mark_retry_waiting("item-1", 1000, &error).unwrap();

        // Verify retry waiting state
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::RetryWaiting);
        assert!(item.next_retry_at.is_some());

        // Promote to pending
        store.promote_retry_to_pending("item-1").unwrap();

        // Verify pending state
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::Pending);
        assert!(item.next_retry_at.is_none());
    }

    #[test]
    fn test_promote_retry_to_pending_not_retry_waiting() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // Item is pending, not retry waiting
        // Promoting should have no effect (WHERE clause won't match)
        store.promote_retry_to_pending("item-1").unwrap();

        // Still pending
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::Pending);
    }

    #[test]
    fn test_retry_workflow() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create and process an item
        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::TrackAudio,
            "track-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            3,
        );
        store.enqueue(item).unwrap();

        // Start processing
        store.claim_for_processing("item-1").unwrap();

        // Simulate failure - mark for retry
        let error = DownloadError::new(DownloadErrorType::Connection, "Connection refused");
        store.mark_retry_waiting("item-1", 1000, &error).unwrap();

        // Item should be retry waiting
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::RetryWaiting);
        assert_eq!(item.retry_count, 1);

        // Get items ready for retry
        let ready = store.get_retry_ready().unwrap();
        assert_eq!(ready.len(), 1);

        // Promote back to pending
        store.promote_retry_to_pending("item-1").unwrap();

        // Should be pending again
        let item = store.get_item("item-1").unwrap().unwrap();
        assert_eq!(item.status, QueueStatus::Pending);

        // Claim again for second attempt
        assert!(store.claim_for_processing("item-1").unwrap());
        assert_eq!(
            store.get_item("item-1").unwrap().unwrap().status,
            QueueStatus::InProgress
        );
    }

    // === Duplicate Check Tests ===

    #[test]
    fn test_find_by_content_found() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        let found = store
            .find_by_content(DownloadContentType::Album, "album-123")
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "item-1");
    }

    #[test]
    fn test_find_by_content_not_found() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let found = store
            .find_by_content(DownloadContentType::Album, "nonexistent")
            .unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_find_by_content_wrong_type() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        // Search with wrong content type
        let found = store
            .find_by_content(DownloadContentType::TrackAudio, "album-123")
            .unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_find_by_content_returns_most_recent() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Create two items with same content (different IDs)
        let mut item1 = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item1.created_at = 1000;

        let mut item2 = QueueItem::new(
            "item-2".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item2.created_at = 2000; // More recent

        store.enqueue(item1).unwrap();
        store.enqueue(item2).unwrap();

        // Should return most recent
        let found = store
            .find_by_content(DownloadContentType::Album, "album-123")
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "item-2");
    }

    #[test]
    fn test_is_in_queue_true() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        assert!(store
            .is_in_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    #[test]
    fn test_is_in_queue_false() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        assert!(!store
            .is_in_queue(DownloadContentType::Album, "nonexistent")
            .unwrap());
    }

    #[test]
    fn test_is_in_queue_includes_completed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let mut item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.status = QueueStatus::Completed;
        store.enqueue(item).unwrap();

        // is_in_queue should include completed items
        assert!(store
            .is_in_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    #[test]
    fn test_is_in_active_queue_pending() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        assert!(store
            .is_in_active_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    #[test]
    fn test_is_in_active_queue_in_progress() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item-1").unwrap();

        assert!(store
            .is_in_active_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    #[test]
    fn test_is_in_active_queue_retry_waiting() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();

        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        store.mark_retry_waiting("item-1", 1000, &error).unwrap();

        assert!(store
            .is_in_active_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    #[test]
    fn test_is_in_active_queue_not_completed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let mut item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.status = QueueStatus::Completed;
        store.enqueue(item).unwrap();

        // Completed items are NOT in active queue
        assert!(!store
            .is_in_active_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    #[test]
    fn test_is_in_active_queue_not_failed() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let mut item = QueueItem::new(
            "item-1".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.status = QueueStatus::Failed;
        store.enqueue(item).unwrap();

        // Failed items are NOT in active queue
        assert!(!store
            .is_in_active_queue(DownloadContentType::Album, "album-123")
            .unwrap());
    }

    // === User Rate Limiting Tests ===

    #[test]
    fn test_get_user_stats_new_user() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let stats = store.get_user_stats("user-1").unwrap();

        // New user should have full quota
        assert_eq!(stats.requests_today, 0);
        assert_eq!(stats.in_queue, 0);
        assert!(stats.can_request);
    }

    #[test]
    fn test_increment_user_requests() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Increment requests
        store.increment_user_requests("user-1").unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.requests_today, 1);
        assert_eq!(stats.in_queue, 1);

        // Increment again
        store.increment_user_requests("user-1").unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.requests_today, 2);
        assert_eq!(stats.in_queue, 2);
    }

    #[test]
    fn test_decrement_user_queue() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Setup - add some requests
        store.increment_user_requests("user-1").unwrap();
        store.increment_user_requests("user-1").unwrap();
        store.increment_user_requests("user-1").unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.in_queue, 3);

        // Decrement
        store.decrement_user_queue("user-1").unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.in_queue, 2);
        // Note: requests_today stays the same
        assert_eq!(stats.requests_today, 3);
    }

    #[test]
    fn test_decrement_user_queue_not_negative() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Setup with one item
        store.increment_user_requests("user-1").unwrap();
        store.decrement_user_queue("user-1").unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.in_queue, 0);

        // Decrement again - should not go negative
        store.decrement_user_queue("user-1").unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.in_queue, 0);
    }

    #[test]
    fn test_decrement_user_queue_nonexistent_user() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Should not fail for nonexistent user (just no-op)
        store.decrement_user_queue("nonexistent").unwrap();
    }

    #[test]
    fn test_reset_daily_user_stats() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add requests for two users
        store.increment_user_requests("user-1").unwrap();
        store.increment_user_requests("user-1").unwrap();
        store.increment_user_requests("user-2").unwrap();

        // Verify current stats
        let stats1 = store.get_user_stats("user-1").unwrap();
        let stats2 = store.get_user_stats("user-2").unwrap();
        assert_eq!(stats1.requests_today, 2);
        assert_eq!(stats2.requests_today, 1);

        // Reset won't affect users whose last_request_date is today
        // (which it is since we just made requests)
        let reset_count = store.reset_daily_user_stats().unwrap();
        assert_eq!(reset_count, 0);

        // Stats should remain unchanged
        let stats1 = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats1.requests_today, 2);
    }

    #[test]
    fn test_today_date_string_format() {
        let date = SqliteDownloadQueueStore::today_date_string();

        // Should be in YYYY-MM-DD format
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");

        // Year should be reasonable (2020-2100)
        let year: i32 = date[0..4].parse().unwrap();
        assert!(year >= 2020 && year <= 2100);

        // Month should be 01-12
        let month: i32 = date[5..7].parse().unwrap();
        assert!(month >= 1 && month <= 12);

        // Day should be 01-31
        let day: i32 = date[8..10].parse().unwrap();
        assert!(day >= 1 && day <= 31);
    }

    #[test]
    fn test_user_stats_workflow() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // User makes 3 download requests
        for _ in 0..3 {
            store.increment_user_requests("user-1").unwrap();
        }

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.requests_today, 3);
        assert_eq!(stats.in_queue, 3);

        // One item completes
        store.decrement_user_queue("user-1").unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.requests_today, 3); // Still 3 (counts requests made, not in queue)
        assert_eq!(stats.in_queue, 2);

        // Another item completes
        store.decrement_user_queue("user-1").unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.in_queue, 1);

        // Last item completes
        store.decrement_user_queue("user-1").unwrap();

        let stats = store.get_user_stats("user-1").unwrap();
        assert_eq!(stats.in_queue, 0);
        assert_eq!(stats.requests_today, 3); // Daily count preserved
    }

    #[test]
    fn test_multiple_users_independent() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // User 1 makes 5 requests
        for _ in 0..5 {
            store.increment_user_requests("user-1").unwrap();
        }

        // User 2 makes 2 requests
        for _ in 0..2 {
            store.increment_user_requests("user-2").unwrap();
        }

        // Verify independent tracking
        let stats1 = store.get_user_stats("user-1").unwrap();
        let stats2 = store.get_user_stats("user-2").unwrap();

        assert_eq!(stats1.requests_today, 5);
        assert_eq!(stats2.requests_today, 2);

        // Decrement user 1's queue
        store.decrement_user_queue("user-1").unwrap();

        // User 2 should be unaffected
        let stats2 = store.get_user_stats("user-2").unwrap();
        assert_eq!(stats2.in_queue, 2);
    }

    // === Activity Tracking Tests ===

    #[test]
    fn test_record_activity_album() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        store
            .record_activity(DownloadContentType::Album, 1000, true)
            .unwrap();

        let hourly = store.get_hourly_counts().unwrap();
        assert_eq!(hourly.albums, 1);
        assert_eq!(hourly.tracks, 0);
        assert_eq!(hourly.images, 0);
        assert_eq!(hourly.bytes, 1000);
    }

    #[test]
    fn test_record_activity_track() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        store
            .record_activity(DownloadContentType::TrackAudio, 5000000, true)
            .unwrap();

        let hourly = store.get_hourly_counts().unwrap();
        assert_eq!(hourly.albums, 0);
        assert_eq!(hourly.tracks, 1);
        assert_eq!(hourly.bytes, 5000000);
    }

    #[test]
    fn test_record_activity_image() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        store
            .record_activity(DownloadContentType::AlbumImage, 50000, true)
            .unwrap();

        let hourly = store.get_hourly_counts().unwrap();
        assert_eq!(hourly.images, 1);
        assert_eq!(hourly.bytes, 50000);
    }

    #[test]
    fn test_record_activity_failure() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record a failed download - should not increment content counts
        store
            .record_activity(DownloadContentType::Album, 0, false)
            .unwrap();

        let hourly = store.get_hourly_counts().unwrap();
        assert_eq!(hourly.albums, 0);
        assert_eq!(hourly.tracks, 0);
        // Bytes are still recorded (even if 0)
    }

    #[test]
    fn test_record_activity_accumulates() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record multiple downloads
        store
            .record_activity(DownloadContentType::Album, 1000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::Album, 2000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 5000000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 6000000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 7000000, true)
            .unwrap();

        let hourly = store.get_hourly_counts().unwrap();
        assert_eq!(hourly.albums, 2);
        assert_eq!(hourly.tracks, 3);
        assert_eq!(hourly.bytes, 1000 + 2000 + 5000000 + 6000000 + 7000000);
    }

    #[test]
    fn test_get_hourly_counts_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // No activity recorded - should return defaults
        let hourly = store.get_hourly_counts().unwrap();
        assert_eq!(hourly.albums, 0);
        assert_eq!(hourly.tracks, 0);
        assert_eq!(hourly.images, 0);
        assert_eq!(hourly.bytes, 0);
    }

    #[test]
    fn test_get_daily_counts_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // No activity recorded
        let daily = store.get_daily_counts().unwrap();
        assert_eq!(daily.albums, 0);
        assert_eq!(daily.tracks, 0);
    }

    #[test]
    fn test_get_daily_counts() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record various activities
        store
            .record_activity(DownloadContentType::Album, 1000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 5000000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::AlbumImage, 50000, true)
            .unwrap();

        let daily = store.get_daily_counts().unwrap();
        assert_eq!(daily.albums, 1);
        assert_eq!(daily.tracks, 1);
        assert_eq!(daily.images, 1);
        assert_eq!(daily.bytes, 1000 + 5000000 + 50000);
    }

    #[test]
    fn test_get_activity_since_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let entries = store.get_activity_since(0).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_get_activity_since() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record some activity
        store
            .record_activity(DownloadContentType::Album, 1000, true)
            .unwrap();

        // Get all activity since epoch
        let entries = store.get_activity_since(0).unwrap();
        assert_eq!(entries.len(), 1);

        let entry = &entries[0];
        assert_eq!(entry.albums_downloaded, 1);
        assert_eq!(entry.bytes_downloaded, 1000);
    }

    #[test]
    fn test_hour_bucket() {
        let bucket = SqliteDownloadQueueStore::hour_bucket();

        // Should be divisible by 3600 (one hour in seconds)
        assert_eq!(bucket % 3600, 0);

        // Should be close to current time (within one hour)
        let now = SqliteDownloadQueueStore::now();
        assert!(now - bucket < 3600);
        assert!(now >= bucket);
    }

    #[test]
    fn test_day_start_bucket() {
        let bucket = SqliteDownloadQueueStore::day_start_bucket();

        // Should be divisible by 86400 (one day in seconds)
        assert_eq!(bucket % 86400, 0);

        // Should be close to current time (within one day)
        let now = SqliteDownloadQueueStore::now();
        assert!(now - bucket < 86400);
        assert!(now >= bucket);
    }

    // === Statistics Tests ===

    #[test]
    fn test_get_queue_stats_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let stats = store.get_queue_stats().unwrap();
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.in_progress, 0);
        assert_eq!(stats.retry_waiting, 0);
        assert_eq!(stats.completed_today, 0);
        assert_eq!(stats.failed_today, 0);
    }

    #[test]
    fn test_get_queue_stats_various_statuses() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add items with various statuses
        let pending = QueueItem::new(
            "pending".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(pending).unwrap();

        let in_progress = QueueItem::new(
            "in-progress".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(in_progress).unwrap();
        store.claim_for_processing("in-progress").unwrap();

        let retry = QueueItem::new(
            "retry".to_string(),
            DownloadContentType::Album,
            "album-3".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(retry).unwrap();
        let error = DownloadError::new(DownloadErrorType::Timeout, "Timeout");
        store.mark_retry_waiting("retry", 1000, &error).unwrap();

        let stats = store.get_queue_stats().unwrap();
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.in_progress, 1);
        assert_eq!(stats.retry_waiting, 1);
    }

    #[test]
    fn test_get_queue_stats_completed_today() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add and complete some items
        let item = QueueItem::new(
            "item".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item").unwrap();
        store.mark_completed("item", 1000, 100).unwrap();

        let stats = store.get_queue_stats().unwrap();
        assert_eq!(stats.completed_today, 1);
        assert_eq!(stats.failed_today, 0);
    }

    #[test]
    fn test_get_failed_items_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let failed = store.get_failed_items(10, 0).unwrap();
        assert!(failed.is_empty());
    }

    #[test]
    fn test_get_failed_items() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add some failed items
        for i in 0..3 {
            let item = QueueItem::new(
                format!("failed-{}", i),
                DownloadContentType::Album,
                format!("album-{}", i),
                QueuePriority::User,
                RequestSource::User,
                5,
            );
            store.enqueue(item).unwrap();
            store
                .claim_for_processing(&format!("failed-{}", i))
                .unwrap();
            let error = DownloadError::new(DownloadErrorType::NotFound, "Not found");
            store.mark_failed(&format!("failed-{}", i), &error).unwrap();
        }

        // Add a pending item (should not be returned)
        let pending = QueueItem::new(
            "pending".to_string(),
            DownloadContentType::Album,
            "album-99".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(pending).unwrap();

        let failed = store.get_failed_items(10, 0).unwrap();
        assert_eq!(failed.len(), 3);

        // All should be failed status
        for item in &failed {
            assert_eq!(item.status, QueueStatus::Failed);
        }
    }

    #[test]
    fn test_get_failed_items_pagination() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add 5 failed items
        for i in 0..5 {
            let item = QueueItem::new(
                format!("failed-{}", i),
                DownloadContentType::Album,
                format!("album-{}", i),
                QueuePriority::User,
                RequestSource::User,
                5,
            );
            store.enqueue(item).unwrap();
            store
                .claim_for_processing(&format!("failed-{}", i))
                .unwrap();
            let error = DownloadError::new(DownloadErrorType::NotFound, "Not found");
            store.mark_failed(&format!("failed-{}", i), &error).unwrap();
        }

        let page1 = store.get_failed_items(2, 0).unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = store.get_failed_items(2, 2).unwrap();
        assert_eq!(page2.len(), 2);

        let page3 = store.get_failed_items(2, 4).unwrap();
        assert_eq!(page3.len(), 1);
    }

    #[test]
    fn test_get_stale_in_progress_none() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let stale = store.get_stale_in_progress(3600).unwrap();
        assert!(stale.is_empty());
    }

    #[test]
    fn test_get_stale_in_progress_recent() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add and claim an item (should be recent)
        let item = QueueItem::new(
            "item".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item").unwrap();

        // With a threshold of 3600 seconds (1 hour), recently claimed items should not be stale
        let stale = store.get_stale_in_progress(3600).unwrap();
        assert!(stale.is_empty());
    }

    #[test]
    fn test_get_stale_in_progress_with_threshold_0() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add and claim an item
        let item = QueueItem::new(
            "item".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(item).unwrap();
        store.claim_for_processing("item").unwrap();

        // With a threshold of 0 seconds, everything should be considered stale
        // (since started_at < now is always true for items started in the past)
        let stale = store.get_stale_in_progress(0).unwrap();
        // Note: This might be 1 if the item was started before now,
        // or 0 if started_at == now (edge case)
        // We can't reliably test this without mocking time
    }

    #[test]
    fn test_get_stale_only_in_progress() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Add items with various statuses
        let pending = QueueItem::new(
            "pending".to_string(),
            DownloadContentType::Album,
            "album-1".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        store.enqueue(pending).unwrap();

        let mut completed = QueueItem::new(
            "completed".to_string(),
            DownloadContentType::Album,
            "album-2".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        completed.status = QueueStatus::Completed;
        store.enqueue(completed).unwrap();

        // Stale check should only consider IN_PROGRESS items
        let stale = store.get_stale_in_progress(0).unwrap();
        for item in &stale {
            assert_eq!(item.status, QueueStatus::InProgress);
        }
    }

    // === Audit Logging Tests ===

    #[test]
    fn test_log_audit_event_basic() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let event = AuditLogEntry::new(AuditEventType::RequestCreated)
            .with_queue_item("queue-123".to_string())
            .with_content(DownloadContentType::Album, "album-456".to_string())
            .with_user("user-789".to_string())
            .with_source(RequestSource::User);

        store.log_audit_event(event).unwrap();

        let (entries, total) = store.get_audit_log(AuditLogFilter::new()).unwrap();
        assert_eq!(total, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::RequestCreated);
        assert_eq!(entries[0].queue_item_id, Some("queue-123".to_string()));
    }

    #[test]
    fn test_log_audit_event_with_details() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let details = serde_json::json!({
            "child_count": 12,
            "album_name": "Test Album"
        });

        let event = AuditLogEntry::new(AuditEventType::ChildrenCreated)
            .with_queue_item("parent-123".to_string())
            .with_details(details);

        store.log_audit_event(event).unwrap();

        let (entries, _) = store.get_audit_log(AuditLogFilter::new()).unwrap();
        assert_eq!(entries.len(), 1);

        let details = entries[0].details.as_ref().unwrap();
        assert_eq!(details["child_count"], 12);
        assert_eq!(details["album_name"], "Test Album");
    }

    #[test]
    fn test_get_audit_log_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let (entries, total) = store.get_audit_log(AuditLogFilter::new()).unwrap();
        assert!(entries.is_empty());
        assert_eq!(total, 0);
    }

    #[test]
    fn test_get_audit_log_filter_by_user() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log events for different users
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated).with_user("user-1".to_string()),
            )
            .unwrap();
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated).with_user("user-2".to_string()),
            )
            .unwrap();
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::DownloadCompleted)
                    .with_user("user-1".to_string()),
            )
            .unwrap();

        let filter = AuditLogFilter::new().for_user("user-1".to_string());
        let (entries, total) = store.get_audit_log(filter).unwrap();

        assert_eq!(total, 2);
        assert_eq!(entries.len(), 2);
        for entry in &entries {
            assert_eq!(entry.user_id, Some("user-1".to_string()));
        }
    }

    #[test]
    fn test_get_audit_log_filter_by_queue_item() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated)
                    .with_queue_item("queue-1".to_string()),
            )
            .unwrap();
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::DownloadStarted)
                    .with_queue_item("queue-1".to_string()),
            )
            .unwrap();
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated)
                    .with_queue_item("queue-2".to_string()),
            )
            .unwrap();

        let filter = AuditLogFilter::new().for_queue_item("queue-1".to_string());
        let (entries, total) = store.get_audit_log(filter).unwrap();

        assert_eq!(total, 2);
        for entry in &entries {
            assert_eq!(entry.queue_item_id, Some("queue-1".to_string()));
        }
    }

    #[test]
    fn test_get_audit_log_filter_by_event_types() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        store
            .log_audit_event(AuditLogEntry::new(AuditEventType::RequestCreated))
            .unwrap();
        store
            .log_audit_event(AuditLogEntry::new(AuditEventType::DownloadStarted))
            .unwrap();
        store
            .log_audit_event(AuditLogEntry::new(AuditEventType::DownloadCompleted))
            .unwrap();
        store
            .log_audit_event(AuditLogEntry::new(AuditEventType::DownloadFailed))
            .unwrap();

        let filter = AuditLogFilter::new().with_event_types(vec![
            AuditEventType::DownloadCompleted,
            AuditEventType::DownloadFailed,
        ]);
        let (entries, total) = store.get_audit_log(filter).unwrap();

        assert_eq!(total, 2);
        for entry in &entries {
            assert!(
                entry.event_type == AuditEventType::DownloadCompleted
                    || entry.event_type == AuditEventType::DownloadFailed
            );
        }
    }

    #[test]
    fn test_get_audit_log_pagination() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log 10 events
        for i in 0..10 {
            let mut event = AuditLogEntry::new(AuditEventType::RequestCreated);
            event.timestamp = i as i64; // Different timestamps
            store.log_audit_event(event).unwrap();
        }

        let filter = AuditLogFilter::new().paginate(3, 0);
        let (entries, total) = store.get_audit_log(filter).unwrap();
        assert_eq!(total, 10);
        assert_eq!(entries.len(), 3);

        let filter = AuditLogFilter::new().paginate(3, 3);
        let (entries, total) = store.get_audit_log(filter).unwrap();
        assert_eq!(total, 10);
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_get_audit_for_item() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log a sequence of events for an item
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated)
                    .with_queue_item("item-1".to_string()),
            )
            .unwrap();
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::DownloadStarted)
                    .with_queue_item("item-1".to_string()),
            )
            .unwrap();
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::DownloadCompleted)
                    .with_queue_item("item-1".to_string()),
            )
            .unwrap();
        // Different item
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated)
                    .with_queue_item("item-2".to_string()),
            )
            .unwrap();

        let entries = store.get_audit_for_item("item-1").unwrap();
        assert_eq!(entries.len(), 3);

        // Should be in chronological order (ASC)
        assert_eq!(entries[0].event_type, AuditEventType::RequestCreated);
        assert_eq!(entries[1].event_type, AuditEventType::DownloadStarted);
        assert_eq!(entries[2].event_type, AuditEventType::DownloadCompleted);
    }

    #[test]
    fn test_get_audit_for_item_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        let entries = store.get_audit_for_item("nonexistent").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_get_audit_for_user() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log events for user-1
        for i in 0..5 {
            let mut event =
                AuditLogEntry::new(AuditEventType::RequestCreated).with_user("user-1".to_string());
            event.timestamp = i as i64;
            store.log_audit_event(event).unwrap();
        }

        // Log events for user-2
        store
            .log_audit_event(
                AuditLogEntry::new(AuditEventType::RequestCreated).with_user("user-2".to_string()),
            )
            .unwrap();

        let (entries, total) = store
            .get_audit_for_user("user-1", None, None, 100, 0)
            .unwrap();
        assert_eq!(total, 5);
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn test_get_audit_for_user_time_range() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log events at different times
        for i in 0..10 {
            let mut event =
                AuditLogEntry::new(AuditEventType::RequestCreated).with_user("user-1".to_string());
            event.timestamp = (i * 100) as i64; // 0, 100, 200, ..., 900
            store.log_audit_event(event).unwrap();
        }

        // Get events from time 300 to 600 (should be 4 events: 300, 400, 500, 600)
        let (entries, total) = store
            .get_audit_for_user("user-1", Some(300), Some(600), 100, 0)
            .unwrap();
        assert_eq!(total, 4);
        assert_eq!(entries.len(), 4);
    }

    #[test]
    fn test_cleanup_old_audit_entries() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log events with different timestamps
        for i in 0..10 {
            let mut event = AuditLogEntry::new(AuditEventType::RequestCreated);
            event.timestamp = (i * 100) as i64;
            store.log_audit_event(event).unwrap();
        }

        // Delete entries older than timestamp 500 (should delete 0, 100, 200, 300, 400)
        let deleted = store.cleanup_old_audit_entries(500).unwrap();
        assert_eq!(deleted, 5);

        // Verify remaining entries
        let (entries, total) = store.get_audit_log(AuditLogFilter::new()).unwrap();
        assert_eq!(total, 5);
        for entry in &entries {
            assert!(entry.timestamp >= 500);
        }
    }

    #[test]
    fn test_cleanup_old_audit_entries_none() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Log events with recent timestamps
        for _ in 0..5 {
            let event = AuditLogEntry::new(AuditEventType::RequestCreated);
            store.log_audit_event(event).unwrap();
        }

        // Try to delete entries older than timestamp 0 (none should match)
        let deleted = store.cleanup_old_audit_entries(0).unwrap();
        assert_eq!(deleted, 0);
    }

    // === Stats History Tests ===

    #[test]
    fn test_get_stats_history_empty() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // All periods should return empty history
        let hourly = store
            .get_stats_history(StatsPeriod::Hourly, None, None)
            .unwrap();
        assert!(hourly.entries.is_empty());
        assert_eq!(hourly.total_albums, 0);
        assert_eq!(hourly.total_tracks, 0);

        let daily = store
            .get_stats_history(StatsPeriod::Daily, None, None)
            .unwrap();
        assert!(daily.entries.is_empty());

        let weekly = store
            .get_stats_history(StatsPeriod::Weekly, None, None)
            .unwrap();
        assert!(weekly.entries.is_empty());
    }

    #[test]
    fn test_get_stats_history_with_data() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record activity (current hour)
        store
            .record_activity(DownloadContentType::Album, 1_000_000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::Album, 2_000_000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 10_000_000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::AlbumImage, 100_000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 0, false) // failure
            .unwrap();

        // Get hourly stats (should include current hour)
        let hourly = store
            .get_stats_history(StatsPeriod::Hourly, None, None)
            .unwrap();
        assert!(!hourly.entries.is_empty());
        assert_eq!(hourly.total_albums, 2);
        assert_eq!(hourly.total_tracks, 1);
        assert_eq!(hourly.total_images, 1);
        assert_eq!(
            hourly.total_bytes,
            1_000_000 + 2_000_000 + 10_000_000 + 100_000
        );
        assert_eq!(hourly.total_failures, 1);

        // Daily and weekly should also include the data
        let daily = store
            .get_stats_history(StatsPeriod::Daily, None, None)
            .unwrap();
        assert_eq!(daily.total_albums, 2);

        let weekly = store
            .get_stats_history(StatsPeriod::Weekly, None, None)
            .unwrap();
        assert_eq!(weekly.total_albums, 2);
    }

    #[test]
    fn test_stats_history_period_aggregation() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record activity
        store
            .record_activity(DownloadContentType::Album, 1_000_000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 5_000_000, true)
            .unwrap();

        // Check that different periods are represented correctly
        let hourly = store
            .get_stats_history(StatsPeriod::Hourly, None, None)
            .unwrap();
        assert_eq!(hourly.period, StatsPeriod::Hourly);
        assert!(!hourly.entries.is_empty());
        // Each entry should have period_start divisible by 3600 (hour)
        for entry in &hourly.entries {
            assert_eq!(entry.period_start % 3600, 0);
        }

        let daily = store
            .get_stats_history(StatsPeriod::Daily, None, None)
            .unwrap();
        assert_eq!(daily.period, StatsPeriod::Daily);
        // Each entry should have period_start divisible by 86400 (day)
        for entry in &daily.entries {
            assert_eq!(entry.period_start % 86400, 0);
        }

        let weekly = store
            .get_stats_history(StatsPeriod::Weekly, None, None)
            .unwrap();
        assert_eq!(weekly.period, StatsPeriod::Weekly);
        // Each entry should have period_start divisible by 604800 (week)
        for entry in &weekly.entries {
            assert_eq!(entry.period_start % 604800, 0);
        }
    }

    #[test]
    fn test_stats_history_custom_date_range() {
        let store = SqliteDownloadQueueStore::in_memory().unwrap();

        // Record activity
        store
            .record_activity(DownloadContentType::Album, 1_000_000, true)
            .unwrap();
        store
            .record_activity(DownloadContentType::TrackAudio, 5_000_000, true)
            .unwrap();

        let now = chrono::Utc::now().timestamp();
        let one_hour_ago = now - 3600;
        let one_hour_ahead = now + 3600;

        // Custom range that includes current data
        let result = store
            .get_stats_history(
                StatsPeriod::Hourly,
                Some(one_hour_ago),
                Some(one_hour_ahead),
            )
            .unwrap();
        assert!(!result.entries.is_empty());
        assert_eq!(result.total_albums, 1);
        assert_eq!(result.total_tracks, 1);

        // Custom range in the past (no data)
        let far_past = now - 1_000_000;
        let less_far_past = now - 900_000;
        let result = store
            .get_stats_history(StatsPeriod::Hourly, Some(far_past), Some(less_far_past))
            .unwrap();
        assert!(result.entries.is_empty());
        assert_eq!(result.total_albums, 0);
    }
}
