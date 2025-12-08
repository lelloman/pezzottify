//! Specific background job implementations.
//!
//! This module contains implementations of the `BackgroundJob` trait
//! for various server maintenance and processing tasks.

pub mod popular_content;

pub use popular_content::PopularContentJob;

// Future job implementations will be added here:
// pub mod event_pruning;
// pub mod search_reindex;
// pub mod catalog_backup;
