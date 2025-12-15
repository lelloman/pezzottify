use super::job::{BackgroundJob, JobError, JobSchedule};
use crate::server_store::{JobAuditEntry, JobRun, ServerStore};
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};

/// Information about a registered job for API responses.
#[derive(Debug, Clone, Serialize)]
pub struct JobInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub schedule: JobScheduleInfo,
    pub is_running: bool,
    pub last_run: Option<JobRunInfo>,
    pub next_run_at: Option<String>,
}

/// Serializable schedule information.
#[derive(Debug, Clone, Serialize)]
pub struct JobScheduleInfo {
    #[serde(rename = "type")]
    pub schedule_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Vec<String>>,
}

impl From<JobSchedule> for JobScheduleInfo {
    fn from(schedule: JobSchedule) -> Self {
        match schedule {
            JobSchedule::Cron(expr) => JobScheduleInfo {
                schedule_type: "cron".to_string(),
                cron: Some(expr),
                value_secs: None,
                hooks: None,
            },
            JobSchedule::Interval(duration) => JobScheduleInfo {
                schedule_type: "interval".to_string(),
                value_secs: Some(duration.as_secs()),
                cron: None,
                hooks: None,
            },
            JobSchedule::Hook(event) => JobScheduleInfo {
                schedule_type: "hook".to_string(),
                hooks: Some(vec![event.to_string()]),
                value_secs: None,
                cron: None,
            },
            JobSchedule::Combined {
                cron,
                interval,
                hooks,
            } => JobScheduleInfo {
                schedule_type: "combined".to_string(),
                cron,
                value_secs: interval.map(|d| d.as_secs()),
                hooks: Some(hooks.iter().map(|h| h.to_string()).collect()),
            },
        }
    }
}

/// Serializable job run information.
#[derive(Debug, Clone, Serialize)]
pub struct JobRunInfo {
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub triggered_by: String,
}

impl From<JobRun> for JobRunInfo {
    fn from(run: JobRun) -> Self {
        JobRunInfo {
            started_at: run.started_at.to_rfc3339(),
            finished_at: run.finished_at.map(|dt| dt.to_rfc3339()),
            status: run.status.as_str().to_string(),
            error_message: run.error_message,
            triggered_by: run.triggered_by,
        }
    }
}

/// Command sent to the scheduler.
pub enum SchedulerCommand {
    TriggerJob {
        job_id: String,
        response: oneshot::Sender<Result<(), JobError>>,
    },
}

/// Shared state between scheduler and handle.
pub struct SharedJobState {
    /// Static job info (set at registration, never changes)
    pub jobs: HashMap<String, Arc<dyn BackgroundJob>>,
    /// Currently running job IDs
    pub running_jobs: std::collections::HashSet<String>,
}

/// Handle to interact with the job scheduler from HTTP handlers.
#[derive(Clone)]
pub struct SchedulerHandle {
    /// Channel to send commands to the scheduler
    command_tx: mpsc::Sender<SchedulerCommand>,
    /// Shared state for reading job info
    shared_state: Arc<RwLock<SharedJobState>>,
    /// Server store for job history queries
    server_store: Arc<dyn ServerStore>,
}

impl SchedulerHandle {
    /// Create a new scheduler handle.
    pub fn new(
        command_tx: mpsc::Sender<SchedulerCommand>,
        shared_state: Arc<RwLock<SharedJobState>>,
        server_store: Arc<dyn ServerStore>,
    ) -> Self {
        Self {
            command_tx,
            shared_state,
            server_store,
        }
    }

    /// Get information about all registered jobs.
    pub async fn list_jobs(&self) -> Result<Vec<JobInfo>> {
        let state = self.shared_state.read().await;
        let mut jobs = Vec::new();

        for (job_id, job) in &state.jobs {
            let is_running = state.running_jobs.contains(job_id);
            let last_run = self
                .server_store
                .get_last_run(job_id)?
                .map(JobRunInfo::from);
            let schedule_state = self.server_store.get_schedule_state(job_id)?;
            let next_run_at = schedule_state.map(|s| s.next_run_at.to_rfc3339());

            jobs.push(JobInfo {
                id: job_id.clone(),
                name: job.name().to_string(),
                description: job.description().to_string(),
                schedule: job.schedule().into(),
                is_running,
                last_run,
                next_run_at,
            });
        }

        // Sort by job ID for consistent ordering
        jobs.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(jobs)
    }

