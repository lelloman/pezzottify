//! Pezzottify Catalog Server Library
//!
//! This library exposes the internal modules for testing and potential reuse.

pub mod catalog;
pub mod search;
pub mod server;
pub mod sqlite_persistence;
pub mod user;

// Re-export commonly used types for convenience
pub use catalog::Catalog;
pub use search::{NoOpSearchVault, SearchVault};
pub use server::{run_server, RequestsLoggingLevel};
pub use user::{SqliteUserStore, UserRole, UserStore};

// Re-export for testing
#[cfg(feature = "no_search")]
pub use search::NoOpSearchVault as DefaultSearchVault;

#[cfg(not(feature = "no_search"))]
pub use search::PezzotHashSearchVault as DefaultSearchVault;
