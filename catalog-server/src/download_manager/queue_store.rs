//! Download queue storage and persistence.
//!
//! Provides SQLite-backed storage for download queue items and related data.

use super::models::*;
use super::schema::DOWNLOAD_QUEUE_VERSIONED_SCHEMAS;
use crate::sqlite_persistence::BASE_DB_VERSION;
use anyhow::{bail, Context, Result};
use rusqlite::Connection;
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
}
