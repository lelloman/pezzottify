//! Specific background job implementations.
//!
//! This module contains implementations of the `BackgroundJob` trait
//! for various server maintenance and processing tasks.

pub mod integrity_watchdog;
pub mod popular_content;

pub use integrity_watchdog::IntegrityWatchdogJob;
pub use popular_content::PopularContentJob;
