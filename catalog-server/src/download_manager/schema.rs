//! Database schema for download_queue.db.
//!
//! Defines versioned schema migrations for the download queue database.

use crate::sqlite_column;
use crate::sqlite_persistence::{
    Column, ForeignKey, ForeignKeyOnChange, SqlType, Table, VersionedSchema,
};

// =============================================================================
// Download Queue Table - Version 1
// =============================================================================

/// Main download queue table
const DOWNLOAD_QUEUE_TABLE_V1: Table = Table {
    name: "download_queue",
    columns: &[
        sqlite_column!("id", &SqlType::Text, is_primary_key = true),
        sqlite_column!(
            "parent_id",
            &SqlType::Text,
            foreign_key = Some(&ForeignKey {
                foreign_table: "download_queue",
                foreign_column: "id",
                on_delete: ForeignKeyOnChange::Cascade,
            })
        ),
        sqlite_column!("status", &SqlType::Text, non_null = true),
        sqlite_column!("priority", &SqlType::Integer, non_null = true),
        sqlite_column!("content_type", &SqlType::Text, non_null = true),
        sqlite_column!("content_id", &SqlType::Text, non_null = true),
        sqlite_column!("content_name", &SqlType::Text),
        sqlite_column!("artist_name", &SqlType::Text),
        sqlite_column!("request_source", &SqlType::Text, non_null = true),
        sqlite_column!("requested_by_user_id", &SqlType::Text),
        sqlite_column!("created_at", &SqlType::Integer, non_null = true),
        sqlite_column!("started_at", &SqlType::Integer),
        sqlite_column!("completed_at", &SqlType::Integer),
        sqlite_column!("last_attempt_at", &SqlType::Integer),
        sqlite_column!("next_retry_at", &SqlType::Integer),
        sqlite_column!("retry_count", &SqlType::Integer, default_value = Some("0")),
        sqlite_column!("max_retries", &SqlType::Integer, default_value = Some("5")),
        sqlite_column!("error_type", &SqlType::Text),
        sqlite_column!("error_message", &SqlType::Text),
        sqlite_column!("bytes_downloaded", &SqlType::Integer),
        sqlite_column!("processing_duration_ms", &SqlType::Integer),
    ],
    indices: &[
        ("idx_queue_status_priority", "status, priority, created_at"),
        ("idx_queue_content", "content_type, content_id"),
        ("idx_queue_user", "requested_by_user_id"),
        ("idx_queue_parent", "parent_id"),
        ("idx_queue_next_retry", "next_retry_at"),
    ],
    unique_constraints: &[],
};

/// Activity tracking for capacity limits
const DOWNLOAD_ACTIVITY_LOG_TABLE_V1: Table = Table {
    name: "download_activity_log",
    columns: &[
        sqlite_column!("hour_bucket", &SqlType::Integer, is_primary_key = true),
        sqlite_column!(
            "albums_downloaded",
            &SqlType::Integer,
            default_value = Some("0")
        ),
        sqlite_column!(
            "tracks_downloaded",
            &SqlType::Integer,
            default_value = Some("0")
        ),
        sqlite_column!(
            "images_downloaded",
            &SqlType::Integer,
            default_value = Some("0")
        ),
        sqlite_column!(
            "bytes_downloaded",
            &SqlType::Integer,
            default_value = Some("0")
        ),
        sqlite_column!("failed_count", &SqlType::Integer, default_value = Some("0")),
        sqlite_column!("last_updated_at", &SqlType::Integer, non_null = true),
    ],
    indices: &[],
    unique_constraints: &[],
};

/// Per-user rate limiting
const USER_REQUEST_STATS_TABLE_V1: Table = Table {
    name: "user_request_stats",
    columns: &[
        sqlite_column!("user_id", &SqlType::Text, is_primary_key = true),
        sqlite_column!(
            "requests_today",
            &SqlType::Integer,
            default_value = Some("0")
        ),
        sqlite_column!(
            "requests_in_queue",
            &SqlType::Integer,
            default_value = Some("0")
        ),
        sqlite_column!("last_request_date", &SqlType::Text),
        sqlite_column!("last_updated_at", &SqlType::Integer, non_null = true),
    ],
    indices: &[],
    unique_constraints: &[],
};

