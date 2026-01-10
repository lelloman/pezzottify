//! Pezzottify Catalog Server Library
//!
//! This library exposes the internal modules for testing and potential reuse.

pub mod background_jobs;
pub mod catalog_store;
pub mod config;
// TODO: Re-enable after updating for Spotify schema
// pub mod download_manager;
// pub mod downloader;
pub mod mcp;
pub mod notifications;
pub mod oidc;
pub mod search;
pub mod server;
pub mod server_store;
// Skeleton sync module removed - Android now uses on-demand discography API
// pub mod skeleton;
pub mod sqlite_persistence;
pub mod user;
// TODO: Re-enable after updating for Spotify schema (uses changelog)
// pub mod whatsnew;

// Re-export commonly used types for convenience
pub use search::{Fts5LevenshteinSearchVault, SearchVault};
pub use server::{run_server, RequestsLoggingLevel};
pub use server_store::{ServerStore, SqliteServerStore};
pub use user::{SqliteUserStore, UserRole, UserStore};
