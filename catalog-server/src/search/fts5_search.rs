//! FTS5-based search implementation using SQLite full-text search

use super::{HashedItemType, SearchResult, SearchVault};
use crate::catalog_store::{CatalogStore, SearchableContentType};
use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

/// FTS5 search vault using SQLite's full-text search with trigram tokenizer
pub struct Fts5SearchVault {
    conn: Mutex<Connection>,
    catalog_store: Arc<dyn CatalogStore>,
}

impl Fts5SearchVault {
    /// Create a new FTS5 search vault
    ///
    /// # Arguments
    /// * `catalog_store` - The catalog store to index content from
    /// * `db_path` - Path to the search database file
    pub fn new(catalog_store: Arc<dyn CatalogStore>, db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Enable WAL mode for better concurrent access
        conn.pragma_update(None, "journal_mode", "WAL")?;

        // Create FTS5 virtual table with trigram tokenizer for fuzzy matching
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

        // Build initial index
        Self::rebuild_index_internal(&conn, &catalog_store)?;

        Ok(Self {
            conn: Mutex::new(conn),
            catalog_store,
        })
    }

    fn rebuild_index_internal(
        conn: &Connection,
        catalog_store: &Arc<dyn CatalogStore>,
    ) -> Result<()> {
        // Clear existing data and reindex
        conn.execute("DELETE FROM search_index", [])?;

        // Index content from catalog
        let searchable = catalog_store.get_searchable_content()?;
        let count = searchable.len();

        {
            let mut stmt = conn
                .prepare("INSERT INTO search_index (item_id, item_type, name) VALUES (?, ?, ?)")?;

            for item in searchable {
                let type_str = match item.content_type {
                    SearchableContentType::Artist => "artist",
                    SearchableContentType::Album => "album",
                    SearchableContentType::Track => "track",
                };
                stmt.execute([&item.id, type_str, &item.name])?;
            }
        }

        debug!("FTS5 search index built with {} items", count);
        Ok(())
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
}

impl SearchVault for Fts5SearchVault {
    fn search(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
    ) -> Vec<SearchResult> {
        let conn = self.conn.lock().unwrap();

        // Escape special FTS5 characters and prepare query
        let escaped_query = query.replace('"', "\"\"");

        // Build query with optional type filter
        let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(types) = &filter {
            let type_placeholders: Vec<&str> = types.iter().map(Self::item_type_to_str).collect();
            let placeholders = type_placeholders
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");

            let sql = format!(
                r#"SELECT item_id, item_type, name, bm25(search_index) as score
                   FROM search_index
                   WHERE search_index MATCH ?
                   AND item_type IN ({})
                   ORDER BY score
                   LIMIT ?"#,
                placeholders
            );

            let mut params: Vec<Box<dyn rusqlite::ToSql>> =
                vec![Box::new(format!("\"{}\"", escaped_query))];
            for t in type_placeholders {
                params.push(Box::new(t.to_string()));
            }
            params.push(Box::new(max_results as i64));

            (sql, params)
        } else {
            let sql = r#"SELECT item_id, item_type, name, bm25(search_index) as score
                         FROM search_index
                         WHERE search_index MATCH ?
                         ORDER BY score
                         LIMIT ?"#
                .to_string();

            let params: Vec<Box<dyn rusqlite::ToSql>> = vec![
                Box::new(format!("\"{}\"", escaped_query)),
                Box::new(max_results as i64),
            ];

            (sql, params)
        };

        // Convert params to references for query
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => {
                warn!("FTS5 search query prepare failed: {}", e);
                return Vec::new();
            }
        };

        let results = stmt.query_map(param_refs.as_slice(), |row| {
            let item_id: String = row.get(0)?;
            let item_type_str: String = row.get(1)?;
            let name: String = row.get(2)?;
            let score: f64 = row.get(3)?;

            Ok((item_id, item_type_str, name, score))
        });