    /// Get information about a specific job.
    pub async fn get_job(&self, job_id: &str) -> Result<Option<JobInfo>> {
        let state = self.shared_state.read().await;

        if let Some(job) = state.jobs.get(job_id) {
            let is_running = state.running_jobs.contains(job_id);
            let last_run = self
                .server_store
                .get_last_run(job_id)?
                .map(JobRunInfo::from);
            let schedule_state = self.server_store.get_schedule_state(job_id)?;
            let next_run_at = schedule_state.map(|s| s.next_run_at.to_rfc3339());

            Ok(Some(JobInfo {
                id: job_id.to_string(),
                name: job.name().to_string(),
                description: job.description().to_string(),
                schedule: job.schedule().into(),
                is_running,
                last_run,
                next_run_at,
            }))
        } else {
            Ok(None)
        }
    }

    /// Trigger a job manually.
    pub async fn trigger_job(&self, job_id: &str) -> Result<(), JobError> {
        let (response_tx, response_rx) = oneshot::channel();

        self.command_tx
            .send(SchedulerCommand::TriggerJob {
                job_id: job_id.to_string(),
                response: response_tx,
            })
            .await
            .map_err(|_| JobError::ExecutionFailed("Scheduler not available".to_string()))?;

        response_rx
            .await
            .map_err(|_| JobError::ExecutionFailed("Scheduler did not respond".to_string()))?
    }

    /// Get job execution history.
    pub fn get_job_history(&self, job_id: &str, limit: usize) -> Result<Vec<JobRunInfo>> {
        let history = self.server_store.get_job_history(job_id, limit)?;
        Ok(history.into_iter().map(JobRunInfo::from).collect())
    }

    /// Check if a job is currently running.
    pub async fn is_job_running(&self, job_id: &str) -> bool {
        let state = self.shared_state.read().await;
        state.running_jobs.contains(job_id)
    }

    /// Check if a job with the given ID exists.
    pub async fn job_exists(&self, job_id: &str) -> bool {
        let state = self.shared_state.read().await;
        state.jobs.contains_key(job_id)
    }

    /// Get job audit log entries (all jobs).
    pub fn get_job_audit_log(&self, limit: usize, offset: usize) -> Result<Vec<JobAuditEntry>> {
        self.server_store.get_job_audit_log(limit, offset)
    }

