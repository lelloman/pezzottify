//! Search proxy for querying the downloader service.
//!
//! Provides search functionality with catalog/queue overlap detection.
//! The SearchProxy delegates to the DownloaderClient for external API calls,
//! then enriches results with `in_catalog` and `in_queue` flags.

use std::sync::Arc;

use anyhow::Result;

use crate::catalog_store::CatalogStore;

use super::downloader_client::DownloaderClient;
use super::downloader_types::ExternalSearchResult;
use super::models::*;
use super::queue_store::DownloadQueueStore;

/// Search proxy that enriches downloader search results with local state.
///
/// For each search result, adds:
/// - `in_catalog`: Whether the content already exists in our catalog
/// - `in_queue`: Whether the content is currently in the download queue
pub struct SearchProxy {
    /// HTTP client for the external downloader service.
    downloader_client: DownloaderClient,
    /// Catalog store for checking existing content.
    #[allow(dead_code)] // Will be used when external ID tracking is added
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
    /// with `in_catalog` and `in_queue` flags.
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `search_type` - Type of content to search for (Album or Artist)
    pub async fn search(&self, query: &str, search_type: SearchType) -> Result<SearchResults> {
        // 1. Call downloader service search endpoint
        let external_results = self.downloader_client.search(query, search_type).await?;

        // 2. Convert and enrich each result
        let total = external_results.len();
        let results = external_results
            .into_iter()
            .map(|ext| self.enrich_search_result(ext, search_type))
            .collect();

        Ok(SearchResults { results, total })
    }

    /// Convert an external search result to an enriched internal search result.
    fn enrich_search_result(
        &self,
        external: ExternalSearchResult,
        search_type: SearchType,
    ) -> SearchResult {
        // Determine content type for queue lookup
        let content_type = match search_type {
            SearchType::Album => DownloadContentType::Album,
            SearchType::Artist => DownloadContentType::Album, // Artists don't have a direct queue type
        };

        // Check if in queue (albums only, artists don't go directly in queue)
        let in_queue = if search_type == SearchType::Album {
            self.check_in_queue(content_type, &external.id).unwrap_or(false)
        } else {
            false
        };

        // Check if in catalog
        let in_catalog = match search_type {
            SearchType::Album => self.check_album_in_catalog(&external.id).unwrap_or(false),
            SearchType::Artist => self.check_artist_in_catalog(&external.id).unwrap_or(false),
        };

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
            .with_in_queue(in_queue),
            SearchType::Artist => SearchResult::artist(external.id, external.name, external.image_url)
                .with_in_catalog(in_catalog)
                .with_in_queue(in_queue),
        }
    }

    /// Search for an artist's discography via the external downloader service.
    ///
    /// # Arguments
    /// * `artist_id` - External artist ID from the music provider
    pub async fn search_discography(&self, artist_id: &str) -> Result<DiscographyResult> {
        // 1. Call downloader service discography endpoint
        let external_disco = self.downloader_client.get_discography(artist_id).await?;

        // 2. Enrich the artist result
        let artist = self.enrich_search_result(external_disco.artist, SearchType::Artist);

        // 3. Enrich each album result
        let albums = external_disco
            .albums
            .into_iter()
            .map(|ext| self.enrich_search_result(ext, SearchType::Album))
            .collect();

        Ok(DiscographyResult { artist, albums })
    }

    /// Check if an album exists in the catalog by external ID.
    ///
    /// Note: Currently returns false as catalog doesn't track external IDs.
    /// Future implementation may add external ID mapping to catalog store.
    fn check_album_in_catalog(&self, _album_id: &str) -> Result<bool> {
        // TODO: Implement catalog lookup when external ID tracking is added
        // This requires mapping external IDs to internal catalog IDs
        Ok(false)
    }

    /// Check if an artist exists in the catalog by external ID.
    ///
    /// Note: Currently returns false as catalog doesn't track external IDs.
    fn check_artist_in_catalog(&self, _artist_id: &str) -> Result<bool> {
        // TODO: Implement catalog lookup when external ID tracking is added
        Ok(false)
    }

    /// Check if content is in the download queue.
    fn check_in_queue(&self, content_type: DownloadContentType, content_id: &str) -> Result<bool> {
        self.queue_store.is_in_active_queue(content_type, content_id)
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
}
