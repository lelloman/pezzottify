use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRunStatus {
    Running,
    Completed,
    Failed,
}

/// Event types for job audit log entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobAuditEventType {
    Started,
    Completed,
    Failed,
    Progress,
}

impl JobAuditEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobAuditEventType::Started => "started",
            JobAuditEventType::Completed => "completed",
            JobAuditEventType::Failed => "failed",
            JobAuditEventType::Progress => "progress",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "started" => Some(JobAuditEventType::Started),
            "completed" => Some(JobAuditEventType::Completed),
            "failed" => Some(JobAuditEventType::Failed),
            "progress" => Some(JobAuditEventType::Progress),
            _ => None,
        }
    }
}

/// An entry in the job audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobAuditEntry {
    pub id: i64,
    pub job_id: String,
    pub event_type: JobAuditEventType,
    /// Unix timestamp when the event occurred
    pub timestamp: i64,
    pub duration_ms: Option<i64>,
    pub details: Option<serde_json::Value>,
    pub error: Option<String>,
}

impl JobRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobRunStatus::Running => "running",
            JobRunStatus::Completed => "completed",
            JobRunStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "running" => Some(JobRunStatus::Running),
            "completed" => Some(JobRunStatus::Completed),
            "failed" => Some(JobRunStatus::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct JobRun {
    pub id: i64,
    pub job_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: JobRunStatus,
    pub error_message: Option<String>,
    /// How the job was triggered: "schedule", "hook:OnStartup", "manual", etc.
    pub triggered_by: String,
}

#[derive(Debug, Clone)]
pub struct JobScheduleState {
    pub job_id: String,
    pub next_run_at: DateTime<Utc>,
    pub last_run_at: Option<DateTime<Utc>>,
}