    /// Get job audit log entries for a specific job.
    pub fn get_job_audit_log_by_job(
        &self,
        job_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<JobAuditEntry>> {
        self.server_store
            .get_job_audit_log_by_job(job_id, limit, offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background_jobs::job::HookEvent;
    use crate::server_store::JobRunStatus;
    use chrono::Utc;
    use std::time::Duration;

    #[test]
    fn test_job_schedule_info_from_cron() {
        let schedule = JobSchedule::Cron("0 0 * * *".to_string());
        let info: JobScheduleInfo = schedule.into();

        assert_eq!(info.schedule_type, "cron");
        assert_eq!(info.cron, Some("0 0 * * *".to_string()));
        assert!(info.value_secs.is_none());
        assert!(info.hooks.is_none());
    }

    #[test]
    fn test_job_schedule_info_from_interval() {
        let schedule = JobSchedule::Interval(Duration::from_secs(3600));
        let info: JobScheduleInfo = schedule.into();

        assert_eq!(info.schedule_type, "interval");
        assert_eq!(info.value_secs, Some(3600));
        assert!(info.cron.is_none());
        assert!(info.hooks.is_none());
    }

    #[test]
    fn test_job_schedule_info_from_hook() {
        let schedule = JobSchedule::Hook(HookEvent::OnStartup);
        let info: JobScheduleInfo = schedule.into();

        assert_eq!(info.schedule_type, "hook");
        assert_eq!(info.hooks, Some(vec!["OnStartup".to_string()]));
        assert!(info.value_secs.is_none());
        assert!(info.cron.is_none());
    }

    #[test]
    fn test_job_schedule_info_from_combined() {
        let schedule = JobSchedule::Combined {
            cron: Some("0 * * * *".to_string()),
            interval: Some(Duration::from_secs(1800)),
            hooks: vec![HookEvent::OnStartup, HookEvent::OnCatalogChange],
        };
        let info: JobScheduleInfo = schedule.into();

        assert_eq!(info.schedule_type, "combined");
        assert_eq!(info.cron, Some("0 * * * *".to_string()));
        assert_eq!(info.value_secs, Some(1800));
        let hooks = info.hooks.unwrap();
        assert_eq!(hooks.len(), 2);
        assert!(hooks.contains(&"OnStartup".to_string()));
        assert!(hooks.contains(&"OnCatalogChange".to_string()));
    }

    #[test]
    fn test_job_schedule_info_combined_without_optional_fields() {
        let schedule = JobSchedule::Combined {
            cron: None,
            interval: None,
            hooks: vec![HookEvent::OnStartup],
        };
        let info: JobScheduleInfo = schedule.into();

        assert_eq!(info.schedule_type, "combined");
        assert!(info.cron.is_none());
        assert!(info.value_secs.is_none());
        assert_eq!(info.hooks, Some(vec!["OnStartup".to_string()]));
    }

    #[test]
    fn test_job_run_info_from_completed() {
        let now = Utc::now();
        let finished = now + chrono::Duration::seconds(10);
        let run = JobRun {
            id: 1,
            job_id: "test_job".to_string(),
            started_at: now,
            finished_at: Some(finished),
            status: JobRunStatus::Completed,
            error_message: None,
            triggered_by: "manual".to_string(),
        };

        let info: JobRunInfo = run.into();

        assert_eq!(info.status, "completed");
        assert!(info.error_message.is_none());
        assert_eq!(info.triggered_by, "manual");
        assert!(info.finished_at.is_some());
    }

    #[test]
    fn test_job_run_info_from_failed() {
        let now = Utc::now();
        let run = JobRun {
            id: 2,
            job_id: "test_job".to_string(),
            started_at: now,
            finished_at: Some(now + chrono::Duration::seconds(5)),
            status: JobRunStatus::Failed,
            error_message: Some("Something went wrong".to_string()),
            triggered_by: "schedule".to_string(),
        };

        let info: JobRunInfo = run.into();

        assert_eq!(info.status, "failed");
        assert_eq!(info.error_message, Some("Something went wrong".to_string()));
        assert_eq!(info.triggered_by, "schedule");
    }

    #[test]
    fn test_job_run_info_from_running() {
        let now = Utc::now();
        let run = JobRun {
            id: 3,
            job_id: "test_job".to_string(),
            started_at: now,
            finished_at: None,
            status: JobRunStatus::Running,
            error_message: None,
            triggered_by: "hook:OnStartup".to_string(),
        };

        let info: JobRunInfo = run.into();

        assert_eq!(info.status, "running");
        assert!(info.finished_at.is_none());
        assert_eq!(info.triggered_by, "hook:OnStartup");
    }

    #[test]
    fn test_job_run_info_datetime_format() {
        let now = Utc::now();
        let run = JobRun {
            id: 1,
            job_id: "test_job".to_string(),
            started_at: now,
            finished_at: Some(now),
            status: JobRunStatus::Completed,
            error_message: None,
            triggered_by: "test".to_string(),
        };

        let info: JobRunInfo = run.into();

        // Should be RFC3339 format
        assert!(info.started_at.contains("T"));
        assert!(info.started_at.contains("+") || info.started_at.contains("Z"));
        assert!(info.finished_at.unwrap().contains("T"));
    }
}
