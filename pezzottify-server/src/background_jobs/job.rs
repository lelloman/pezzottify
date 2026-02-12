use super::context::JobContext;
use anyhow::Result;
use serde_json::Value as JsonValue;
use std::time::Duration;

/// Schedule for when a job should run.
#[derive(Debug, Clone)]
pub enum JobSchedule {
    /// Run at specific times using cron syntax
    Cron(String),
    /// Run at fixed intervals
    Interval(Duration),
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
    ExecutionFailed(String),
    Cancelled,
    Timeout,
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobError::NotFound => write!(f, "Job not found"),
            JobError::AlreadyRunning => write!(f, "Job is already running"),
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

    /// How this job should be handled during shutdown.
    fn shutdown_behavior(&self) -> ShutdownBehavior {
        ShutdownBehavior::Cancellable
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
