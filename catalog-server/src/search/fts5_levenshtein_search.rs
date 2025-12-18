//! FTS5 search with Levenshtein-based typo tolerance
//!
//! This search vault combines SQLite FTS5 full-text search with
//! Levenshtein distance-based query correction for typo tolerance.
//!
//! The search index is persistent and only rebuilt when:
//! - The catalog version changes (detected via skeleton_version)
//! - The index is incomplete or corrupted
//! - Explicitly triggered (e.g., on batch close)

use super::levenshtein::Vocabulary;
use super::{HashedItemType, SearchResult, SearchVault};
use crate::catalog_store::{CatalogStore, SearchableContentType};
use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use tracing::{debug, info, warn};

/// FTS5 search vault with Levenshtein-based typo correction.
///
/// This implementation builds a vocabulary from all indexed content and
/// uses Levenshtein distance to correct typos in search queries before
/// passing them to FTS5.
///
/// The search index is persistent across restarts. It tracks the catalog
/// version and only rebuilds when the catalog has changed.
pub struct Fts5LevenshteinSearchVault {
    conn: Mutex<Connection>,
    vocabulary: RwLock<Vocabulary>,
    catalog_store: Arc<dyn CatalogStore>,
    /// Maximum edit distance for typo correction (default: 2)
    max_edit_distance: usize,
}

impl Fts5LevenshteinSearchVault {
    /// Create a new FTS5 + Levenshtein search vault
    ///
    /// # Arguments
    /// * `catalog_store` - The catalog store to index content from
    /// * `db_path` - Path to the search database file
    pub fn new(catalog_store: Arc<dyn CatalogStore>, db_path: &Path) -> Result<Self> {
        Self::with_max_distance(catalog_store, db_path, 2)
    }

    /// Create a new vault with custom max edit distance
    ///
    /// The search index is persistent. On startup, it checks if:
    /// 1. The stored catalog version matches the current catalog version
    /// 2. The index item count matches the catalog item count
    ///
    /// If either check fails, the index is rebuilt automatically.
    pub fn with_max_distance(
        catalog_store: Arc<dyn CatalogStore>,
        db_path: &Path,
        max_edit_distance: usize,
    ) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Enable WAL mode for better concurrent access
        conn.pragma_update(None, "journal_mode", "WAL")?;

        // Create tables if they don't exist
        Self::create_tables(&conn)?;

        // Check if we need to rebuild the index
        let current_catalog_version = catalog_store.get_skeleton_version().unwrap_or(0);
        let expected_item_count = catalog_store
            .get_searchable_content()
            .map(|items| items.len())
            .unwrap_or(0);

        let needs_rebuild =
            Self::check_needs_rebuild(&conn, current_catalog_version, expected_item_count);

        // Build vocabulary from existing index or rebuild
        let vocabulary = if needs_rebuild {
            Self::rebuild_index_internal(&conn, &catalog_store, current_catalog_version)?
        } else {
            Self::load_vocabulary_from_index(&conn)?
        };

