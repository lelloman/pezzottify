//! Search vault trait and result types

use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum HashedItemType {
    Track,
    Artist,
    Album,
}

#[derive(Debug, Clone, Eq, Serialize)]
pub struct SearchResult {
    pub item_type: HashedItemType,
    pub item_id: String,
    pub score: u32,
    pub adjusted_score: i64,
    pub matchable_text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchedAlbum {
    pub id: String,
    pub name: String,
    pub artists_ids_names: Vec<(String, String)>,
    pub image_id: Option<String>,
    pub year: Option<i64>,
    pub availability: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchedArtist {
    pub id: String,
    pub name: String,
    pub image_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchedTrack {
    pub id: String,
    pub name: String,
    pub duration: u32,
    pub artists_ids_names: Vec<(String, String)>,
    pub image_id: Option<String>,
    pub album_id: String,
    pub availability: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ResolvedSearchResult {
    Track(SearchedTrack),
    Album(SearchedAlbum),
    Artist(SearchedArtist),
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.adjusted_score == other.adjusted_score
    }
}

impl std::cmp::Ord for SearchResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.adjusted_score.cmp(&other.adjusted_score)
    }
}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Item to add to the search index.
#[derive(Debug, Clone)]
pub struct SearchIndexItem {
    pub id: String,
    pub name: String,
    pub item_type: HashedItemType,
}

pub trait SearchVault: Send + Sync {
    fn search(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
    ) -> Vec<SearchResult>;

    /// Rebuild the entire search index from the catalog.
    ///
    /// This should be called when the catalog changes (e.g., when a batch is closed).
    /// Returns an error if the rebuild fails.
    fn rebuild_index(&self) -> anyhow::Result<()>;

    /// Add or update items in the search index.
    ///
    /// If an item with the same ID already exists, it will be updated.
    /// This also updates the vocabulary for typo correction.
    fn upsert_items(&self, items: &[SearchIndexItem]) -> anyhow::Result<()>;

    /// Remove items from the search index.
    ///
    /// Items are identified by their ID and type.
    fn remove_items(&self, items: &[(String, HashedItemType)]) -> anyhow::Result<()>;

    /// Update popularity scores for items.
    ///
    /// This allows search results to be boosted based on listening history.
    /// Items with higher popularity scores will rank higher in search results.
    ///
    /// # Arguments
    /// * `items` - Slice of (item_id, item_type, play_count, normalized_score) tuples
    ///   - `item_id`: The unique identifier of the item
    ///   - `item_type`: The type of item (Track, Album, Artist)
    ///   - `play_count`: Raw play count for analytics
    ///   - `normalized_score`: Score normalized 0.0-1.0 within each item type
    fn update_popularity(&self, items: &[(String, HashedItemType, u64, f64)]);

    /// Get statistics about the search index.
    fn get_stats(&self) -> SearchVaultStats;

    /// Record an impression (page view) for an item.
    /// Increments today's impression count for the given item.
    fn record_impression(&self, item_id: &str, item_type: HashedItemType);

    /// Get total impressions for all items within a date range.
    /// Returns a map of (item_id, item_type) -> total impression count.
    ///
    /// # Arguments
    /// * `min_date` - Minimum date in YYYYMMDD format
    fn get_impression_totals(
        &self,
        min_date: i64,
    ) -> std::collections::HashMap<(String, HashedItemType), u64>;

    /// Prune old impression records.
    /// Deletes records older than the specified date.
    ///
    /// # Arguments
    /// * `before_date` - Date threshold in YYYYMMDD format
    ///
    /// # Returns
    /// Number of records deleted
    fn prune_impressions(&self, before_date: i64) -> usize;
}

/// Statistics about the search vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchVaultStats {
    /// Number of items in the search index
    pub indexed_items: usize,
    /// Type of search index (e.g., "FTS5+Levenshtein")
    pub index_type: String,
    /// Current indexing state
    pub state: IndexState,
}

/// State of the search index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "status")]
pub enum IndexState {
    /// Index is empty/not started
    #[default]
    Empty,
    /// Index is being built
    Building {
        /// Number of items processed so far
        processed: usize,
        /// Total items to process (if known)
        total: Option<usize>,
    },
    /// Index is ready for queries
    Ready,
    /// Index build failed
    Failed { error: String },
}

/// A no-op search vault that returns empty results.
/// Used for fast startup when search is not needed.
pub struct NoopSearchVault;

impl SearchVault for NoopSearchVault {
    fn search(
        &self,
        _query: &str,
        _max_results: usize,
        _filter: Option<Vec<HashedItemType>>,
    ) -> Vec<SearchResult> {
        Vec::new()
    }

    fn rebuild_index(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn upsert_items(&self, _items: &[SearchIndexItem]) -> anyhow::Result<()> {
        Ok(())
    }

    fn remove_items(&self, _items: &[(String, HashedItemType)]) -> anyhow::Result<()> {
        Ok(())
    }

    fn update_popularity(&self, _items: &[(String, HashedItemType, u64, f64)]) {}

    fn get_stats(&self) -> SearchVaultStats {
        SearchVaultStats {
            indexed_items: 0,
            index_type: "Noop (disabled)".to_string(),
            state: IndexState::Ready,
        }
    }

    fn record_impression(&self, _item_id: &str, _item_type: HashedItemType) {}

    fn get_impression_totals(
        &self,
        _min_date: i64,
    ) -> std::collections::HashMap<(String, HashedItemType), u64> {
        std::collections::HashMap::new()
    }

    fn prune_impressions(&self, _before_date: i64) -> usize {
        0
    }
}

/// Implement SearchVault for Arc<T> to allow shared ownership with background tasks.
impl<T: SearchVault + ?Sized> SearchVault for std::sync::Arc<T> {
    fn search(
        &self,
        query: &str,
        max_results: usize,
        filter: Option<Vec<HashedItemType>>,
    ) -> Vec<SearchResult> {
        (**self).search(query, max_results, filter)
    }

    fn rebuild_index(&self) -> anyhow::Result<()> {
        (**self).rebuild_index()
    }

    fn upsert_items(&self, items: &[SearchIndexItem]) -> anyhow::Result<()> {
        (**self).upsert_items(items)
    }

    fn remove_items(&self, items: &[(String, HashedItemType)]) -> anyhow::Result<()> {
        (**self).remove_items(items)
    }

    fn update_popularity(&self, items: &[(String, HashedItemType, u64, f64)]) {
        (**self).update_popularity(items)
    }

    fn get_stats(&self) -> SearchVaultStats {
        (**self).get_stats()
    }

    fn record_impression(&self, item_id: &str, item_type: HashedItemType) {
        (**self).record_impression(item_id, item_type)
    }

    fn get_impression_totals(
        &self,
        min_date: i64,
    ) -> std::collections::HashMap<(String, HashedItemType), u64> {
        (**self).get_impression_totals(min_date)
    }

    fn prune_impressions(&self, before_date: i64) -> usize {
        (**self).prune_impressions(before_date)
    }
}
