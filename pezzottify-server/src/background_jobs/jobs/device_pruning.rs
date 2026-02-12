//! Device pruning background job.
//!
//! Periodically removes all inactive devices (regardless of user association)
//! whose `last_seen` timestamp is older than a configurable retention period.

use crate::background_jobs::{
    context::JobContext,
    job::{BackgroundJob, JobError, JobSchedule, ShutdownBehavior},
    JobAuditLogger,
};
use crate::config::DevicePruningJobSettings;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

/// Background job that prunes inactive devices.
pub struct DevicePruningJob {
    interval_hours: u64,
    retention_days: u64,
}

impl DevicePruningJob {
    pub fn from_settings(settings: &DevicePruningJobSettings) -> Self {
        Self {
            interval_hours: settings.interval_hours,
            retention_days: settings.retention_days,
        }
    }
}

impl BackgroundJob for DevicePruningJob {
    fn id(&self) -> &'static str {
        "device_pruning"
    }

    fn name(&self) -> &'static str {
        "Device Pruning"
    }

    fn description(&self) -> &'static str {
        "Prune inactive devices that haven't been seen within the retention period"
    }

    fn schedule(&self) -> JobSchedule {
        JobSchedule::Interval(Duration::from_secs(self.interval_hours * 60 * 60))
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(Arc::clone(&ctx.server_store), self.id());

        if ctx.is_cancelled() {
            return Err(JobError::Cancelled);
        }

        audit.log_started(Some(serde_json::json!({
            "retention_days": self.retention_days,
        })));

        match ctx
            .user_store
            .prune_inactive_devices(self.retention_days as u32)
        {
            Ok(deleted) => {
                if deleted > 0 {
                    info!("Pruned {} inactive devices", deleted);
                }
                audit.log_completed(Some(serde_json::json!({
                    "devices_deleted": deleted,
                })));
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to prune inactive devices: {}", e);
                audit.log_failed(&error_msg, None);
                Err(JobError::ExecutionFailed(error_msg))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_metadata() {
        let settings = DevicePruningJobSettings {
            interval_hours: 12,
            retention_days: 60,
        };
        let job = DevicePruningJob::from_settings(&settings);

        assert_eq!(job.id(), "device_pruning");
        assert_eq!(job.name(), "Device Pruning");
        assert!(!job.description().is_empty());
        assert_eq!(job.shutdown_behavior(), ShutdownBehavior::Cancellable);
    }

    #[test]
    fn test_job_schedule() {
        let settings = DevicePruningJobSettings {
            interval_hours: 24,
            retention_days: 90,
        };
        let job = DevicePruningJob::from_settings(&settings);

        match job.schedule() {
            JobSchedule::Interval(duration) => {
                assert_eq!(duration, Duration::from_secs(24 * 60 * 60));
            }
            _ => panic!("Expected Interval schedule"),
        }
    }

    #[test]
    fn test_default_settings() {
        let settings = DevicePruningJobSettings::default();
        let job = DevicePruningJob::from_settings(&settings);

        assert_eq!(job.interval_hours, 24);
        assert_eq!(job.retention_days, 90);
    }
}