        Ok(Self {
            conn: Mutex::new(conn),
            vocabulary: RwLock::new(vocabulary),
            catalog_store,
            max_edit_distance,
        })
    }

    /// Check if the search index needs to be rebuilt.
    ///
    /// Returns true if:
    /// - No stored catalog version exists
    /// - Stored version doesn't match current catalog version
    /// - Index item count doesn't match expected count (integrity check)
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
        // Create FTS5 virtual table with trigram tokenizer
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

        // Create item_popularity table for popularity-weighted search
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

        // Create metadata table for tracking index state
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

        for name_result in rows {
            if let Ok(name) = name_result {
                vocabulary.add_text(&name);
            }
        }

        debug!(
            "Loaded vocabulary with {} words from existing index",
            vocabulary.len()
        );
        Ok(vocabulary)
    }

    /// Internal rebuild that works with a connection reference.
    ///
    /// This method uses a transaction to ensure atomicity - if the rebuild
    /// fails partway through, the index will be rolled back to its previous
    /// state rather than being left in a corrupted state.
    fn rebuild_index_internal(
        conn: &Connection,
        catalog_store: &Arc<dyn CatalogStore>,
        catalog_version: i64,
    ) -> Result<Vocabulary> {
        // Get searchable content before starting transaction
        let searchable = catalog_store.get_searchable_content()?;
        let count = searchable.len();

        // Build vocabulary (this doesn't touch the database)
        let mut vocabulary = Vocabulary::new();
        for item in &searchable {
            vocabulary.add_text(&item.name);
        }

        // Use a transaction for atomicity
        conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            // Clear existing index data
            conn.execute("DELETE FROM search_index", [])?;

            // Insert all items
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

            // Store the catalog version
            Self::set_stored_catalog_version(conn, catalog_version)?;

            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])?;
                info!(
                    "Search index built with {} items, vocabulary has {} words (catalog version {})",
                    count,
                    vocabulary.len(),
                    catalog_version
                );
                Ok(vocabulary)
            }
            Err(e) => {
                // Rollback on error
                if let Err(rollback_err) = conn.execute("ROLLBACK", []) {
                    warn!("Failed to rollback transaction: {}", rollback_err);
                }
                Err(e)
            }
        }
    }

    /// Rebuild the search index from the catalog.
    ///
    /// This should be called after catalog changes (e.g., when a batch is closed).
    pub fn rebuild_index(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let catalog_version = self.catalog_store.get_skeleton_version().unwrap_or(0);

        let vocabulary = Self::rebuild_index_internal(&conn, &self.catalog_store, catalog_version)?;

        // Update the vocabulary
        let mut vocab_lock = self.vocabulary.write().unwrap();
        *vocab_lock = vocabulary;

        Ok(())
    }

    /// Update popularity scores for items.
    ///
    /// This method updates the `item_popularity` table with normalized scores
    /// for items based on their play counts. Scores should be normalized 0.0-1.0
    /// within each item type.
    ///
    /// # Arguments
    /// * `items` - Slice of (item_id, item_type, play_count, normalized_score) tuples
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
        vocabulary.correct_query(query, self.max_edit_distance)
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
        // First, correct the query using Levenshtein distance
        let corrected_query = self.correct_query(query);

        if corrected_query != query {
            debug!("Query corrected: '{}' -> '{}'", query, corrected_query);
        }

        let conn = self.conn.lock().unwrap();

        // Escape special FTS5 characters
        let escaped_query = corrected_query.replace('"', "\"\"");

        // Build query with optional type filter
        // Uses LEFT JOIN with item_popularity to boost popular items in ranking
        // Formula: bm25_score * (1.0 + popularity * weight)
        // BM25 scores are negative (more negative = better match), so multiplying
        // by (1 + popularity * weight) makes popular items more negative (ranked higher)
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
                            // Compute the combined score (more negative = better)
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
        // Call the public rebuild_index method on self
        Fts5LevenshteinSearchVault::rebuild_index(self)
    }

    fn update_popularity(&self, items: &[(String, HashedItemType, u64, f64)]) {
        // Call the public update_popularity method on self
        Fts5LevenshteinSearchVault::update_popularity(self, items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::SearchableItem;
    use std::sync::Arc;
    use tempfile::TempDir;

    // Reuse the mock from fts5_search tests
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

            pub fn set_version(&self, version: i64) {
                self.skeleton_version.store(version, Ordering::SeqCst);
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
            fn create_artist(&self, _data: serde_json::Value) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn update_artist(
                &self,
                _id: &str,
                _data: serde_json::Value,
            ) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn delete_artist(&self, _id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn create_album(&self, _data: serde_json::Value) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn update_album(
                &self,
                _id: &str,
                _data: serde_json::Value,
            ) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn delete_album(&self, _id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn create_track(&self, _data: serde_json::Value) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn update_track(
                &self,
                _id: &str,
                _data: serde_json::Value,
            ) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn delete_track(&self, _id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn create_image(&self, _data: serde_json::Value) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn update_image(
                &self,
                _id: &str,
                _data: serde_json::Value,
            ) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn delete_image(&self, _id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn create_changelog_batch(
                &self,
                _name: &str,
                _description: Option<&str>,
            ) -> anyhow::Result<crate::catalog_store::CatalogBatch> {
                unimplemented!()
            }
            fn get_changelog_batch(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::CatalogBatch>> {
                Ok(None)
            }
            fn get_active_changelog_batch(
                &self,
            ) -> anyhow::Result<Option<crate::catalog_store::CatalogBatch>> {
                Ok(None)
            }
            fn close_changelog_batch(&self, _id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn list_changelog_batches(
                &self,
                _is_open: Option<bool>,
            ) -> anyhow::Result<Vec<crate::catalog_store::CatalogBatch>> {
                Ok(vec![])
            }
            fn delete_changelog_batch(&self, _id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn get_changelog_batch_changes(
                &self,
                _batch_id: &str,
            ) -> anyhow::Result<Vec<crate::catalog_store::ChangeEntry>> {
                Ok(vec![])
            }
            fn get_changelog_entity_history(
                &self,
                _entity_type: crate::catalog_store::ChangeEntityType,
                _entity_id: &str,
            ) -> anyhow::Result<Vec<crate::catalog_store::ChangeEntry>> {
                Ok(vec![])
            }
            fn get_whats_new_batches(
                &self,
                _limit: usize,
            ) -> anyhow::Result<Vec<crate::catalog_store::WhatsNewBatch>> {
                Ok(vec![])
            }
            fn get_stale_batches(
                &self,
                _stale_threshold_hours: u64,
            ) -> anyhow::Result<Vec<crate::catalog_store::CatalogBatch>> {
                Ok(vec![])
            }
            fn close_stale_batches(&self) -> anyhow::Result<usize> {
                Ok(0)
            }
            fn get_changelog_batch_summary(
                &self,
                _batch_id: &str,
            ) -> anyhow::Result<crate::catalog_store::BatchChangeSummary> {
                Ok(crate::catalog_store::BatchChangeSummary::default())
            }
            fn list_all_track_ids(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn list_all_album_image_ids(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn list_all_artist_image_ids(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn get_artists_without_related(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn get_orphan_related_artist_ids(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn add_artist_image(
                &self,
                _artist_id: &str,
                _image_id: &str,
                _image_type: &crate::catalog_store::ImageType,
                _position: i32,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            fn add_album_image(
                &self,
                _album_id: &str,
                _image_id: &str,
                _image_type: &crate::catalog_store::ImageType,
                _position: i32,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            fn set_artist_display_image(
                &self,
                _artist_id: &str,
                _image_id: &str,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            fn set_album_display_image(
                &self,
                _album_id: &str,
                _image_id: &str,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            fn get_album_display_image_id(
                &self,
                _album_id: &str,
            ) -> anyhow::Result<Option<String>> {
                Ok(None)
            }
            fn get_skeleton_version(&self) -> anyhow::Result<i64> {
                Ok(self
                    .skeleton_version
                    .load(std::sync::atomic::Ordering::SeqCst))
            }
            fn get_skeleton_checksum(&self) -> anyhow::Result<String> {
                Ok(String::new())
            }
            fn get_skeleton_events_since(
                &self,
                _seq: i64,
            ) -> anyhow::Result<Vec<crate::skeleton::SkeletonEvent>> {
                Ok(vec![])
            }
            fn get_skeleton_earliest_seq(&self) -> anyhow::Result<i64> {
                Ok(0)
            }
            fn get_skeleton_latest_seq(&self) -> anyhow::Result<i64> {
                Ok(0)
            }
            fn get_all_artist_ids(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn get_all_albums_skeleton(
                &self,
            ) -> anyhow::Result<Vec<crate::skeleton::SkeletonAlbumEntry>> {
                Ok(vec![])
            }
            fn get_all_tracks_skeleton(
                &self,
            ) -> anyhow::Result<Vec<crate::skeleton::SkeletonTrackEntry>> {
                Ok(vec![])
            }
        }
    }

    use mock::MockCatalogStore;

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
            SearchableItem {
                id: "a3".to_string(),
                name: "Abbey Road".to_string(),
                content_type: SearchableContentType::Album,
                additional_text: vec![],
            },
        ]));

        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Exact search
        let results = vault.search("Beatles", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "a1");

        // Search with typo - "Beatels" should find "Beatles"
        let results = vault.search("Beatels", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "a1");

        // Search with typo - "Metalica" should find "Metallica"
        let results = vault.search("Metalica", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "a2");

        // Search with typo - "Abby" should find "Abbey"
        let results = vault.search("Abby Road", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "a3");
    }

    #[test]
    fn test_rebuild_updates_vocabulary() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Start with empty catalog, version 1
        let catalog = Arc::new(MockCatalogStore::with_version(vec![], 1));
        let vault = Fts5LevenshteinSearchVault::new(catalog.clone(), &db_path).unwrap();

        // Initially empty - no results
        let results = vault.search("Nirvana", 10, None);
        assert_eq!(results.len(), 0);

        // Create a new catalog with Nirvana, version 2
        // Note: Since we can't modify the mock in place, we need to recreate vault
        // This tests that rebuilding picks up new content
        let catalog2 = Arc::new(MockCatalogStore::with_version(
            vec![SearchableItem {
                id: "new1".to_string(),
                name: "Nirvana".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            }],
            2,
        ));

        // Create new vault - should rebuild due to version change
        let vault2 = Fts5LevenshteinSearchVault::new(catalog2, &db_path).unwrap();

        // Search with typo should work after rebuild
        let results = vault2.search("Nirvna", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "new1");
    }

    #[test]
    fn test_item_popularity_table_created() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let catalog = Arc::new(MockCatalogStore::new(vec![]));

        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Verify the item_popularity table exists by inserting and querying
        let conn = vault.conn.lock().unwrap();

        // Insert a test row
        conn.execute(
            "INSERT INTO item_popularity (item_id, item_type, play_count, score, updated_at) VALUES (?, ?, ?, ?, ?)",
            ["test_id", "track", "100", "0.5", "1234567890"],
        )
        .expect("item_popularity table should exist");

        // Query it back
        let mut stmt = conn
            .prepare("SELECT item_id, item_type, play_count, score, updated_at FROM item_popularity WHERE item_id = ?")
            .unwrap();
        let result: (String, String, i64, f64, i64) = stmt
            .query_row(["test_id"], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .expect("Should be able to query item_popularity");

        assert_eq!(result.0, "test_id");
        assert_eq!(result.1, "track");
        assert_eq!(result.2, 100);
        assert_eq!(result.3, 0.5);
        assert_eq!(result.4, 1234567890);
    }

    #[test]
    fn test_index_persists_across_restarts() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let items = vec![SearchableItem {
            id: "a1".to_string(),
            name: "The Beatles".to_string(),
            content_type: SearchableContentType::Artist,
            additional_text: vec![],
        }];

        // Create vault with version 1
        let catalog = Arc::new(MockCatalogStore::with_version(items.clone(), 1));
        {
            let vault = Fts5LevenshteinSearchVault::new(catalog.clone(), &db_path).unwrap();
            let results = vault.search("Beatles", 10, None);
            assert_eq!(results.len(), 1);
        }

        // Create vault again with same version - should reuse existing index
        let catalog2 = Arc::new(MockCatalogStore::with_version(items, 1));
        let vault2 = Fts5LevenshteinSearchVault::new(catalog2, &db_path).unwrap();
        let results = vault2.search("Beatles", 10, None);
        assert_eq!(results.len(), 1, "Index should persist across restarts");
    }

    #[test]
    fn test_index_rebuilds_on_version_change() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Create vault with version 1 and one item
        let items_v1 = vec![SearchableItem {
            id: "a1".to_string(),
            name: "The Beatles".to_string(),
            content_type: SearchableContentType::Artist,
            additional_text: vec![],
        }];
        let catalog = Arc::new(MockCatalogStore::with_version(items_v1, 1));
        {
            let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();
            let results = vault.search("Beatles", 10, None);
            assert_eq!(results.len(), 1);
        }

        // Create vault with version 2 and different items - should rebuild
        let items_v2 = vec![SearchableItem {
            id: "a2".to_string(),
            name: "Metallica".to_string(),
            content_type: SearchableContentType::Artist,
            additional_text: vec![],
        }];
        let catalog2 = Arc::new(MockCatalogStore::with_version(items_v2, 2));
        let vault2 = Fts5LevenshteinSearchVault::new(catalog2, &db_path).unwrap();

        // Old item should not be found
        let results = vault2.search("Beatles", 10, None);
        assert_eq!(results.len(), 0, "Old item should not exist after rebuild");

        // New item should be found
        let results = vault2.search("Metallica", 10, None);
        assert_eq!(results.len(), 1, "New item should exist after rebuild");
    }

    #[test]
    fn test_rebuild_index_updates_catalog_version() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let items = vec![SearchableItem {
            id: "a1".to_string(),
            name: "The Beatles".to_string(),
            content_type: SearchableContentType::Artist,
            additional_text: vec![],
        }];

        let catalog = Arc::new(MockCatalogStore::with_version(items, 1));
        let vault = Fts5LevenshteinSearchVault::new(catalog.clone(), &db_path).unwrap();

        // Change catalog version and add new item
        catalog.set_version(2);

        // Manually trigger rebuild
        vault.rebuild_index().unwrap();

        // Check stored version was updated
        let conn = vault.conn.lock().unwrap();
        let stored_version = Fts5LevenshteinSearchVault::get_stored_catalog_version(&conn);
        assert_eq!(
            stored_version,
            Some(2),
            "Stored version should be updated after rebuild"
        );
    }

    #[test]
    fn test_metadata_table_created() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let catalog = Arc::new(MockCatalogStore::with_version(vec![], 42));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Verify the metadata table exists and has the catalog version
        let conn = vault.conn.lock().unwrap();
        let stored_version = Fts5LevenshteinSearchVault::get_stored_catalog_version(&conn);
        assert_eq!(
            stored_version,
            Some(42),
            "Catalog version should be stored in metadata"
        );
    }

    #[test]
    fn test_index_rebuilds_on_item_count_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Create vault with 2 items, version 1
        let items_v1 = vec![
            SearchableItem {
                id: "a1".to_string(),
                name: "The Beatles".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            },
            SearchableItem {
                id: "a2".to_string(),
                name: "Led Zeppelin".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            },
        ];
        let catalog1 = Arc::new(MockCatalogStore::with_version(items_v1, 1));
        {
            let vault = Fts5LevenshteinSearchVault::new(catalog1, &db_path).unwrap();
            // Verify both items are searchable
            let results = vault.search("Beatles", 10, None);
            assert_eq!(results.len(), 1);
            let results = vault.search("Zeppelin", 10, None);
            assert_eq!(results.len(), 1);
        }

        // Create vault with same version but 3 items (simulating corruption or missed update)
        let items_v2 = vec![
            SearchableItem {
                id: "a1".to_string(),
                name: "The Beatles".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            },
            SearchableItem {
                id: "a2".to_string(),
                name: "Led Zeppelin".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            },
            SearchableItem {
                id: "a3".to_string(),
                name: "Pink Floyd".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            },
        ];
        let catalog2 = Arc::new(MockCatalogStore::with_version(items_v2, 1));
        let vault2 = Fts5LevenshteinSearchVault::new(catalog2, &db_path).unwrap();

        // All 3 items should be searchable after rebuild due to count mismatch
        let results = vault2.search("Beatles", 10, None);
        assert_eq!(results.len(), 1, "Beatles should be found");
        let results = vault2.search("Floyd", 10, None);
        assert_eq!(results.len(), 1, "Pink Floyd should be found after rebuild");
    }

    #[test]
    fn test_empty_catalog_creates_empty_index() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let catalog = Arc::new(MockCatalogStore::with_version(vec![], 1));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Search on empty index should return no results
        let results = vault.search("anything", 10, None);
        assert_eq!(results.len(), 0, "Empty index should return no results");

        // Verify index item count is 0
        let conn = vault.conn.lock().unwrap();
        let count = Fts5LevenshteinSearchVault::get_index_item_count(&conn);
        assert_eq!(count, Some(0), "Empty index should have 0 items");
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
        assert_eq!(results.len(), 3, "Should find all 3 Beatles items");

        // Search with Artist filter only
        let results = vault.search("Beatles", 10, Some(vec![HashedItemType::Artist]));
        assert_eq!(results.len(), 1, "Should find only artist");
        assert_eq!(results[0].item_id, "artist1");

        // Search with Album filter only
        let results = vault.search("Beatles", 10, Some(vec![HashedItemType::Album]));
        assert_eq!(results.len(), 1, "Should find only album");
        assert_eq!(results[0].item_id, "album1");

        // Search with Track filter only
        let results = vault.search("Beatles", 10, Some(vec![HashedItemType::Track]));
        assert_eq!(results.len(), 1, "Should find only track");
        assert_eq!(results[0].item_id, "track1");

        // Search with multiple filters
        let results = vault.search(
            "Beatles",
            10,
            Some(vec![HashedItemType::Artist, HashedItemType::Album]),
        );
        assert_eq!(results.len(), 2, "Should find artist and album");
    }

    #[test]
    fn test_search_max_results_limit() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Create many items
        let items: Vec<SearchableItem> = (0..20)
            .map(|i| SearchableItem {
                id: format!("track{}", i),
                name: format!("Song Number {}", i),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
            })
            .collect();

        let catalog = Arc::new(MockCatalogStore::with_version(items, 1));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Search with different limits
        let results = vault.search("Song", 5, None);
        assert_eq!(results.len(), 5, "Should respect max_results limit of 5");

        let results = vault.search("Song", 10, None);
        assert_eq!(results.len(), 10, "Should respect max_results limit of 10");

        let results = vault.search("Song", 100, None);
        assert_eq!(
            results.len(),
            20,
            "Should return all 20 when limit exceeds count"
        );
    }

    #[test]
    fn test_special_characters_in_search() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let items = vec![
            SearchableItem {
                id: "t1".to_string(),
                name: "Rock & Roll".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
            },
            SearchableItem {
                id: "t2".to_string(),
                name: "AC/DC".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            },
            SearchableItem {
                id: "t3".to_string(),
                name: "\"Hello World\"".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
            },
        ];

        let catalog = Arc::new(MockCatalogStore::with_version(items, 1));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Search with ampersand - should find "Rock & Roll"
        let results = vault.search("Rock", 10, None);
        assert_eq!(results.len(), 1, "Should find Rock & Roll");

        // Search for AC/DC with exact slash - FTS5 trigram handles special chars
        let results = vault.search("AC/DC", 10, None);
        assert_eq!(results.len(), 1, "Should find AC/DC with slash");

        // Search with quotes - should find the track
        let results = vault.search("Hello", 10, None);
        assert_eq!(results.len(), 1, "Should find Hello World");

        // Roll should also find Rock & Roll
        let results = vault.search("Roll", 10, None);
        assert_eq!(results.len(), 1, "Should find Rock & Roll via Roll");
    }

    #[test]
    fn test_unicode_search() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let items = vec![
            SearchableItem {
                id: "t1".to_string(),
                name: "Café del Mar".to_string(),
                content_type: SearchableContentType::Album,
                additional_text: vec![],
            },
            SearchableItem {
                id: "t2".to_string(),
                name: "日本語タイトル".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
            },
            SearchableItem {
                id: "t3".to_string(),
                name: "Résumé".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            },
        ];

        let catalog = Arc::new(MockCatalogStore::with_version(items, 1));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Search with accented character
        let results = vault.search("Café", 10, None);
        assert_eq!(results.len(), 1, "Should find Café del Mar");

        // Search for Japanese
        let results = vault.search("日本語", 10, None);
        assert_eq!(results.len(), 1, "Should find Japanese title");
    }

    #[test]
    fn test_case_insensitive_typo_correction() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let items = vec![SearchableItem {
            id: "a1".to_string(),
            name: "METALLICA".to_string(),
            content_type: SearchableContentType::Artist,
            additional_text: vec![],
        }];

        let catalog = Arc::new(MockCatalogStore::with_version(items, 1));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Lower case search should find upper case name
        let results = vault.search("metallica", 10, None);
        assert_eq!(results.len(), 1, "Case insensitive search should work");

        // Mixed case with typo
        let results = vault.search("Metalica", 10, None);
        assert_eq!(
            results.len(),
            1,
            "Typo correction should work with mixed case"
        );
    }

    #[test]
    fn test_rebuild_index_method_works() {
        use std::sync::RwLock;

        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Create a dynamic mock that can have items changed
        struct DynamicMockCatalogStore {
            items: RwLock<Vec<SearchableItem>>,
            version: std::sync::atomic::AtomicI64,
        }

        impl DynamicMockCatalogStore {
            fn set_items(&self, items: Vec<SearchableItem>) {
                *self.items.write().unwrap() = items;
            }
            fn set_version(&self, v: i64) {
                self.version.store(v, std::sync::atomic::Ordering::SeqCst);
            }
        }

        impl CatalogStore for DynamicMockCatalogStore {
            fn get_searchable_content(&self) -> anyhow::Result<Vec<SearchableItem>> {
                Ok(self.items.read().unwrap().clone())
            }
            fn get_skeleton_version(&self) -> anyhow::Result<i64> {
                Ok(self.version.load(std::sync::atomic::Ordering::SeqCst))
            }
            // All other methods are stubs
            fn get_artist_json(&self, _id: &str) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_album_json(&self, _id: &str) -> anyhow::Result<Option<serde_json::Value>> {
                Ok(None)
            }
            fn get_track_json(&self, _id: &str) -> anyhow::Result<Option<serde_json::Value>> {
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
            fn get_image_path(&self, _id: &str) -> std::path::PathBuf {
                std::path::PathBuf::new()
            }
            fn get_track_audio_path(&self, _track_id: &str) -> Option<std::path::PathBuf> {
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
            fn create_artist(&self, _data: serde_json::Value) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn update_artist(
                &self,
                _id: &str,
                _data: serde_json::Value,
            ) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn delete_artist(&self, _id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn create_album(&self, _data: serde_json::Value) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn update_album(
                &self,
                _id: &str,
                _data: serde_json::Value,
            ) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn delete_album(&self, _id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn create_track(&self, _data: serde_json::Value) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn update_track(
                &self,
                _id: &str,
                _data: serde_json::Value,
            ) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn delete_track(&self, _id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn create_image(&self, _data: serde_json::Value) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn update_image(
                &self,
                _id: &str,
                _data: serde_json::Value,
            ) -> anyhow::Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
            fn delete_image(&self, _id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn create_changelog_batch(
                &self,
                _name: &str,
                _description: Option<&str>,
            ) -> anyhow::Result<crate::catalog_store::CatalogBatch> {
                unimplemented!()
            }
            fn get_changelog_batch(
                &self,
                _id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::CatalogBatch>> {
                Ok(None)
            }
            fn get_active_changelog_batch(
                &self,
            ) -> anyhow::Result<Option<crate::catalog_store::CatalogBatch>> {
                Ok(None)
            }
            fn close_changelog_batch(&self, _id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn list_changelog_batches(
                &self,
                _is_open: Option<bool>,
            ) -> anyhow::Result<Vec<crate::catalog_store::CatalogBatch>> {
                Ok(vec![])
            }
            fn delete_changelog_batch(&self, _id: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn get_changelog_batch_changes(
                &self,
                _batch_id: &str,
            ) -> anyhow::Result<Vec<crate::catalog_store::ChangeEntry>> {
                Ok(vec![])
            }
            fn get_changelog_entity_history(
                &self,
                _entity_type: crate::catalog_store::ChangeEntityType,
                _entity_id: &str,
            ) -> anyhow::Result<Vec<crate::catalog_store::ChangeEntry>> {
                Ok(vec![])
            }
            fn get_whats_new_batches(
                &self,
                _limit: usize,
            ) -> anyhow::Result<Vec<crate::catalog_store::WhatsNewBatch>> {
                Ok(vec![])
            }
            fn get_stale_batches(
                &self,
                _stale_threshold_hours: u64,
            ) -> anyhow::Result<Vec<crate::catalog_store::CatalogBatch>> {
                Ok(vec![])
            }
            fn close_stale_batches(&self) -> anyhow::Result<usize> {
                Ok(0)
            }
            fn get_changelog_batch_summary(
                &self,
                _batch_id: &str,
            ) -> anyhow::Result<crate::catalog_store::BatchChangeSummary> {
                Ok(crate::catalog_store::BatchChangeSummary::default())
            }
            fn list_all_track_ids(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn list_all_album_image_ids(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn list_all_artist_image_ids(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn get_artists_without_related(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn get_orphan_related_artist_ids(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn add_artist_image(
                &self,
                _artist_id: &str,
                _image_id: &str,
                _image_type: &crate::catalog_store::ImageType,
                _position: i32,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            fn add_album_image(
                &self,
                _album_id: &str,
                _image_id: &str,
                _image_type: &crate::catalog_store::ImageType,
                _position: i32,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            fn set_artist_display_image(
                &self,
                _artist_id: &str,
                _image_id: &str,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            fn set_album_display_image(
                &self,
                _album_id: &str,
                _image_id: &str,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            fn get_album_display_image_id(
                &self,
                _album_id: &str,
            ) -> anyhow::Result<Option<String>> {
                Ok(None)
            }
            fn get_skeleton_checksum(&self) -> anyhow::Result<String> {
                Ok(String::new())
            }
            fn get_skeleton_events_since(
                &self,
                _seq: i64,
            ) -> anyhow::Result<Vec<crate::skeleton::SkeletonEvent>> {
                Ok(vec![])
            }
            fn get_skeleton_earliest_seq(&self) -> anyhow::Result<i64> {
                Ok(0)
            }
            fn get_skeleton_latest_seq(&self) -> anyhow::Result<i64> {
                Ok(0)
            }
            fn get_all_artist_ids(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec![])
            }
            fn get_all_albums_skeleton(
                &self,
            ) -> anyhow::Result<Vec<crate::skeleton::SkeletonAlbumEntry>> {
                Ok(vec![])
            }
            fn get_all_tracks_skeleton(
                &self,
            ) -> anyhow::Result<Vec<crate::skeleton::SkeletonTrackEntry>> {
                Ok(vec![])
            }
        }

        let catalog = Arc::new(DynamicMockCatalogStore {
            items: RwLock::new(vec![SearchableItem {
                id: "a1".to_string(),
                name: "Original Artist".to_string(),
                content_type: SearchableContentType::Artist,
                additional_text: vec![],
            }]),
            version: std::sync::atomic::AtomicI64::new(1),
        });

        let vault = Fts5LevenshteinSearchVault::new(catalog.clone(), &db_path).unwrap();

        // Initial search works
        let results = vault.search("Original", 10, None);
        assert_eq!(results.len(), 1, "Should find original artist");

        // Change the catalog items and version
        catalog.set_items(vec![SearchableItem {
            id: "a2".to_string(),
            name: "New Artist".to_string(),
            content_type: SearchableContentType::Artist,
            additional_text: vec![],
        }]);
        catalog.set_version(2);

        // Old search still works (index not rebuilt yet)
        let results = vault.search("Original", 10, None);
        assert_eq!(
            results.len(),
            1,
            "Should still find original before rebuild"
        );

        // Trigger rebuild
        vault.rebuild_index().unwrap();

        // Now old item is gone, new item is found
        let results = vault.search("Original", 10, None);
        assert_eq!(
            results.len(),
            0,
            "Original should not be found after rebuild"
        );

        let results = vault.search("New Artist", 10, None);
        assert_eq!(results.len(), 1, "New artist should be found after rebuild");
    }

    #[test]
    fn test_update_popularity() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let items = vec![
            SearchableItem {
                id: "track1".to_string(),
                name: "Popular Song".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
            },
            SearchableItem {
                id: "track2".to_string(),
                name: "Less Popular Song".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
            },
            SearchableItem {
                id: "album1".to_string(),
                name: "Hit Album".to_string(),
                content_type: SearchableContentType::Album,
                additional_text: vec![],
            },
        ];

        let catalog = Arc::new(MockCatalogStore::with_version(items, 1));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Update popularity scores
        let popularity_data = vec![
            ("track1".to_string(), HashedItemType::Track, 1000u64, 1.0),
            ("track2".to_string(), HashedItemType::Track, 500u64, 0.5),
            ("album1".to_string(), HashedItemType::Album, 750u64, 0.75),
        ];

        vault.update_popularity(&popularity_data);

        // Verify the data was written correctly
        let conn = vault.conn.lock().unwrap();

        // Check track1
        let (play_count, score): (i64, f64) = conn
            .query_row(
                "SELECT play_count, score FROM item_popularity WHERE item_id = ? AND item_type = ?",
                ["track1", "track"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("track1 should exist in item_popularity");
        assert_eq!(play_count, 1000);
        assert!((score - 1.0).abs() < 0.001);

        // Check track2
        let (play_count, score): (i64, f64) = conn
            .query_row(
                "SELECT play_count, score FROM item_popularity WHERE item_id = ? AND item_type = ?",
                ["track2", "track"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("track2 should exist in item_popularity");
        assert_eq!(play_count, 500);
        assert!((score - 0.5).abs() < 0.001);

        // Check album1
        let (play_count, score): (i64, f64) = conn
            .query_row(
                "SELECT play_count, score FROM item_popularity WHERE item_id = ? AND item_type = ?",
                ["album1", "album"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("album1 should exist in item_popularity");
        assert_eq!(play_count, 750);
        assert!((score - 0.75).abs() < 0.001);

        // Verify updated_at was set
        let updated_at: i64 = conn
            .query_row(
                "SELECT updated_at FROM item_popularity WHERE item_id = ?",
                ["track1"],
                |row| row.get(0),
            )
            .unwrap();
        assert!(updated_at > 0);
    }

    #[test]
    fn test_update_popularity_replaces_existing() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let catalog = Arc::new(MockCatalogStore::with_version(vec![], 1));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Initial popularity
        vault.update_popularity(&[("track1".to_string(), HashedItemType::Track, 100u64, 0.5)]);

        // Update with new values
        vault.update_popularity(&[("track1".to_string(), HashedItemType::Track, 200u64, 1.0)]);

        // Verify the data was replaced
        let conn = vault.conn.lock().unwrap();
        let (play_count, score): (i64, f64) = conn
            .query_row(
                "SELECT play_count, score FROM item_popularity WHERE item_id = ? AND item_type = ?",
                ["track1", "track"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("track1 should exist");
        assert_eq!(play_count, 200);
        assert!((score - 1.0).abs() < 0.001);

        // Should only have one row
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM item_popularity", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_popular_items_boosted_in_search() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Create three tracks with similar names
        let items = vec![
            SearchableItem {
                id: "track_unpopular".to_string(),
                name: "Song About Love".to_string(),
                content_type: SearchableContentType::Track,
                additional_text: vec![],
            },
            SearchableItem {
                id: "track_popular".to_string(),
                name: "Song About Love".to_string(), // Same name as above
                content_type: SearchableContentType::Track,
                additional_text: vec![],
            },
            SearchableItem {
                id: "track_mid".to_string(),
                name: "Song About Love".to_string(), // Same name as above
                content_type: SearchableContentType::Track,
                additional_text: vec![],
            },
        ];

        let catalog = Arc::new(MockCatalogStore::with_version(items, 1));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Set popularity scores: track_popular = 1.0, track_mid = 0.5, track_unpopular = no entry (0.0)
        vault.update_popularity(&[
            (
                "track_popular".to_string(),
                HashedItemType::Track,
                1000u64,
                1.0,
            ),
            ("track_mid".to_string(), HashedItemType::Track, 500u64, 0.5),
        ]);

        // Search for "Song About Love"
        let results = vault.search("Song About Love", 10, None);

        // All three tracks should be returned
        assert_eq!(results.len(), 3, "Should find all 3 tracks");

        // The popular track should be first (boosted by popularity in SQL ORDER BY)
        assert_eq!(
            results[0].item_id, "track_popular",
            "Most popular track should be ranked first"
        );

        // The mid-popularity track should be second
        assert_eq!(
            results[1].item_id, "track_mid",
            "Mid-popularity track should be ranked second"
        );

        // The unpopular track (no popularity entry) should be last
        assert_eq!(
            results[2].item_id, "track_unpopular",
            "Unpopular track should be ranked last"
        );

        // Note: adjusted_score may be the same due to integer rounding when BM25 scores
        // are very small (e.g., -0.000001). The ordering is still correct because SQL
        // uses floating-point precision in ORDER BY.
    }

    #[test]
    fn test_search_works_with_empty_popularity_table() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let items = vec![SearchableItem {
            id: "track1".to_string(),
            name: "Test Track".to_string(),
            content_type: SearchableContentType::Track,
            additional_text: vec![],
        }];

        let catalog = Arc::new(MockCatalogStore::with_version(items, 1));
        let vault = Fts5LevenshteinSearchVault::new(catalog, &db_path).unwrap();

        // Don't add any popularity data - table is empty
        // Search should still work
        let results = vault.search("Test Track", 10, None);
        assert_eq!(
            results.len(),
            1,
            "Search should work with empty popularity table"
        );
        assert_eq!(results[0].item_id, "track1");
    }
}
