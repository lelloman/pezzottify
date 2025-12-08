use super::context::JobContext;
use super::job::{BackgroundJob, HookEvent, JobError, JobSchedule, ShutdownBehavior};
use crate::server_store::{JobRunStatus, ServerStore};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Manages background job scheduling and execution.
pub struct JobScheduler {
    /// Registered jobs indexed by their ID.
    jobs: HashMap<String, Arc<dyn BackgroundJob>>,

    /// Currently running jobs with their task handles.
    running_jobs: HashMap<String, JoinHandle<()>>,

    /// Cancellation tokens for each running job.
    job_cancel_tokens: HashMap<String, CancellationToken>,

    /// Server store for persisting job history.
    server_store: Arc<dyn ServerStore>,

    /// Receiver for hook events from the HTTP server.
    hook_receiver: mpsc::Receiver<HookEvent>,

    /// Token to signal scheduler shutdown.
    shutdown_token: CancellationToken,

    /// Shared context provided to jobs during execution.
    job_context: JobContext,
}

impl JobScheduler {
    /// Create a new job scheduler.
    pub fn new(
        server_store: Arc<dyn ServerStore>,
        hook_receiver: mpsc::Receiver<HookEvent>,
        shutdown_token: CancellationToken,
        job_context: JobContext,
    ) -> Self {
        Self {
            jobs: HashMap::new(),
            running_jobs: HashMap::new(),
            job_cancel_tokens: HashMap::new(),
            server_store,
            hook_receiver,
            shutdown_token,
            job_context,
        }
    }

    /// Register a job with the scheduler.
    pub fn register_job(&mut self, job: Arc<dyn BackgroundJob>) {
        let job_id = job.id().to_string();
        info!(
            "Registering job: {} - {}",
            job_id,
            job.description()
        );
        self.jobs.insert(job_id, job);
    }

    /// Manually trigger a job by ID.
    pub fn trigger_job(&mut self, job_id: &str) -> Result<(), JobError> {
        if !self.jobs.contains_key(job_id) {
            return Err(JobError::NotFound);
        }

        if self.running_jobs.contains_key(job_id) {
            return Err(JobError::AlreadyRunning);
        }

        self.spawn_job(job_id, "manual");
        Ok(())
    }

    /// Get the IDs of all registered jobs.
    pub fn registered_job_ids(&self) -> Vec<&str> {
        self.jobs.keys().map(|s| s.as_str()).collect()
    }

    /// Get a reference to a registered job by ID.
    pub fn get_job(&self, job_id: &str) -> Option<&Arc<dyn BackgroundJob>> {
        self.jobs.get(job_id)
    }

    /// Check if a job is currently running.
    pub fn is_job_running(&self, job_id: &str) -> bool {
        self.running_jobs.contains_key(job_id)
    }

