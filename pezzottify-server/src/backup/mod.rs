pub mod db_registry;

pub use db_registry::DbRegistry;

use rusqlite::Connection;
use serde::Serialize;
use tracing::{error, info};

/// Result of a checkpoint operation on a single database.
#[derive(Debug, Serialize)]
pub struct DatabaseCheckpointResult {
    pub db_name: String,
    pub success: bool,
    /// Number of WAL pages that were checkpointed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages_checkpointed: Option<i64>,
    /// Total number of pages in the WAL before checkpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_wal_pages: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of a full backup preparation (checkpoint all databases).
#[derive(Debug, Serialize)]
pub struct BackupPrepareResult {
    pub all_succeeded: bool,
    pub databases: Vec<DatabaseCheckpointResult>,
}

/// Run a TRUNCATE checkpoint on all registered databases.
///
/// This writes all WAL content back into the .db files and truncates the WAL,
/// making the .db files self-contained and safe for rsync backup.
///
/// Opens its own connections — no store mutexes are held.
pub fn prepare_backup(registry: &DbRegistry) -> BackupPrepareResult {
    let paths = registry.all();
    let mut results = Vec::with_capacity(paths.len());

    for path in &paths {
        let db_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        let result = match Connection::open(path) {
            Ok(conn) => match conn.query_row("PRAGMA wal_checkpoint(TRUNCATE);", [], |row| {
                Ok((
                    row.get::<_, i64>(0)?, // blocked (0 = success)
                    row.get::<_, i64>(1)?, // total WAL pages
                    row.get::<_, i64>(2)?, // checkpointed pages
                ))
            }) {
                Ok((blocked, total, checkpointed)) => {
                    if blocked == 0 {
                        info!(
                            "Checkpoint TRUNCATE succeeded for {}: {}/{} pages",
                            db_name, checkpointed, total
                        );
                        DatabaseCheckpointResult {
                            db_name,
                            success: true,
                            pages_checkpointed: Some(checkpointed),
                            total_wal_pages: Some(total),
                            error: None,
                        }
                    } else {
                        let msg = format!(
                            "Checkpoint blocked (busy): {}/{} pages checkpointed",
                            checkpointed, total
                        );
                        error!("Checkpoint TRUNCATE blocked for {}: {}", db_name, msg);
                        DatabaseCheckpointResult {
                            db_name,
                            success: false,
                            pages_checkpointed: Some(checkpointed),
                            total_wal_pages: Some(total),
                            error: Some(msg),
                        }
                    }
                }
                Err(e) => {
                    error!("Checkpoint TRUNCATE failed for {}: {}", db_name, e);
                    DatabaseCheckpointResult {
                        db_name,
                        success: false,
                        pages_checkpointed: None,
                        total_wal_pages: None,
                        error: Some(e.to_string()),
                    }
                }
            },
            Err(e) => {
                error!("Failed to open {} for checkpoint: {}", db_name, e);
                DatabaseCheckpointResult {
                    db_name,
                    success: false,
                    pages_checkpointed: None,
                    total_wal_pages: None,
                    error: Some(e.to_string()),
                }
            }
        };
        results.push(result);
    }

    let all_succeeded = results.iter().all(|r| r.success);
    BackupPrepareResult {
        all_succeeded,
        databases: results,
    }
}

/// Run a PASSIVE checkpoint on all registered databases.
///
/// Non-blocking: checkpoints as many WAL pages as possible without waiting
/// for readers/writers to finish. Used for periodic WAL size management.
///
/// Opens its own connections — no store mutexes are held.
pub fn passive_checkpoint_all(registry: &DbRegistry) {
    let paths = registry.all();

    for path in &paths {
        let db_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        match Connection::open(path) {
            Ok(conn) => {
                match conn.query_row("PRAGMA wal_checkpoint(PASSIVE);", [], |row| {
                    Ok((row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
                }) {
                    Ok((total, checkpointed)) => {
                        if total > 0 {
                            info!(
                                "Passive checkpoint for {}: {}/{} pages",
                                db_name, checkpointed, total
                            );
                        }
                    }
                    Err(e) => {
                        error!("Passive checkpoint failed for {}: {}", db_name, e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to open {} for passive checkpoint: {}", db_name, e);
            }
        }
    }
}
