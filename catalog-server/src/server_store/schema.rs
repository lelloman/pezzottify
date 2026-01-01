//! SQLite schema definitions for the server database.
//!
//! This module defines the database schema for storing server state,
//! background job runs, schedules, and audit logs.

use crate::sqlite_column;
use crate::sqlite_persistence::{Column, SqlType, Table, VersionedSchema};

// =============================================================================
// Version 1 - Job runs and schedules
// =============================================================================

/// Job runs table - stores history of background job executions
const JOB_RUNS_TABLE_V1: Table = Table {
    name: "job_runs",
    columns: &[
        sqlite_column!("id", &SqlType::Integer, is_primary_key = true), // AUTOINCREMENT
        sqlite_column!("job_id", &SqlType::Text, non_null = true),
        sqlite_column!("started_at", &SqlType::Text, non_null = true),
        sqlite_column!("finished_at", &SqlType::Text),
        sqlite_column!("status", &SqlType::Text, non_null = true),
        sqlite_column!("error_message", &SqlType::Text),
        sqlite_column!("triggered_by", &SqlType::Text, non_null = true),
    ],
    indices: &[
        ("idx_job_runs_job_id_started", "job_id, started_at DESC"),
        ("idx_job_runs_status", "status"),
    ],
    unique_constraints: &[],
};

/// Job schedules table - stores next run times for scheduled jobs
const JOB_SCHEDULES_TABLE_V1: Table = Table {
    name: "job_schedules",
    columns: &[
        sqlite_column!("job_id", &SqlType::Text, is_primary_key = true),
        sqlite_column!("next_run_at", &SqlType::Text, non_null = true),
        sqlite_column!("last_run_at", &SqlType::Text),
    ],
    indices: &[],
    unique_constraints: &[],
};

// =============================================================================
// Version 2 - Server state key-value store
// =============================================================================

/// Server state table - key-value store for server configuration/state
const SERVER_STATE_TABLE_V2: Table = Table {
    name: "server_state",
    columns: &[
        sqlite_column!("key", &SqlType::Text, is_primary_key = true),
        sqlite_column!("value", &SqlType::Text, non_null = true),
        sqlite_column!(
            "updated_at",
            &SqlType::Text,
            non_null = true,
            default_value = Some("(datetime('now'))")
        ),
    ],
    indices: &[],
    unique_constraints: &[],
};

