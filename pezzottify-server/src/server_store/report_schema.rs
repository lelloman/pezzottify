//! Reporting extension schema, preserving legacy bug-report metadata.
use crate::sqlite_column;
use crate::sqlite_persistence::{Column, SqlType, Table};
pub(super) const REPORT_METADATA: Table = Table {
    name: "report_metadata",
    columns: &[
        sqlite_column!("seq", &SqlType::Integer, is_primary_key = true),
        sqlite_column!("report_id", &SqlType::Text, non_null = true),
        sqlite_column!("user_id", &SqlType::Integer, non_null = true),
        sqlite_column!("kind", &SqlType::Text, non_null = true),
        sqlite_column!("category", &SqlType::Text, non_null = true),
        sqlite_column!("status", &SqlType::Text, non_null = true),
        sqlite_column!("version", &SqlType::Integer, non_null = true),
        sqlite_column!("request_key", &SqlType::Text, non_null = true),
        sqlite_column!("payload_hash", &SqlType::Text, non_null = true),
        sqlite_column!("updated_at", &SqlType::Integer, non_null = true),
        sqlite_column!("duplicate_of", &SqlType::Text),
    ],
    indices: &[("idx_report_metadata_report", "report_id")],
    unique_constraints: &[&["report_id"], &["user_id", "request_key"]],
};
pub(super) const REPORT_ATTACHMENTS: Table = Table {
    name: "report_attachments",
    columns: &[
        sqlite_column!("id", &SqlType::Text, is_primary_key = true),
        sqlite_column!("report_id", &SqlType::Text, non_null = true),
        sqlite_column!("kind", &SqlType::Text, non_null = true),
        sqlite_column!("content", &SqlType::Text),
        sqlite_column!("size_bytes", &SqlType::Integer, non_null = true),
        sqlite_column!("created_at", &SqlType::Integer, non_null = true),
        sqlite_column!("expires_at", &SqlType::Integer, non_null = true),
        sqlite_column!("state", &SqlType::Text, non_null = true),
    ],
    indices: &[
        ("idx_report_attachments_report", "report_id"),
        ("idx_report_attachments_expiry", "expires_at"),
    ],
    unique_constraints: &[],
};
pub(super) const REPORT_EVENTS: Table = Table {
    name: "report_events",
    columns: &[
        sqlite_column!("seq", &SqlType::Integer, is_primary_key = true),
        sqlite_column!("id", &SqlType::Text, non_null = true),
        sqlite_column!("report_id", &SqlType::Text, non_null = true),
        sqlite_column!("actor_id", &SqlType::Integer, non_null = true),
        sqlite_column!("kind", &SqlType::Text, non_null = true),
        sqlite_column!("data", &SqlType::Text, non_null = true),
        sqlite_column!("created_at", &SqlType::Integer, non_null = true),
    ],
    indices: &[("idx_report_events_report", "report_id, seq DESC")],
    unique_constraints: &[&["id"]],
};
pub(super) fn migrate(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    REPORT_METADATA.create(conn)?;
    REPORT_ATTACHMENTS.create(conn)?;
    REPORT_EVENTS.create(conn)?;
    conn.execute_batch("INSERT INTO report_metadata(report_id,user_id,kind,category,status,version,request_key,payload_hash,updated_at)
 SELECT id,user_id,'bug','other','new',1,id,'legacy',COALESCE(unixepoch(created_at),0) FROM bug_reports ORDER BY created_at,id;
 INSERT INTO report_attachments(id,report_id,kind,content,size_bytes,created_at,expires_at,state)
 SELECT id || '-logs',id,'technical_logs',logs,length(CAST(logs AS BLOB)),COALESCE(unixepoch(created_at),0),COALESCE(unixepoch(created_at),0)+2592000,'active' FROM bug_reports WHERE logs IS NOT NULL;
 INSERT INTO report_attachments(id,report_id,kind,content,size_bytes,created_at,expires_at,state)
 SELECT id || '-images',id,'legacy_images',attachments,length(CAST(attachments AS BLOB)),COALESCE(unixepoch(created_at),0),COALESCE(unixepoch(created_at),0)+2592000,'active' FROM bug_reports WHERE attachments IS NOT NULL;")?;
    Ok(())
}
