//! Catalog proxy for on-demand content fetching from the downloader service.
//!
//! The proxy detects when content is incomplete (e.g., artist with no albums)
//! and fetches missing data from the external downloader service.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

use crate::catalog_store::CatalogStore;
use crate::downloader::models::{DownloaderAlbum, DownloaderArtist, DownloaderImage, DownloaderTrack};
use crate::downloader::DownloaderClient;

/// Proxy for fetching and storing content from the downloader service.
pub struct CatalogProxy {
    downloader: Arc<DownloaderClient>,
    catalog_store: Arc<dyn CatalogStore>,
    media_base_path: PathBuf,
}

impl CatalogProxy {
    /// Create a new catalog proxy.
    pub fn new(
        downloader: Arc<DownloaderClient>,
        catalog_store: Arc<dyn CatalogStore>,
        media_base_path: PathBuf,
    ) -> Self {
        Self {
            downloader,
            catalog_store,
            media_base_path,
        }
    }

    /// Ensure an artist has complete data, fetching from downloader if needed.
    ///
    /// Checks if the artist:
    /// - Exists in catalog
    /// - Has albums (discography)
    /// - Has related artists
    ///
    /// If any data is missing, attempts to fetch from downloader.
    pub async fn ensure_artist_complete(&self, id: &str) -> Result<()> {
        // Check if artist exists
        let artist_exists = self.catalog_store.get_artist_json(id)?.is_some();

        if !artist_exists {
            // Artist doesn't exist, fetch everything
            info!("Artist {} not found, fetching from downloader", id);
            self.fetch_and_store_artist(id).await?;
            return Ok(());
        }

        // Check discography
        let discography = self.catalog_store.get_artist_discography_json(id)?;
        let has_albums = discography
            .as_ref()
            .and_then(|d| d.get("albums"))
            .and_then(|a| a.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        if !has_albums {
            info!("Artist {} has no albums, fetching from downloader", id);
            self.fetch_artist_albums(id).await?;
        }

        Ok(())
    }

    /// Ensure an album has complete data, fetching from downloader if needed.
    pub async fn ensure_album_complete(&self, id: &str) -> Result<()> {
        // Check if album exists
        let album_exists = self.catalog_store.get_album_json(id)?.is_some();

        if !album_exists {
            info!("Album {} not found, fetching from downloader", id);
            self.fetch_and_store_album(id).await?;
            return Ok(());
        }

        // Check if album has tracks
        let resolved = self.catalog_store.get_resolved_album_json(id)?;
        let has_tracks = resolved
            .as_ref()
            .and_then(|r| r.get("discs"))
            .and_then(|d| d.as_array())
            .map(|discs| {
                discs.iter().any(|disc| {
                    disc.get("tracks")
                        .and_then(|t| t.as_array())
                        .map(|t| !t.is_empty())
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        if !has_tracks {
            info!("Album {} has no tracks, fetching from downloader", id);
            self.fetch_album_tracks(id).await?;
        }

        Ok(())
    }

    /// Fetch artist metadata and related artists from downloader.
    pub async fn fetch_and_store_artist(&self, id: &str) -> Result<()> {
        let dl_artist = self.downloader.get_artist(id).await?;

        // Store the artist
        self.store_artist(&dl_artist).await?;

        // Store portrait images
        for image in dl_artist.get_images() {
            if let Err(e) = self.store_image(image, "artists").await {
                warn!("Failed to store artist image {}: {}", image.id, e);
            }
        }

        // Fetch and store albums
        self.fetch_artist_albums(id).await?;

        Ok(())
    }

    /// Fetch artist's albums from downloader.
    async fn fetch_artist_albums(&self, artist_id: &str) -> Result<()> {
        // Get artist from downloader to find album IDs
        let dl_artist = self.downloader.get_artist(artist_id).await?;

        // The downloader artist response doesn't include album IDs directly,
        // so we need to fetch albums separately. For now, we'll skip this
        // and rely on albums being fetched when accessed directly.
        //
        // TODO: Add endpoint to get artist's albums from downloader

        info!("Artist {} albums would be fetched here (not implemented)", artist_id);

        Ok(())
    }

    /// Fetch and store an album with all its tracks.
    pub async fn fetch_and_store_album(&self, id: &str) -> Result<()> {
        let dl_album = self.downloader.get_album(id).await?;

        // Ensure album artists exist
        for artist_id in &dl_album.artists_ids {
            if self.catalog_store.get_artist_json(artist_id)?.is_none() {
                info!("Fetching album artist {}", artist_id);
                if let Err(e) = self.fetch_and_store_artist(artist_id).await {
                    warn!("Failed to fetch album artist {}: {}", artist_id, e);
                }
            }
        }

        // Store the album
        self.store_album(&dl_album).await?;

        // Store cover images
        for image in dl_album.get_images() {
            if let Err(e) = self.store_image(image, "albums").await {
                warn!("Failed to store album image {}: {}", image.id, e);
            }
        }

        // Fetch and store tracks
        self.fetch_album_tracks(id).await?;

        Ok(())
    }

    /// Fetch and store tracks for an album.
    async fn fetch_album_tracks(&self, album_id: &str) -> Result<()> {
        // Get album from downloader to find track IDs
        let dl_album = self.downloader.get_album(album_id).await?;
        let track_ids = dl_album.get_all_track_ids();

        for track_id in track_ids {
            if let Err(e) = self.fetch_and_store_track(&track_id).await {
                warn!("Failed to fetch track {}: {}", track_id, e);
            }
        }

        Ok(())
    }

    /// Fetch and store a single track.
    pub async fn fetch_and_store_track(&self, id: &str) -> Result<()> {
        // Check if track already exists
        if self.catalog_store.get_track_json(id)?.is_some() {
            return Ok(());
        }

        let dl_track = self.downloader.get_track(id).await?;

        // Get format info
        let (format_str, format) = dl_track
            .get_best_format()
            .context("Track has no available formats")?;

        // Determine file extension from format
        let ext = match format_str.as_str() {
            s if s.starts_with("OGG") => "ogg",
            s if s.starts_with("MP3") => "mp3",
            s if s.starts_with("AAC") => "m4a",
            s if s.starts_with("FLAC") => "flac",
            _ => "audio",
        };

        // Construct audio path
        let relative_uri = format!("tracks/{}/{}.{}", dl_track.album_id, id, ext);
        let audio_path = self.media_base_path.join(&relative_uri);

        // Download audio file
        info!("Downloading track {} audio to {:?}", id, audio_path);
        self.downloader.download_track_audio(id, &audio_path).await?;

        // Store track in catalog
        self.store_track(&dl_track, relative_uri, format).await?;

        Ok(())
    }

    /// Store an artist in the catalog.
    async fn store_artist(&self, dl_artist: &DownloaderArtist) -> Result<()> {
        let artist = dl_artist.to_catalog_artist();
        let json = serde_json::to_value(&artist)?;

        // Check if already exists
        if self.catalog_store.get_artist_json(&artist.id)?.is_some() {
            self.catalog_store.update_artist(&artist.id, json)?;
        } else {
            self.catalog_store.create_artist(json)?;
        }

        // Store related artists relationships
        // Note: This requires the related artists to exist first
        // For now, we just store the artist itself

        Ok(())
    }

    /// Store an album in the catalog.
    async fn store_album(&self, dl_album: &DownloaderAlbum) -> Result<()> {
        let album = dl_album.to_catalog_album();

        // Build JSON with artist relationships
        let mut json = serde_json::to_value(&album)?;
        if let Some(obj) = json.as_object_mut() {
            obj.insert(
                "artists_ids".to_string(),
                serde_json::to_value(&dl_album.artists_ids)?,
            );
        }

        // Check if already exists
        if self.catalog_store.get_album_json(&album.id)?.is_some() {
            self.catalog_store.update_album(&album.id, json)?;
        } else {
            self.catalog_store.create_album(json)?;
        }

        Ok(())
    }

    /// Store a track in the catalog.
    async fn store_track(
        &self,
        dl_track: &DownloaderTrack,
        audio_uri: String,
        format: crate::catalog_store::Format,
    ) -> Result<()> {
        let track = dl_track.to_catalog_track(audio_uri, format);

        // Build JSON with artist relationships
        let mut json = serde_json::to_value(&track)?;
        if let Some(obj) = json.as_object_mut() {
            // Add artists with roles
            let artists_with_role: Vec<serde_json::Value> = dl_track
                .artists_with_role
                .iter()
                .map(|awr| {
                    serde_json::json!({
                        "artist_id": awr.artist_id,
                        "role": awr.to_catalog_role().to_db_str()
                    })
                })
                .collect();
            obj.insert(
                "artists".to_string(),
                serde_json::to_value(&artists_with_role)?,
            );
        }

        self.catalog_store.create_track(json)?;

        Ok(())
    }

    /// Store an image in the catalog and download the file.
    async fn store_image(&self, dl_image: &DownloaderImage, subdir: &str) -> Result<()> {
        // Construct image path
        let relative_uri = format!("images/{}/{}.jpg", subdir, dl_image.id);
        let image_path = self.media_base_path.join(&relative_uri);

        // Download image file
        self.downloader.download_image(&dl_image.id, &image_path).await?;

        // Create catalog image record
        let image = dl_image.to_catalog_image(relative_uri);
        let json = serde_json::to_value(&image)?;

        // Check if already exists
        if self.catalog_store.get_image_path(&image.id).exists() {
            // Image file exists, might need to update record
        } else {
            self.catalog_store.create_image(json)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Integration tests would go here with a mock downloader
}
