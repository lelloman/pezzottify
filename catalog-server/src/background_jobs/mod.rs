//! Background job scheduling and execution system.
//!
//! This module provides infrastructure for running periodic and event-triggered
//! background tasks like search reindexing, cache precomputation, and event log pruning.

mod audit_logger;
mod context;
mod handle;
mod job;
pub mod jobs;
mod scheduler;

pub use audit_logger::JobAuditLogger;
pub use context::{GuardedSearchVault, JobContext};
pub use handle::{JobInfo, SchedulerHandle};
pub use job::{BackgroundJob, HookEvent, JobError, JobSchedule, ShutdownBehavior};
pub use scheduler::{create_scheduler, JobScheduler};
