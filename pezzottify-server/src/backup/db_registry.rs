use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::info;

/// Registry of all SQLite database paths managed by the server.
///
/// Each store registers its database at construction time. The registry enables
/// WAL mode and disables auto-checkpoints on the write connection, making the
/// .db file stable for external backup (rsync --inplace) between explicit
/// checkpoint calls.
pub struct DbRegistry {
    paths: Mutex<Vec<PathBuf>>,
}

impl Default for DbRegistry {
    fn default() -> Self {
        DbRegistry {
            paths: Mutex::new(Vec::new()),
        }
    }
}

impl DbRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a database. Enables WAL mode and disables auto-checkpoint on the connection.
    ///
    /// After this call the .db file will only be modified by explicit checkpoint operations,
    /// making it safe to copy at any time between checkpoints.
    pub fn register(&self, path: PathBuf, conn: &Connection) -> Result<()> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "wal_autocheckpoint", 0)?;
        info!("Registered database for backup: {:?}", path);
        self.paths.lock().unwrap().push(path);
        Ok(())
    }

    /// Returns all registered database paths.
    pub fn all(&self) -> Vec<PathBuf> {
        self.paths.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::ErrorCode;
    use tempfile::TempDir;

    fn database_path(temp_dir: &TempDir) -> PathBuf {
        temp_dir.path().join("policy.db")
    }

    fn registered_connection(temp_dir: &TempDir) -> Connection {
        let path = database_path(temp_dir);
        let conn = Connection::open(&path).unwrap();
        DbRegistry::new().register(path, &conn).unwrap();
        conn
    }

    #[test]
    fn registration_enables_wal_and_reserves_checkpoint_control() {
        let temp_dir = TempDir::new().unwrap();
        let conn = registered_connection(&temp_dir);

        let journal_mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        let wal_autocheckpoint: i64 = conn
            .pragma_query_value(None, "wal_autocheckpoint", |row| row.get(0))
            .unwrap();

        assert_eq!(journal_mode, "wal");
        assert_eq!(wal_autocheckpoint, 0);
    }

    #[test]
    fn wal_reader_observes_a_stable_snapshot_while_writer_is_open() {
        let temp_dir = TempDir::new().unwrap();
        let mut writer = registered_connection(&temp_dir);
        writer
            .execute("CREATE TABLE values_to_read (value INTEGER NOT NULL)", [])
            .unwrap();
        writer
            .execute("INSERT INTO values_to_read VALUES (1)", [])
            .unwrap();

        let reader = Connection::open(database_path(&temp_dir)).unwrap();
        let transaction = writer.transaction().unwrap();
        transaction
            .execute("INSERT INTO values_to_read VALUES (2)", [])
            .unwrap();

        let count_during_write: i64 = reader
            .query_row("SELECT COUNT(*) FROM values_to_read", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count_during_write, 1);

        transaction.commit().unwrap();
        let count_after_commit: i64 = reader
            .query_row("SELECT COUNT(*) FROM values_to_read", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count_after_commit, 2);
    }

    #[test]
    fn competing_writer_reports_busy_and_can_retry_after_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let mut first_writer = registered_connection(&temp_dir);
        first_writer
            .execute("CREATE TABLE writes (value INTEGER NOT NULL)", [])
            .unwrap();
        let second_writer = Connection::open(database_path(&temp_dir)).unwrap();

        let transaction = first_writer.transaction().unwrap();
        transaction
            .execute("INSERT INTO writes VALUES (1)", [])
            .unwrap();

        let error = second_writer
            .execute("INSERT INTO writes VALUES (2)", [])
            .unwrap_err();
        assert_eq!(error.sqlite_error_code(), Some(ErrorCode::DatabaseBusy));

        transaction.rollback().unwrap();
        second_writer
            .execute("INSERT INTO writes VALUES (2)", [])
            .unwrap();
        let values: Vec<i64> = second_writer
            .prepare("SELECT value FROM writes ORDER BY value")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(values, vec![2]);
    }
}
