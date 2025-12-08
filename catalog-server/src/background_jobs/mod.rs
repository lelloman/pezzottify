//! Background job scheduling and execution system.
//!
//! This module provides infrastructure for running periodic and event-triggered
//! background tasks like search reindexing, cache precomputation, and event log pruning.

mod context;
mod job;
pub mod jobs;
mod scheduler;

pub use context::JobContext;
pub use job::{BackgroundJob, HookEvent, JobError, JobSchedule, ShutdownBehavior};
pub use scheduler::JobScheduler;
