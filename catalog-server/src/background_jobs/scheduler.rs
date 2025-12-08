use super::context::JobContext;
use super::handle::{SchedulerCommand, SharedJobState};
use super::job::{BackgroundJob, HookEvent, JobError, JobSchedule, ShutdownBehavior};
use crate::server_store::{JobRunStatus, ServerStore};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Manages background job scheduling and execution.
pub struct JobScheduler {
    /// Shared state accessible by SchedulerHandle
    shared_state: Arc<RwLock<SharedJobState>>,

    /// Currently running jobs with their task handles (not shared, managed by scheduler loop)
    running_handles: HashMap<String, JoinHandle<()>>,

    /// Cancellation tokens for each running job.
    job_cancel_tokens: HashMap<String, CancellationToken>,

    /// Server store for persisting job history.
    server_store: Arc<dyn ServerStore>,

    /// Receiver for hook events from the HTTP server.
    hook_receiver: mpsc::Receiver<HookEvent>,

    /// Receiver for commands from SchedulerHandle
    command_receiver: mpsc::Receiver<SchedulerCommand>,

    /// Token to signal scheduler shutdown.
    shutdown_token: CancellationToken,

    /// Shared context provided to jobs during execution.
    job_context: JobContext,
}

impl JobScheduler {
    /// Create a new job scheduler and return a handle for interacting with it.
    pub fn new(
        server_store: Arc<dyn ServerStore>,
        hook_receiver: mpsc::Receiver<HookEvent>,
        command_receiver: mpsc::Receiver<SchedulerCommand>,
        shutdown_token: CancellationToken,
        job_context: JobContext,
        shared_state: Arc<RwLock<SharedJobState>>,
    ) -> Self {
        Self {
            shared_state,
            running_handles: HashMap::new(),
            job_cancel_tokens: HashMap::new(),
            server_store,
            hook_receiver,
            command_receiver,
            shutdown_token,
            job_context,
        }
    }

    /// Register a job with the scheduler.
    pub async fn register_job(&mut self, job: Arc<dyn BackgroundJob>) {
        let job_id = job.id().to_string();
        info!(
            "Registering job: {} - {}",
            job_id,
            job.description()
        );
        let mut state = self.shared_state.write().await;
        state.jobs.insert(job_id, job);
    }

    /// Get the number of registered jobs.
    pub async fn job_count(&self) -> usize {
        self.shared_state.read().await.jobs.len()
    }

    /// Main scheduler loop.
    pub async fn run(&mut self) {
        let job_count = self.job_count().await;
        info!("Starting job scheduler with {} registered jobs", job_count);

        // On startup: mark any stale running jobs as failed
        match self.server_store.mark_stale_jobs_failed() {
            Ok(count) if count > 0 => {
                info!("Marked {} stale jobs as failed from previous run", count);
            }
            Ok(_) => {}
            Err(e) => {
                error!("Failed to mark stale jobs: {}", e);
            }
        }

        // Fire OnStartup hooks
        self.trigger_jobs_for_hook(HookEvent::OnStartup).await;

        loop {
            // Clean up completed job handles
            self.cleanup_completed_jobs().await;

            let sleep_duration = self.time_until_next_scheduled_job().await;
            debug!(
                "Scheduler sleeping for {:?} until next scheduled job",
                sleep_duration
            );

            tokio::select! {
                _ = tokio::time::sleep(sleep_duration) => {
                    self.run_due_jobs().await;
                }
                Some(event) = self.hook_receiver.recv() => {
                    debug!("Received hook event: {}", event);
                    self.trigger_jobs_for_hook(event).await;
                }
                Some(cmd) = self.command_receiver.recv() => {
                    self.handle_command(cmd).await;
                }
                _ = self.shutdown_token.cancelled() => {
                    info!("Scheduler received shutdown signal");
                    self.shutdown().await;
                    break;
                }
            }
        }

        info!("Job scheduler stopped");
    }

    /// Handle a command from the SchedulerHandle.
    async fn handle_command(&mut self, cmd: SchedulerCommand) {
        match cmd {
            SchedulerCommand::TriggerJob { job_id, response } => {
                let result = self.trigger_job(&job_id).await;
                let _ = response.send(result);
            }
        }
    }

    /// Manually trigger a job by ID.
    async fn trigger_job(&mut self, job_id: &str) -> Result<(), JobError> {
        let state = self.shared_state.read().await;
        if !state.jobs.contains_key(job_id) {
            return Err(JobError::NotFound);
        }

        if state.running_jobs.contains(job_id) {
            return Err(JobError::AlreadyRunning);
        }
        drop(state);

        self.spawn_job(job_id, "manual").await;
        Ok(())
    }

