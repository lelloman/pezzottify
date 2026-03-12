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
