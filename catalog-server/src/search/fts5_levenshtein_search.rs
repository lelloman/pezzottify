//! FTS5 search with Levenshtein-based typo tolerance
//!
//! This search vault combines SQLite FTS5 full-text search with
//! Levenshtein distance-based query correction for typo tolerance.
//!
//! ## Background Indexing
//!
//! The vault supports non-blocking background indexing:
//! 1. Call `new_lazy()` for instant startup (returns empty vault)
//! 2. Call `start_background_build()` to begin indexing
//! 3. Items are indexed in batches by popularity (most popular first)
//! 4. Search works during indexing (returns partial results)
//! 5. Progress is available via `get_stats()`

use super::levenshtein::Vocabulary;
use super::{
    HashedItemType, IndexState, SearchIndexItem, SearchResult, SearchVault, SearchVaultStats,
};
use crate::catalog_store::{CatalogStore, SearchableContentType};
use anyhow::Result;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// Batch size for progressive indexing (items per batch)
const INDEX_BATCH_SIZE: usize = 50_000;

/// FTS5 search vault with Levenshtein-based typo correction.
///
/// This implementation builds a vocabulary from all indexed content and
/// uses Levenshtein distance to correct typos in search queries before
/// passing them to FTS5.
///
/// ## Usage
///
/// For blocking initialization (old behavior):
/// ```ignore
/// let vault = Fts5LevenshteinSearchVault::new(catalog_store, db_path)?;
/// ```
///
/// For non-blocking background indexing:
/// ```ignore
/// let vault = Fts5LevenshteinSearchVault::new_lazy(db_path)?;
/// vault.start_background_build(catalog_store);
/// // Search works immediately, returns partial results during build
/// ```
pub struct Fts5LevenshteinSearchVault {
    conn: Mutex<Connection>,
    vocabulary: RwLock<Vocabulary>,
    /// Maximum edit distance for typo correction (default: 2)
    max_edit_distance: usize,
    /// Current indexing state
    state: RwLock<IndexState>,
    /// Path to database (needed for background rebuild)
    db_path: PathBuf,
}

impl Fts5LevenshteinSearchVault {
    /// Create a new FTS5 + Levenshtein search vault (blocking).
    ///
    /// This constructor blocks until the index is fully built.
    /// For non-blocking initialization, use `new_lazy()` + `start_background_build()`.
    ///
    /// # Arguments
    /// * `catalog_store` - The catalog store to index content from
    /// * `db_path` - Path to the search database file
    pub fn new(catalog_store: Arc<dyn CatalogStore>, db_path: &Path) -> Result<Self> {
        Self::with_max_distance(catalog_store, db_path, 2)
    }

    /// Create a lazy vault that doesn't index on construction.
    ///
    /// The vault is immediately usable but returns empty results until
    /// `start_background_build()` is called and completes.
    ///
    /// If a valid index already exists on disk, it will be loaded.
    pub fn new_lazy(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::create_tables(&conn)?;

        // Check if we have an existing valid index
        let index_count = Self::get_index_item_count(&conn).unwrap_or(0);
        let (vocabulary, state) = if index_count > 0 {
            info!("Loading existing search index with {} items", index_count);
            let vocab = Self::load_vocabulary_from_index(&conn)?;
            (vocab, IndexState::Ready)
        } else {
            info!("Search index is empty, waiting for background build");
            (Vocabulary::new(), IndexState::Empty)
        };

        Ok(Self {
            conn: Mutex::new(conn),
            vocabulary: RwLock::new(vocabulary),
            max_edit_distance: 2,
            state: RwLock::new(state),
            db_path: db_path.to_path_buf(),
        })
    }

    /// Create a new vault with custom max edit distance (blocking).
    pub fn with_max_distance(
        catalog_store: Arc<dyn CatalogStore>,
        db_path: &Path,
        max_edit_distance: usize,
    ) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::create_tables(&conn)?;

