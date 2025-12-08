use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRunStatus {
    Running,
    Completed,
    Failed,
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