    /// Calculate time until the next scheduled job should run.
    async fn time_until_next_scheduled_job(&self) -> Duration {
        let mut min_duration = Duration::from_secs(60); // Default check interval

        let state = self.shared_state.read().await;
        for (job_id, job) in &state.jobs {
            if state.running_jobs.contains(job_id) {
                continue; // Skip already running jobs
            }

            if let Some(next_run) = self.get_next_run_time(job_id, job.schedule()) {
                let now = chrono::Utc::now();
                if next_run > now {
                    let duration = (next_run - now).to_std().unwrap_or(Duration::from_secs(1));
                    if duration < min_duration {
                        min_duration = duration;
                    }
                } else {
                    // Job is due now
                    return Duration::from_secs(0);
                }
            }
        }

        min_duration
    }

    /// Get the next scheduled run time for a job.
    fn get_next_run_time(
        &self,
        job_id: &str,
        schedule: JobSchedule,
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        match schedule {
            JobSchedule::Interval(_interval) => {
                // Get last run time from server store
                if let Ok(Some(state)) = self.server_store.get_schedule_state(job_id) {
                    Some(state.next_run_at)
                } else {
                    // No schedule state - run immediately on first interval
                    Some(chrono::Utc::now())
                }
            }
            JobSchedule::Cron(ref cron_expr) => {
                // Parse cron expression and calculate next run
                // For now, return None (cron will be implemented later)
                warn!("Cron scheduling not yet implemented for job {}: {}", job_id, cron_expr);
                None
            }
            JobSchedule::Hook(_) => {
                // Hook-only jobs don't have scheduled runs
                None
            }
            JobSchedule::Combined { cron, interval, .. } => {
                // Return the earliest of cron and interval schedules
                let interval_time = interval.and_then(|_int| {
                    if let Ok(Some(state)) = self.server_store.get_schedule_state(job_id) {
                        Some(state.next_run_at)
                    } else {
                        Some(chrono::Utc::now())
                    }
                });

                // Cron not implemented yet
                if cron.is_some() {
                    warn!("Cron scheduling in Combined not yet implemented for job {}", job_id);
                }

                interval_time
            }
        }
    }

    /// Run all jobs that are due for scheduled execution.
    async fn run_due_jobs(&mut self) {
        let now = chrono::Utc::now();
        let mut jobs_to_run = Vec::new();

        {
            let state = self.shared_state.read().await;
            for (job_id, job) in &state.jobs {
                if state.running_jobs.contains(job_id) {
                    continue;
                }

                if let Some(next_run) = self.get_next_run_time(job_id, job.schedule()) {
                    if next_run <= now {
                        jobs_to_run.push(job_id.clone());
                    }
                }
            }
        }

        for job_id in jobs_to_run {
            self.spawn_job(&job_id, "schedule").await;
        }
    }

    /// Trigger all jobs that listen for a specific hook event.
    async fn trigger_jobs_for_hook(&mut self, event: HookEvent) {
        let mut jobs_to_trigger = Vec::new();

        {
            let state = self.shared_state.read().await;
            for (job_id, job) in &state.jobs {
                if state.running_jobs.contains(job_id) {
                    debug!("Skipping hook trigger for already running job: {}", job_id);
                    continue;
                }

                let should_trigger = match job.schedule() {
                    JobSchedule::Hook(hook_event) => hook_event == event,
                    JobSchedule::Combined { ref hooks, .. } => hooks.contains(&event),
                    _ => false,
                };

                if should_trigger {
                    jobs_to_trigger.push(job_id.clone());
                }
            }
        }

        for job_id in jobs_to_trigger {
            let trigger = format!("hook:{}", event);
            self.spawn_job(&job_id, &trigger).await;
        }
    }

    /// Spawn a job execution task.
    async fn spawn_job(&mut self, job_id: &str, triggered_by: &str) {
        let job = {
            let state = self.shared_state.read().await;
            match state.jobs.get(job_id) {
                Some(job) => Arc::clone(job),
                None => {
                    error!("Attempted to spawn unknown job: {}", job_id);
                    return;
                }
            }
        };

        // Record job start
        let run_id = match self.server_store.record_job_start(job_id, triggered_by) {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to record job start for {}: {}", job_id, e);
                return;
            }
        };

        info!("Starting job: {} (run_id: {}, triggered_by: {})", job_id, run_id, triggered_by);

