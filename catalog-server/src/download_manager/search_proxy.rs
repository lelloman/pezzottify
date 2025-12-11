//! Search proxy for querying the downloader service.
//!
//! Provides search functionality with catalog/queue overlap detection.
//! The SearchProxy delegates to the DownloaderClient for external API calls,
//! then enriches results with `in_catalog`, `in_queue` flags and relevance scores.

use std::sync::Arc;

use anyhow::Result;

use crate::catalog_store::CatalogStore;

use super::downloader_client::DownloaderClient;
use super::downloader_types::ExternalSearchResult;
use super::models::*;
use super::queue_store::DownloadQueueStore;

// =============================================================================
// Relevance Scoring
// =============================================================================

/// Compute Levenshtein distance between two strings.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    // Use two rows instead of full matrix for memory efficiency
    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row: Vec<usize> = vec![0; b_len + 1];

    for i in 1..=a_len {
        curr_row[0] = i;
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr_row[j] = (prev_row[j] + 1) // deletion
                .min(curr_row[j - 1] + 1) // insertion
                .min(prev_row[j - 1] + cost); // substitution
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

/// Compute relevance score for a search result against a query.
/// Returns a score between 0.0 and 1.0 (higher is better).
fn compute_relevance_score(query: &str, name: &str, artist_name: Option<&str>) -> f32 {
    let query_lower = query.to_lowercase();
    let name_lower = name.to_lowercase();

    // Exact match is perfect score
    if name_lower == query_lower {
        return 1.0;
    }

    // Start with Levenshtein-based score for the name
    let max_len = query_lower.len().max(name_lower.len());
    let distance = levenshtein_distance(&query_lower, &name_lower);
    let lev_score = if max_len > 0 {
        1.0 - (distance as f32 / max_len as f32)
    } else {
        0.0
    };

    // Bonus for "starts with" match
    let starts_with_bonus = if name_lower.starts_with(&query_lower) {
        0.2
    } else {
        0.0
    };

    // Bonus for "contains" match
    let contains_bonus = if name_lower.contains(&query_lower) {
        0.1
    } else {
        0.0
    };

    // Artist name bonus (for albums)
    let artist_bonus = if let Some(artist) = artist_name {
        let artist_lower = artist.to_lowercase();
        if artist_lower == query_lower {
            0.3 // Artist exact match
        } else if artist_lower.contains(&query_lower) || query_lower.contains(&artist_lower) {
            0.15 // Partial artist match
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Combine scores, capped at 1.0
    (lev_score + starts_with_bonus + contains_bonus + artist_bonus).min(1.0)
}

/// Search proxy that enriches downloader search results with local state.
///
/// For each search result, adds:
/// - `in_catalog`: Whether the content already exists in our catalog
/// - `in_queue`: Whether the content is currently in the download queue
pub struct SearchProxy {
    /// HTTP client for the external downloader service.
    downloader_client: DownloaderClient,
    /// Catalog store for checking existing content.
    catalog_store: Arc<dyn CatalogStore>,
    /// Queue store for checking pending downloads.
    queue_store: Arc<dyn DownloadQueueStore>,
}

impl SearchProxy {
    /// Create a new SearchProxy.
    pub fn new(
        downloader_client: DownloaderClient,
        catalog_store: Arc<dyn CatalogStore>,
        queue_store: Arc<dyn DownloadQueueStore>,
    ) -> Self {
        Self {
            downloader_client,
            catalog_store,
            queue_store,
        }
    }

    /// Search for content via the external downloader service.
    ///
    /// Forwards the search request to the downloader and enriches results
    /// with `in_catalog`, `in_queue` flags and relevance scores.
    /// Results are sorted by score (highest first).
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `search_type` - Type of content to search for (Album or Artist)
    pub async fn search(&self, query: &str, search_type: SearchType) -> Result<SearchResults> {
        // 1. Call downloader service search endpoint
        let external_results = self.downloader_client.search(query, search_type).await?;

        // 2. Convert and enrich each result with scores
        let total = external_results.len();
        let mut results: Vec<SearchResult> = external_results
            .into_iter()
            .map(|ext| self.enrich_search_result(ext, search_type, query))
            .collect();

        // 3. Sort by score (highest first)
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(SearchResults { results, total })
    }

    /// Convert an external search result to an enriched internal search result.
    fn enrich_search_result(
        &self,
        external: ExternalSearchResult,
        search_type: SearchType,
        query: &str,
    ) -> SearchResult {
        // Determine content type for queue lookup
        let content_type = match search_type {
            SearchType::Album => DownloadContentType::Album,
            SearchType::Artist => DownloadContentType::Album, // Artists don't have a direct queue type
        };

        // Check if in queue (albums only, artists don't go directly in queue)
        let in_queue = if search_type == SearchType::Album {
            self.check_in_queue(content_type, &external.id)
                .unwrap_or(false)
        } else {
            false
        };

        // Check if in catalog
        let in_catalog = match search_type {
            SearchType::Album => self.check_album_in_catalog(&external.id).unwrap_or(false),
            SearchType::Artist => self.check_artist_in_catalog(&external.id).unwrap_or(false),
        };

        // Compute relevance score
        let score = compute_relevance_score(query, &external.name, external.artist_name.as_deref());

        // Create the search result based on type
        match search_type {
            SearchType::Album => SearchResult::album(
                external.id,
                external.name,
                external.artist_name.unwrap_or_default(),
                external.year,
                external.image_url,
            )
            .with_in_catalog(in_catalog)
            .with_in_queue(in_queue)
            .with_score(score),
            SearchType::Artist => {
                SearchResult::artist(external.id, external.name, external.image_url)
                    .with_in_catalog(in_catalog)
                    .with_in_queue(in_queue)
                    .with_score(score)
            }
        }
    }

    /// Search for an artist's discography via the external downloader service.
    ///
    /// # Arguments
    /// * `artist_id` - External artist ID from the music provider
    pub async fn search_discography(&self, artist_id: &str) -> Result<DiscographyResult> {
        // 1. Call downloader service discography endpoint
        let external_disco = self.downloader_client.get_discography(artist_id).await?;

        // 2. Enrich the artist result (no query for discography, score is irrelevant)
        let artist = self.enrich_search_result(external_disco.artist, SearchType::Artist, "");

        // 3. Enrich each album result with catalog status, queue status, and request status
        let albums = external_disco
            .albums
            .into_iter()
            .map(|ext| {
                let album_id = ext.id.clone();
                let mut result = self.enrich_search_result(ext, SearchType::Album, "");
                // Add request status if album is in queue
                if result.in_queue {
                    result.request_status = self.get_request_status_for_album(&album_id).ok().flatten();
                }
                result
            })
            .collect();

        Ok(DiscographyResult { artist, albums })
    }

    /// Get detailed information about an external album.
    ///
    /// Fetches album metadata and tracks from the downloader service,
    /// then enriches with catalog and queue status.
    ///
    /// # Arguments
    /// * `album_id` - External album ID from the music provider
    pub async fn get_album_details(&self, album_id: &str) -> Result<ExternalAlbumDetails> {
        // 1. Fetch album metadata
        let album = self.downloader_client.get_album(album_id).await?;

        // 2. Fetch album tracks
        let tracks = self.downloader_client.get_album_tracks(album_id).await?;

        // 3. Get the primary artist name
        let artist_id = album.artists_ids.first().cloned().unwrap_or_default();
        let artist_name = if !artist_id.is_empty() {
            // Try to get artist name from downloader
            match self.downloader_client.get_artist(&artist_id).await {
                Ok(artist) => artist.name,
                Err(_) => String::new(),
            }
        } else {
            String::new()
        };

        // 4. Get year from album date
        let year = if album.date > 0 {
            Some(
                chrono::DateTime::from_timestamp(album.date, 0)
                    .map(|dt| dt.format("%Y").to_string().parse::<i32>().unwrap_or(0))
                    .unwrap_or(0),
            )
            .filter(|&y| y > 0)
        } else {
            None
        };

        // 5. Get image URL from covers
        let image_url = album
            .covers
            .iter()
            .chain(album.cover_group.iter())
            .find(|img| img.size == "large" || img.size == "xlarge")
            .or_else(|| album.covers.first())
            .or_else(|| album.cover_group.first())
            .map(|img| format!("{}/image/{}", self.downloader_client.base_url(), img.id));

        // 6. Convert tracks to ExternalTrackInfo
        let track_infos: Vec<ExternalTrackInfo> = tracks
            .iter()
            .map(|t| ExternalTrackInfo {
                id: t.id.clone(),
                name: t.name.clone(),
                track_number: t.number,
                disc_number: Some(t.disc_number),
                duration_ms: Some(t.duration),
            })
            .collect();

        // 7. Check if album is in catalog
        let in_catalog = self.check_album_in_catalog(album_id).unwrap_or(false);

        // 8. Get request status if in queue
        let request_status = self.get_request_status_for_album(album_id)?;

        Ok(ExternalAlbumDetails {
            id: album.id,
            name: album.name,
            artist_id,
            artist_name,
            image_url,
            year,
            album_type: Some(album.album_type),
            total_tracks: track_infos.len() as i32,
            tracks: track_infos,
            in_catalog,
            request_status,
        })
    }

    /// Get request status for an album if it's in the download queue.
    fn get_request_status_for_album(&self, album_id: &str) -> Result<Option<RequestStatusInfo>> {
        // Find the queue item for this album
        let item = self
            .queue_store
            .find_by_content(DownloadContentType::Album, album_id)?;

        match item {
            Some(item) => {
                // Get queue position for pending items
                let queue_position = if item.status == QueueStatus::Pending {
                    self.queue_store.get_queue_position(&item.id)?
                } else {
                    None
                };

                // Get progress for album downloads
                let progress = self
                    .queue_store
                    .get_children_progress(&item.id)
                    .ok()
                    .filter(|p| p.total_children > 0);

                Ok(Some(RequestStatusInfo::from_queue_item(
                    &item,
                    queue_position,
                    progress,
                )))
            }
            None => Ok(None),
        }
    }

    /// Check if an album exists in the catalog by external ID.
    ///
    /// Albums ingested via the downloader use the external ID as their catalog ID.
    fn check_album_in_catalog(&self, album_id: &str) -> Result<bool> {
        match self.catalog_store.get_album_json(album_id) {
            Ok(Some(_)) => Ok(true),
            _ => Ok(false),
        }
    }

    /// Check if an artist exists in the catalog by external ID.
    ///
    /// Artists ingested via the downloader use the external ID as their catalog ID.
    fn check_artist_in_catalog(&self, artist_id: &str) -> Result<bool> {
        match self.catalog_store.get_artist_json(artist_id) {
            Ok(Some(_)) => Ok(true),
            _ => Ok(false),
        }
    }

    /// Check if content is in the download queue.
    fn check_in_queue(&self, content_type: DownloadContentType, content_id: &str) -> Result<bool> {
        self.queue_store
            .is_in_active_queue(content_type, content_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::SqliteCatalogStore;
    use crate::download_manager::SqliteDownloadQueueStore;
    use tempfile::TempDir;

    fn create_test_proxy() -> (SearchProxy, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let catalog_db_path = temp_dir.path().join("catalog.db");
        let catalog_store =
            Arc::new(SqliteCatalogStore::new(&catalog_db_path, temp_dir.path()).unwrap());
        let queue_store = Arc::new(SqliteDownloadQueueStore::in_memory().unwrap());
        let downloader_client =
            DownloaderClient::new("http://localhost:8080".to_string(), 30).unwrap();

        let proxy = SearchProxy::new(downloader_client, catalog_store, queue_store);
        (proxy, temp_dir)
    }

    #[test]
    fn test_new_proxy() {
        let (_proxy, _temp_dir) = create_test_proxy();
        // Just verify construction works
    }

    #[test]
    fn test_check_in_queue_empty() {
        let (proxy, _temp_dir) = create_test_proxy();

        let result = proxy
            .check_in_queue(DownloadContentType::Album, "album-123")
            .unwrap();

        assert!(!result);
    }

    // =========================================================================
    // Levenshtein Distance Tests
    // =========================================================================

    #[test]
    fn test_levenshtein_identical_strings() {
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
        assert_eq!(levenshtein_distance("", ""), 0);
    }

    #[test]
    fn test_levenshtein_empty_string() {
        assert_eq!(levenshtein_distance("", "hello"), 5);
        assert_eq!(levenshtein_distance("hello", ""), 5);
    }

    #[test]
    fn test_levenshtein_single_edit() {
        // Substitution
        assert_eq!(levenshtein_distance("cat", "bat"), 1);
        // Insertion
        assert_eq!(levenshtein_distance("cat", "cats"), 1);
        // Deletion
        assert_eq!(levenshtein_distance("cats", "cat"), 1);
    }

    #[test]
    fn test_levenshtein_multiple_edits() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("saturday", "sunday"), 3);
    }

    // =========================================================================
    // Relevance Scoring Tests
    // =========================================================================

    #[test]
    fn test_relevance_exact_match() {
        let score = compute_relevance_score("purple rain", "Purple Rain", None);
        assert!(
            (score - 1.0).abs() < 0.001,
            "Exact match should be 1.0, got {}",
            score
        );
    }

    #[test]
    fn test_relevance_starts_with_bonus() {
        let score_starts = compute_relevance_score("purple", "Purple Rain", None);
        let score_contains = compute_relevance_score("rain", "Purple Rain", None);

        // "Purple" starts with query, so should score higher than "Rain" which only contains
        assert!(
            score_starts > score_contains,
            "Starts-with should score higher: {} vs {}",
            score_starts,
            score_contains
        );
    }

    #[test]
    fn test_relevance_artist_bonus() {
        let score_with_artist = compute_relevance_score("prince", "Purple Rain", Some("Prince"));
        let score_without_artist = compute_relevance_score("prince", "Purple Rain", None);

        // Artist match should boost score
        assert!(
            score_with_artist > score_without_artist,
            "Artist match should boost: {} vs {}",
            score_with_artist,
            score_without_artist
        );
    }

    #[test]
    fn test_relevance_case_insensitive() {
        let score1 = compute_relevance_score("PURPLE", "purple", None);
        let score2 = compute_relevance_score("purple", "PURPLE", None);

        assert!(
            (score1 - score2).abs() < 0.001,
            "Scores should be equal regardless of case"
        );
    }

    #[test]
    fn test_relevance_completely_different() {
        let score = compute_relevance_score("xyz", "abcdefgh", None);

        // Completely different strings should have low score
        assert!(
            score < 0.5,
            "Completely different strings should score low: {}",
            score
        );
    }
}
