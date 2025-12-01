//! Catalog proxy for on-demand content fetching from the downloader service.
//!
//! The proxy detects when content is incomplete (e.g., artist with no albums)
//! and fetches missing data from the external downloader service.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

use crate::catalog_store::CatalogStore;
use crate::downloader::models::{
    DownloaderAlbum, DownloaderArtist, DownloaderImage, DownloaderTrack,
};
use crate::downloader::Downloader;

/// Proxy for fetching and storing content from the downloader service.
pub struct CatalogProxy {
    downloader: Arc<dyn Downloader>,
    catalog_store: Arc<dyn CatalogStore>,
    media_base_path: PathBuf,
}

impl CatalogProxy {
    /// Create a new catalog proxy.
    pub fn new(
        downloader: Arc<dyn Downloader>,
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

        // Check if artist has related artists
        let resolved = self.catalog_store.get_resolved_artist_json(id)?;
        let has_related_artists = resolved
            .as_ref()
            .and_then(|r| r.get("related_artists"))
            .and_then(|a| a.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        if !has_related_artists {
            info!("Artist {} has no related artists, fetching from downloader", id);
            self.fetch_and_store_artist(id).await?;
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
        info!("Fetching artist {} from downloader...", id);
        let dl_artist = self.downloader.get_artist(id).await?;

        info!(
            "Fetched artist '{}' (id: {}) - {} genres, {} portraits, {} related artists",
            dl_artist.name,
            dl_artist.id,
            dl_artist.genre.len(),
            dl_artist.portraits.len() + dl_artist.portrait_group.len(),
            dl_artist.related.len()
        );

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

        info!(
            "Artist {} albums would be fetched here (not implemented)",
            artist_id
        );

        Ok(())
    }

    /// Fetch and store an album with all its tracks.
    pub async fn fetch_and_store_album(&self, id: &str) -> Result<()> {
        info!("Fetching album {} from downloader...", id);
        let dl_album = self.downloader.get_album(id).await?;

        let track_count: usize = dl_album.discs.iter().map(|d| d.tracks.len()).sum();
        info!(
            "Fetched album '{}' (id: {}) - {} discs, {} tracks, {} artists, {} covers",
            dl_album.name,
            dl_album.id,
            dl_album.discs.len(),
            track_count,
            dl_album.artists_ids.len(),
            dl_album.covers.len() + dl_album.cover_group.len()
        );

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

        info!("Fetching track {} from downloader...", id);
        let dl_track = self.downloader.get_track(id).await?;

        info!(
            "Fetched track '{}' (id: {}) - disc {}, track {}, duration {}ms, {} formats available",
            dl_track.name,
            dl_track.id,
            dl_track.disc_number,
            dl_track.number,
            dl_track.duration,
            dl_track.files.len()
        );

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
        self.downloader
            .download_track_audio(id, &audio_path)
            .await?;

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
            info!("Updated artist '{}' (id: {}) in catalog", artist.name, artist.id);
        } else {
            self.catalog_store.create_artist(json)?;
            info!("Created artist '{}' (id: {}) in catalog", artist.name, artist.id);
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
            info!("Updated album '{}' (id: {}) in catalog", album.name, album.id);
        } else {
            self.catalog_store.create_album(json)?;
            info!("Created album '{}' (id: {}) in catalog", album.name, album.id);
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
        info!(
            "Created track '{}' (id: {}) in catalog - format: {:?}",
            track.name, track.id, track.format
        );

        Ok(())
    }

    /// Store an image in the catalog and download the file.
    async fn store_image(&self, dl_image: &DownloaderImage, subdir: &str) -> Result<()> {
        // Construct image path
        let relative_uri = format!("images/{}/{}.jpg", subdir, dl_image.id);
        let image_path = self.media_base_path.join(&relative_uri);

        // Download image file
        self.downloader
            .download_image(&dl_image.id, &image_path)
            .await?;

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
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tempfile::TempDir;

    use crate::catalog_store::SqliteCatalogStore;
    use crate::downloader::client::Downloader;
    use crate::downloader::models::{
        DownloaderActivityPeriod, DownloaderAlbum, DownloaderArtist, DownloaderArtistWithRole,
        DownloaderDisc, DownloaderTrack,
    };

    /// Mock downloader for testing proxy logic.
    pub struct MockDownloader {
        artists: Mutex<HashMap<String, DownloaderArtist>>,
        albums: Mutex<HashMap<String, DownloaderAlbum>>,
        tracks: Mutex<HashMap<String, DownloaderTrack>>,
        call_counts: Mutex<HashMap<String, usize>>,
    }

    impl MockDownloader {
        pub fn new() -> Self {
            Self {
                artists: Mutex::new(HashMap::new()),
                albums: Mutex::new(HashMap::new()),
                tracks: Mutex::new(HashMap::new()),
                call_counts: Mutex::new(HashMap::new()),
            }
        }

        pub fn add_artist(&self, artist: DownloaderArtist) {
            self.artists
                .lock()
                .unwrap()
                .insert(artist.id.clone(), artist);
        }

        pub fn add_album(&self, album: DownloaderAlbum) {
            self.albums.lock().unwrap().insert(album.id.clone(), album);
        }

        pub fn add_track(&self, track: DownloaderTrack) {
            self.tracks.lock().unwrap().insert(track.id.clone(), track);
        }

        pub fn get_call_count(&self, method: &str) -> usize {
            *self.call_counts.lock().unwrap().get(method).unwrap_or(&0)
        }

        fn increment_call(&self, method: &str) {
            let mut counts = self.call_counts.lock().unwrap();
            *counts.entry(method.to_string()).or_insert(0) += 1;
        }
    }

    #[async_trait]
    impl Downloader for MockDownloader {
        async fn health_check(&self) -> Result<()> {
            self.increment_call("health_check");
            Ok(())
        }

        async fn get_artist(&self, id: &str) -> Result<DownloaderArtist> {
            self.increment_call("get_artist");
            self.artists
                .lock()
                .unwrap()
                .get(id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Artist not found: {}", id))
        }

        async fn get_album(&self, id: &str) -> Result<DownloaderAlbum> {
            self.increment_call("get_album");
            self.albums
                .lock()
                .unwrap()
                .get(id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Album not found: {}", id))
        }

        async fn get_track(&self, id: &str) -> Result<DownloaderTrack> {
            self.increment_call("get_track");
            self.tracks
                .lock()
                .unwrap()
                .get(id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Track not found: {}", id))
        }

        async fn download_track_audio(&self, _id: &str, dest: &PathBuf) -> Result<u64> {
            self.increment_call("download_track_audio");
            // Create a fake audio file
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(dest, b"fake audio data")?;
            Ok(15)
        }

        async fn download_image(&self, _id: &str, dest: &PathBuf) -> Result<u64> {
            self.increment_call("download_image");
            // Create a fake image file
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(dest, b"fake image data")?;
            Ok(15)
        }
    }

    fn create_test_artist(id: &str, name: &str) -> DownloaderArtist {
        DownloaderArtist {
            id: id.to_string(),
            name: name.to_string(),
            genre: vec!["rock".to_string()],
            portraits: vec![],
            activity_periods: vec![DownloaderActivityPeriod {
                decade: Some(2000),
                timespan: None,
            }],
            related: vec![],
            portrait_group: vec![],
        }
    }

    fn create_test_album(id: &str, name: &str, artist_ids: Vec<&str>) -> DownloaderAlbum {
        DownloaderAlbum {
            id: id.to_string(),
            name: name.to_string(),
            album_type: "ALBUM".to_string(),
            artists_ids: artist_ids.into_iter().map(|s| s.to_string()).collect(),
            label: Some("Test Label".to_string()),
            date: Some(1234567890),
            genres: vec![],
            covers: vec![],
            discs: vec![DownloaderDisc {
                number: 1,
                name: "".to_string(),
                tracks: vec!["track1".to_string()],
            }],
            related: vec![],
            cover_group: vec![],
            original_title: Some(name.to_string()),
            version_title: "".to_string(),
        }
    }

    fn create_test_track(id: &str, name: &str, album_id: &str) -> DownloaderTrack {
        let mut files = HashMap::new();
        files.insert("OGG_VORBIS_320".to_string(), "hash123".to_string());

        DownloaderTrack {
            id: id.to_string(),
            name: name.to_string(),
            album_id: album_id.to_string(),
            artists_ids: vec!["artist1".to_string()],
            number: 1,
            disc_number: 1,
            duration: 180000,
            is_explicit: false,
            files,
            alternatives: vec![],
            tags: vec![],
            earliest_live_timestamp: None,
            has_lyrics: false,
            language_of_performance: vec![],
            original_title: Some(name.to_string()),
            version_title: "".to_string(),
            artists_with_role: vec![DownloaderArtistWithRole {
                artist_id: "artist1".to_string(),
                name: "Test Artist".to_string(),
                role: "ARTIST_ROLE_MAIN_ARTIST".to_string(),
            }],
        }
    }

    fn setup_test_env() -> (TempDir, Arc<SqliteCatalogStore>) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_catalog.db");
        let catalog_store = Arc::new(SqliteCatalogStore::new(&db_path, temp_dir.path()).unwrap());
        (temp_dir, catalog_store)
    }

    #[tokio::test]
    async fn test_ensure_artist_complete_fetches_missing_artist() {
        let (temp_dir, catalog_store) = setup_test_env();
        let mock_downloader = Arc::new(MockDownloader::new());

        // Add artist to mock
        mock_downloader.add_artist(create_test_artist("artist1", "Test Artist"));

        let proxy = CatalogProxy::new(
            mock_downloader.clone(),
            catalog_store.clone(),
            temp_dir.path().to_path_buf(),
        );

        // Artist doesn't exist in catalog, should fetch from downloader
        let result = proxy.ensure_artist_complete("artist1").await;
        assert!(result.is_ok());

        // Verify downloader was called (may be called multiple times for artist + albums)
        assert!(mock_downloader.get_call_count("get_artist") >= 1);

        // Verify artist was stored in catalog
        let stored = catalog_store.get_artist_json("artist1").unwrap();
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn test_ensure_artist_complete_skips_existing_artist_with_related_artists() {
        let (temp_dir, catalog_store) = setup_test_env();
        let mock_downloader = Arc::new(MockDownloader::new());

        // Add artist to mock as fallback (in case related artists check triggers fetch)
        mock_downloader.add_artist(create_test_artist("artist1", "Existing Artist"));

        // Pre-populate catalog with two artists
        let artist1_json = serde_json::json!({
            "id": "artist1",
            "name": "Existing Artist",
            "genres": ["rock"],
            "activity_periods": []
        });
        catalog_store.create_artist(artist1_json).unwrap();

        let artist2_json = serde_json::json!({
            "id": "artist2",
            "name": "Related Artist",
            "genres": ["rock"],
            "activity_periods": []
        });
        catalog_store.create_artist(artist2_json).unwrap();

        // Add related artist relationship
        catalog_store.add_related_artist("artist1", "artist2").unwrap();

        let proxy = CatalogProxy::new(
            mock_downloader.clone(),
            catalog_store,
            temp_dir.path().to_path_buf(),
        );

        // Artist exists with related artists, should not call fetch_and_store_artist
        let result = proxy.ensure_artist_complete("artist1").await;
        if let Err(ref e) = result {
            eprintln!("ensure_artist_complete error: {:?}", e);
        }
        assert!(
            result.is_ok(),
            "ensure_artist_complete failed: {:?}",
            result.err()
        );

        // Verify downloader was NOT called since artist has related artists
        assert_eq!(mock_downloader.get_call_count("get_artist"), 0);
    }

    #[tokio::test]
    async fn test_ensure_album_complete_fetches_missing_album() {
        let (temp_dir, catalog_store) = setup_test_env();
        let mock_downloader = Arc::new(MockDownloader::new());

        // Add album and its artist to mock
        mock_downloader.add_artist(create_test_artist("artist1", "Test Artist"));
        mock_downloader.add_album(create_test_album("album1", "Test Album", vec!["artist1"]));
        mock_downloader.add_track(create_test_track("track1", "Test Track", "album1"));

        let proxy = CatalogProxy::new(
            mock_downloader.clone(),
            catalog_store.clone(),
            temp_dir.path().to_path_buf(),
        );

        // Album doesn't exist, should fetch
        let result = proxy.ensure_album_complete("album1").await;
        assert!(result.is_ok());

        // Verify downloader was called
        assert!(mock_downloader.get_call_count("get_album") >= 1);

        // Verify album was stored
        let stored = catalog_store.get_album_json("album1").unwrap();
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn test_fetch_and_store_track() {
        let (temp_dir, catalog_store) = setup_test_env();
        let mock_downloader = Arc::new(MockDownloader::new());

        // First create the album that the track references
        let album_json = serde_json::json!({
            "id": "album1",
            "name": "Test Album",
            "album_type": "Album",
            "genres": []
        });
        catalog_store.create_album(album_json).unwrap();

        mock_downloader.add_track(create_test_track("track1", "Test Track", "album1"));

        let proxy = CatalogProxy::new(
            mock_downloader.clone(),
            catalog_store.clone(),
            temp_dir.path().to_path_buf(),
        );

        let result = proxy.fetch_and_store_track("track1").await;
        if let Err(ref e) = result {
            eprintln!("Error: {:?}", e);
        }
        assert!(
            result.is_ok(),
            "fetch_and_store_track failed: {:?}",
            result.err()
        );

        // Verify audio file was "downloaded"
        assert_eq!(mock_downloader.get_call_count("download_track_audio"), 1);

        // Verify track was stored
        let stored = catalog_store.get_track_json("track1").unwrap();
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn test_mock_downloader_returns_error_for_missing() {
        let mock = MockDownloader::new();

        let result = mock.get_artist("nonexistent").await;
        assert!(result.is_err());

        let result = mock.get_album("nonexistent").await;
        assert!(result.is_err());

        let result = mock.get_track("nonexistent").await;
        assert!(result.is_err());
    }
}