        // Mark job as running in shared state
        {
            let mut state = self.shared_state.write().await;
            state.running_jobs.insert(job_id.to_string());
        }

        // Create cancellation token for this job
        let cancel_token = self.job_context.cancellation_token.child_token();
        self.job_cancel_tokens.insert(job_id.to_string(), cancel_token.clone());

        // Create job context with the specific cancellation token
        let ctx = JobContext::new(
            cancel_token,
            Arc::clone(&self.job_context.catalog_store),
            Arc::clone(&self.job_context.user_store),
            Arc::clone(&self.job_context.server_store),
        );

        let server_store = Arc::clone(&self.server_store);
        let job_id_owned = job_id.to_string();
        let shared_state = Arc::clone(&self.shared_state);

        // Spawn the job in a blocking task since jobs are synchronous
        let handle = tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                job.execute(&ctx)
            }).await;

            // Record job completion
            let (status, error_msg) = match result {
                Ok(Ok(())) => {
                    info!("Job {} completed successfully", job_id_owned);
                    (JobRunStatus::Completed, None)
                }
                Ok(Err(e)) => {
                    match e {
                        JobError::Cancelled => {
                            info!("Job {} was cancelled", job_id_owned);
                            (JobRunStatus::Failed, Some("Cancelled".to_string()))
                        }
                        _ => {
                            error!("Job {} failed: {}", job_id_owned, e);
                            (JobRunStatus::Failed, Some(e.to_string()))
                        }
                    }
                }
                Err(e) => {
                    error!("Job {} panicked: {}", job_id_owned, e);
                    (JobRunStatus::Failed, Some(format!("Task panic: {}", e)))
                }
            };

            if let Err(e) = server_store.record_job_finish(run_id, status, error_msg) {
                error!("Failed to record job finish for {}: {}", job_id_owned, e);
            }

            // Mark job as not running in shared state
            {
                let mut state = shared_state.write().await;
                state.running_jobs.remove(&job_id_owned);
            }
        });

        self.running_handles.insert(job_id.to_string(), handle);
    }

    /// Update schedule state after a job completes (for interval-based jobs).
    async fn update_schedule_after_run(&self, job_id: &str) {
        let job = {
            let state = self.shared_state.read().await;
            match state.jobs.get(job_id) {
                Some(job) => Arc::clone(job),
                None => return,
            }
        };

        let interval = match job.schedule() {
            JobSchedule::Interval(int) => Some(int),
            JobSchedule::Combined { interval, .. } => interval,
            _ => None,
        };

        if let Some(interval) = interval {
            let next_run = chrono::Utc::now() + chrono::Duration::from_std(interval).unwrap_or_default();
            let state = crate::server_store::JobScheduleState {
                job_id: job_id.to_string(),
                next_run_at: next_run,
                last_run_at: Some(chrono::Utc::now()),
            };

            if let Err(e) = self.server_store.update_schedule_state(&state) {
                error!("Failed to update schedule state for {}: {}", job_id, e);
            }
        }
    }

    /// Clean up handles for completed jobs.
    async fn cleanup_completed_jobs(&mut self) {
        let mut completed = Vec::new();

        for (job_id, handle) in &self.running_handles {
            if handle.is_finished() {
                completed.push(job_id.clone());
            }
        }

        for job_id in completed {
            if let Some(handle) = self.running_handles.remove(&job_id) {
                let _ = handle.await;
            }
            self.job_cancel_tokens.remove(&job_id);
            self.update_schedule_after_run(&job_id).await;
        }
    }

    /// Gracefully shut down the scheduler.
    async fn shutdown(&mut self) {
        info!("Shutting down scheduler...");

        // Cancel cancellable jobs
        {
            let state = self.shared_state.read().await;
            for job_id in &state.running_jobs {
                if let Some(job) = state.jobs.get(job_id) {
                    if job.shutdown_behavior() == ShutdownBehavior::Cancellable {
                        if let Some(token) = self.job_cancel_tokens.get(job_id) {
                            debug!("Cancelling job: {}", job_id);
                            token.cancel();
                        }
                    }
                }
            }
        }

        // Wait for all jobs to complete
        let mut wait_jobs = Vec::new();
        for (job_id, handle) in self.running_handles.drain() {
            let behavior = {
                let state = self.shared_state.read().await;
                state.jobs.get(&job_id)
                    .map(|j| j.shutdown_behavior())
                    .unwrap_or(ShutdownBehavior::Cancellable)
            };
            wait_jobs.push((job_id, handle, behavior));
        }

        for (job_id, handle, behavior) in wait_jobs {
            if behavior == ShutdownBehavior::WaitForCompletion {
                info!("Waiting for job {} to complete...", job_id);
            }
            let _ = tokio::time::timeout(Duration::from_secs(30), handle).await;
        }

        self.job_cancel_tokens.clear();
        info!("Scheduler shutdown complete");
    }
}

