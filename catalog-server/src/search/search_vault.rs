//! Search vault trait and result types

use serde::Serialize;

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
}
