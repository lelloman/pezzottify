//! HTTP client for the external downloader service.
#![allow(dead_code)]

use anyhow::{Context, Result};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::header::{ACCEPT_ENCODING, CONTENT_TYPE};
use std::path::Path;
use std::pin::Pin;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::server::metrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPriority {
    Foreground,
    Normal,
    Prefetch,
}

impl DownloadPriority {
    fn as_header(self) -> &'static str {
        match self {
            Self::Foreground => "foreground",
            Self::Normal => "normal",
            Self::Prefetch => "prefetch",
        }
    }
}

pub type AudioByteStream = Pin<Box<dyn Stream<Item = Result<bytes::Bytes>> + Send + 'static>>;

pub struct AudioDownload {
    pub content_length: u64,
    pub content_type: String,
    pub extension: String,
    pub stream: AudioByteStream,
}

/// Trait for downloading content from an external service.
///
/// This trait abstracts the downloader functionality to enable testing
/// with mock implementations.
#[async_trait]
pub trait Downloader: Send + Sync {
    /// Check if the downloader service is healthy.
    async fn health_check(&self) -> Result<()>;

    /// Download track audio to a file.
    async fn download_track_audio(&self, id: &str, dest: &Path) -> Result<u64>;

    /// Open a progressively readable full-track audio response.
    async fn open_track_audio(&self, id: &str, priority: DownloadPriority)
        -> Result<AudioDownload>;

    /// Download image to a file.
    async fn download_image(&self, id: &str, dest: &Path) -> Result<u64>;
}

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
            // Body progress is supervised by TrackMaterializer. A total request
            // timeout would incorrectly abort a healthy, long progressive stream.
            .connect_timeout(Duration::from_secs(timeout_sec))
            .build()
            .expect("Failed to create HTTP client");

        // Ensure base_url doesn't have trailing slash
        let base_url = base_url.trim_end_matches('/').to_string();

        Self { client, base_url }
    }

    /// Get the base URL of the downloader service.
    pub fn base_url(&self) -> &str {
        &self.base_url
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
}

#[async_trait]
impl Downloader for DownloaderClient {
    async fn health_check(&self) -> Result<()> {
        let start = Instant::now();
        let url = format!("{}/health", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .inspect_err(|_| {
                metrics::record_downloader_error("health_check", "connection");
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

    async fn download_track_audio(&self, id: &str, dest: &Path) -> Result<u64> {
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

    async fn open_track_audio(
        &self,
        id: &str,
        priority: DownloadPriority,
    ) -> Result<AudioDownload> {
        let start = Instant::now();
        let url = format!("{}/track/{}/audio", self.base_url, id);
        let response = self
            .client
            .get(url)
            .header(ACCEPT_ENCODING, "identity")
            .header("x-pezzottify-priority", priority.as_header())
            .send()
            .await
            .context("Failed to connect for progressive audio download")?;

        if !response.status().is_success() {
            anyhow::bail!("Audio download failed with status: {}", response.status());
        }

        let content_length = response
            .content_length()
            .context("Downloader audio response is missing Content-Length")?;
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .filter(|value| value.starts_with("audio/"))
            .context("Downloader audio response has invalid Content-Type")?
            .to_string();
        let extension = response
            .headers()
            .get("x-pezzottify-audio-extension")
            .and_then(|value| value.to_str().ok())
            .filter(|value| {
                matches!(
                    *value,
                    "aac" | "flac" | "m4a" | "mp3" | "ogg" | "opus" | "wav"
                )
            })
            .context("Downloader audio response has invalid X-Pezzottify-Audio-Extension")?
            .to_string();
        let stream = response
            .bytes_stream()
            .map(|item| item.map_err(anyhow::Error::from));
        metrics::record_downloader_request("open_audio", start.elapsed());

        Ok(AudioDownload {
            content_length,
            content_type,
            extension,
            stream: Box::pin(stream),
        })
    }

    async fn download_image(&self, id: &str, dest: &Path) -> Result<u64> {
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
