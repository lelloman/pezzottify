use super::context::JobContext;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::time::Duration;

/// Coarse resource domain used to isolate jobs that compete for the same host capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobResourceClass {
    General,
    Lightweight,
    IoBound,
    CpuBound,
}

impl JobResourceClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Lightweight => "lightweight",
            Self::IoBound => "io_bound",
            Self::CpuBound => "cpu_bound",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "general" => Some(Self::General),
            "lightweight" => Some(Self::Lightweight),
            "io_bound" => Some(Self::IoBound),
            "cpu_bound" => Some(Self::CpuBound),
            _ => None,
        }
    }
}

/// Scheduler-enforced execution budgets declared by each background job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobExecutionPolicy {
    pub resource_class: JobResourceClass,
    pub queue_timeout: Duration,
    pub max_runtime: Option<Duration>,
}

impl JobExecutionPolicy {
    pub const fn new(resource_class: JobResourceClass) -> Self {
        Self {
            resource_class,
            queue_timeout: Duration::from_secs(30),
            max_runtime: None,
        }
    }

    pub fn with_queue_timeout(mut self, timeout: Duration) -> Self {
        assert!(!timeout.is_zero(), "job queue timeout must be non-zero");
        self.queue_timeout = timeout;
        self
    }

    pub fn with_max_runtime(mut self, timeout: Duration) -> Self {
        assert!(!timeout.is_zero(), "job runtime budget must be non-zero");
        self.max_runtime = Some(timeout);
        self
    }
}

impl Default for JobExecutionPolicy {
    fn default() -> Self {
        Self::new(JobResourceClass::General)
    }
}

/// Schedule for when a job should run.
#[derive(Debug, Clone)]
pub enum JobSchedule {
    /// Run only when explicitly triggered through the admin API.
    Manual,
    /// Run at specific times using cron syntax
    Cron(String),
    /// Run at fixed intervals
    Interval(Duration),
    /// Run at fixed intervals plus a random positive jitter.
    JitteredInterval {
        interval: Duration,
        jitter: Duration,
    },
    /// Run only in response to hooks
    Hook(HookEvent),
    /// Combination of scheduled and hook-triggered
    Combined {
        cron: Option<String>,
        interval: Option<Duration>,
        hooks: Vec<HookEvent>,
    },
}

/// Events that can trigger hook-based jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookEvent {
    OnStartup,
    OnCatalogChange,
    OnUserCreated,
    OnDownloadComplete,
}

impl std::fmt::Display for HookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookEvent::OnStartup => write!(f, "OnStartup"),
            HookEvent::OnCatalogChange => write!(f, "OnCatalogChange"),
            HookEvent::OnUserCreated => write!(f, "OnUserCreated"),
            HookEvent::OnDownloadComplete => write!(f, "OnDownloadComplete"),
        }
    }
}

/// How a job should be handled during server shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShutdownBehavior {
    /// Job can be cancelled immediately
    #[default]
    Cancellable,
    /// Wait for job to complete before shutdown
    WaitForCompletion,
}

/// Errors that can occur during job execution.
#[derive(Debug)]
pub enum JobError {
    NotFound,
    AlreadyRunning,
    NotRunning,
    Paused,
    ExecutionFailed(String),
    Cancelled,
    Timeout,
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobError::NotFound => write!(f, "Job not found"),
            JobError::AlreadyRunning => write!(f, "Job is already running"),
            JobError::NotRunning => write!(f, "Job is not running"),
            JobError::Paused => write!(f, "Job execution is paused"),
            JobError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            JobError::Cancelled => write!(f, "Job was cancelled"),
            JobError::Timeout => write!(f, "Job timed out"),
        }
    }
}

impl std::error::Error for JobError {}

/// Trait for background jobs.
///
/// Jobs are executed synchronously in a blocking context.
/// Long-running work should spawn tasks internally and check for cancellation.
pub trait BackgroundJob: Send + Sync {
    /// Unique identifier for this job.
    fn id(&self) -> &'static str;

    /// Human-readable name for this job.
    fn name(&self) -> &'static str;

    /// Description of what this job does.
    fn description(&self) -> &'static str;

    /// When this job should be scheduled to run.
    fn schedule(&self) -> JobSchedule;

    /// Resource class and execution budgets enforced by the scheduler.
    fn execution_policy(&self) -> JobExecutionPolicy {
        JobExecutionPolicy::default()
    }

    /// How this job should be handled during shutdown.
    fn shutdown_behavior(&self) -> ShutdownBehavior {
        ShutdownBehavior::Cancellable
    }

    /// Whether an interval job with no persisted schedule should run immediately.
    /// Heavy jobs should return false so first startup establishes a future run.
    fn run_on_startup(&self) -> bool {
        true
    }

    /// Execute the job.
    ///
    /// This method is called from a blocking context using `spawn_blocking`.
    /// The implementation should periodically check `ctx.is_cancelled()` for
    /// long-running operations and return early with `JobError::Cancelled` if true.
    fn execute(&self, ctx: &JobContext) -> Result<(), JobError>;

    /// Execute the job with optional parameters.
    ///
    /// This method is called when a job is triggered manually via the admin API
    /// with optional JSON parameters in the request body. The default implementation
    /// ignores the parameters and delegates to `execute()`.
    ///
    /// Jobs that need to accept runtime parameters should override this method.
    fn execute_with_params(
        &self,
        ctx: &JobContext,
        _params: Option<JsonValue>,
    ) -> Result<(), JobError> {
        self.execute(ctx)
    }
}
