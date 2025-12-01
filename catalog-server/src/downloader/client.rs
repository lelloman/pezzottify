//! HTTP client for the external downloader service.

use anyhow::{Context, Result};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use super::models::{DownloaderAlbum, DownloaderArtist, DownloaderTrack};
use crate::server::metrics;

/// HTTP client for communicating with the downloader service.
pub struct DownloaderClient {
    client: reqwest::Client,
    base_url: String,
}

impl DownloaderClient {
    /// Create a new downloader client.
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the downloader service (e.g., "http://localhost:8080")
    /// * `timeout_sec` - Request timeout in seconds
    pub fn new(base_url: String, timeout_sec: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_sec))
            .build()
            .expect("Failed to create HTTP client");

        // Ensure base_url doesn't have trailing slash
        let base_url = base_url.trim_end_matches('/').to_string();

        Self { client, base_url }
    }

    /// Check if the downloader service is healthy.
    pub async fn health_check(&self) -> Result<()> {
        let start = Instant::now();
        let url = format!("{}/health", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                metrics::record_downloader_error("health_check", "connection");
                e
            })
            .context("Failed to connect to downloader service")?;

        if response.status().is_success() {
            metrics::record_downloader_request("health_check", start.elapsed());
            Ok(())
        } else {
            metrics::record_downloader_error("health_check", "status");
            anyhow::bail!(
                "Downloader health check failed with status: {}",
                response.status()
            )
        }
    }

    /// Get artist metadata from the downloader.
    pub async fn get_artist(&self, id: &str) -> Result<DownloaderArtist> {
        let start = Instant::now();
        let url = format!("{}/artist/{}", self.base_url, id);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                metrics::record_downloader_error("get_artist", "connection");
                e
            })
            .context("Failed to fetch artist from downloader")?;

        if !response.status().is_success() {
            metrics::record_downloader_error("get_artist", "status");
            anyhow::bail!(
                "Failed to fetch artist {}: status {}",
                id,
                response.status()
            );
        }

        let result = response
            .json()
            .await
            .map_err(|e| {
                metrics::record_downloader_error("get_artist", "parse");
                e
            })
            .context("Failed to parse artist response")?;

        metrics::record_downloader_request("get_artist", start.elapsed());
        Ok(result)
    }

    /// Get album metadata from the downloader.
    pub async fn get_album(&self, id: &str) -> Result<DownloaderAlbum> {
        let start = Instant::now();
        let url = format!("{}/album/{}", self.base_url, id);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                metrics::record_downloader_error("get_album", "connection");
                e
            })
            .context("Failed to fetch album from downloader")?;

        if !response.status().is_success() {
            metrics::record_downloader_error("get_album", "status");
            anyhow::bail!(
                "Failed to fetch album {}: status {}",
                id,
                response.status()
            );
        }

        let result = response
            .json()
            .await
            .map_err(|e| {
                metrics::record_downloader_error("get_album", "parse");
                e
            })
            .context("Failed to parse album response")?;

        metrics::record_downloader_request("get_album", start.elapsed());
        Ok(result)
    }

    /// Get track metadata from the downloader.
    pub async fn get_track(&self, id: &str) -> Result<DownloaderTrack> {
        let start = Instant::now();
        let url = format!("{}/track/{}", self.base_url, id);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                metrics::record_downloader_error("get_track", "connection");
                e
            })
            .context("Failed to fetch track from downloader")?;

        if !response.status().is_success() {
            metrics::record_downloader_error("get_track", "status");
            anyhow::bail!(
                "Failed to fetch track {}: status {}",
                id,
                response.status()
            );
        }

        let result = response
            .json()
            .await
            .map_err(|e| {
                metrics::record_downloader_error("get_track", "parse");
                e
            })
            .context("Failed to parse track response")?;

        metrics::record_downloader_request("get_track", start.elapsed());
        Ok(result)
    }

    /// Download track audio to a file.
    ///
    /// Creates parent directories if they don't exist.
    pub async fn download_track_audio(&self, id: &str, dest: &Path) -> Result<u64> {
        let start = Instant::now();
        let url = format!("{}/track/{}/audio", self.base_url, id);
        match self.download_file(&url, dest).await {
            Ok(bytes) => {
                metrics::record_downloader_request("download_audio", start.elapsed());
                metrics::record_downloader_bytes("audio", bytes);
                Ok(bytes)
            }
            Err(e) => {
                metrics::record_downloader_error("download_audio", "download");
                Err(e).with_context(|| format!("Failed to download audio for track {}", id))
            }
        }
    }

    /// Download image to a file.
    ///
    /// Creates parent directories if they don't exist.
    pub async fn download_image(&self, id: &str, dest: &Path) -> Result<u64> {
        let start = Instant::now();
        let url = format!("{}/image/{}", self.base_url, id);
        match self.download_file(&url, dest).await {
            Ok(bytes) => {
                metrics::record_downloader_request("download_image", start.elapsed());
                metrics::record_downloader_bytes("image", bytes);
                Ok(bytes)
            }
            Err(e) => {
                metrics::record_downloader_error("download_image", "download");
                Err(e).with_context(|| format!("Failed to download image {}", id))
            }
        }
    }

    /// Internal helper to download a file from a URL.
    ///
    /// Returns the number of bytes written.
    async fn download_file(&self, url: &str, dest: &Path) -> Result<u64> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to connect for download")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed with status: {}", response.status());
        }

        // Create parent directories if needed
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directories")?;
        }

        // Stream response to file
        let bytes = response
            .bytes()
            .await
            .context("Failed to read response body")?;

        let mut file = File::create(dest)
            .await
            .context("Failed to create destination file")?;

        file.write_all(&bytes)
            .await
            .context("Failed to write to file")?;

        file.flush().await.context("Failed to flush file")?;

        Ok(bytes.len() as u64)
    }

    /// Get the base URL of the downloader service.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = DownloaderClient::new("http://localhost:8080".to_string(), 300);
        assert_eq!(client.base_url(), "http://localhost:8080");
    }

    #[test]
    fn test_trailing_slash_removal() {
        let client = DownloaderClient::new("http://localhost:8080/".to_string(), 300);
        assert_eq!(client.base_url(), "http://localhost:8080");
    }
}