/// Create a scheduler and its handle.
pub fn create_scheduler(
    server_store: Arc<dyn ServerStore>,
    hook_receiver: mpsc::Receiver<HookEvent>,
    shutdown_token: CancellationToken,
    job_context: JobContext,
) -> (JobScheduler, super::handle::SchedulerHandle) {
    let (command_tx, command_rx) = mpsc::channel(100);
    let shared_state = Arc::new(RwLock::new(SharedJobState {
        jobs: HashMap::new(),
        running_jobs: HashSet::new(),
    }));

    let scheduler = JobScheduler::new(
        server_store.clone(),
        hook_receiver,
        command_rx,
        shutdown_token,
        job_context,
        Arc::clone(&shared_state),
    );

    let handle = super::handle::SchedulerHandle::new(
        command_tx,
        shared_state,
        server_store,
    );

    (scheduler, handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::NullCatalogStore;
    use crate::server_store::SqliteServerStore;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tempfile::TempDir;

    // Test job implementation
    struct TestJob {
        id: &'static str,
        execution_count: Arc<AtomicUsize>,
        should_fail: Arc<AtomicBool>,
    }

    impl BackgroundJob for TestJob {
        fn id(&self) -> &'static str {
            self.id
        }

        fn name(&self) -> &'static str {
            "Test Job"
        }

        fn description(&self) -> &'static str {
            "A test job for unit tests"
        }

        fn schedule(&self) -> JobSchedule {
            JobSchedule::Hook(HookEvent::OnStartup)
        }

        fn execute(&self, _ctx: &JobContext) -> Result<(), JobError> {
            self.execution_count.fetch_add(1, Ordering::SeqCst);
            if self.should_fail.load(Ordering::SeqCst) {
                Err(JobError::ExecutionFailed("Test failure".to_string()))
            } else {
                Ok(())
            }
        }
    }

    fn create_test_scheduler() -> (JobScheduler, super::super::handle::SchedulerHandle, TempDir, mpsc::Sender<HookEvent>) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("server.db");
        let server_store = Arc::new(SqliteServerStore::new(&db_path).unwrap());

        let (hook_sender, hook_receiver) = mpsc::channel(100);
        let shutdown_token = CancellationToken::new();

        // Use NullCatalogStore for tests
        let catalog_store: Arc<dyn crate::catalog_store::CatalogStore> = Arc::new(NullCatalogStore);

        // For user store, we need to create a real one since it's complex
        let user_db_path = temp_dir.path().join("user.db");
        let user_store: Arc<dyn crate::user::FullUserStore> =
            Arc::new(crate::user::SqliteUserStore::new(&user_db_path).unwrap());

        let job_context = JobContext::new(
            shutdown_token.child_token(),
            catalog_store,
            user_store,
            server_store.clone(),
        );

        let (scheduler, handle) = create_scheduler(
            server_store,
            hook_receiver,
            shutdown_token,
            job_context,
        );

        (scheduler, handle, temp_dir, hook_sender)
    }

    #[tokio::test]
    async fn test_register_job() {
        let (mut scheduler, handle, _temp_dir, _hook_sender) = create_test_scheduler();

        let exec_count = Arc::new(AtomicUsize::new(0));
        let job = Arc::new(TestJob {
            id: "test_job",
            execution_count: exec_count,
            should_fail: Arc::new(AtomicBool::new(false)),
        });

        scheduler.register_job(job).await;

        let jobs = handle.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "test_job");
    }

    #[tokio::test]
    async fn test_job_exists_check() {
        let (mut scheduler, handle, _temp_dir, _hook_sender) = create_test_scheduler();

        // Check that nonexistent job returns false
        assert!(!handle.job_exists("nonexistent").await);

        // Register a job
        let exec_count = Arc::new(AtomicUsize::new(0));
        let job = Arc::new(TestJob {
            id: "test_job",
            execution_count: exec_count,
            should_fail: Arc::new(AtomicBool::new(false)),
        });
        scheduler.register_job(job).await;

        // Now check that existing job returns true
        assert!(handle.job_exists("test_job").await);
        assert!(!handle.job_exists("nonexistent").await);
    }
}
