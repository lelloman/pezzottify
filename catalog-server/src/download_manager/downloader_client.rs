//! HTTP client for communicating with the external downloader service.
//!
//! Provides methods for search, metadata retrieval, and binary downloads.

use std::time::Duration;

use anyhow::Result;
use reqwest::Client;

/// Client for communicating with the external downloader service.
///
/// Handles HTTP requests to the downloader service for:
/// - Searching for albums and tracks
/// - Fetching artist discographies
/// - Downloading album/track data and media files
#[derive(Clone)]
pub struct DownloaderClient {
    client: Client,
    base_url: String,
}

impl DownloaderClient {
    /// Create a new DownloaderClient.
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the downloader service (e.g., "http://localhost:8080")
    /// * `timeout_secs` - Request timeout in seconds
    pub fn new(base_url: String, timeout_secs: u64) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()?;

        Ok(Self { client, base_url })
    }

    /// Get the base URL of the downloader service.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Check if the downloader service is reachable.
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_client() {
        let client = DownloaderClient::new("http://localhost:8080".to_string(), 30);
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.base_url(), "http://localhost:8080");
    }
}
