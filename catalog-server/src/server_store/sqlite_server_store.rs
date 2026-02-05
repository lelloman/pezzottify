use super::models::{
    CatalogContentType, CatalogEvent, CatalogEventType, JobRun, JobRunStatus, JobScheduleState,
};
use super::schema::SERVER_VERSIONED_SCHEMAS;
use super::ServerStore;
use crate::sqlite_persistence::BASE_DB_VERSION;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

pub struct SqliteServerStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteServerStore {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let path = db_path.as_ref();
        let is_new_db = !path.exists();

        let mut conn = Connection::open(path).context("Failed to open server database")?;
        conn.execute("PRAGMA foreign_keys = ON;", [])?;

        if is_new_db {
            // Fresh database - create with latest schema
            info!("Creating new server database at {:?}", path);
            SERVER_VERSIONED_SCHEMAS.last().unwrap().create(&conn)?;
        } else {
            // Existing database - check version and migrate if needed
            let raw_version: i64 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
            let db_version = raw_version - BASE_DB_VERSION as i64;

            if db_version < 1 {
                anyhow::bail!(
                    "Server database version {} is invalid (expected >= 1)",
                    db_version
                );
            }

            let current_schema_version = SERVER_VERSIONED_SCHEMAS.last().unwrap().version as i64;

            // Validate schema matches expected structure
            let version_index = SERVER_VERSIONED_SCHEMAS
                .iter()
                .position(|s| s.version == db_version as usize)
                .with_context(|| format!("Unknown server database version {}", db_version))?;
            SERVER_VERSIONED_SCHEMAS[version_index]
                .validate(&conn)
                .with_context(|| {
                    format!(
                        "Server database schema validation failed for version {}",
                        db_version
                    )
                })?;

            if db_version < current_schema_version {
                info!(
                    "Migrating server database from version {} to {}",
                    db_version, current_schema_version
                );
                Self::migrate_if_needed(&mut conn, db_version as usize)?;
            }
        }

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn migrate_if_needed(conn: &mut Connection, from_version: usize) -> Result<()> {
        let tx = conn.transaction()?;
        let mut latest_from = from_version;
        for schema in SERVER_VERSIONED_SCHEMAS.iter().skip(from_version) {
            if schema.version > from_version {
                info!(
                    "Running server database migration from version {} to {}",
                    latest_from, schema.version
                );
                if let Some(migration_fn) = schema.migration {
                    migration_fn(&tx).with_context(|| {
                        format!("Failed to run migration to version {}", schema.version)
                    })?;
                }
                latest_from = schema.version;
            }
        }
        tx.execute(
            &format!("PRAGMA user_version = {}", BASE_DB_VERSION + latest_from),
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn format_datetime(dt: &DateTime<Utc>) -> String {
        dt.to_rfc3339()
    }

    fn row_to_job_run(row: &rusqlite::Row) -> rusqlite::Result<JobRun> {
        let status_str: String = row.get("status")?;
        let status = JobRunStatus::parse(&status_str).unwrap_or(JobRunStatus::Failed);

        let started_at_str: String = row.get("started_at")?;
        let finished_at_str: Option<String> = row.get("finished_at")?;

        Ok(JobRun {
            id: row.get("id")?,
            job_id: row.get("job_id")?,
            started_at: DateTime::parse_from_rfc3339(&started_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            finished_at: finished_at_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            }),
            status,
            error_message: row.get("error_message")?,
            triggered_by: row.get("triggered_by")?,
        })
    }

    fn row_to_schedule_state(row: &rusqlite::Row) -> rusqlite::Result<JobScheduleState> {
        let next_run_at_str: String = row.get("next_run_at")?;
        let last_run_at_str: Option<String> = row.get("last_run_at")?;

        Ok(JobScheduleState {
            job_id: row.get("job_id")?,
            next_run_at: DateTime::parse_from_rfc3339(&next_run_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            last_run_at: last_run_at_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            }),
        })
    }

    fn row_to_audit_entry(row: &rusqlite::Row) -> rusqlite::Result<super::JobAuditEntry> {
        let event_type_str: String = row.get("event_type")?;
        let event_type = super::JobAuditEventType::parse(&event_type_str)
            .unwrap_or(super::JobAuditEventType::Progress);

        let timestamp_str: String = row.get("timestamp")?;
        let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
            .map(|dt| dt.with_timezone(&Utc).timestamp())
            .unwrap_or_else(|_| Utc::now().timestamp());

        let details_str: Option<String> = row.get("details")?;
        let details = details_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(super::JobAuditEntry {
            id: row.get("id")?,
            job_id: row.get("job_id")?,
            event_type,
            timestamp,
            duration_ms: row.get("duration_ms")?,
            details,
            error: row.get("error")?,
        })
    }

