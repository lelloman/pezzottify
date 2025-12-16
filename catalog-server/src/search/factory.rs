//! Factory function for creating search vault instances

use super::{
    Fts5LevenshteinSearchVault, Fts5SearchVault, NoOpSearchVault, PezzotHashSearchVault,
    SearchVault,
};
use crate::catalog_store::CatalogStore;
use crate::config::SearchEngine;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

/// Create a search vault based on the configured search engine
///
/// # Arguments
/// * `engine` - The search engine type to create
/// * `catalog_store` - The catalog store to index content from
/// * `db_dir` - Directory for database files (used by FTS5 variants)
///
/// # Returns
/// A boxed SearchVault implementation
pub fn create_search_vault(
    engine: &SearchEngine,
    catalog_store: Arc<dyn CatalogStore>,
    db_dir: &Path,
) -> Result<Box<dyn SearchVault>> {
    match engine {
        SearchEngine::PezzotHash => {
            info!("Creating PezzotHash search vault");
            Ok(Box::new(PezzotHashSearchVault::new(catalog_store)))
        }
        SearchEngine::Fts5 => {
            let db_path = db_dir.join("search.db");
            info!("Creating FTS5 search vault at {:?}", db_path);
            Ok(Box::new(Fts5SearchVault::new(catalog_store, &db_path)?))
        }
        SearchEngine::Fts5Levenshtein => {
            let db_path = db_dir.join("search.db");
            info!(
                "Creating FTS5+Levenshtein search vault at {:?} (typo-tolerant)",
                db_path
            );
            Ok(Box::new(Fts5LevenshteinSearchVault::new(
                catalog_store,
                &db_path,
            )?))
        }
        SearchEngine::NoOp => {
            info!("Creating NoOp search vault (search disabled)");
            Ok(Box::new(NoOpSearchVault {}))
        }
    }
}
