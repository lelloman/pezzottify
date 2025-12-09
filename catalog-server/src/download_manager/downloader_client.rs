//! HTTP client for communicating with the external downloader service.
//!
//! Provides methods for search, metadata retrieval, and binary downloads.

use std::time::Duration;

use anyhow::{anyhow, Result};
use reqwest::Client;

use super::downloader_types::*;
use super::models::SearchType;

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

    // =========================================================================
    // Health Check
    // =========================================================================

    /// Check if the downloader service is reachable.
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Get the current status of the downloader service.
    pub async fn get_status(&self) -> Result<crate::downloader::models::DownloaderStatus> {
        let url = format!("{}/status", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Downloader status request failed with status: {}",
                response.status()
            ));
        }

        let status = response.json().await?;
        Ok(status)
    }

    // =========================================================================
    // Search Endpoints
    // =========================================================================

    /// Search for content via the downloader service.
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `search_type` - Type of content to search for
    pub async fn search(
        &self,
        query: &str,
        search_type: SearchType,
    ) -> Result<Vec<ExternalSearchResult>> {
        let type_param = match search_type {
            SearchType::Album => "album",
            SearchType::Artist => "artist",
        };

        let url = format!("{}/search", self.base_url);
        let response = self
            .client
            .get(&url)
            .query(&[("q", query), ("type", type_param)])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Search request failed with status: {}",
                response.status()
            ));
        }

        let search_response: SearchResponse = response.json().await?;
        Ok(search_response.results)
    }

    /// Get an artist's discography from the downloader service.
    ///
    /// # Arguments
    /// * `artist_id` - External artist ID
    pub async fn get_discography(&self, artist_id: &str) -> Result<ExternalDiscographyResult> {
        let url = format!("{}/artist/{}/discography", self.base_url, artist_id);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Discography request failed with status: {}",
                response.status()
            ));
        }

        let discography: ExternalDiscographyResult = response.json().await?;
        Ok(discography)
    }

    // =========================================================================
    // Metadata Endpoints
    // =========================================================================

    /// Get album metadata from the downloader service.
    ///
    /// # Arguments
    /// * `album_id` - External album ID
    pub async fn get_album(&self, album_id: &str) -> Result<ExternalAlbum> {
        let url = format!("{}/album/{}", self.base_url, album_id);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Album metadata request failed with status: {}",
                response.status()
            ));
        }

        let album: ExternalAlbum = response.json().await?;
        Ok(album)
    }

    /// Get tracks for an album from the downloader service.
    ///
    /// # Arguments
    /// * `album_id` - External album ID
    pub async fn get_album_tracks(&self, album_id: &str) -> Result<Vec<ExternalTrack>> {
        let url = format!("{}/album/{}/tracks", self.base_url, album_id);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Album tracks request failed with status: {}",
                response.status()
            ));
        }

        let tracks: Vec<ExternalTrack> = response.json().await?;
        Ok(tracks)
    }

    /// Get artist metadata from the downloader service.
    ///
    /// # Arguments
    /// * `artist_id` - External artist ID
    pub async fn get_artist(&self, artist_id: &str) -> Result<ExternalArtist> {
        let url = format!("{}/artist/{}", self.base_url, artist_id);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Artist metadata request failed with status: {}",
                response.status()
            ));
        }

        let artist: ExternalArtist = response.json().await?;
        Ok(artist)
    }

    /// Get track metadata from the downloader service.
    ///
    /// # Arguments
    /// * `track_id` - External track ID
    pub async fn get_track(&self, track_id: &str) -> Result<ExternalTrack> {
        let url = format!("{}/track/{}", self.base_url, track_id);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Track metadata request failed with status: {}",
                response.status()
            ));
        }

        let track: ExternalTrack = response.json().await?;
        Ok(track)
    }

    // =========================================================================
    // Download Endpoints
    // =========================================================================

    /// Download track audio from the downloader service.
    ///
    /// Returns the audio bytes and content type (e.g., "audio/flac").
    ///
    /// # Arguments
    /// * `track_id` - External track ID
    pub async fn download_track_audio(&self, track_id: &str) -> Result<(Vec<u8>, String)> {
        let url = format!("{}/track/{}/audio", self.base_url, track_id);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Track audio download failed with status: {}",
                response.status()
            ));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("audio/flac")
            .to_string();

        let bytes = response.bytes().await?.to_vec();
        Ok((bytes, content_type))
    }

    /// Download an image from the downloader service.
    ///
    /// # Arguments
    /// * `image_id` - Hex image ID
    pub async fn download_image(&self, image_id: &str) -> Result<Vec<u8>> {
        let url = format!("{}/image/{}", self.base_url, image_id);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Image download failed with status: {}",
                response.status()
            ));
        }

        let bytes = response.bytes().await?.to_vec();
        Ok(bytes)
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

    #[test]
    fn test_new_client_with_trailing_slash() {
        let client = DownloaderClient::new("http://localhost:8080/".to_string(), 30).unwrap();
        // Note: trailing slash is preserved, which may need to be handled by callers
        assert_eq!(client.base_url(), "http://localhost:8080/");
    }
}
