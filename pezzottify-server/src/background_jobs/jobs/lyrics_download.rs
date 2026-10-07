use crate::background_jobs::{
    context::JobContext,
    job::{
        BackgroundJob, JobError, JobExecutionPolicy, JobResourceClass, JobSchedule,
        ShutdownBehavior,
    },
    JobAuditLogger,
};
use crate::config::LyricsJobSettings;
use crate::db_executor::DbPriority;
use crate::lyrics::LyricsFetcher;
use std::time::Duration;

pub struct LyricsDownloadJob {
    settings: LyricsJobSettings,
}
impl LyricsDownloadJob {
    pub fn new(settings: LyricsJobSettings) -> Self {
        Self { settings }
    }
}
impl BackgroundJob for LyricsDownloadJob {
    fn id(&self) -> &'static str {
        "lyrics_download"
    }
    fn name(&self) -> &'static str {
        "Lyrics download"
    }
    fn description(&self) -> &'static str {
        "Fetch lyrics for the most popular available tracks without lyrics"
    }
    fn schedule(&self) -> JobSchedule {
        JobSchedule::Interval(Duration::from_secs(24 * 60 * 60))
    }
    fn run_on_startup(&self) -> bool {
        false
    }
    fn shutdown_behavior(&self) -> ShutdownBehavior {
        ShutdownBehavior::Cancellable
    }
    fn execution_policy(&self) -> JobExecutionPolicy {
        JobExecutionPolicy::new(JobResourceClass::IoBound)
            .with_queue_timeout(Duration::from_secs(60))
            .with_max_runtime(Duration::from_secs(6 * 60 * 60))
            .with_circuit_breaker(3, Duration::from_secs(60 * 60))
    }
    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(ctx.server_db.clone(), self.id());
        let limit = self.settings.batch_size;
        audit.log_started(Some(serde_json::json!({"batch_size": limit})));
        let result = (|| -> anyhow::Result<_> {
            let ids = ctx
                .catalog_db
                .run_blocking(DbPriority::Background, move |store| {
                    store.lyrics_candidates(limit, chrono::Utc::now().timestamp())
                })?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async {
                let fetcher = LyricsFetcher::new()?;
                tokio::select! {
                    _ = ctx.cancellation_token.cancelled() => Err(anyhow::anyhow!("Cancelled")),
                    result = fetcher.download(&ctx.catalog_db, ids, DbPriority::Background, false) => result,
                }
            })
        })();
        match result {
            Ok(summary) => {
                audit.log_completed(Some(serde_json::to_value(summary).unwrap()));
                Ok(())
            }
            Err(error) => {
                audit.log_failed(&error.to_string(), None);
                if ctx.is_cancelled() {
                    Err(JobError::Cancelled)
                } else {
                    Err(JobError::ExecutionFailed(error.to_string()))
                }
            }
        }
    }
}