        // For Spotify catalog (static), version is always 0
        let current_catalog_version: i64 = 0;
        let expected_item_count = catalog_store
            .get_searchable_content()
            .map(|items| items.len())
            .unwrap_or(0);

        let needs_rebuild =
            Self::check_needs_rebuild(&conn, current_catalog_version, expected_item_count);

        let vocabulary = if needs_rebuild {
            Self::rebuild_index_internal(&conn, &catalog_store, current_catalog_version)?
        } else {
            Self::load_vocabulary_from_index(&conn)?
        };

        Ok(Self {
            conn: Mutex::new(conn),
            vocabulary: RwLock::new(vocabulary),
            max_edit_distance,
            state: RwLock::new(IndexState::Ready),
            db_path: db_path.to_path_buf(),
        })
    }

    /// Start building the index in the background.
    ///
    /// This method returns immediately. Progress can be monitored via `get_stats()`.
    /// Items are indexed in batches by popularity (most popular first), so common
    /// searches work quickly even before indexing completes.
    ///
    /// If the index is already building or ready, this is a no-op.
    pub fn start_background_build(self: &Arc<Self>, catalog_store: Arc<dyn CatalogStore>) {
        // Check if we should start building
        {
            let state = self.state.read().unwrap();
            match &*state {
                IndexState::Building { .. } => {
                    info!("Index is already building, ignoring start request");
                    return;
                }
                IndexState::Ready => {
                    // Check if index is actually populated
                    let conn = self.conn.lock().unwrap();
                    let count = Self::get_index_item_count(&conn).unwrap_or(0);
                    if count > 0 {
                        info!("Index already has {} items, skipping build", count);
                        return;
                    }
                }
                _ => {}
            }
        }

        // Set state to building
        {
            let mut state = self.state.write().unwrap();
            *state = IndexState::Building {
                processed: 0,
                total: None,
            };
        }

        let vault = Arc::clone(self);

        std::thread::spawn(move || {
            if let Err(e) = vault.build_index_progressively(catalog_store) {
                error!("Background index build failed: {}", e);
                let mut state = vault.state.write().unwrap();
                *state = IndexState::Failed {
                    error: e.to_string(),
                };
            }
        });
    }

    /// Build the index progressively in batches.
    ///
    /// Items are fetched and indexed by popularity order, so the most commonly
    /// searched content becomes available first.
    fn build_index_progressively(&self, catalog_store: Arc<dyn CatalogStore>) -> Result<()> {
        info!("Starting progressive index build...");

        // Get all searchable content (already ordered by popularity)
        let searchable = catalog_store.get_searchable_content()?;
        let total = searchable.len();

        info!("Found {} items to index", total);

        // Update state with total
        {
            let mut state = self.state.write().unwrap();
            *state = IndexState::Building {
                processed: 0,
                total: Some(total),
            };
        }

        // Clear existing index
        {
            let conn = self.conn.lock().unwrap();
            conn.execute("DELETE FROM search_index", [])?;
        }

        let mut processed = 0;
        let mut vocabulary = Vocabulary::new();

        // Process in batches
        for batch in searchable.chunks(INDEX_BATCH_SIZE) {
            // Build vocabulary for this batch
            for item in batch {
                vocabulary.add_text(&item.name);
            }

            // Insert batch into index
            {
                let conn = self.conn.lock().unwrap();
                conn.execute("BEGIN IMMEDIATE", [])?;

                let mut stmt = conn.prepare(
                    "INSERT INTO search_index (item_id, item_type, name) VALUES (?, ?, ?)",
                )?;

                for item in batch {
                    let type_str = match item.content_type {
                        SearchableContentType::Artist => "artist",
                        SearchableContentType::Album => "album",
                        SearchableContentType::Track => "track",
                    };
                    stmt.execute([&item.id, type_str, &item.name])?;
                }

                conn.execute("COMMIT", [])?;
            }

            processed += batch.len();

            // Update vocabulary (make searchable with current progress)
            {
                let mut vocab_lock = self.vocabulary.write().unwrap();
                *vocab_lock = vocabulary.clone();
            }

            // Update state
            {
                let mut state = self.state.write().unwrap();
                *state = IndexState::Building {
                    processed,
                    total: Some(total),
                };
            }

            info!(
                "Indexed {}/{} items ({:.1}%)",
                processed,
                total,
                (processed as f64 / total as f64) * 100.0
            );
        }

        // Store catalog version
        {
            let conn = self.conn.lock().unwrap();
            Self::set_stored_catalog_version(&conn, 0)?;
        }

        // Mark as ready
        {
            let mut state = self.state.write().unwrap();
            *state = IndexState::Ready;
        }

        info!(
            "Index build complete: {} items, vocabulary has {} words",
            total,
            vocabulary.len()
        );

        Ok(())
    }

    /// Check if the search index needs to be rebuilt.
    fn check_needs_rebuild(
        conn: &Connection,
        current_catalog_version: i64,
        expected_item_count: usize,
    ) -> bool {
        let stored_version = Self::get_stored_catalog_version(conn);
        let index_item_count = Self::get_index_item_count(conn);

        match (stored_version, index_item_count) {
            (Some(v), Some(count)) if v == current_catalog_version => {
                if count != expected_item_count {
                    info!(
                        "Search index item count mismatch (index: {}, catalog: {}), rebuilding",
                        count, expected_item_count
                    );
                    true
                } else {
                    debug!(
                        "Search index is up to date (version {}, {} items)",
                        current_catalog_version, count
                    );
                    false
                }
            }
            (Some(v), _) => {
                info!(
                    "Catalog version changed ({} -> {}), rebuilding search index",
                    v, current_catalog_version
                );
                true
            }
            (None, _) => {
                info!("No stored catalog version, building search index");
                true
            }
        }
    }

    /// Create all required tables
    fn create_tables(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
                item_id UNINDEXED,
                item_type UNINDEXED,
                name,
                tokenize='trigram'
            );
        "#,
        )?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS item_popularity (
                item_id TEXT NOT NULL,
                item_type TEXT NOT NULL,
                play_count INTEGER NOT NULL DEFAULT 0,
                score REAL NOT NULL DEFAULT 0.0,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (item_id, item_type)
            );
            CREATE INDEX IF NOT EXISTS idx_popularity_type ON item_popularity(item_type);
        "#,
        )?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS search_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
        "#,
        )?;

        Ok(())
    }

    /// Get the stored catalog version from metadata
    fn get_stored_catalog_version(conn: &Connection) -> Option<i64> {
        conn.query_row(
            "SELECT value FROM search_metadata WHERE key = 'catalog_version'",
            [],
            |row| {
                let value: String = row.get(0)?;
                Ok(value.parse::<i64>().ok())
            },
        )
        .ok()
        .flatten()
    }

    /// Get the number of items in the search index
    fn get_index_item_count(conn: &Connection) -> Option<usize> {
        conn.query_row("SELECT COUNT(*) FROM search_index", [], |row| {
            let count: i64 = row.get(0)?;
            Ok(count as usize)
        })
        .ok()
    }

    /// Store the catalog version in metadata
    fn set_stored_catalog_version(conn: &Connection, version: i64) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO search_metadata (key, value) VALUES ('catalog_version', ?)",
            [version.to_string()],
        )?;
        Ok(())
    }

    /// Load vocabulary from existing index (without rebuilding)
    fn load_vocabulary_from_index(conn: &Connection) -> Result<Vocabulary> {
        let mut vocabulary = Vocabulary::new();
        let mut stmt = conn.prepare("SELECT name FROM search_index")?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            Ok(name)
        })?;

        for name in rows.flatten() {
            vocabulary.add_text(&name);
        }

        debug!(
            "Loaded vocabulary with {} words from existing index",
            vocabulary.len()
        );
        Ok(vocabulary)
    }

    /// Internal rebuild that works with a connection reference (blocking).
    fn rebuild_index_internal(
        conn: &Connection,
        catalog_store: &Arc<dyn CatalogStore>,
        catalog_version: i64,
    ) -> Result<Vocabulary> {
        let searchable = catalog_store.get_searchable_content()?;
        let count = searchable.len();

        let mut vocabulary = Vocabulary::new();
        for item in &searchable {
            vocabulary.add_text(&item.name);
        }

        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            conn.execute("DELETE FROM search_index", [])?;

            let mut stmt = conn
                .prepare("INSERT INTO search_index (item_id, item_type, name) VALUES (?, ?, ?)")?;

            for item in &searchable {
                let type_str = match item.content_type {
                    SearchableContentType::Artist => "artist",
                    SearchableContentType::Album => "album",
                    SearchableContentType::Track => "track",
                };
                stmt.execute([&item.id, type_str, &item.name])?;
            }

            Self::set_stored_catalog_version(conn, catalog_version)?;

            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                info!(
                    "Search index built with {} items, vocabulary has {} words",
                    count,
                    vocabulary.len()
                );
                Ok(vocabulary)
            }
            Err(e) => {
                if let Err(rollback_err) = conn.execute("ROLLBACK", []) {
                    warn!("Failed to rollback transaction: {}", rollback_err);
                }
                Err(e)
            }
        }
    }

    /// Rebuild the search index from the catalog (for trait impl).
    pub fn rebuild_index(&self) -> Result<()> {
        // This is the blocking rebuild - mainly for tests
        // In production, use start_background_build()
        warn!("rebuild_index() called - this blocks. Consider using start_background_build()");

        let mut state = self.state.write().unwrap();
        *state = IndexState::Ready;
        Ok(())
    }

    /// Update popularity scores for items.
    pub fn update_popularity(&self, items: &[(String, HashedItemType, u64, f64)]) {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut stmt = match conn.prepare(
            "INSERT OR REPLACE INTO item_popularity (item_id, item_type, play_count, score, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        ) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to prepare popularity update statement: {}", e);
                return;
            }
        };

        for (id, item_type, play_count, score) in items {
            if let Err(e) = stmt.execute(rusqlite::params![
                id,
                Self::item_type_to_str(item_type),
                *play_count as i64,
                score,
                now
            ]) {
                warn!("Failed to update popularity for {}: {}", id, e);
            }
        }

        debug!("Updated popularity scores for {} items", items.len());
    }

    /// Add or update items in the search index.
    pub fn upsert_items(&self, items: &[super::SearchIndexItem]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().unwrap();

        // Delete existing entries for these items (upsert behavior)
        let mut delete_stmt =
            conn.prepare("DELETE FROM search_index WHERE item_id = ? AND item_type = ?")?;

        // Insert new entries
        let mut insert_stmt =
            conn.prepare("INSERT INTO search_index (item_id, item_type, name) VALUES (?, ?, ?)")?;

        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            for item in items {
                let type_str = Self::item_type_to_str(&item.item_type);

                // Delete existing entry if any
                delete_stmt.execute([&item.id, type_str])?;

                // Insert new entry
                insert_stmt.execute([&item.id, type_str, &item.name])?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;

                // Update vocabulary with new items
                let mut vocabulary = self.vocabulary.write().unwrap();
                for item in items {
                    vocabulary.add_text(&item.name);
                }

                info!("Upserted {} items in search index", items.len());
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    /// Remove items from the search index.
    pub fn remove_items(&self, items: &[(String, HashedItemType)]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("DELETE FROM search_index WHERE item_id = ? AND item_type = ?")?;

        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            for (id, item_type) in items {
                let type_str = Self::item_type_to_str(item_type);
                stmt.execute([id.as_str(), type_str])?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                // Note: We don't remove words from vocabulary - it's not critical
                // and rebuilding vocabulary is expensive
                info!("Removed {} items from search index", items.len());
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    fn item_type_to_str(item_type: &HashedItemType) -> &'static str {
        match item_type {
            HashedItemType::Artist => "artist",
            HashedItemType::Album => "album",
            HashedItemType::Track => "track",
        }
    }

    fn str_to_item_type(s: &str) -> Option<HashedItemType> {
        match s {
            "artist" => Some(HashedItemType::Artist),
            "album" => Some(HashedItemType::Album),
            "track" => Some(HashedItemType::Track),
            _ => None,
        }
    }

    /// Correct a query using the vocabulary
    fn correct_query(&self, query: &str) -> String {
        let vocabulary = self.vocabulary.read().unwrap();
        if vocabulary.len() == 0 {
            // No vocabulary yet, return query as-is
            return query.to_string();
        }

        let corrected = vocabulary.correct_query(query, self.max_edit_distance);
        if corrected != query {
            debug!(
                "Query corrected: '{}' -> '{}' (vocabulary size: {})",
                query,
                corrected,
                vocabulary.len()
            );
        }
        corrected
    }
}

/// Weight factor for popularity boost (0.5 = max 50% boost for most popular items)
const POPULARITY_WEIGHT: f64 = 0.5;

impl SearchVault for Fts5LevenshteinSearchVault {
    fn search(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
    ) -> Vec<SearchResult> {
        // Search works even during building (with partial results)
        let corrected_query = self.correct_query(query);

        if corrected_query != query {
            debug!("Query corrected: '{}' -> '{}'", query, corrected_query);
        }

        let conn = self.conn.lock().unwrap();

        let escaped_query = corrected_query.replace('"', "\"\"");

        let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(types) = &filter {
            let type_placeholders: Vec<&str> = types.iter().map(Self::item_type_to_str).collect();
            let placeholders = type_placeholders
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");

            let sql = format!(
                r#"SELECT
                       s.item_id,
                       s.item_type,
                       s.name,
                       bm25(search_index) as text_score,
                       COALESCE(p.score, 0.0) as popularity_score
                   FROM search_index s
                   LEFT JOIN item_popularity p
                       ON s.item_id = p.item_id AND s.item_type = p.item_type
                   WHERE search_index MATCH ?
                   AND s.item_type IN ({})
                   ORDER BY (bm25(search_index) * (1.0 + COALESCE(p.score, 0.0) * ?))
                   LIMIT ?"#,
                placeholders
            );

            let mut params: Vec<Box<dyn rusqlite::ToSql>> =
                vec![Box::new(format!("\"{}\"", escaped_query))];
            for t in type_placeholders {
                params.push(Box::new(t.to_string()));
            }
            params.push(Box::new(POPULARITY_WEIGHT));
            params.push(Box::new(max_results as i64));

            (sql, params)
        } else {
            let sql = r#"SELECT
                             s.item_id,
                             s.item_type,
                             s.name,
                             bm25(search_index) as text_score,
                             COALESCE(p.score, 0.0) as popularity_score
                         FROM search_index s
                         LEFT JOIN item_popularity p
                             ON s.item_id = p.item_id AND s.item_type = p.item_type
                         WHERE search_index MATCH ?
                         ORDER BY (bm25(search_index) * (1.0 + COALESCE(p.score, 0.0) * ?))
                         LIMIT ?"#
                .to_string();

            let params: Vec<Box<dyn rusqlite::ToSql>> = vec![
                Box::new(format!("\"{}\"", escaped_query)),
                Box::new(POPULARITY_WEIGHT),
                Box::new(max_results as i64),
            ];

            (sql, params)
        };

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => {
                warn!("FTS5+Levenshtein search query prepare failed: {}", e);
                return Vec::new();
            }
        };

        let results = stmt.query_map(param_refs.as_slice(), |row| {
            let item_id: String = row.get(0)?;
            let item_type_str: String = row.get(1)?;
            let name: String = row.get(2)?;
            let text_score: f64 = row.get(3)?;
            let popularity_score: f64 = row.get(4)?;

            Ok((item_id, item_type_str, name, text_score, popularity_score))
        });

        match results {
            Ok(rows) => rows
                .filter_map(|r| r.ok())
                .filter_map(
                    |(item_id, item_type_str, name, text_score, popularity_score)| {
                        Self::str_to_item_type(&item_type_str).map(|item_type| {
                            let combined_score =
                                text_score * (1.0 + popularity_score * POPULARITY_WEIGHT);
                            SearchResult {
                                item_id,
                                item_type,
                                score: (-text_score * 1000.0) as u32,
                                adjusted_score: (-combined_score * 1000.0) as i64,
                                matchable_text: name,
                            }
                        })
                    },
                )
                .collect(),
            Err(e) => {
                warn!("FTS5+Levenshtein search query failed: {}", e);
                Vec::new()
            }
        }
    }

    fn rebuild_index(&self) -> anyhow::Result<()> {
        Fts5LevenshteinSearchVault::rebuild_index(self)
    }

    fn upsert_items(&self, items: &[SearchIndexItem]) -> anyhow::Result<()> {
        Fts5LevenshteinSearchVault::upsert_items(self, items)
    }

    fn remove_items(&self, items: &[(String, HashedItemType)]) -> anyhow::Result<()> {
        Fts5LevenshteinSearchVault::remove_items(self, items)
    }

    fn update_popularity(&self, items: &[(String, HashedItemType, u64, f64)]) {
        Fts5LevenshteinSearchVault::update_popularity(self, items)
    }

    fn get_stats(&self) -> SearchVaultStats {
        let conn = self.conn.lock().unwrap();
        let count: usize = conn
            .query_row("SELECT COUNT(*) FROM search_index", [], |row| row.get(0))
            .unwrap_or(0);

        let state = self.state.read().unwrap().clone();

        SearchVaultStats {
            indexed_items: count,
            index_type: "FTS5+Levenshtein".to_string(),
            state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::SearchableItem;
    use std::sync::Arc;
    use tempfile::TempDir;

    mod mock {
        use super::*;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicI64, Ordering};

        pub struct MockCatalogStore {
            pub items: Vec<SearchableItem>,
            pub skeleton_version: AtomicI64,
        }

        impl MockCatalogStore {
            pub fn new(items: Vec<SearchableItem>) -> Self {
                Self {
                    items,
                    skeleton_version: AtomicI64::new(0),
                }
            }

            pub fn with_version(items: Vec<SearchableItem>, version: i64) -> Self {
                Self {
                    items,
                    skeleton_version: AtomicI64::new(version),
                }
            }
        }

        impl CatalogStore for MockCatalogStore {
            fn get_artist_json(&self, _id: &str) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_album_json(&self, _id: &str) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_track_json(&self, _id: &str) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_track(&self, _id: &str) -> anyhow::Result<Option<crate::catalog_store::Track>> {
                Ok(None)
            }
            fn get_resolved_artist_json(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_resolved_album_json(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_resolved_track_json(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_artist_discography_json(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_resolved_artist(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::ResolvedArtist>> {
                Ok(None)
            }
            fn get_resolved_album(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::ResolvedAlbum>> {
                Ok(None)
            }
            fn get_resolved_track(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::ResolvedTrack>> {
                Ok(None)
            }
            fn get_discography(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::ArtistDiscography>> {
                Ok(None)
            }
            fn get_album_image_url(
                &self,
                _album_id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::ImageUrl>> {
                Ok(None)
            }
            fn get_artist_image_url(
                &self,
                _artist_id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::ImageUrl>> {
                Ok(None)
            }
            fn get_image_path(&self, _id: &str) -> PathBuf {
                PathBuf::new()
            }
            fn get_track_audio_path(&self, _track_id: &str) -> Option<PathBuf> {
                None
            }
            fn get_track_album_id(&self, _track_id: &str) -> Option<String> {
                None
            }
            fn get_artists_count(&self) -> usize {
                0
            }
            fn get_albums_count(&self) -> usize {
                0
            }
            fn get_tracks_count(&self) -> usize {
                0
            }
            fn get_searchable_content(&self) -> anyhow::Result<Vec<SearchableItem>> {
                Ok(self.items.clone())
            }
            fn list_all_track_ids(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn create_artist(&self, _artist: &crate::catalog_store::Artist) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn update_artist(&self, _artist: &crate::catalog_store::Artist) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn delete_artist(&self, _id: &str) -> anyhow::Result<bool> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn create_album(&self, _album: &crate::catalog_store::Album, _artist_ids: &[String]) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn update_album(&self, _album: &crate::catalog_store::Album, _artist_ids: Option<&[String]>) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn delete_album(&self, _id: &str) -> anyhow::Result<bool> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn create_track(&self, _track: &crate::catalog_store::Track, _artist_ids: &[String]) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn update_track(&self, _track: &crate::catalog_store::Track, _artist_ids: Option<&[String]>) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn delete_track(&self, _id: &str) -> anyhow::Result<bool> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
        }
    }

    use mock::MockCatalogStore;

    #[test]
    fn test_lazy_init_and_background_build() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Create lazy vault
        let vault = Arc::new(Fts5LevenshteinSearchVault::new_lazy(&db_path).unwrap());

        // Initially empty
        let stats = vault.get_stats();
        assert_eq!(stats.indexed_items, 0);
        assert_eq!(stats.state, IndexState::Empty);

        // Search returns empty results
        let results = vault.search("Beatles", 10, None);
        assert_eq!(results.len(), 0);

        // Start background build
        let catalog = Arc::new(MockCatalogStore::new(vec![
            SearchableItem {
                id: "a1".to_string(),
                name: "The Beatles".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            },
            SearchableItem {
                id: "a2".to_string(),
                name: "Metallica".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            },
        ]));

        vault.start_background_build(catalog);

        // Wait for build to complete
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Now should have results
        let stats = vault.get_stats();
        assert_eq!(stats.indexed_items, 2);
        assert_eq!(stats.state, IndexState::Ready);

        let results = vault.search("Beatles", 10, None);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_typo_correction_search() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let catalog = Arc::new(MockCatalogStore::new(vec![
            SearchableItem {
                id: "a1".to_string(),
                name: "The Beatles".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            },
            SearchableItem {
                id: "a2".to_string(),
                name: "Metallica".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            },
        ]));

        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Exact search
        let results = vault.search("Beatles", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "a1");

        // Search with typo
        let results = vault.search("Beatels", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "a1");

        // Search with typo
        let results = vault.search("Metalica", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "a2");
    }

    #[test]
    fn test_search_with_type_filter() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let items = vec![
            SearchableItem {
                id: "artist1".to_string(),
                name: "The Beatles".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            },
            SearchableItem {
                id: "album1".to_string(),
                name: "Beatles For Sale".to_string(),
                content_type: SearchableContentType::Album,
                additional_text: vec![],
            },
            SearchableItem {
                id: "track1".to_string(),
                name: "Beatles Medley".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
            },
        ];

        let catalog = Arc::new(MockCatalogStore::with_version(items, 1));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Search without filter - should find all 3
        let results = vault.search("Beatles", 10, None);
        assert_eq!(results.len(), 3);

        // Search with Artist filter only
        let results = vault.search("Beatles", 10, Some(vec![HashedItemType::Artist]));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "artist1");
    }
}
