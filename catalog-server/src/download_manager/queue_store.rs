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
    fn list_all(
        &self,
        status: Option<QueueStatus>,
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
    fn mark_retry_waiting(
        &self,
        id: &str,
        next_retry_at: i64,
        error: &DownloadError,
    ) -> Result<()>;

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
            info!("Created new download queue database at {:?}", db_path.as_ref());
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

        for schema in DOWNLOAD_QUEUE_VERSIONED_SCHEMAS.iter().skip(current_version + 1) {
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
            &format!(
                "PRAGMA user_version = {}",
                BASE_DB_VERSION + target_version
            ),
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
        let mut stmt = conn.prepare(
            "SELECT * FROM download_queue WHERE id = ?1",
        )?;

        let item = stmt
            .query_row([id], Self::row_to_queue_item)
            .optional()?;

        Ok(item)
    }

    fn get_next_pending(&self) -> Result<Option<QueueItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT * FROM download_queue
               WHERE status = 'PENDING'
               ORDER BY priority ASC, created_at ASC
               LIMIT 1"#,
        )?;

        let item = stmt
            .query_row([], Self::row_to_queue_item)
            .optional()?;

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
                   LIMIT ?3 OFFSET ?4"#.to_string(),
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
                   LIMIT ?2 OFFSET ?3"#.to_string(),
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
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueueItem>> {
        let conn = self.conn.lock().unwrap();

        let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = match status {
            Some(s) => (
                r#"SELECT * FROM download_queue
                   WHERE status = ?1
                   ORDER BY priority ASC, created_at ASC
                   LIMIT ?2 OFFSET ?3"#.to_string(),
                vec![
                    Box::new(s.as_db_str().to_string()),
                    Box::new(limit as i64),
                    Box::new(offset as i64),
                ],
            ),
            None => (
                r#"SELECT * FROM download_queue
                   ORDER BY priority ASC, created_at ASC
                   LIMIT ?1 OFFSET ?2"#.to_string(),
                vec![
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
            None => Ok(None), // Item doesn't exist
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

    // Stub implementations for remaining trait methods (to be implemented in later tasks)

    fn claim_for_processing(&self, _id: &str) -> Result<bool> {
        todo!("Implement in DM-1.4.4")
    }

    fn mark_completed(&self, _id: &str, _bytes: u64, _duration_ms: i64) -> Result<()> {
        todo!("Implement in DM-1.4.4")
    }

    fn mark_retry_waiting(&self, _id: &str, _next_retry_at: i64, _error: &DownloadError) -> Result<()> {
        todo!("Implement in DM-1.4.4")
    }

    fn mark_failed(&self, _id: &str, _error: &DownloadError) -> Result<()> {
        todo!("Implement in DM-1.4.4")
    }

    fn create_children(&self, _parent_id: &str, _children: Vec<QueueItem>) -> Result<()> {
        todo!("Implement in DM-1.4.5")
    }

    fn get_children(&self, _parent_id: &str) -> Result<Vec<QueueItem>> {
        todo!("Implement in DM-1.4.5")
    }

    fn get_children_progress(&self, _parent_id: &str) -> Result<DownloadProgress> {
        todo!("Implement in DM-1.4.5")
    }

    fn check_parent_completion(&self, _parent_id: &str) -> Result<Option<QueueStatus>> {
        todo!("Implement in DM-1.4.5")
    }

    fn get_user_requests(&self, _user_id: &str, _limit: usize, _offset: usize) -> Result<Vec<QueueItem>> {
        todo!("Implement in DM-1.4.5")
    }

    fn get_retry_ready(&self) -> Result<Vec<QueueItem>> {
        todo!("Implement in DM-1.4.6")
    }

    fn promote_retry_to_pending(&self, _id: &str) -> Result<()> {
        todo!("Implement in DM-1.4.6")
    }

    fn find_by_content(&self, _content_type: DownloadContentType, _content_id: &str) -> Result<Option<QueueItem>> {
        todo!("Implement in DM-1.4.7")
    }

    fn is_in_queue(&self, _content_type: DownloadContentType, _content_id: &str) -> Result<bool> {
        todo!("Implement in DM-1.4.7")
    }

    fn is_in_active_queue(&self, _content_type: DownloadContentType, _content_id: &str) -> Result<bool> {
        todo!("Implement in DM-1.4.7")
    }

    fn get_user_stats(&self, _user_id: &str) -> Result<UserLimitStatus> {
        todo!("Implement in DM-1.4.8")
    }

    fn increment_user_requests(&self, _user_id: &str) -> Result<()> {
        todo!("Implement in DM-1.4.8")
    }

    fn decrement_user_queue(&self, _user_id: &str) -> Result<()> {
        todo!("Implement in DM-1.4.8")
    }

    fn reset_daily_user_stats(&self) -> Result<usize> {
        todo!("Implement in DM-1.4.8")
    }

    fn record_activity(&self, _content_type: DownloadContentType, _bytes: u64, _success: bool) -> Result<()> {
        todo!("Implement in DM-1.4.9")
    }

    fn get_activity_since(&self, _since: i64) -> Result<Vec<ActivityLogEntry>> {
        todo!("Implement in DM-1.4.9")
    }

    fn get_hourly_counts(&self) -> Result<HourlyCounts> {
        todo!("Implement in DM-1.4.9")
    }

    fn get_daily_counts(&self) -> Result<DailyCounts> {
        todo!("Implement in DM-1.4.9")
    }

    fn get_queue_stats(&self) -> Result<QueueStats> {
        todo!("Implement in DM-1.4.10")
    }

    fn get_failed_items(&self, _limit: usize, _offset: usize) -> Result<Vec<QueueItem>> {
        todo!("Implement in DM-1.4.10")
    }

    fn get_stale_in_progress(&self, _stale_threshold_secs: i64) -> Result<Vec<QueueItem>> {
        todo!("Implement in DM-1.4.10")
    }

    fn log_audit_event(&self, _event: AuditLogEntry) -> Result<()> {
        todo!("Implement in DM-1.4.11")
    }

    fn get_audit_log(&self, _filter: AuditLogFilter) -> Result<(Vec<AuditLogEntry>, usize)> {
        todo!("Implement in DM-1.4.11")
    }

    fn get_audit_for_item(&self, _queue_item_id: &str) -> Result<Vec<AuditLogEntry>> {
        todo!("Implement in DM-1.4.11")
    }

    fn get_audit_for_user(
        &self,
        _user_id: &str,
        _since: Option<i64>,
        _until: Option<i64>,
        _limit: usize,
        _offset: usize,
    ) -> Result<(Vec<AuditLogEntry>, usize)> {
        todo!("Implement in DM-1.4.11")
    }

    fn cleanup_old_audit_entries(&self, _older_than: i64) -> Result<usize> {
        todo!("Implement in DM-1.4.11")
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
        .with_names(Some("Test Album".to_string()), Some("Test Artist".to_string()))
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
        let all = store.list_all(None, 100, 0).unwrap();
        assert_eq!(all.len(), 2);

        // List pending only
        let pending = store.list_all(Some(QueueStatus::Pending), 100, 0).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "item-1");

        // List completed only
        let completed = store.list_all(Some(QueueStatus::Completed), 100, 0).unwrap();
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
        let page1 = store.list_all(None, 2, 0).unwrap();
        assert_eq!(page1.len(), 2);

        // Get second page
        let page2 = store.list_all(None, 2, 2).unwrap();
        assert_eq!(page2.len(), 2);

        // Get third page
        let page3 = store.list_all(None, 2, 4).unwrap();
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
}
