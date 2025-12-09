//! Download queue storage and persistence.
//!
//! Provides SQLite-backed storage for download queue items and related data.

use super::schema::DOWNLOAD_QUEUE_VERSIONED_SCHEMAS;
use crate::sqlite_persistence::BASE_DB_VERSION;
use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

// TODO: Define trait methods in Task DM-1.4.1
pub trait DownloadQueueStore: Send + Sync {
    // Trait methods will be added in DM-1.4.1
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
