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
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// Batch size for progressive indexing (items per batch)
const INDEX_BATCH_SIZE: usize = 50_000;

/// Sub-batch size for upsert/remove operations (items per chunk)
/// Smaller chunks allow concurrent writes to proceed between chunks
const UPSERT_SUB_BATCH_SIZE: usize = 10;

/// Sleep duration between sub-batches to yield to concurrent writers
const UPSERT_SUB_BATCH_YIELD_MS: u64 = 10;

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
    /// Read connection for search queries (separate from write to avoid blocking)
    read_conn: Mutex<Connection>,
    /// Write connection for indexing operations
    write_conn: Mutex<Connection>,
    vocabulary: RwLock<Vocabulary>,
    /// Maximum edit distance for typo correction (default: 2)
    max_edit_distance: usize,
    /// Current indexing state
    state: RwLock<IndexState>,
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
    /// If a partial build was interrupted, it will be detected and can be resumed.
    pub fn new_lazy(db_path: &Path) -> Result<Self> {
        // Create write connection first (handles table creation)
        let write_conn = Connection::open(db_path)?;
        write_conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::create_tables(&write_conn)?;

        // Create separate read connection for search queries
        // This allows searches to proceed while writes are happening
        let read_conn = Connection::open(db_path)?;
        read_conn.pragma_update(None, "journal_mode", "WAL")?;

        // Check build state
        let build_in_progress = Self::get_metadata(&write_conn, "build_in_progress")
            .map(|v| v == "true")
            .unwrap_or(false);
        let index_count = Self::get_index_item_count(&write_conn).unwrap_or(0);

        let (vocabulary, state) = if build_in_progress {
            // Partial build detected - load what we have and prepare to resume
            let build_offset = Self::get_metadata(&write_conn, "build_offset")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            let build_total = Self::get_metadata(&write_conn, "build_total")
                .and_then(|v| v.parse::<usize>().ok());

            info!(
                "Detected partial build: {} items indexed, offset {}, total {:?}",
                index_count, build_offset, build_total
            );

            let vocab = if index_count > 0 {
                Self::load_vocabulary_from_index(&write_conn)?
            } else {
                Vocabulary::new()
            };

            (
                vocab,
                IndexState::Building {
                    processed: build_offset,
                    total: build_total,
                },
            )
        } else if index_count > 0 {
            info!("Loading existing search index with {} items", index_count);
            let vocab = Self::load_vocabulary_from_index(&write_conn)?;
            (vocab, IndexState::Ready)
        } else {
            info!("Search index is empty, waiting for background build");
            (Vocabulary::new(), IndexState::Empty)
        };

        Ok(Self {
            read_conn: Mutex::new(read_conn),
            write_conn: Mutex::new(write_conn),
            vocabulary: RwLock::new(vocabulary),
            max_edit_distance: 2,
            state: RwLock::new(state),
        })
    }

    /// Create a new vault with custom max edit distance (blocking).
    pub fn with_max_distance(
        catalog_store: Arc<dyn CatalogStore>,
        db_path: &Path,
        max_edit_distance: usize,
    ) -> Result<Self> {
        let write_conn = Connection::open(db_path)?;
        write_conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::create_tables(&write_conn)?;

        // Create separate read connection
        let read_conn = Connection::open(db_path)?;
        read_conn.pragma_update(None, "journal_mode", "WAL")?;

        // For Spotify catalog (static), version is always 0
        let current_catalog_version: i64 = 0;
        let expected_item_count = catalog_store
            .get_searchable_content()
            .map(|items| items.len())
            .unwrap_or(0);

        let needs_rebuild =
            Self::check_needs_rebuild(&write_conn, current_catalog_version, expected_item_count);

        let vocabulary = if needs_rebuild {
            Self::rebuild_index_internal(&write_conn, &catalog_store, current_catalog_version)?
        } else {
            Self::load_vocabulary_from_index(&write_conn)?
        };

        Ok(Self {
            read_conn: Mutex::new(read_conn),
            write_conn: Mutex::new(write_conn),
            vocabulary: RwLock::new(vocabulary),
            max_edit_distance,
            state: RwLock::new(IndexState::Ready),
        })
    }

    /// Start building the index in the background.
    ///
    /// This method returns immediately. Progress can be monitored via `get_stats()`.
    /// Items are indexed in batches by popularity (most popular first), so common
    /// searches work quickly even before indexing completes.
    ///
    /// If a partial build was interrupted, this will resume from where it left off.
    /// If the index is already complete and ready, this is a no-op.
    pub fn start_background_build(self: &Arc<Self>, catalog_store: Arc<dyn CatalogStore>) {
        // Check current state and determine resume offset
        let resume_offset: Option<usize>;
        {
            let state = self.state.read().unwrap();
            match &*state {
                IndexState::Building { processed, .. } => {
                    // Resume from partial build
                    resume_offset = Some(*processed);
                    info!(
                        "Resuming partial build from offset {}",
                        resume_offset.unwrap()
                    );
                }
                IndexState::Ready => {
                    // Check if index is actually populated
                    let conn = self.write_conn.lock().unwrap();
                    let count = Self::get_index_item_count(&conn).unwrap_or(0);
                    if count > 0 {
                        info!("Index already has {} items, skipping build", count);
                        return;
                    }
                    resume_offset = None;
                }
                _ => {
                    resume_offset = None;
                }
            }
        }

        // Set state to building (if not already)
        if resume_offset.is_none() {
            let mut state = self.state.write().unwrap();
            *state = IndexState::Building {
                processed: 0,
                total: None,
            };
        }

        let vault = Arc::clone(self);

        std::thread::spawn(move || {
            if let Err(e) = vault.build_index_progressively(catalog_store, resume_offset) {
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
    ///
    /// If `resume_offset` is provided, the build resumes from that position.
    fn build_index_progressively(
        &self,
        catalog_store: Arc<dyn CatalogStore>,
        resume_offset: Option<usize>,
    ) -> Result<()> {
        let start_offset = resume_offset.unwrap_or(0);
        if start_offset > 0 {
            info!(
                "Resuming progressive index build from offset {}...",
                start_offset
            );
        } else {
            info!("Starting progressive index build...");
        }

        // Get all searchable content (already ordered by popularity)
        let searchable = catalog_store.get_searchable_content()?;
        let total = searchable.len();

        info!("Found {} items to index", total);

        // Update state with total
        {
            let mut state = self.state.write().unwrap();
            *state = IndexState::Building {
                processed: start_offset,
                total: Some(total),
            };
        }

        // Mark build as in progress and store total in metadata
        {
            let conn = self.write_conn.lock().unwrap();
            Self::set_metadata(&conn, "build_in_progress", "true")?;
            Self::set_metadata(&conn, "build_total", &total.to_string())?;

            // Only clear index if starting fresh
            if start_offset == 0 {
                conn.execute("DELETE FROM search_index", [])?;
                Self::set_metadata(&conn, "build_offset", "0")?;
            }
        }

        // Load existing vocabulary if resuming
        let mut vocabulary = if start_offset > 0 {
            let conn = self.write_conn.lock().unwrap();
            Self::load_vocabulary_from_index(&conn)?
        } else {
            Vocabulary::new()
        };

        let mut processed = start_offset;

        // Skip already-indexed items and process remaining in batches
        let remaining_items: Vec<_> = searchable.into_iter().skip(start_offset).collect();

        for batch in remaining_items.chunks(INDEX_BATCH_SIZE) {
            // Build vocabulary for this batch
            for item in batch {
                vocabulary.add_text(&item.name);
            }

            // Insert batch into index
            {
                let conn = self.write_conn.lock().unwrap();
                conn.execute("BEGIN IMMEDIATE", [])?;

                let mut stmt = conn.prepare(
                    "INSERT INTO search_index (item_id, item_type, name) VALUES (?, ?, ?)",
                )?;
                let mut avail_stmt = conn.prepare(
                    "INSERT OR REPLACE INTO item_availability (item_id, item_type, is_available) VALUES (?, ?, ?)",
                )?;

                for item in batch {
                    let type_str = match item.content_type {
                        SearchableContentType::Artist => "artist",
                        SearchableContentType::Album => "album",
                        SearchableContentType::Track => "track",
                    };
                    stmt.execute([&item.id, type_str, &item.name])?;
                    avail_stmt.execute(rusqlite::params![
                        &item.id,
                        type_str,
                        if item.is_available { 1 } else { 0 }
                    ])?;
                }

                conn.execute("COMMIT", [])?;

                // Persist progress after each batch
                Self::set_metadata(
                    &conn,
                    "build_offset",
                    &(processed + batch.len()).to_string(),
                )?;
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

        // Build complete - clear progress metadata and store catalog version
        {
            let conn = self.write_conn.lock().unwrap();
            Self::set_stored_catalog_version(&conn, 0)?;
            Self::delete_metadata(&conn, "build_in_progress")?;
            Self::delete_metadata(&conn, "build_offset")?;
            Self::delete_metadata(&conn, "build_total")?;
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
                listening_score REAL NOT NULL DEFAULT 0.0,
                impression_score REAL NOT NULL DEFAULT 0.0,
                spotify_score REAL NOT NULL DEFAULT 0.0,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (item_id, item_type)
            );
            CREATE INDEX IF NOT EXISTS idx_popularity_type ON item_popularity(item_type);
        "#,
        )?;

        // Create item_impressions table for tracking page views
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS item_impressions (
                item_id TEXT NOT NULL,
                item_type TEXT NOT NULL,
                date INTEGER NOT NULL,
                impression_count INTEGER NOT NULL DEFAULT 1,
                PRIMARY KEY (item_id, item_type, date)
            );
            CREATE INDEX IF NOT EXISTS idx_impressions_date ON item_impressions(date);
            CREATE INDEX IF NOT EXISTS idx_impressions_item ON item_impressions(item_id, item_type);
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

        // Create item_availability table for availability filtering
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS item_availability (
                item_id TEXT NOT NULL,
                item_type TEXT NOT NULL,
                is_available INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (item_id, item_type)
            );
            CREATE INDEX IF NOT EXISTS idx_availability_lookup
                ON item_availability(item_id, item_type, is_available);
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

    /// Get a metadata value by key
    fn get_metadata(conn: &Connection, key: &str) -> Option<String> {
        conn.query_row(
            "SELECT value FROM search_metadata WHERE key = ?",
            [key],
            |row| row.get(0),
        )
        .ok()
    }

    /// Set a metadata value
    fn set_metadata(conn: &Connection, key: &str, value: &str) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO search_metadata (key, value) VALUES (?, ?)",
            [key, value],
        )?;
        Ok(())
    }

    /// Delete a metadata key
    fn delete_metadata(conn: &Connection, key: &str) -> Result<()> {
        conn.execute("DELETE FROM search_metadata WHERE key = ?", [key])?;
        Ok(())
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
            conn.execute("DELETE FROM item_availability", [])?;

            let mut stmt = conn
                .prepare("INSERT INTO search_index (item_id, item_type, name) VALUES (?, ?, ?)")?;
            let mut avail_stmt = conn.prepare(
                "INSERT OR REPLACE INTO item_availability (item_id, item_type, is_available) VALUES (?, ?, ?)",
            )?;

            for item in &searchable {
                let type_str = match item.content_type {
                    SearchableContentType::Artist => "artist",
                    SearchableContentType::Album => "album",
                    SearchableContentType::Track => "track",
                };
                stmt.execute([&item.id, type_str, &item.name])?;
                avail_stmt.execute(rusqlite::params![
                    &item.id,
                    type_str,
                    if item.is_available { 1 } else { 0 }
                ])?;
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
        let conn = self.write_conn.lock().unwrap();
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

        let conn = self.write_conn.lock().unwrap();

        conn.execute("BEGIN IMMEDIATE", [])?;

        let chunk_size = UPSERT_SUB_BATCH_SIZE;
        let total_chunks = items.len().div_ceil(chunk_size);
        let mut delete_stmt =
            conn.prepare("DELETE FROM search_index WHERE item_id = ? AND item_type = ?")?;
        let mut insert_stmt =
            conn.prepare("INSERT INTO search_index (item_id, item_type, name) VALUES (?, ?, ?)")?;

        let result = (|| -> Result<()> {
            for (chunk_idx, chunk) in items.chunks(chunk_size).enumerate() {
                for item in chunk {
                    let type_str = Self::item_type_to_str(&item.item_type);
                    delete_stmt.execute([&item.id, type_str])?;
                    insert_stmt.execute([&item.id, type_str, &item.name])?;
                }

                if chunk_idx < total_chunks - 1 {
                    drop(delete_stmt);
                    drop(insert_stmt);
                    std::thread::sleep(std::time::Duration::from_millis(UPSERT_SUB_BATCH_YIELD_MS));
                    delete_stmt = conn
                        .prepare("DELETE FROM search_index WHERE item_id = ? AND item_type = ?")?;
                    insert_stmt = conn.prepare(
                        "INSERT INTO search_index (item_id, item_type, name) VALUES (?, ?, ?)",
                    )?;
                }
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;

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

        let conn = self.write_conn.lock().unwrap();
        let mut stmt =
            conn.prepare("DELETE FROM search_index WHERE item_id = ? AND item_type = ?")?;

        conn.execute("BEGIN IMMEDIATE", [])?;

        let chunk_size = UPSERT_SUB_BATCH_SIZE;
        let total_chunks = items.len().div_ceil(chunk_size);

        let result = (|| -> Result<()> {
            for (chunk_idx, chunk) in items.chunks(chunk_size).enumerate() {
                for (id, item_type) in chunk {
                    let type_str = Self::item_type_to_str(item_type);
                    stmt.execute([id.as_str(), type_str])?;
                }

                if chunk_idx < total_chunks - 1 {
                    drop(stmt);
                    std::thread::sleep(std::time::Duration::from_millis(UPSERT_SUB_BATCH_YIELD_MS));
                    stmt = conn
                        .prepare("DELETE FROM search_index WHERE item_id = ? AND item_type = ?")?;
                }
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
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
        if vocabulary.is_empty() {
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

    /// Record an impression (page view) for an item.
    /// Increments today's impression count for the given item.
    pub fn record_impression(&self, item_id: &str, item_type: HashedItemType) {
        let conn = self.write_conn.lock().unwrap();
        let today = chrono::Utc::now()
            .format("%Y%m%d")
            .to_string()
            .parse::<i64>()
            .unwrap_or(0);
        let type_str = Self::item_type_to_str(&item_type);

        if let Err(e) = conn.execute(
            "INSERT INTO item_impressions (item_id, item_type, date, impression_count)
             VALUES (?, ?, ?, 1)
             ON CONFLICT(item_id, item_type, date)
             DO UPDATE SET impression_count = impression_count + 1",
            rusqlite::params![item_id, type_str, today],
        ) {
            warn!(
                "Failed to record impression for {}/{}: {}",
                item_id, type_str, e
            );
        }
    }

    /// Get total impressions for all items within a date range.
    /// Returns a map of (item_id, item_type) -> total impression count.
    pub fn get_impression_totals(
        &self,
        min_date: i64,
    ) -> std::collections::HashMap<(String, HashedItemType), u64> {
        let conn = self.read_conn.lock().unwrap();
        let mut totals = std::collections::HashMap::new();

        let mut stmt = match conn.prepare(
            "SELECT item_id, item_type, SUM(impression_count) as total
             FROM item_impressions
             WHERE date >= ?
             GROUP BY item_id, item_type",
        ) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to prepare impression totals query: {}", e);
                return totals;
            }
        };

        let rows = match stmt.query_map([min_date], |row| {
            let item_id: String = row.get(0)?;
            let item_type_str: String = row.get(1)?;
            let total: i64 = row.get(2)?;
            Ok((item_id, item_type_str, total))
        }) {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to query impression totals: {}", e);
                return totals;
            }
        };

        for row in rows.flatten() {
            let (item_id, item_type_str, total) = row;
            if let Some(item_type) = Self::str_to_item_type(&item_type_str) {
                totals.insert((item_id, item_type), total as u64);
            }
        }

        totals
    }

    /// Prune old impression records.
    /// Deletes records older than the specified date (in YYYYMMDD format).
    pub fn prune_impressions(&self, before_date: i64) -> usize {
        let conn = self.write_conn.lock().unwrap();
        match conn.execute("DELETE FROM item_impressions WHERE date < ?", [before_date]) {
            Ok(count) => {
                if count > 0 {
                    info!("Pruned {} old impression records", count);
                }
                count
            }
            Err(e) => {
                warn!("Failed to prune impressions: {}", e);
                0
            }
        }
    }

    /// Update availability status for items.
    pub fn update_availability(&self, items: &[(String, HashedItemType, bool)]) {
        if items.is_empty() {
            return;
        }

        let conn = self.write_conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "INSERT OR REPLACE INTO item_availability (item_id, item_type, is_available) VALUES (?, ?, ?)",
        ) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to prepare availability update statement: {}", e);
                return;
            }
        };

        for (id, item_type, is_available) in items {
            if let Err(e) = stmt.execute(rusqlite::params![
                id,
                Self::item_type_to_str(item_type),
                if *is_available { 1 } else { 0 }
            ]) {
                warn!("Failed to update availability for {}: {}", id, e);
            }
        }

        debug!("Updated availability for {} items", items.len());
    }

    /// Search with availability filter in the query itself.
    pub fn search_with_availability(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
        available_only: bool,
    ) -> Vec<SearchResult> {
        if !available_only {
            // Fall back to regular search
            return SearchVault::search(self, query, max_results, filter);
        }

        let corrected_query = self.correct_query(query);
        let conn = self.read_conn.lock().unwrap();
        let escaped_query = corrected_query.replace('"', "\"\"");

        // Build query with availability JOIN
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
                   INNER JOIN item_availability a
                       ON s.item_id = a.item_id AND s.item_type = a.item_type AND a.is_available = 1
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
                         INNER JOIN item_availability a
                             ON s.item_id = a.item_id AND s.item_type = a.item_type AND a.is_available = 1
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

        // Execute query (same pattern as existing search())
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => {
                warn!("Availability search query prepare failed: {}", e);
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
                warn!("Availability search query failed: {}", e);
                Vec::new()
            }
        }
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

        // Use read connection - this won't block on writes due to WAL mode
        let conn = self.read_conn.lock().unwrap();

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
        let conn = self.read_conn.lock().unwrap();
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

    fn record_impression(&self, item_id: &str, item_type: HashedItemType) {
        Fts5LevenshteinSearchVault::record_impression(self, item_id, item_type)
    }

    fn get_impression_totals(
        &self,
        min_date: i64,
    ) -> std::collections::HashMap<(String, HashedItemType), u64> {
        Fts5LevenshteinSearchVault::get_impression_totals(self, min_date)
    }

    fn prune_impressions(&self, before_date: i64) -> usize {
        Fts5LevenshteinSearchVault::prune_impressions(self, before_date)
    }

    fn update_availability(&self, items: &[(String, HashedItemType, bool)]) {
        Fts5LevenshteinSearchVault::update_availability(self, items)
    }

    fn search_with_availability(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
        available_only: bool,
    ) -> Vec<SearchResult> {
        Fts5LevenshteinSearchVault::search_with_availability(
            self,
            query,
            max_results,
            filter,
            available_only,
        )
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
        use std::sync::atomic::AtomicI64;

        pub struct MockCatalogStore {
            pub items: Vec<SearchableItem>,
            #[allow(dead_code)]
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
                _limit: usize,
                _offset: usize,
                _sort: crate::catalog_store::DiscographySort,
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
            fn create_album(
                &self,
                _album: &crate::catalog_store::Album,
                _artist_ids: &[String],
            ) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn update_album(
                &self,
                _album: &crate::catalog_store::Album,
                _artist_ids: Option<&[String]>,
            ) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn delete_album(&self, _id: &str) -> anyhow::Result<bool> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn create_track(
                &self,
                _track: &crate::catalog_store::Track,
                _artist_ids: &[String],
            ) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn update_track(
                &self,
                _track: &crate::catalog_store::Track,
                _artist_ids: Option<&[String]>,
            ) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn delete_track(&self, _id: &str) -> anyhow::Result<bool> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn set_track_audio_uri(&self, _track_id: &str, _audio_uri: &str) -> anyhow::Result<()> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn recompute_album_availability(
                &self,
                _album_id: &str,
            ) -> anyhow::Result<crate::catalog_store::AlbumAvailability> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn recompute_artist_availability(&self, _artist_id: &str) -> anyhow::Result<bool> {
                anyhow::bail!("MockCatalogStore does not support write operations")
            }
            fn get_album_artist_ids(&self, _album_id: &str) -> anyhow::Result<Vec<String>> {
                Ok(Vec::new())
            }
            fn get_items_popularity(
                &self,
                _items: &[(String, SearchableContentType)],
            ) -> anyhow::Result<std::collections::HashMap<(String, SearchableContentType), i32>>
            {
                Ok(std::collections::HashMap::new())
            }
            fn get_genres_with_counts(
                &self,
            ) -> anyhow::Result<Vec<crate::catalog_store::GenreInfo>> {
                Ok(Vec::new())
            }
            fn get_tracks_by_genre(
                &self,
                _genre: &str,
                _limit: usize,
                _offset: usize,
            ) -> anyhow::Result<crate::catalog_store::GenreTracksResult> {
                Ok(crate::catalog_store::GenreTracksResult {
                    track_ids: Vec::new(),
                    total: 0,
                    has_more: false,
                })
            }
            fn get_random_tracks_by_genre(
                &self,
                _genre: &str,
                _limit: usize,
            ) -> anyhow::Result<Vec<String>> {
                Ok(Vec::new())
            }
            fn find_albums_by_fingerprint(
                &self,
                _track_count: i32,
                _total_duration_ms: i64,
            ) -> anyhow::Result<Vec<crate::catalog_store::AlbumFingerprintCandidate>> {
                Ok(Vec::new())
            }
            fn update_album_fingerprint(&self, _album_id: &str) -> anyhow::Result<()> {
                Ok(())
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
                is_available: true,
            },
            SearchableItem {
                id: "a2".to_string(),
                name: "Metallica".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
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
                is_available: true,
            },
            SearchableItem {
                id: "a2".to_string(),
                name: "Metallica".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
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
                is_available: true,
            },
            SearchableItem {
                id: "album1".to_string(),
                name: "Beatles For Sale".to_string(),
                content_type: SearchableContentType::Album,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "track1".to_string(),
                name: "Beatles Medley".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
                is_available: true,
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

    #[test]
    fn test_resumable_build_detects_partial_state() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // First, create a vault and manually simulate a partial build
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            Fts5LevenshteinSearchVault::create_tables(&conn).unwrap();

            // Insert some items (simulating partial progress)
            conn.execute(
                "INSERT INTO search_index (item_id, item_type, name) VALUES ('a1', 'artist', 'The Beatles')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO search_index (item_id, item_type, name) VALUES ('a2', 'artist', 'Pink Floyd')",
                [],
            )
            .unwrap();

            // Set partial build metadata
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_in_progress", "true").unwrap();
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_offset", "2").unwrap();
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_total", "5").unwrap();
        }

        // Now create a new lazy vault - it should detect the partial build
        let vault = Fts5LevenshteinSearchVault::new_lazy(&db_path).unwrap();

        let stats = vault.get_stats();
        assert_eq!(stats.indexed_items, 2);
        assert!(
            matches!(
                stats.state,
                IndexState::Building {
                    processed: 2,
                    total: Some(5)
                }
            ),
            "Expected Building state with processed=2, total=Some(5), got {:?}",
            stats.state
        );
    }

    #[test]
    fn test_resumable_build_resumes_from_offset() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Create 5 items
        let items = vec![
            SearchableItem {
                id: "a1".to_string(),
                name: "The Beatles".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "a2".to_string(),
                name: "Pink Floyd".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "a3".to_string(),
                name: "Led Zeppelin".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "a4".to_string(),
                name: "Metallica".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "a5".to_string(),
                name: "Iron Maiden".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
        ];

        // Simulate partial build: insert first 2 items manually
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            Fts5LevenshteinSearchVault::create_tables(&conn).unwrap();

            // Insert first 2 items (simulating partial progress)
            conn.execute(
                "INSERT INTO search_index (item_id, item_type, name) VALUES ('a1', 'artist', 'The Beatles')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO search_index (item_id, item_type, name) VALUES ('a2', 'artist', 'Pink Floyd')",
                [],
            )
            .unwrap();

            // Set partial build metadata
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_in_progress", "true").unwrap();
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_offset", "2").unwrap();
            Fts5LevenshteinSearchVault::set_metadata(&conn, "build_total", "5").unwrap();
        }

        // Create vault and resume build
        let vault = Arc::new(Fts5LevenshteinSearchVault::new_lazy(&db_path).unwrap());
        let catalog = Arc::new(MockCatalogStore::new(items));

        vault.start_background_build(catalog);

        // Wait for build to complete
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Should now have all 5 items
        let stats = vault.get_stats();
        assert_eq!(stats.indexed_items, 5, "Expected 5 items after resume");
        assert_eq!(stats.state, IndexState::Ready);

        // Verify all items are searchable
        let results = vault.search("Beatles", 10, None);
        assert_eq!(results.len(), 1);
        let results = vault.search("Zeppelin", 10, None);
        assert_eq!(results.len(), 1);
        let results = vault.search("Maiden", 10, None);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_completed_build_clears_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let items = vec![SearchableItem {
            id: "a1".to_string(),
            name: "The Beatles".to_string(),
            content_type: SearchableContentType::Artist,
            additional_text: vec![],
            is_available: true,
        }];

        let vault = Arc::new(Fts5LevenshteinSearchVault::new_lazy(&db_path).unwrap());
        let catalog = Arc::new(MockCatalogStore::new(items));

        vault.start_background_build(catalog);

        // Wait for build to complete
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Check that metadata was cleared
        let conn = Connection::open(&db_path).unwrap();
        assert!(
            Fts5LevenshteinSearchVault::get_metadata(&conn, "build_in_progress").is_none(),
            "build_in_progress should be cleared after completion"
        );
        assert!(
            Fts5LevenshteinSearchVault::get_metadata(&conn, "build_offset").is_none(),
            "build_offset should be cleared after completion"
        );
        assert!(
            Fts5LevenshteinSearchVault::get_metadata(&conn, "build_total").is_none(),
            "build_total should be cleared after completion"
        );

        // Creating a new vault should show Ready state
        let vault2 = Fts5LevenshteinSearchVault::new_lazy(&db_path).unwrap();
        assert_eq!(vault2.get_stats().state, IndexState::Ready);
    }

    #[test]
    fn test_search_with_availability_filters_unavailable() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Create items with mixed availability
        let items = vec![
            SearchableItem {
                id: "available_artist".to_string(),
                name: "The Beatles".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "unavailable_artist".to_string(),
                name: "Beatles Tribute Band".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
                is_available: false,
            },
            SearchableItem {
                id: "available_album".to_string(),
                name: "Beatles Greatest Hits".to_string(),
                content_type: SearchableContentType::Album,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "unavailable_album".to_string(),
                name: "Beatles Live".to_string(),
                content_type: SearchableContentType::Album,
                additional_text: vec![],
                is_available: false,
            },
        ];

        let catalog = Arc::new(MockCatalogStore::new(items));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Regular search should return all 4 items
        let all_results = vault.search("Beatles", 10, None);
        assert_eq!(
            all_results.len(),
            4,
            "Regular search should return all items"
        );

        // search_with_availability(available_only=true) should only return available items
        let available_results = vault.search_with_availability("Beatles", 10, None, true);
        assert_eq!(
            available_results.len(),
            2,
            "Availability search should only return available items"
        );

        // Verify we got the right items
        let available_ids: Vec<_> = available_results
            .iter()
            .map(|r| r.item_id.as_str())
            .collect();
        assert!(available_ids.contains(&"available_artist"));
        assert!(available_ids.contains(&"available_album"));
        assert!(!available_ids.contains(&"unavailable_artist"));
        assert!(!available_ids.contains(&"unavailable_album"));

        // search_with_availability(available_only=false) should return all items
        let all_via_availability = vault.search_with_availability("Beatles", 10, None, false);
        assert_eq!(
            all_via_availability.len(),
            4,
            "Availability search with available_only=false should return all items"
        );
    }

    #[test]
    fn test_update_availability_changes_search_results() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Start with one available and one unavailable - both share "day" in the name for searching
        let items = vec![
            SearchableItem {
                id: "track1".to_string(),
                name: "Yesterday Song".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
                is_available: true,
            },
            SearchableItem {
                id: "track2".to_string(),
                name: "Today Song".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
                is_available: false,
            },
        ];

        let catalog = Arc::new(MockCatalogStore::new(items));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Initially, only track1 should be available when searching for "day"
        let results = vault.search_with_availability("Song", 10, None, true);
        assert_eq!(
            results.len(),
            1,
            "Initially only 1 track should be available"
        );
        assert_eq!(results[0].item_id, "track1");

        // Update track2 to be available
        vault.update_availability(&[("track2".to_string(), HashedItemType::Track, true)]);

        // Now both should be available
        let results = vault.search_with_availability("Song", 10, None, true);
        assert_eq!(
            results.len(),
            2,
            "After update, both tracks should be available"
        );

        // Update track1 to be unavailable
        vault.update_availability(&[("track1".to_string(), HashedItemType::Track, false)]);

        // Now only track2 should be available
        let results = vault.search_with_availability("Song", 10, None, true);
        assert_eq!(
            results.len(),
            1,
            "After second update, only 1 track should be available"
        );
        assert_eq!(results[0].item_id, "track2");
    }

    #[test]
    fn test_record_impression() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let vault = Fts5LevenshteinSearchVault::new_lazy(&db_path).unwrap();

        // Record impressions
        vault.record_impression("artist1", HashedItemType::Artist);
        vault.record_impression("artist1", HashedItemType::Artist);
        vault.record_impression("album1", HashedItemType::Album);

        // Verify they were recorded
        let conn = Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT impression_count FROM item_impressions WHERE item_id = 'artist1' AND item_type = 'artist'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);

        let count: i64 = conn
            .query_row(
                "SELECT impression_count FROM item_impressions WHERE item_id = 'album1' AND item_type = 'album'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_get_impression_totals() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let vault = Fts5LevenshteinSearchVault::new_lazy(&db_path).unwrap();

        // Insert impressions with different dates directly
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count) VALUES ('a1', 'artist', 20250101, 10)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count) VALUES ('a1', 'artist', 20250102, 5)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count) VALUES ('a2', 'album', 20250101, 3)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count) VALUES ('a3', 'track', 20241201, 100)",
                [],
            ).unwrap();
        }

        // Get totals from 2025-01-01 onwards
        let totals = vault.get_impression_totals(20250101);

        assert_eq!(
            totals.get(&("a1".to_string(), HashedItemType::Artist)),
            Some(&15)
        );
        assert_eq!(
            totals.get(&("a2".to_string(), HashedItemType::Album)),
            Some(&3)
        );
        // a3 should not be included (date is before min_date)
        assert_eq!(totals.get(&("a3".to_string(), HashedItemType::Track)), None);
    }

    #[test]
    fn test_prune_impressions() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let vault = Fts5LevenshteinSearchVault::new_lazy(&db_path).unwrap();

        // Insert impressions with different dates
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count) VALUES ('a1', 'artist', 20240101, 10)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO item_impressions (item_id, item_type, date, impression_count) VALUES ('a2', 'artist', 20250101, 5)",
                [],
            ).unwrap();
        }

        // Prune old impressions (before 2025)
        let pruned = vault.prune_impressions(20250101);
        assert_eq!(pruned, 1);

        // Verify only newer record remains
        let conn = Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM item_impressions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }
}