    /// Main scheduler loop.
    pub async fn run(&mut self) {
        info!("Starting job scheduler with {} registered jobs", self.jobs.len());

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
        self.trigger_jobs_for_hook(HookEvent::OnStartup);

        loop {
            // Clean up completed job handles
            self.cleanup_completed_jobs().await;

            let sleep_duration = self.time_until_next_scheduled_job();
            debug!(
                "Scheduler sleeping for {:?} until next scheduled job",
                sleep_duration
            );

            tokio::select! {
                _ = tokio::time::sleep(sleep_duration) => {
                    self.run_due_jobs();
                }
                Some(event) = self.hook_receiver.recv() => {
                    debug!("Received hook event: {}", event);
                    self.trigger_jobs_for_hook(event);
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

    /// Calculate time until the next scheduled job should run.
    fn time_until_next_scheduled_job(&self) -> Duration {
        let mut min_duration = Duration::from_secs(60); // Default check interval

        for (job_id, job) in &self.jobs {
            if self.running_jobs.contains_key(job_id) {
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
            JobSchedule::Interval(interval) => {
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
                let interval_time = interval.and_then(|int| {
                    if let Ok(Some(state)) = self.server_store.get_schedule_state(job_id) {
                        Some(state.next_run_at)
                    } else {
                        Some(chrono::Utc::now() + chrono::Duration::from_std(int).ok()?)
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
    fn run_due_jobs(&mut self) {
        let now = chrono::Utc::now();
        let mut jobs_to_run = Vec::new();

        for (job_id, job) in &self.jobs {
            if self.running_jobs.contains_key(job_id) {
                continue;
            }

            if let Some(next_run) = self.get_next_run_time(job_id, job.schedule()) {
                if next_run <= now {
                    jobs_to_run.push(job_id.clone());
                }
            }
        }

        for job_id in jobs_to_run {
            self.spawn_job(&job_id, "schedule");
        }
    }

    /// Trigger all jobs that listen for a specific hook event.
    fn trigger_jobs_for_hook(&mut self, event: HookEvent) {
        let mut jobs_to_trigger = Vec::new();

        for (job_id, job) in &self.jobs {
            if self.running_jobs.contains_key(job_id) {
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

        for job_id in jobs_to_trigger {
            let trigger = format!("hook:{}", event);
            self.spawn_job(&job_id, &trigger);
        }
    }

    /// Spawn a job execution task.
    fn spawn_job(&mut self, job_id: &str, triggered_by: &str) {
        let job = match self.jobs.get(job_id) {
            Some(job) => Arc::clone(job),
            None => {
                error!("Attempted to spawn unknown job: {}", job_id);
                return;
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
        });

        self.running_jobs.insert(job_id.to_string(), handle);
    }

    /// Update schedule state after a job completes (for interval-based jobs).
    fn update_schedule_after_run(&self, job_id: &str) {
        let job = match self.jobs.get(job_id) {
            Some(job) => job,
            None => return,
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

        for (job_id, handle) in &self.running_jobs {
            if handle.is_finished() {
                completed.push(job_id.clone());
            }
        }

        for job_id in completed {
            if let Some(handle) = self.running_jobs.remove(&job_id) {
                let _ = handle.await;
            }
            self.job_cancel_tokens.remove(&job_id);
            self.update_schedule_after_run(&job_id);
        }
    }

    /// Gracefully shut down the scheduler.
    async fn shutdown(&mut self) {
        info!("Shutting down scheduler...");

        // Cancel cancellable jobs
        for (job_id, _) in &self.running_jobs {
            if let Some(job) = self.jobs.get(job_id) {
                if job.shutdown_behavior() == ShutdownBehavior::Cancellable {
                    if let Some(token) = self.job_cancel_tokens.get(job_id) {
                        debug!("Cancelling job: {}", job_id);
                        token.cancel();
                    }
                }
            }
        }

        // Wait for all jobs to complete
        let mut wait_jobs = Vec::new();
        for (job_id, handle) in self.running_jobs.drain() {
            let behavior = self.jobs.get(&job_id)
                .map(|j| j.shutdown_behavior())
                .unwrap_or(ShutdownBehavior::Cancellable);

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

    fn create_test_scheduler() -> (JobScheduler, TempDir, mpsc::Sender<HookEvent>) {
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

        let scheduler = JobScheduler::new(
            server_store,
            hook_receiver,
            shutdown_token,
            job_context,
        );

        (scheduler, temp_dir, hook_sender)
    }

    #[test]
    fn test_register_job() {
        let (mut scheduler, _temp_dir, _hook_sender) = create_test_scheduler();

        let exec_count = Arc::new(AtomicUsize::new(0));
        let job = Arc::new(TestJob {
            id: "test_job",
            execution_count: exec_count,
            should_fail: Arc::new(AtomicBool::new(false)),
        });

        scheduler.register_job(job);

        assert!(scheduler.get_job("test_job").is_some());
        assert!(scheduler.get_job("nonexistent").is_none());
    }

    #[test]
    fn test_trigger_nonexistent_job() {
        let (mut scheduler, _temp_dir, _hook_sender) = create_test_scheduler();

        let result = scheduler.trigger_job("nonexistent");
        assert!(matches!(result, Err(JobError::NotFound)));
    }

    #[tokio::test]
    async fn test_trigger_job_manually() {
        let (mut scheduler, _temp_dir, _hook_sender) = create_test_scheduler();

        let exec_count = Arc::new(AtomicUsize::new(0));
        let job = Arc::new(TestJob {
            id: "manual_test",
            execution_count: exec_count.clone(),
            should_fail: Arc::new(AtomicBool::new(false)),
        });

        scheduler.register_job(job);
        scheduler.trigger_job("manual_test").unwrap();

        // Wait for job to complete
        tokio::time::sleep(Duration::from_millis(100)).await;
        scheduler.cleanup_completed_jobs().await;

        assert_eq!(exec_count.load(Ordering::SeqCst), 1);
    }
}
