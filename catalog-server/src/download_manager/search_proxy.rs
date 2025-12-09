//! Search proxy for querying the downloader service.
//!
//! Provides search functionality with catalog/queue overlap detection.
//! The SearchProxy delegates to the DownloaderClient for external API calls,
//! then enriches results with `in_catalog` and `in_queue` flags.

use std::sync::Arc;

use anyhow::Result;

use crate::catalog_store::CatalogStore;

use super::downloader_client::DownloaderClient;
use super::models::*;
use super::queue_store::DownloadQueueStore;

/// Search proxy that enriches downloader search results with local state.
///
/// For each search result, adds:
/// - `in_catalog`: Whether the content already exists in our catalog
/// - `in_queue`: Whether the content is currently in the download queue
pub struct SearchProxy {
    /// HTTP client for the external downloader service.
    #[allow(dead_code)]
    downloader_client: DownloaderClient,
    /// Catalog store for checking existing content.
    #[allow(dead_code)]
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
    pub async fn search(&self, _query: &str, _search_type: SearchType) -> Result<SearchResults> {
        // TODO: Implement in DM-2.2.1
        // 1. Call downloader service search endpoint via downloader_client
        // 2. For each result, check in_catalog via catalog_store
        // 3. For each result, check in_queue via queue_store
        // 4. Return enriched results
        todo!("search proxy not yet implemented - requires DM-2.1.1 and DM-2.2.1")
    }

    /// Search for an artist's discography via the external downloader service.
    ///
    /// # Arguments
    /// * `artist_id` - External artist ID from the music provider
    pub async fn search_discography(&self, _artist_id: &str) -> Result<DiscographyResult> {
        // TODO: Implement in DM-2.2.2
        // 1. Call downloader service discography endpoint via downloader_client
        // 2. For the artist, check in_catalog via catalog_store
        // 3. For each album, check in_catalog and in_queue
        // 4. Return enriched results
        todo!("search_discography proxy not yet implemented - requires DM-2.1.1 and DM-2.2.2")
    }

    /// Check if an album exists in the catalog.
    #[allow(dead_code)]
    fn check_album_in_catalog(&self, _album_id: &str) -> Result<bool> {
        // TODO: Implement catalog lookup
        // Note: May need to convert external ID format to internal ID
        Ok(false)
    }

    /// Check if an artist exists in the catalog.
    #[allow(dead_code)]
    fn check_artist_in_catalog(&self, _artist_id: &str) -> Result<bool> {
        // TODO: Implement catalog lookup
        Ok(false)
    }

    /// Check if content is in the download queue.
    #[allow(dead_code)]
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