/// Audit log table
const DOWNLOAD_AUDIT_LOG_TABLE_V1: Table = Table {
    name: "download_audit_log",
    columns: &[
        sqlite_column!(
            "id",
            &SqlType::Integer,
            is_primary_key = true,
            is_unique = true
        ),
        sqlite_column!("timestamp", &SqlType::Integer, non_null = true),
        sqlite_column!("event_type", &SqlType::Text, non_null = true),
        sqlite_column!("queue_item_id", &SqlType::Text),
        sqlite_column!("content_type", &SqlType::Text),
        sqlite_column!("content_id", &SqlType::Text),
        sqlite_column!("user_id", &SqlType::Text),
        sqlite_column!("request_source", &SqlType::Text),
        sqlite_column!("details", &SqlType::Text),
    ],
    indices: &[
        ("idx_audit_timestamp", "timestamp"),
        ("idx_audit_queue_item", "queue_item_id"),
        ("idx_audit_user", "user_id"),
        ("idx_audit_event_type", "event_type"),
        ("idx_audit_content", "content_type, content_id"),
    ],
    unique_constraints: &[],
};

pub const DOWNLOAD_QUEUE_VERSIONED_SCHEMAS: &[VersionedSchema] = &[VersionedSchema {
    version: 0,
    tables: &[
        DOWNLOAD_QUEUE_TABLE_V1,
        DOWNLOAD_ACTIVITY_LOG_TABLE_V1,
        USER_REQUEST_STATS_TABLE_V1,
        DOWNLOAD_AUDIT_LOG_TABLE_V1,
    ],
    migration: None,
}];

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_schema_version_1_creates_and_validates() {
        let conn = Connection::open_in_memory().unwrap();

        let schema = &DOWNLOAD_QUEUE_VERSIONED_SCHEMAS[0];
        schema.create(&conn).expect("Schema v1 should create successfully");
        schema.validate(&conn).expect("Schema v1 should validate successfully");
    }

    #[test]
    fn test_all_tables_exist() {
        let conn = Connection::open_in_memory().unwrap();
        DOWNLOAD_QUEUE_VERSIONED_SCHEMAS[0].create(&conn).unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"download_queue".to_string()));
        assert!(tables.contains(&"download_activity_log".to_string()));
        assert!(tables.contains(&"user_request_stats".to_string()));
        assert!(tables.contains(&"download_audit_log".to_string()));
    }

    #[test]
    fn test_download_queue_insert_and_query() {
        let conn = Connection::open_in_memory().unwrap();
        DOWNLOAD_QUEUE_VERSIONED_SCHEMAS[0].create(&conn).unwrap();

        // Insert a parent item
        conn.execute(
            r#"INSERT INTO download_queue (
                id, status, priority, content_type, content_id, request_source, created_at
            ) VALUES ('test-1', 'PENDING', 2, 'ALBUM', 'album-123', 'USER', 1700000000)"#,
            [],
        )
        .expect("Should insert into download_queue");

        // Insert a child item
        conn.execute(
            r#"INSERT INTO download_queue (
                id, parent_id, status, priority, content_type, content_id, request_source, created_at
            ) VALUES ('test-2', 'test-1', 'PENDING', 2, 'TRACK_AUDIO', 'track-456', 'USER', 1700000001)"#,
            [],
        )
        .expect("Should insert child into download_queue");

        // Verify parent-child relationship
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM download_queue WHERE parent_id = 'test-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_cascade_delete_on_parent() {
        let conn = Connection::open_in_memory().unwrap();
        DOWNLOAD_QUEUE_VERSIONED_SCHEMAS[0].create(&conn).unwrap();

        // Insert parent
        conn.execute(
            r#"INSERT INTO download_queue (
                id, status, priority, content_type, content_id, request_source, created_at
            ) VALUES ('parent-1', 'PENDING', 2, 'ALBUM', 'album-123', 'USER', 1700000000)"#,
            [],
        )
        .unwrap();

        // Insert children
        conn.execute(
            r#"INSERT INTO download_queue (
                id, parent_id, status, priority, content_type, content_id, request_source, created_at
            ) VALUES ('child-1', 'parent-1', 'PENDING', 2, 'TRACK_AUDIO', 'track-1', 'USER', 1700000001)"#,
            [],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO download_queue (
                id, parent_id, status, priority, content_type, content_id, request_source, created_at
            ) VALUES ('child-2', 'parent-1', 'PENDING', 2, 'TRACK_AUDIO', 'track-2', 'USER', 1700000002)"#,
            [],
        )
        .unwrap();

        // Delete parent
        conn.execute("DELETE FROM download_queue WHERE id = 'parent-1'", [])
            .unwrap();

        // Verify children are also deleted
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM download_queue", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "Children should be deleted with parent");
    }

    #[test]
    fn test_activity_log_upsert() {
        let conn = Connection::open_in_memory().unwrap();
        DOWNLOAD_QUEUE_VERSIONED_SCHEMAS[0].create(&conn).unwrap();

        // Insert activity log entry
        conn.execute(
            r#"INSERT INTO download_activity_log (
                hour_bucket, albums_downloaded, tracks_downloaded, images_downloaded,
                bytes_downloaded, failed_count, last_updated_at
            ) VALUES (1700000000, 5, 50, 10, 1073741824, 2, 1700003600)"#,
            [],
        )
        .expect("Should insert into download_activity_log");

        // Verify upsert behavior (hour_bucket is PRIMARY KEY)
        let result = conn.execute(
            r#"INSERT OR REPLACE INTO download_activity_log (
                hour_bucket, albums_downloaded, tracks_downloaded, images_downloaded,
                bytes_downloaded, failed_count, last_updated_at
            ) VALUES (1700000000, 6, 55, 11, 1200000000, 3, 1700003700)"#,
            [],
        );
        assert!(result.is_ok(), "Should allow upsert on hour_bucket");

        let albums: i32 = conn
            .query_row(
                "SELECT albums_downloaded FROM download_activity_log WHERE hour_bucket = 1700000000",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(albums, 6, "Albums should be updated to 6");
    }

    #[test]
    fn test_user_request_stats() {
        let conn = Connection::open_in_memory().unwrap();
        DOWNLOAD_QUEUE_VERSIONED_SCHEMAS[0].create(&conn).unwrap();

        conn.execute(
            r#"INSERT INTO user_request_stats (
                user_id, requests_today, requests_in_queue, last_request_date, last_updated_at
            ) VALUES ('user-123', 5, 2, '2024-01-15', 1700000000)"#,
            [],
        )
        .expect("Should insert into user_request_stats");

        let requests: i32 = conn
            .query_row(
                "SELECT requests_today FROM user_request_stats WHERE user_id = 'user-123'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(requests, 5);
    }

    #[test]
    fn test_audit_log_autoincrement() {
        let conn = Connection::open_in_memory().unwrap();
        DOWNLOAD_QUEUE_VERSIONED_SCHEMAS[0].create(&conn).unwrap();

        // Insert first audit log entry
        conn.execute(
            r#"INSERT INTO download_audit_log (
                timestamp, event_type, queue_item_id, content_type, content_id,
                user_id, request_source, details
            ) VALUES (1700000000, 'REQUEST_CREATED', 'queue-123', 'ALBUM', 'album-456',
                      'user-789', 'USER', '{"album_name":"Test Album"}')"#,
            [],
        )
        .expect("Should insert into download_audit_log");

        // Insert second entry
        conn.execute(
            r#"INSERT INTO download_audit_log (
                timestamp, event_type, queue_item_id
            ) VALUES (1700000001, 'DOWNLOAD_STARTED', 'queue-123')"#,
            [],
        )
        .unwrap();

        // Verify auto-increment IDs
        let ids: Vec<i64> = conn
            .prepare("SELECT id FROM download_audit_log ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], 1);
        assert_eq!(ids[1], 2);
    }

    #[test]
    fn test_indexes_exist() {
        let conn = Connection::open_in_memory().unwrap();
        DOWNLOAD_QUEUE_VERSIONED_SCHEMAS[0].create(&conn).unwrap();

        let indexes: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%' ORDER BY name",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        // Verify queue indexes
        assert!(indexes.contains(&"idx_queue_status_priority".to_string()));
        assert!(indexes.contains(&"idx_queue_content".to_string()));
        assert!(indexes.contains(&"idx_queue_user".to_string()));
        assert!(indexes.contains(&"idx_queue_parent".to_string()));
        assert!(indexes.contains(&"idx_queue_next_retry".to_string()));

        // Verify audit indexes
        assert!(indexes.contains(&"idx_audit_timestamp".to_string()));
        assert!(indexes.contains(&"idx_audit_queue_item".to_string()));
        assert!(indexes.contains(&"idx_audit_user".to_string()));
        assert!(indexes.contains(&"idx_audit_event_type".to_string()));
        assert!(indexes.contains(&"idx_audit_content".to_string()));
    }

    #[test]
    fn test_default_values() {
        let conn = Connection::open_in_memory().unwrap();
        DOWNLOAD_QUEUE_VERSIONED_SCHEMAS[0].create(&conn).unwrap();

        // Insert with minimal required fields
        conn.execute(
            r#"INSERT INTO download_queue (
                id, status, priority, content_type, content_id, request_source, created_at
            ) VALUES ('test-1', 'PENDING', 2, 'ALBUM', 'album-123', 'USER', 1700000000)"#,
            [],
        )
        .unwrap();

        // Verify default values
        let (retry_count, max_retries): (i32, i32) = conn
            .query_row(
                "SELECT retry_count, max_retries FROM download_queue WHERE id = 'test-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(retry_count, 0, "retry_count should default to 0");
        assert_eq!(max_retries, 5, "max_retries should default to 5");
    }
}