    fn row_to_bug_report(row: &rusqlite::Row) -> rusqlite::Result<super::BugReport> {
        let created_at_str: String = row.get("created_at")?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let user_id: i64 = row.get("user_id")?;

        Ok(super::BugReport {
            id: row.get("id")?,
            user_id: user_id as usize,
            user_handle: row.get("user_handle")?,
            title: row.get("title")?,
            description: row.get("description")?,
            client_type: row.get("client_type")?,
            client_version: row.get("client_version")?,
            device_info: row.get("device_info")?,
            logs: row.get("logs")?,
            attachments: row.get("attachments")?,
            created_at,
        })
    }

    fn row_to_bug_report_summary(row: &rusqlite::Row) -> rusqlite::Result<super::BugReportSummary> {
        let created_at_str: String = row.get("created_at")?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let user_id: i64 = row.get("user_id")?;
        let size_bytes: i64 = row.get("size_bytes")?;

        Ok(super::BugReportSummary {
            id: row.get("id")?,
            user_id: user_id as usize,
            user_handle: row.get("user_handle")?,
            title: row.get("title")?,
            client_type: row.get("client_type")?,
            created_at,
            size_bytes: size_bytes as usize,
        })
    }

    fn row_to_catalog_event(row: &rusqlite::Row) -> rusqlite::Result<CatalogEvent> {
        let event_type_str: String = row.get("event_type")?;
        let content_type_str: String = row.get("content_type")?;

        Ok(CatalogEvent {
            seq: row.get("seq")?,
            event_type: CatalogEventType::parse(&event_type_str)
                .unwrap_or(CatalogEventType::AlbumUpdated),
            content_type: CatalogContentType::parse(&content_type_str)
                .unwrap_or(CatalogContentType::Album),
            content_id: row.get("content_id")?,
            timestamp: row.get("timestamp")?,
            triggered_by: row.get("triggered_by")?,
        })
    }
}

impl ServerStore for SqliteServerStore {
    fn record_job_start(&self, job_id: &str, triggered_by: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Self::format_datetime(&Utc::now());

        conn.execute(
            "INSERT INTO job_runs (job_id, started_at, status, triggered_by)
             VALUES (?1, ?2, ?3, ?4)",
            params![job_id, now, JobRunStatus::Running.as_str(), triggered_by],
        )?;

        Ok(conn.last_insert_rowid())
    }

    fn record_job_finish(
        &self,
        run_id: i64,
        status: JobRunStatus,
        error_message: Option<String>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::format_datetime(&Utc::now());

        conn.execute(
            "UPDATE job_runs SET finished_at = ?1, status = ?2, error_message = ?3 WHERE id = ?4",
            params![now, status.as_str(), error_message, run_id],
        )?;

        Ok(())
    }

