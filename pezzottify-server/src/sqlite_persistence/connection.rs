use rusqlite::Connection;
use std::time::Duration;

/// How long a connection may wait for another writer before returning
/// `SQLITE_BUSY`. Keeping this bounded prevents hidden multi-second stalls in
/// database executor workers.
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(2);

/// Approximate number of SQLite VM instructions between cancellation checks.
pub const CANCELLATION_PROGRESS_OPS: i32 = 10_000;

/// Apply the policy shared by every application-owned SQLite connection.
///
/// WAL lifecycle settings are configured separately by `DbRegistry`, because
/// they are properties of a persistent database rather than an individual
/// connection.
pub fn configure_connection(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

struct ProgressHandlerGuard<'a> {
    conn: &'a Connection,
}

impl Drop for ProgressHandlerGuard<'_> {
    fn drop(&mut self) {
        self.conn.progress_handler(0, None::<fn() -> bool>);
    }
}

/// Run an operation while a SQLite progress handler is installed.
///
/// The handler is removed through an RAII guard, including when the operation
/// returns early or unwinds. Returning `true` from `handler` interrupts the
/// active SQLite statement with `SQLITE_INTERRUPT`.
pub fn with_progress_handler<T, H, F>(
    conn: &Connection,
    num_ops: i32,
    handler: H,
    operation: F,
) -> T
where
    H: FnMut() -> bool + Send + 'static,
    F: FnOnce() -> T,
{
    conn.progress_handler(num_ops, Some(handler));
    let _guard = ProgressHandlerGuard { conn };
    operation()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::ErrorCode;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn common_policy_is_explicit_and_queryable() {
        let conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();

        let foreign_keys: i64 = conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        let busy_timeout_ms: i64 = conn
            .pragma_query_value(None, "busy_timeout", |row| row.get(0))
            .unwrap();
        let synchronous: i64 = conn
            .pragma_query_value(None, "synchronous", |row| row.get(0))
            .unwrap();

        assert_eq!(foreign_keys, 1);
        assert_eq!(busy_timeout_ms, 2_000);
        assert_eq!(synchronous, 1); // SQLITE_SYNC_NORMAL
    }

    #[test]
    fn progress_handler_interrupts_and_is_removed_after_operation() {
        let conn = Connection::open_in_memory().unwrap();
        let cancellation_checked = Arc::new(AtomicBool::new(false));
        let checked_in_handler = cancellation_checked.clone();

        let error = with_progress_handler(
            &conn,
            1,
            move || {
                checked_in_handler.store(true, Ordering::SeqCst);
                true
            },
            || {
                conn.query_row(
                    "WITH RECURSIVE values_to_scan(value) AS (
                         VALUES(1) UNION ALL
                         SELECT value + 1 FROM values_to_scan WHERE value < 1000
                     ) SELECT SUM(value) FROM values_to_scan",
                    [],
                    |row| row.get::<_, i64>(0),
                )
            },
        )
        .unwrap_err();

        assert!(cancellation_checked.load(Ordering::SeqCst));
        assert_eq!(
            error.sqlite_error_code(),
            Some(ErrorCode::OperationInterrupted)
        );
        assert_eq!(
            conn.query_row("SELECT 42", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            42
        );
    }
}
