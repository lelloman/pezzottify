//! Adapter to implement CatalogStore for the legacy Catalog type.
//!
//! This allows the existing filesystem-based Catalog to be used
//! through the CatalogStore trait during the migration period.

use super::trait_def::{CatalogStore, SearchableContentType, SearchableItem};
use crate::catalog::Catalog;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Mutex;

/// Wrapper that implements CatalogStore for the legacy Catalog.
pub struct LegacyCatalogAdapter {
    catalog: Mutex<Catalog>,
}

impl LegacyCatalogAdapter {
    /// Create a new adapter wrapping a Catalog.
    pub fn new(catalog: Catalog) -> Self {
        LegacyCatalogAdapter {
            catalog: Mutex::new(catalog),
        }
    }
}

impl CatalogStore for LegacyCatalogAdapter {
    fn get_artist_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let catalog = self.catalog.lock().unwrap();
        match catalog.get_artist(id) {
            Some(artist) => Ok(Some(serde_json::to_value(artist)?)),
            None => Ok(None),
        }
    }

    fn get_album_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let catalog = self.catalog.lock().unwrap();
        match catalog.get_album(id) {
            Some(album) => Ok(Some(serde_json::to_value(album)?)),
            None => Ok(None),
        }
    }

    fn get_track_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let catalog = self.catalog.lock().unwrap();
        match catalog.get_track(id) {
            Some(track) => Ok(Some(serde_json::to_value(track)?)),
            None => Ok(None),
        }
    }

    fn get_resolved_album_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let catalog = self.catalog.lock().unwrap();
        match catalog.get_resolved_album(id)? {
            Some(album) => Ok(Some(serde_json::to_value(album)?)),
            None => Ok(None),
        }
    }

    fn get_resolved_track_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let catalog = self.catalog.lock().unwrap();
        match catalog.get_resolved_track(id)? {
            Some(track) => Ok(Some(serde_json::to_value(track)?)),
            None => Ok(None),
        }
    }

    fn get_artist_discography_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let catalog = self.catalog.lock().unwrap();
        match catalog.get_artist_discography(id.to_string()) {
            Some(discography) => Ok(Some(serde_json::to_value(discography)?)),
            None => Ok(None),
        }
    }

    fn get_image_path(&self, id: &str) -> PathBuf {
        let catalog = self.catalog.lock().unwrap();
        catalog.get_image_path(id.to_string())
    }

    fn get_track_audio_path(&self, track_id: &str) -> Option<PathBuf> {
        let catalog = self.catalog.lock().unwrap();
        let track = catalog.get_track(track_id)?;
        catalog.get_track_audio_path(&track.album_id, track_id)
    }

    fn get_track_album_id(&self, track_id: &str) -> Option<String> {
        let catalog = self.catalog.lock().unwrap();
        catalog.get_track(track_id).map(|t| t.album_id.clone())
    }

    fn get_artists_count(&self) -> usize {
        let catalog = self.catalog.lock().unwrap();
        catalog.get_artists_count()
    }

    fn get_albums_count(&self) -> usize {
        let catalog = self.catalog.lock().unwrap();
        catalog.get_albums_count()
    }

    fn get_tracks_count(&self) -> usize {
        let catalog = self.catalog.lock().unwrap();
        catalog.get_tracks_count()
    }

    fn get_searchable_content(&self) -> Result<Vec<SearchableItem>> {
        let catalog = self.catalog.lock().unwrap();
        let mut items = Vec::new();

        // Add artists
        for artist in catalog.iter_artists() {
            items.push(SearchableItem {
                id: artist.id.clone(),
                name: artist.name.clone(),
                content_type: SearchableContentType::Artist,
                additional_text: artist.genre.clone(),
            });
        }

        // Add albums
        for album in catalog.iter_albums() {
            items.push(SearchableItem {
                id: album.id.clone(),
                name: album.name.clone(),
                content_type: SearchableContentType::Album,
                additional_text: album.genres.clone(),
            });
        }

        // Add tracks
        for track in catalog.iter_tracks() {
            items.push(SearchableItem {
                id: track.id.clone(),
                name: track.name.clone(),
                content_type: SearchableContentType::Track,
                additional_text: track.tags.clone(),
            });
        }

        Ok(items)
    }

    // =========================================================================
    // Write Operations - Not supported in legacy adapter
    // =========================================================================

    fn create_artist(&self, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("Write operations not supported in legacy catalog adapter. Use --catalog-db to enable SQLite backend.")
    }

    fn update_artist(&self, _id: &str, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("Write operations not supported in legacy catalog adapter. Use --catalog-db to enable SQLite backend.")
    }

    fn delete_artist(&self, _id: &str) -> Result<()> {
        anyhow::bail!("Write operations not supported in legacy catalog adapter. Use --catalog-db to enable SQLite backend.")
    }

    fn create_album(&self, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("Write operations not supported in legacy catalog adapter. Use --catalog-db to enable SQLite backend.")
    }

    fn update_album(&self, _id: &str, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("Write operations not supported in legacy catalog adapter. Use --catalog-db to enable SQLite backend.")
    }

    fn delete_album(&self, _id: &str) -> Result<()> {
        anyhow::bail!("Write operations not supported in legacy catalog adapter. Use --catalog-db to enable SQLite backend.")
    }

    fn create_track(&self, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("Write operations not supported in legacy catalog adapter. Use --catalog-db to enable SQLite backend.")
    }

    fn update_track(&self, _id: &str, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("Write operations not supported in legacy catalog adapter. Use --catalog-db to enable SQLite backend.")
    }

    fn delete_track(&self, _id: &str) -> Result<()> {
        anyhow::bail!("Write operations not supported in legacy catalog adapter. Use --catalog-db to enable SQLite backend.")
    }

    fn create_image(&self, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("Write operations not supported in legacy catalog adapter. Use --catalog-db to enable SQLite backend.")
    }

    fn update_image(&self, _id: &str, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("Write operations not supported in legacy catalog adapter. Use --catalog-db to enable SQLite backend.")
    }

    fn delete_image(&self, _id: &str) -> Result<()> {
        anyhow::bail!("Write operations not supported in legacy catalog adapter. Use --catalog-db to enable SQLite backend.")
    }
}
