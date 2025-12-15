//! Job audit logging utilities.
//!
//! Provides a convenient interface for background jobs to log audit events.

use crate::server_store::{JobAuditEventType, ServerStore};
use std::sync::Arc;
use std::time::Instant;

/// Helper for logging job audit events.
///
/// Provides a convenient interface for background jobs to log their execution
/// progress and results to the centralized audit log.
pub struct JobAuditLogger {
    server_store: Arc<dyn ServerStore>,
    job_id: String,
    start_time: Instant,
}

impl JobAuditLogger {
    /// Create a new audit logger for a job.
    pub fn new(server_store: Arc<dyn ServerStore>, job_id: &str) -> Self {
        Self {
            server_store,
            job_id: job_id.to_string(),
            start_time: Instant::now(),
        }
    }

    /// Log that the job has started.
    pub fn log_started(&self, details: Option<serde_json::Value>) {
        let _ = self.server_store.log_job_audit(
            &self.job_id,
            JobAuditEventType::Started,
            None,
            details.as_ref(),
            None,
        );
    }

    /// Log that the job has completed successfully.
    pub fn log_completed(&self, details: Option<serde_json::Value>) {
        let duration_ms = self.start_time.elapsed().as_millis() as i64;
        let _ = self.server_store.log_job_audit(
            &self.job_id,
            JobAuditEventType::Completed,
            Some(duration_ms),
            details.as_ref(),
            None,
        );
    }

    /// Log that the job has failed.
    pub fn log_failed(&self, error: &str, details: Option<serde_json::Value>) {
        let duration_ms = self.start_time.elapsed().as_millis() as i64;
        let _ = self.server_store.log_job_audit(
            &self.job_id,
            JobAuditEventType::Failed,
            Some(duration_ms),
            details.as_ref(),
            Some(error),
        );
    }

    /// Log a progress update during job execution.
    pub fn log_progress(&self, details: serde_json::Value) {
        let _ = self.server_store.log_job_audit(
            &self.job_id,
            JobAuditEventType::Progress,
            None,
            Some(&details),
            None,
        );
    }

    /// Get the elapsed time since the job started.
    pub fn elapsed_ms(&self) -> i64 {
        self.start_time.elapsed().as_millis() as i64
    }
}