/// Migration from version 1 to version 2: add server_state table
fn migrate_v1_to_v2(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE server_state (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;
    Ok(())
}

// =============================================================================
// Version 3 - Job audit log
// =============================================================================

/// Job audit log table - detailed audit trail for job executions
const JOB_AUDIT_LOG_TABLE_V3: Table = Table {
    name: "job_audit_log",
    columns: &[
        sqlite_column!("id", &SqlType::Integer, is_primary_key = true), // AUTOINCREMENT
        sqlite_column!("job_id", &SqlType::Text, non_null = true),
        sqlite_column!("event_type", &SqlType::Text, non_null = true),
        sqlite_column!("timestamp", &SqlType::Text, non_null = true),
        sqlite_column!("duration_ms", &SqlType::Integer),
        sqlite_column!("details", &SqlType::Text),
        sqlite_column!("error", &SqlType::Text),
    ],
    indices: &[
        ("idx_job_audit_log_job_id", "job_id"),
        ("idx_job_audit_log_timestamp", "timestamp DESC"),
        ("idx_job_audit_log_event_type", "event_type"),
    ],
    unique_constraints: &[],
};

/// Migration from version 2 to version 3: add job_audit_log table
fn migrate_v2_to_v3(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE job_audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            job_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            duration_ms INTEGER,
            details TEXT,
            error TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX idx_job_audit_log_job_id ON job_audit_log(job_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX idx_job_audit_log_timestamp ON job_audit_log(timestamp DESC)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX idx_job_audit_log_event_type ON job_audit_log(event_type)",
        [],
    )?;
    Ok(())
}

// =============================================================================
// Version 4 - Bug reports
// =============================================================================

/// Bug reports table - stores user-submitted bug reports
///
/// Size limits (enforced at ingestion):
/// - description: max 100KB
/// - logs: max 6MB
/// - attachments: max 5 images, 25MB each (JSON array of base64 strings)
const BUG_REPORTS_TABLE_V4: Table = Table {
    name: "bug_reports",
    columns: &[
        sqlite_column!("id", &SqlType::Text, is_primary_key = true), // UUID
        sqlite_column!("user_id", &SqlType::Integer, non_null = true),
        sqlite_column!("user_handle", &SqlType::Text, non_null = true),
        sqlite_column!("title", &SqlType::Text, non_null = true),
        sqlite_column!("description", &SqlType::Text, non_null = true),
        sqlite_column!("client_type", &SqlType::Text, non_null = true), // 'web' or 'android'
        sqlite_column!("client_version", &SqlType::Text),
        sqlite_column!("device_info", &SqlType::Text), // JSON
        sqlite_column!("logs", &SqlType::Text),
        sqlite_column!("attachments", &SqlType::Text), // JSON array of base64 images
        sqlite_column!("created_at", &SqlType::Text, non_null = true), // ISO 8601
    ],
    indices: &[
        ("idx_bug_reports_user_id", "user_id"),
        ("idx_bug_reports_client_type", "client_type"),
        ("idx_bug_reports_created_at", "created_at DESC"),
    ],
    unique_constraints: &[],
};

/// Migration from version 3 to version 4: add bug_reports table
fn migrate_v3_to_v4(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE bug_reports (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            user_handle TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            client_type TEXT NOT NULL,
            client_version TEXT,
            device_info TEXT,
            logs TEXT,
            attachments TEXT,
            created_at TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX idx_bug_reports_user_id ON bug_reports(user_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX idx_bug_reports_client_type ON bug_reports(client_type)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX idx_bug_reports_created_at ON bug_reports(created_at DESC)",
        [],
    )?;
    Ok(())
}

// =============================================================================
// Versioned Schema Definition
// =============================================================================

/// All versioned schemas for the server database.
///
/// Version 1: Job runs and schedules tables
/// Version 2: Server state key-value store
/// Version 3: Job audit log table
/// Version 4: Bug reports table
pub const SERVER_VERSIONED_SCHEMAS: &[VersionedSchema] = &[
    VersionedSchema {
        version: 1,
        tables: &[JOB_RUNS_TABLE_V1, JOB_SCHEDULES_TABLE_V1],
        migration: None, // Initial version has no migration
    },
    VersionedSchema {
        version: 2,
        tables: &[
            JOB_RUNS_TABLE_V1,
            JOB_SCHEDULES_TABLE_V1,
            SERVER_STATE_TABLE_V2,
        ],
        migration: Some(migrate_v1_to_v2),
    },
    VersionedSchema {
        version: 3,
        tables: &[
            JOB_RUNS_TABLE_V1,
            JOB_SCHEDULES_TABLE_V1,
            SERVER_STATE_TABLE_V2,
            JOB_AUDIT_LOG_TABLE_V3,
        ],
        migration: Some(migrate_v2_to_v3),
    },
    VersionedSchema {
        version: 4,
        tables: &[
            JOB_RUNS_TABLE_V1,
            JOB_SCHEDULES_TABLE_V1,
            SERVER_STATE_TABLE_V2,
            JOB_AUDIT_LOG_TABLE_V3,
            BUG_REPORTS_TABLE_V4,
        ],
        migration: Some(migrate_v3_to_v4),
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_v1_schema_creates_successfully() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &SERVER_VERSIONED_SCHEMAS[0];
        schema.create(&conn).unwrap();
        schema.validate(&conn).unwrap();
    }

    #[test]
    fn test_v3_schema_creates_successfully() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &SERVER_VERSIONED_SCHEMAS[2];
        schema.create(&conn).unwrap();
        schema.validate(&conn).unwrap();
    }

    #[test]
    fn test_job_runs_indices_created() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &SERVER_VERSIONED_SCHEMAS[0];
        schema.create(&conn).unwrap();

        // Verify indices exist
        let idx_job_id_started: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_job_runs_job_id_started'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_job_id_started, 1);

        let idx_status: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_job_runs_status'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_status, 1);
    }

    #[test]
    fn test_v3_audit_log_indices_created() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &SERVER_VERSIONED_SCHEMAS[2];
        schema.create(&conn).unwrap();

        // Verify audit log indices exist
        let idx_job_id: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_job_audit_log_job_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_job_id, 1);

        let idx_timestamp: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_job_audit_log_timestamp'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_timestamp, 1);

        let idx_event_type: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_job_audit_log_event_type'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_event_type, 1);
    }

    #[test]
    fn test_migration_v1_to_v3() {
        use crate::sqlite_persistence::BASE_DB_VERSION;

        let conn = Connection::open_in_memory().unwrap();

        // Create V1 schema
        let v1_schema = &SERVER_VERSIONED_SCHEMAS[0];
        v1_schema.create(&conn).unwrap();

        // Verify we're at V1
        let db_version: i64 = conn
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .unwrap();
        assert_eq!(db_version, BASE_DB_VERSION as i64 + 1);

        // Run migrations to V2 and V3
        if let Some(migrate_fn) = SERVER_VERSIONED_SCHEMAS[1].migration {
            migrate_fn(&conn).unwrap();
        }
        if let Some(migrate_fn) = SERVER_VERSIONED_SCHEMAS[2].migration {
            migrate_fn(&conn).unwrap();
        }

        // Verify new tables exist
        let server_state_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='server_state'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(server_state_exists, 1);

        let audit_log_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='job_audit_log'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(audit_log_exists, 1);

        // Verify V3 schema validates
        let v3_schema = &SERVER_VERSIONED_SCHEMAS[2];
        v3_schema.validate(&conn).unwrap();
    }

    #[test]
    fn test_v4_schema_creates_successfully() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &SERVER_VERSIONED_SCHEMAS[3];
        schema.create(&conn).unwrap();
        schema.validate(&conn).unwrap();
    }

    #[test]
    fn test_v4_bug_reports_indices_created() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &SERVER_VERSIONED_SCHEMAS[3];
        schema.create(&conn).unwrap();

        // Verify bug reports indices exist
        let idx_user_id: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_bug_reports_user_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_user_id, 1);

        let idx_client_type: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_bug_reports_client_type'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_client_type, 1);

        let idx_created_at: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_bug_reports_created_at'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_created_at, 1);
    }

    #[test]
    fn test_migration_v3_to_v4() {
        let conn = Connection::open_in_memory().unwrap();

        // Create V3 schema
        let v3_schema = &SERVER_VERSIONED_SCHEMAS[2];
        v3_schema.create(&conn).unwrap();

        // Run migration to V4
        if let Some(migrate_fn) = SERVER_VERSIONED_SCHEMAS[3].migration {
            migrate_fn(&conn).unwrap();
        }

        // Verify bug_reports table exists
        let bug_reports_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='bug_reports'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(bug_reports_exists, 1);

        // Verify V4 schema validates
        let v4_schema = &SERVER_VERSIONED_SCHEMAS[3];
        v4_schema.validate(&conn).unwrap();
    }

    #[test]
    fn test_bug_reports_table_columns() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &SERVER_VERSIONED_SCHEMAS[3];
        schema.create(&conn).unwrap();

        // Insert a test bug report to verify schema
        conn.execute(
            "INSERT INTO bug_reports (id, user_id, user_handle, title, description, client_type, created_at)
             VALUES ('test-id', 1, 'testuser', 'Test Title', 'Test Description', 'web', '2024-01-15T10:30:00Z')",
            [],
        )
        .unwrap();

        // Verify we can read it back
        let (id, user_id, client_type, created_at): (String, i64, String, String) = conn
            .query_row(
                "SELECT id, user_id, client_type, created_at FROM bug_reports WHERE id = 'test-id'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(id, "test-id");
        assert_eq!(user_id, 1);
        assert_eq!(client_type, "web");
        assert_eq!(created_at, "2024-01-15T10:30:00Z");
    }

    #[test]
    fn test_bug_reports_optional_columns() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = &SERVER_VERSIONED_SCHEMAS[3];
        schema.create(&conn).unwrap();

        // Insert with optional fields
        conn.execute(
            "INSERT INTO bug_reports (id, user_id, user_handle, title, description, client_type, client_version, device_info, logs, attachments, created_at)
             VALUES ('test-id', 1, 'testuser', 'Title', 'Desc', 'android', '1.2.3', '{\"model\":\"Pixel\"}', 'some logs', '[\"base64img\"]', '2024-01-15T10:30:00Z')",
            [],
        )
        .unwrap();

        let (client_version, device_info, logs, attachments): (String, String, String, String) = conn
            .query_row(
                "SELECT client_version, device_info, logs, attachments FROM bug_reports WHERE id = 'test-id'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(client_version, "1.2.3");
        assert_eq!(device_info, "{\"model\":\"Pixel\"}");
        assert_eq!(logs, "some logs");
        assert_eq!(attachments, "[\"base64img\"]");
    }
}
