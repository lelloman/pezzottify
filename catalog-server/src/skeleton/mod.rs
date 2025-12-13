//! Skeleton event store for catalog sync.
//!
//! This module provides the skeleton event infrastructure for syncing
//! catalog structure (IDs and relationships) to client devices.

pub mod models;
pub mod schema;
pub mod store;

pub use models::*;
pub use store::SkeletonEventStore;
