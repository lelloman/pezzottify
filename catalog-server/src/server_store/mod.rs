mod models;
mod schema;
mod sqlite_server_store;

pub use models::*;
pub use schema::{ServerSchema, SERVER_VERSIONED_SCHEMAS};
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
}