        match results {
            Ok(rows) => rows
                .filter_map(|r| r.ok())
                .filter_map(|(item_id, item_type_str, name, score)| {
                    Self::str_to_item_type(&item_type_str).map(|item_type| SearchResult {
                        item_id,
                        item_type,
                        // BM25 scores are negative (more negative = better match)
                        // Convert to positive score where lower is better
                        score: (-score * 1000.0) as u32,
                        adjusted_score: (-score * 1000.0) as i64,
                        matchable_text: name,
                    })
                })
                .collect(),
            Err(e) => {
                warn!("FTS5 search query failed: {}", e);
                Vec::new()
            }
        }
    }

    fn rebuild_index(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        Self::rebuild_index_internal(&conn, &self.catalog_store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::{SearchableContentType, SearchableItem};
    use std::sync::Arc;
    use tempfile::TempDir;

    // A minimal mock that only implements get_searchable_content
    // Using #[cfg(test)] mockall or similar would be cleaner, but for simplicity
    // we use a simple struct with the items we want to index.
    mod mock {
        use super::*;
        use std::path::PathBuf;

        pub struct MockCatalogStore {
            pub items: Vec<SearchableItem>,
        }

        // Implement all CatalogStore methods - only get_searchable_content is meaningful
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
            fn get_album_display_image(
                &self,
                _album_id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::Image>> {
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
                Ok(0)
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
    fn test_basic_search() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        let catalog = Arc::new(MockCatalogStore {
            items: vec![
                SearchableItem {
                    id: "a1".to_string(),
                    name: "The Beatles".to_string(),
                    content_type: SearchableContentType::Artist,
                    additional_text: vec![],
                },
                SearchableItem {
                    id: "a2".to_string(),
                    name: "Abbey Road".to_string(),
                    content_type: SearchableContentType::Album,
                    additional_text: vec![],
                },
                SearchableItem {
                    id: "t1".to_string(),
                    name: "Come Together".to_string(),
                    content_type: SearchableContentType::Track,
                    additional_text: vec![],
                },
            ],
        });

        let vault = Fts5SearchVault::new(catalog, &db_path).unwrap();

        // Search for Beatles
        let results = vault.search("Beatles", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "a1");
        assert_eq!(results[0].item_type, HashedItemType::Artist);

        // Search with filter
        let results = vault.search("Abbey", 10, Some(vec![HashedItemType::Album]));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "a2");
    }

    #[test]
    fn test_rebuild_index() {
        use std::sync::RwLock;

        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("search.db");

        // Create a mock that can have items added dynamically
        struct DynamicMockCatalogStore {
            items: RwLock<Vec<SearchableItem>>,
        }

        impl DynamicMockCatalogStore {
            fn add_item(&self, item: SearchableItem) {
                self.items.write().unwrap().push(item);
            }
        }

        impl CatalogStore for DynamicMockCatalogStore {
            fn get_searchable_content(&self) -> anyhow::Result<Vec<SearchableItem>> {
                Ok(self.items.read().unwrap().clone())
            }
            // All other methods are no-ops
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
            fn get_album_display_image(
                &self,
                _album_id: &str,
            ) -> anyhow::Result<Option<crate::catalog_store::Image>> {
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
            fn get_skeleton_version(&self) -> anyhow::Result<i64> {
                Ok(0)
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
            items: RwLock::new(vec![]),
        });

        let vault = Fts5SearchVault::new(catalog.clone(), &db_path).unwrap();

        // Initially empty
        let results = vault.search("New Artist", 10, None);
        assert_eq!(results.len(), 0);

        // Add item to catalog
        catalog.add_item(SearchableItem {
            id: "new1".to_string(),
            name: "New Artist".to_string(),
            content_type: SearchableContentType::Artist,
            additional_text: vec![],
        });

        // Still empty until rebuild
        let results = vault.search("New Artist", 10, None);
        assert_eq!(results.len(), 0);

        // After rebuild, item is searchable
        vault.rebuild_index().unwrap();
        let results = vault.search("New Artist", 10, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_id, "new1");
    }
}
