mod models;
mod schema;
mod sqlite_server_store;

pub use models::{
    BugReport, BugReportSummary, CatalogContentType, CatalogEvent, CatalogEventType, JobAuditEntry,
    JobAuditEventType, JobRun, JobRunStatus, JobScheduleState, WhatsNewBatch,
    BUG_REPORT_ATTACHMENT_MAX_SIZE, BUG_REPORT_DESCRIPTION_MAX_SIZE, BUG_REPORT_LOGS_MAX_SIZE,
    BUG_REPORT_MAX_ATTACHMENTS, BUG_REPORT_TITLE_MAX_LEN, BUG_REPORT_TOTAL_MAX_SIZE,
};
pub use schema::SERVER_VERSIONED_SCHEMAS;
pub use sqlite_server_store::SqliteServerStore;

use anyhow::Result;

pub trait ServerStore: Send + Sync {
    fn record_job_start(&self, job_id: &str, triggered_by: &str) -> Result<i64>;
    fn record_job_finish(
        &self,
        run_id: i64,
        status: JobRunStatus,
        error_message: Option<String>,
    ) -> Result<()>;
    fn get_running_jobs(&self) -> Result<Vec<JobRun>>;
    fn get_job_history(&self, job_id: &str, limit: usize) -> Result<Vec<JobRun>>;
    fn get_last_run(&self, job_id: &str) -> Result<Option<JobRun>>;
    fn mark_stale_jobs_failed(&self) -> Result<usize>;

    // Schedule state
    fn get_schedule_state(&self, job_id: &str) -> Result<Option<JobScheduleState>>;
    fn update_schedule_state(&self, state: &JobScheduleState) -> Result<()>;
    fn get_all_schedule_states(&self) -> Result<Vec<JobScheduleState>>;

    // Key-value state storage
    fn get_state(&self, key: &str) -> Result<Option<String>>;
    fn set_state(&self, key: &str, value: &str) -> Result<()>;
    fn delete_state(&self, key: &str) -> Result<()>;

    // Job audit log
    fn log_job_audit(
        &self,
        job_id: &str,
        event_type: JobAuditEventType,
        duration_ms: Option<i64>,
        details: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> Result<i64>;
    fn get_job_audit_log(&self, limit: usize, offset: usize) -> Result<Vec<JobAuditEntry>>;
    fn get_job_audit_log_by_job(
        &self,
        job_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<JobAuditEntry>>;
    fn cleanup_old_job_audit_entries(&self, before_timestamp: i64) -> Result<usize>;

    // Bug reports
    fn insert_bug_report(&self, report: &BugReport) -> Result<()>;
    fn get_bug_report(&self, id: &str) -> Result<Option<BugReport>>;
    fn list_bug_reports(&self, limit: usize, offset: usize) -> Result<Vec<BugReportSummary>>;
    fn delete_bug_report(&self, id: &str) -> Result<bool>;
    /// Returns total size in bytes of all bug reports (description + logs + attachments)
    fn get_bug_reports_total_size(&self) -> Result<usize>;
    /// Deletes oldest bug reports until total size is under the given limit.
    /// Returns the number of reports deleted.
    fn cleanup_bug_reports_to_size(&self, max_size: usize) -> Result<usize>;

    // Catalog events
    /// Append a new catalog event. Returns the sequence number.
    fn append_catalog_event(
        &self,
        event_type: CatalogEventType,
        content_type: CatalogContentType,
        content_id: &str,
        triggered_by: Option<&str>,
    ) -> Result<i64>;
    /// Get catalog events since a given sequence number (exclusive).
    fn get_catalog_events_since(&self, since_seq: i64) -> Result<Vec<CatalogEvent>>;
    /// Get the current (highest) sequence number for catalog events.
    fn get_catalog_events_current_seq(&self) -> Result<i64>;
    /// Delete catalog events older than a given timestamp (for pruning).
    fn cleanup_old_catalog_events(&self, before_timestamp: i64) -> Result<usize>;

    // What's New - pending albums
    /// Add an album to the pending What's New list.
    /// If the album is already pending or in a batch, this is a no-op.
    fn add_pending_whatsnew_album(&self, album_id: &str) -> Result<()>;
    /// Get all pending albums with their added_at timestamps.
    fn get_pending_whatsnew_albums(&self) -> Result<Vec<(String, i64)>>;
    /// Clear all pending albums (after batching).
    fn clear_pending_whatsnew_albums(&self) -> Result<()>;
    /// Check if an album is already in What's New (pending or batched).
    fn is_album_in_whatsnew(&self, album_id: &str) -> Result<bool>;

    // What's New - batches
    /// Create a new batch with the given albums.
    fn create_whatsnew_batch(&self, id: &str, closed_at: i64, album_ids: &[String]) -> Result<()>;
    /// List recent batches, most recent first.
    fn list_whatsnew_batches(&self, limit: usize) -> Result<Vec<WhatsNewBatch>>;
    /// Get album IDs for a specific batch.
    fn get_whatsnew_batch_album_ids(&self, batch_id: &str) -> Result<Vec<String>>;
}