    fn get_running_jobs(&self) -> Result<Vec<JobRun>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, job_id, started_at, finished_at, status, error_message, triggered_by
             FROM job_runs WHERE status = ?1 ORDER BY started_at DESC",
        )?;

        let jobs = stmt
            .query_map(
                params![JobRunStatus::Running.as_str()],
                Self::row_to_job_run,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(jobs)
    }

    fn get_job_history(&self, job_id: &str, limit: usize) -> Result<Vec<JobRun>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, job_id, started_at, finished_at, status, error_message, triggered_by
             FROM job_runs WHERE job_id = ?1 ORDER BY started_at DESC LIMIT ?2",
        )?;

        let jobs = stmt
            .query_map(params![job_id, limit as i64], Self::row_to_job_run)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(jobs)
    }

    fn get_last_run(&self, job_id: &str) -> Result<Option<JobRun>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, job_id, started_at, finished_at, status, error_message, triggered_by
             FROM job_runs WHERE job_id = ?1 ORDER BY started_at DESC LIMIT 1",
        )?;

        let job = stmt
            .query_row(params![job_id], Self::row_to_job_run)
            .optional()?;

        Ok(job)
    }

    fn mark_stale_jobs_failed(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Self::format_datetime(&Utc::now());

        // Mark any jobs that are still "running" as failed
        // This is called at startup to clean up jobs that were interrupted
        let count = conn.execute(
            "UPDATE job_runs SET status = ?1, finished_at = ?2, error_message = ?3
             WHERE status = ?4",
            params![
                JobRunStatus::Failed.as_str(),
                now,
                "Job was interrupted (server restart)",
                JobRunStatus::Running.as_str()
            ],
        )?;

        Ok(count)
    }

    fn get_schedule_state(&self, job_id: &str) -> Result<Option<JobScheduleState>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT job_id, next_run_at, last_run_at FROM job_schedules WHERE job_id = ?1",
        )?;

        let state = stmt
            .query_row(params![job_id], Self::row_to_schedule_state)
            .optional()?;

        Ok(state)
    }

    fn update_schedule_state(&self, state: &JobScheduleState) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let next_run_at = Self::format_datetime(&state.next_run_at);
        let last_run_at = state.last_run_at.as_ref().map(Self::format_datetime);

        conn.execute(
            "INSERT INTO job_schedules (job_id, next_run_at, last_run_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(job_id) DO UPDATE SET next_run_at = ?2, last_run_at = ?3",
            params![state.job_id, next_run_at, last_run_at],
        )?;

        Ok(())
    }

    fn get_all_schedule_states(&self) -> Result<Vec<JobScheduleState>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT job_id, next_run_at, last_run_at FROM job_schedules")?;

        let states = stmt
            .query_map([], Self::row_to_schedule_state)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(states)
    }

    fn get_state(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM server_state WHERE key = ?1")?;

        let value: Option<String> = stmt.query_row(params![key], |row| row.get(0)).optional()?;

        Ok(value)
    }

    fn set_state(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::format_datetime(&Utc::now());

        conn.execute(
            "INSERT INTO server_state (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
            params![key, value, now],
        )?;

        Ok(())
    }

    fn delete_state(&self, key: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM server_state WHERE key = ?1", params![key])?;
        Ok(())
    }

    fn log_job_audit(
        &self,
        job_id: &str,
        event_type: super::JobAuditEventType,
        duration_ms: Option<i64>,
        details: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Self::format_datetime(&Utc::now());
        let details_str = details.map(|d| d.to_string());

        conn.execute(
            "INSERT INTO job_audit_log (job_id, event_type, timestamp, duration_ms, details, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                job_id,
                event_type.as_str(),
                now,
                duration_ms,
                details_str,
                error
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    fn get_job_audit_log(&self, limit: usize, offset: usize) -> Result<Vec<super::JobAuditEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, job_id, event_type, timestamp, duration_ms, details, error
             FROM job_audit_log
             ORDER BY timestamp DESC
             LIMIT ?1 OFFSET ?2",
        )?;

        let entries = stmt
            .query_map(
                params![limit as i64, offset as i64],
                Self::row_to_audit_entry,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(entries)
    }

    fn get_job_audit_log_by_job(
        &self,
        job_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<super::JobAuditEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, job_id, event_type, timestamp, duration_ms, details, error
             FROM job_audit_log
             WHERE job_id = ?1
             ORDER BY timestamp DESC
             LIMIT ?2 OFFSET ?3",
        )?;

        let entries = stmt
            .query_map(
                params![job_id, limit as i64, offset as i64],
                Self::row_to_audit_entry,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(entries)
    }

    fn cleanup_old_job_audit_entries(&self, before_timestamp: i64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let cutoff = chrono::DateTime::from_timestamp(before_timestamp, 0)
            .map(|dt| Self::format_datetime(&dt.with_timezone(&Utc)))
            .unwrap_or_default();

        let deleted = conn.execute(
            "DELETE FROM job_audit_log WHERE timestamp < ?1",
            params![cutoff],
        )?;

        Ok(deleted)
    }

    fn insert_bug_report(&self, report: &super::BugReport) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let created_at = Self::format_datetime(&report.created_at);

        conn.execute(
            "INSERT INTO bug_reports (id, user_id, user_handle, title, description, client_type, client_version, device_info, logs, attachments, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                report.id,
                report.user_id as i64,
                report.user_handle,
                report.title,
                report.description,
                report.client_type,
                report.client_version,
                report.device_info,
                report.logs,
                report.attachments,
                created_at,
            ],
        )?;

        Ok(())
    }

    fn get_bug_report(&self, id: &str) -> Result<Option<super::BugReport>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, user_handle, title, description, client_type, client_version, device_info, logs, attachments, created_at
             FROM bug_reports WHERE id = ?1",
        )?;

        let report = stmt
            .query_row(params![id], Self::row_to_bug_report)
            .optional()?;

        Ok(report)
    }

    fn list_bug_reports(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<super::BugReportSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, user_handle, title, client_type, created_at,
                    LENGTH(COALESCE(description, '')) + LENGTH(COALESCE(logs, '')) + LENGTH(COALESCE(attachments, '')) as size_bytes
             FROM bug_reports
             ORDER BY created_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;

        let summaries = stmt
            .query_map(
                params![limit as i64, offset as i64],
                Self::row_to_bug_report_summary,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(summaries)
    }

    fn delete_bug_report(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute("DELETE FROM bug_reports WHERE id = ?1", params![id])?;
        Ok(deleted > 0)
    }

    fn get_bug_reports_total_size(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let size: i64 = conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(COALESCE(description, '')) + LENGTH(COALESCE(logs, '')) + LENGTH(COALESCE(attachments, ''))), 0)
             FROM bug_reports",
            [],
            |row| row.get(0),
        )?;
        Ok(size as usize)
    }

    fn cleanup_bug_reports_to_size(&self, max_size: usize) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let mut deleted_count = 0;

        loop {
            // Check current total size
            let current_size: i64 = conn.query_row(
                "SELECT COALESCE(SUM(LENGTH(COALESCE(description, '')) + LENGTH(COALESCE(logs, '')) + LENGTH(COALESCE(attachments, ''))), 0)
                 FROM bug_reports",
                [],
                |row| row.get(0),
            )?;

            if (current_size as usize) <= max_size {
                break;
            }

            // Delete the oldest report
            let deleted = conn.execute(
                "DELETE FROM bug_reports WHERE id = (
                    SELECT id FROM bug_reports ORDER BY created_at ASC LIMIT 1
                )",
                [],
            )?;

            if deleted == 0 {
                break; // No more reports to delete
            }

            deleted_count += deleted;
        }

        Ok(deleted_count)
    }

    fn append_catalog_event(
        &self,
        event_type: CatalogEventType,
        content_type: CatalogContentType,
        content_id: &str,
        triggered_by: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp();

        conn.execute(
            "INSERT INTO catalog_events (event_type, content_type, content_id, timestamp, triggered_by)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                event_type.as_str(),
                content_type.as_str(),
                content_id,
                now,
                triggered_by
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    fn get_catalog_events_since(&self, since_seq: i64) -> Result<Vec<CatalogEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT seq, event_type, content_type, content_id, timestamp, triggered_by
             FROM catalog_events
             WHERE seq > ?1
             ORDER BY seq ASC",
        )?;

        let events = stmt
            .query_map(params![since_seq], Self::row_to_catalog_event)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(events)
    }

    fn get_catalog_events_current_seq(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let seq: Option<i64> = conn
            .query_row("SELECT MAX(seq) FROM catalog_events", [], |row| row.get(0))
            .optional()?
            .flatten();

        Ok(seq.unwrap_or(0))
    }

    fn cleanup_old_catalog_events(&self, before_timestamp: i64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute(
            "DELETE FROM catalog_events WHERE timestamp < ?1",
            params![before_timestamp],
        )?;
        Ok(deleted)
    }

    fn add_pending_whatsnew_album(&self, album_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Check if album is already in a batch
        let in_batch: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM whatsnew_batch_albums WHERE album_id = ?1",
                params![album_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if in_batch > 0 {
            return Ok(()); // Already batched, skip
        }

        // Insert or ignore if already pending
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT OR IGNORE INTO whatsnew_pending_albums (album_id, added_at) VALUES (?1, ?2)",
            params![album_id, now],
        )?;
        Ok(())
    }

    fn get_pending_whatsnew_albums(&self) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT album_id, added_at FROM whatsnew_pending_albums ORDER BY added_at")?;
        let albums = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(albums)
    }

    fn clear_pending_whatsnew_albums(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM whatsnew_pending_albums", [])?;
        Ok(())
    }

    fn is_album_in_whatsnew(&self, album_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();

        // Check pending
        let in_pending: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM whatsnew_pending_albums WHERE album_id = ?1",
                params![album_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if in_pending > 0 {
            return Ok(true);
        }

        // Check batched
        let in_batch: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM whatsnew_batch_albums WHERE album_id = ?1",
                params![album_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(in_batch > 0)
    }

    fn create_whatsnew_batch(&self, id: &str, closed_at: i64, album_ids: &[String]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Create the batch
        tx.execute(
            "INSERT INTO whatsnew_batches (id, closed_at) VALUES (?1, ?2)",
            params![id, closed_at],
        )?;

        // Add album associations
        for album_id in album_ids {
            tx.execute(
                "INSERT INTO whatsnew_batch_albums (batch_id, album_id) VALUES (?1, ?2)",
                params![id, album_id],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    fn list_whatsnew_batches(&self, limit: usize) -> Result<Vec<super::WhatsNewBatch>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, closed_at FROM whatsnew_batches ORDER BY closed_at DESC LIMIT ?1",
        )?;
        let batches = stmt
            .query_map(params![limit as i64], |row| {
                Ok(super::WhatsNewBatch {
                    id: row.get(0)?,
                    closed_at: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(batches)
    }

    fn get_whatsnew_batch_album_ids(&self, batch_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT album_id FROM whatsnew_batch_albums WHERE batch_id = ?1")?;
        let album_ids = stmt
            .query_map(params![batch_id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(album_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server_store::BugReport;
    use tempfile::TempDir;

    struct TestStore {
        store: SqliteServerStore,
        _temp_dir: TempDir, // Keep temp dir alive
    }

    fn create_test_store() -> TestStore {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("server.db");
        let store = SqliteServerStore::new(&db_path).unwrap();
        TestStore {
            store,
            _temp_dir: temp_dir,
        }
    }

    #[test]
    fn test_record_job_start_and_finish() {
        let test = create_test_store();
        let store = &test.store;

        // Start a job
        let run_id = store.record_job_start("test_job", "manual").unwrap();
        assert!(run_id > 0);

        // Check it's running
        let running = store.get_running_jobs().unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].job_id, "test_job");
        assert_eq!(running[0].status, JobRunStatus::Running);

        // Finish the job
        store
            .record_job_finish(run_id, JobRunStatus::Completed, None)
            .unwrap();

        // Check it's no longer running
        let running = store.get_running_jobs().unwrap();
        assert!(running.is_empty());

        // Check history
        let history = store.get_job_history("test_job", 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status, JobRunStatus::Completed);
        assert!(history[0].finished_at.is_some());
    }

    #[test]
    fn test_record_job_failure_with_error() {
        let test = create_test_store();
        let store = &test.store;

        let run_id = store.record_job_start("failing_job", "schedule").unwrap();
        store
            .record_job_finish(
                run_id,
                JobRunStatus::Failed,
                Some("Something went wrong".to_string()),
            )
            .unwrap();

        let last_run = store.get_last_run("failing_job").unwrap().unwrap();
        assert_eq!(last_run.status, JobRunStatus::Failed);
        assert_eq!(
            last_run.error_message,
            Some("Something went wrong".to_string())
        );
    }

    #[test]
    fn test_get_job_history_limit() {
        let test = create_test_store();
        let store = &test.store;

        // Create multiple job runs
        for i in 0..5 {
            let run_id = store
                .record_job_start("history_job", &format!("run_{}", i))
                .unwrap();
            store
                .record_job_finish(run_id, JobRunStatus::Completed, None)
                .unwrap();
        }

        // Get limited history
        let history = store.get_job_history("history_job", 3).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_mark_stale_jobs_failed() {
        let test = create_test_store();
        let store = &test.store;

        // Start jobs but don't finish them
        store.record_job_start("stale_job_1", "schedule").unwrap();
        store.record_job_start("stale_job_2", "hook").unwrap();

        // Mark stale jobs as failed
        let count = store.mark_stale_jobs_failed().unwrap();
        assert_eq!(count, 2);

        // Verify they're now failed
        let running = store.get_running_jobs().unwrap();
        assert!(running.is_empty());

        let last_run = store.get_last_run("stale_job_1").unwrap().unwrap();
        assert_eq!(last_run.status, JobRunStatus::Failed);
        assert!(last_run.error_message.unwrap().contains("server restart"));
    }

    #[test]
    fn test_schedule_state_crud() {
        let test = create_test_store();
        let store = &test.store;

        // Initially no state
        let state = store.get_schedule_state("scheduled_job").unwrap();
        assert!(state.is_none());

        // Create state
        let new_state = JobScheduleState {
            job_id: "scheduled_job".to_string(),
            next_run_at: Utc::now(),
            last_run_at: None,
        };
        store.update_schedule_state(&new_state).unwrap();

        // Read state
        let state = store.get_schedule_state("scheduled_job").unwrap().unwrap();
        assert_eq!(state.job_id, "scheduled_job");
        assert!(state.last_run_at.is_none());

        // Update state
        let updated_state = JobScheduleState {
            job_id: "scheduled_job".to_string(),
            next_run_at: Utc::now(),
            last_run_at: Some(Utc::now()),
        };
        store.update_schedule_state(&updated_state).unwrap();

        let state = store.get_schedule_state("scheduled_job").unwrap().unwrap();
        assert!(state.last_run_at.is_some());
    }

    #[test]
    fn test_get_all_schedule_states() {
        let test = create_test_store();
        let store = &test.store;

        // Create multiple schedule states
        for i in 0..3 {
            let state = JobScheduleState {
                job_id: format!("job_{}", i),
                next_run_at: Utc::now(),
                last_run_at: None,
            };
            store.update_schedule_state(&state).unwrap();
        }

        let states = store.get_all_schedule_states().unwrap();
        assert_eq!(states.len(), 3);
    }

    #[test]
    fn test_get_last_run_nonexistent_job() {
        let test = create_test_store();
        let store = &test.store;
        let last_run = store.get_last_run("nonexistent").unwrap();
        assert!(last_run.is_none());
    }

    #[test]
    fn test_state_get_nonexistent() {
        let test = create_test_store();
        let store = &test.store;
        let value = store.get_state("nonexistent").unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn test_state_set_and_get() {
        let test = create_test_store();
        let store = &test.store;

        store.set_state("test_key", "test_value").unwrap();
        let value = store.get_state("test_key").unwrap();
        assert_eq!(value, Some("test_value".to_string()));
    }

    #[test]
    fn test_state_update() {
        let test = create_test_store();
        let store = &test.store;

        store.set_state("key", "value1").unwrap();
        store.set_state("key", "value2").unwrap();

        let value = store.get_state("key").unwrap();
        assert_eq!(value, Some("value2".to_string()));
    }

    #[test]
    fn test_state_delete() {
        let test = create_test_store();
        let store = &test.store;

        store.set_state("to_delete", "value").unwrap();
        assert!(store.get_state("to_delete").unwrap().is_some());

        store.delete_state("to_delete").unwrap();
        assert!(store.get_state("to_delete").unwrap().is_none());
    }

    #[test]
    fn test_state_json_value() {
        let test = create_test_store();
        let store = &test.store;

        let json = r#"{"level":2,"successes":5}"#;
        store.set_state("corruption_handler", json).unwrap();

        let value = store.get_state("corruption_handler").unwrap();
        assert_eq!(value, Some(json.to_string()));
    }

    // Bug report tests

    fn create_test_bug_report(id: &str, user_id: usize) -> BugReport {
        BugReport {
            id: id.to_string(),
            user_id,
            user_handle: format!("user_{}", user_id),
            title: Some("Test Bug".to_string()),
            description: "This is a test bug report".to_string(),
            client_type: "android".to_string(),
            client_version: Some("1.0.0".to_string()),
            device_info: Some("Pixel 6".to_string()),
            logs: Some("Some log data".to_string()),
            attachments: Some(r#"["base64data1","base64data2"]"#.to_string()),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_bug_report_insert_and_get() {
        let test = create_test_store();
        let store = &test.store;

        let report = create_test_bug_report("bug-1", 1);
        store.insert_bug_report(&report).unwrap();

        let retrieved = store.get_bug_report("bug-1").unwrap().unwrap();
        assert_eq!(retrieved.id, "bug-1");
        assert_eq!(retrieved.user_id, 1);
        assert_eq!(retrieved.user_handle, "user_1");
        assert_eq!(retrieved.title, Some("Test Bug".to_string()));
        assert_eq!(retrieved.description, "This is a test bug report");
        assert_eq!(retrieved.client_type, "android");
        assert_eq!(retrieved.client_version, Some("1.0.0".to_string()));
        assert_eq!(retrieved.logs, Some("Some log data".to_string()));
    }

    #[test]
    fn test_bug_report_get_nonexistent() {
        let test = create_test_store();
        let store = &test.store;

        let result = store.get_bug_report("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_bug_report_list() {
        let test = create_test_store();
        let store = &test.store;

        // Insert multiple reports
        for i in 0..5 {
            let report = create_test_bug_report(&format!("bug-{}", i), i);
            store.insert_bug_report(&report).unwrap();
        }

        // List with limit
        let list = store.list_bug_reports(3, 0).unwrap();
        assert_eq!(list.len(), 3);

        // Verify summaries have size_bytes
        for summary in &list {
            assert!(summary.size_bytes > 0);
        }

        // List with offset
        let list = store.list_bug_reports(10, 3).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_bug_report_delete() {
        let test = create_test_store();
        let store = &test.store;

        let report = create_test_bug_report("bug-delete", 1);
        store.insert_bug_report(&report).unwrap();

        // Verify it exists
        assert!(store.get_bug_report("bug-delete").unwrap().is_some());

        // Delete it
        let deleted = store.delete_bug_report("bug-delete").unwrap();
        assert!(deleted);

        // Verify it's gone
        assert!(store.get_bug_report("bug-delete").unwrap().is_none());

        // Deleting again should return false
        let deleted_again = store.delete_bug_report("bug-delete").unwrap();
        assert!(!deleted_again);
    }

    #[test]
    fn test_bug_report_total_size() {
        let test = create_test_store();
        let store = &test.store;

        // Initially empty
        assert_eq!(store.get_bug_reports_total_size().unwrap(), 0);

        // Insert a report
        let report = create_test_bug_report("bug-size", 1);
        store.insert_bug_report(&report).unwrap();

        // Size should be positive
        let size = store.get_bug_reports_total_size().unwrap();
        assert!(size > 0);
    }

    #[test]
    fn test_bug_report_cleanup_to_size() {
        let test = create_test_store();
        let store = &test.store;

        // Insert reports with known sizes
        for i in 0..5 {
            let mut report = create_test_bug_report(&format!("bug-cleanup-{}", i), i);
            report.description = "x".repeat(1000); // 1KB each roughly
            store.insert_bug_report(&report).unwrap();
        }

        let initial_size = store.get_bug_reports_total_size().unwrap();
        assert!(initial_size > 4000);

        // Cleanup to a small size - should delete some reports
        let deleted = store.cleanup_bug_reports_to_size(2000).unwrap();
        assert!(deleted > 0);

        let final_size = store.get_bug_reports_total_size().unwrap();
        assert!(final_size <= 2000);
    }

    #[test]
    fn test_bug_report_cleanup_already_under_limit() {
        let test = create_test_store();
        let store = &test.store;

        let report = create_test_bug_report("bug-small", 1);
        store.insert_bug_report(&report).unwrap();

        // Cleanup with a large limit - should delete nothing
        let deleted = store.cleanup_bug_reports_to_size(1_000_000).unwrap();
        assert_eq!(deleted, 0);
    }

    // Catalog event tests

    #[test]
    fn test_catalog_event_append_and_get() {
        use crate::server_store::{CatalogContentType, CatalogEventType};

        let test = create_test_store();
        let store = &test.store;

        // Initially no events
        let events = store.get_catalog_events_since(0).unwrap();
        assert!(events.is_empty());
        assert_eq!(store.get_catalog_events_current_seq().unwrap(), 0);

        // Append an event
        let seq = store
            .append_catalog_event(
                CatalogEventType::AlbumUpdated,
                CatalogContentType::Album,
                "album-123",
                Some("download_completion"),
            )
            .unwrap();
        assert_eq!(seq, 1);

        // Get events since 0
        let events = store.get_catalog_events_since(0).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].seq, 1);
        assert_eq!(events[0].event_type, CatalogEventType::AlbumUpdated);
        assert_eq!(events[0].content_type, CatalogContentType::Album);
        assert_eq!(events[0].content_id, "album-123");
        assert_eq!(
            events[0].triggered_by,
            Some("download_completion".to_string())
        );

        // Current seq should be 1
        assert_eq!(store.get_catalog_events_current_seq().unwrap(), 1);
    }

    #[test]
    fn test_catalog_event_sequence_numbers() {
        use crate::server_store::{CatalogContentType, CatalogEventType};

        let test = create_test_store();
        let store = &test.store;

        // Append multiple events
        for i in 1..=5 {
            let seq = store
                .append_catalog_event(
                    CatalogEventType::AlbumUpdated,
                    CatalogContentType::Album,
                    &format!("album-{}", i),
                    None,
                )
                .unwrap();
            assert_eq!(seq, i as i64);
        }

        // Get events since seq 2 (should return events 3, 4, 5)
        let events = store.get_catalog_events_since(2).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].seq, 3);
        assert_eq!(events[1].seq, 4);
        assert_eq!(events[2].seq, 5);

        // Current seq should be 5
        assert_eq!(store.get_catalog_events_current_seq().unwrap(), 5);
    }

    #[test]
    fn test_catalog_event_different_types() {
        use crate::server_store::{CatalogContentType, CatalogEventType};

        let test = create_test_store();
        let store = &test.store;

        // Append events of different types
        store
            .append_catalog_event(
                CatalogEventType::AlbumAdded,
                CatalogContentType::Album,
                "album-1",
                Some("ingestion"),
            )
            .unwrap();
        store
            .append_catalog_event(
                CatalogEventType::ArtistUpdated,
                CatalogContentType::Artist,
                "artist-1",
                Some("admin_edit"),
            )
            .unwrap();
        store
            .append_catalog_event(
                CatalogEventType::TrackUpdated,
                CatalogContentType::Track,
                "track-1",
                None,
            )
            .unwrap();

        let events = store.get_catalog_events_since(0).unwrap();
        assert_eq!(events.len(), 3);

        assert_eq!(events[0].event_type, CatalogEventType::AlbumAdded);
        assert_eq!(events[0].content_type, CatalogContentType::Album);

        assert_eq!(events[1].event_type, CatalogEventType::ArtistUpdated);
        assert_eq!(events[1].content_type, CatalogContentType::Artist);

        assert_eq!(events[2].event_type, CatalogEventType::TrackUpdated);
        assert_eq!(events[2].content_type, CatalogContentType::Track);
        assert!(events[2].triggered_by.is_none());
    }

    #[test]
    fn test_catalog_event_cleanup() {
        use crate::server_store::{CatalogContentType, CatalogEventType};

        let test = create_test_store();
        let store = &test.store;

        // Append events with specific timestamps (we need to do this manually)
        {
            let conn = store.conn.lock().unwrap();
            // Event 1 at timestamp 1000
            conn.execute(
                "INSERT INTO catalog_events (event_type, content_type, content_id, timestamp)
                 VALUES ('album_updated', 'album', 'old-1', 1000)",
                [],
            )
            .unwrap();
            // Event 2 at timestamp 2000
            conn.execute(
                "INSERT INTO catalog_events (event_type, content_type, content_id, timestamp)
                 VALUES ('album_updated', 'album', 'old-2', 2000)",
                [],
            )
            .unwrap();
            // Event 3 at timestamp 5000 (should survive cleanup)
            conn.execute(
                "INSERT INTO catalog_events (event_type, content_type, content_id, timestamp)
                 VALUES ('album_updated', 'album', 'new-1', 5000)",
                [],
            )
            .unwrap();
        }

        assert_eq!(store.get_catalog_events_since(0).unwrap().len(), 3);

        // Cleanup events before timestamp 3000
        let deleted = store.cleanup_old_catalog_events(3000).unwrap();
        assert_eq!(deleted, 2);

        // Only one event should remain
        let events = store.get_catalog_events_since(0).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].content_id, "new-1");
    }

    // What's New tests

    #[test]
    fn test_whatsnew_pending_add_and_get() {
        let test = create_test_store();
        let store = &test.store;

        // Initially empty
        let pending = store.get_pending_whatsnew_albums().unwrap();
        assert!(pending.is_empty());

        // Add albums
        store.add_pending_whatsnew_album("album-1").unwrap();
        store.add_pending_whatsnew_album("album-2").unwrap();

        let pending = store.get_pending_whatsnew_albums().unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].0, "album-1");
        assert_eq!(pending[1].0, "album-2");
        assert!(pending[0].1 > 0); // Has a timestamp
    }

    #[test]
    fn test_whatsnew_pending_duplicate_ignored() {
        let test = create_test_store();
        let store = &test.store;

        store.add_pending_whatsnew_album("album-1").unwrap();
        store.add_pending_whatsnew_album("album-1").unwrap(); // Duplicate

        let pending = store.get_pending_whatsnew_albums().unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn test_whatsnew_pending_clear() {
        let test = create_test_store();
        let store = &test.store;

        store.add_pending_whatsnew_album("album-1").unwrap();
        store.add_pending_whatsnew_album("album-2").unwrap();

        store.clear_pending_whatsnew_albums().unwrap();

        let pending = store.get_pending_whatsnew_albums().unwrap();
        assert!(pending.is_empty());
    }

    #[test]
    fn test_whatsnew_create_batch() {
        let test = create_test_store();
        let store = &test.store;

        let closed_at = chrono::Utc::now().timestamp();
        store
            .create_whatsnew_batch(
                "batch-1",
                closed_at,
                &["album-1".to_string(), "album-2".to_string()],
            )
            .unwrap();

        let batches = store.list_whatsnew_batches(10).unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].id, "batch-1");
        assert_eq!(batches[0].closed_at, closed_at);

        let album_ids = store.get_whatsnew_batch_album_ids("batch-1").unwrap();
        assert_eq!(album_ids.len(), 2);
        assert!(album_ids.contains(&"album-1".to_string()));
        assert!(album_ids.contains(&"album-2".to_string()));
    }

    #[test]
    fn test_whatsnew_list_batches_order() {
        let test = create_test_store();
        let store = &test.store;

        // Create batches with different timestamps
        store
            .create_whatsnew_batch("batch-old", 1000, &["album-1".to_string()])
            .unwrap();
        store
            .create_whatsnew_batch("batch-new", 2000, &["album-2".to_string()])
            .unwrap();

        // Should be ordered by closed_at DESC (newest first)
        let batches = store.list_whatsnew_batches(10).unwrap();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].id, "batch-new");
        assert_eq!(batches[1].id, "batch-old");
    }

    #[test]
    fn test_whatsnew_list_batches_limit() {
        let test = create_test_store();
        let store = &test.store;

        for i in 0..5 {
            store
                .create_whatsnew_batch(&format!("batch-{}", i), i as i64, &[])
                .unwrap();
        }

        let batches = store.list_whatsnew_batches(3).unwrap();
        assert_eq!(batches.len(), 3);
    }

    #[test]
    fn test_whatsnew_is_album_in_pending() {
        let test = create_test_store();
        let store = &test.store;

        assert!(!store.is_album_in_whatsnew("album-1").unwrap());

        store.add_pending_whatsnew_album("album-1").unwrap();

        assert!(store.is_album_in_whatsnew("album-1").unwrap());
        assert!(!store.is_album_in_whatsnew("album-2").unwrap());
    }

    #[test]
    fn test_whatsnew_is_album_in_batch() {
        let test = create_test_store();
        let store = &test.store;

        store
            .create_whatsnew_batch("batch-1", 1000, &["album-1".to_string()])
            .unwrap();

        assert!(store.is_album_in_whatsnew("album-1").unwrap());
        assert!(!store.is_album_in_whatsnew("album-2").unwrap());
    }

    #[test]
    fn test_whatsnew_batched_album_not_re_added_to_pending() {
        let test = create_test_store();
        let store = &test.store;

        // Create a batch with an album
        store
            .create_whatsnew_batch("batch-1", 1000, &["album-1".to_string()])
            .unwrap();

        // Try to add the same album to pending
        store.add_pending_whatsnew_album("album-1").unwrap();

        // Should not be in pending (already batched)
        let pending = store.get_pending_whatsnew_albums().unwrap();
        assert!(pending.is_empty());
    }

    #[test]
    fn test_whatsnew_get_nonexistent_batch_album_ids() {
        let test = create_test_store();
        let store = &test.store;

        let album_ids = store.get_whatsnew_batch_album_ids("nonexistent").unwrap();
        assert!(album_ids.is_empty());
    }
}
