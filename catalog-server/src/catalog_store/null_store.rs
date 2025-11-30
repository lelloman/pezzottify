//! Null catalog store implementation.
//!
//! A no-op implementation of CatalogStore for use cases where catalog
//! functionality is not needed (e.g., CLI tools that only manage users).

use super::trait_def::{CatalogStore, SearchableItem};
use anyhow::Result;
use std::path::PathBuf;

/// A no-op catalog store that returns empty/none for all operations.
pub struct NullCatalogStore;

impl CatalogStore for NullCatalogStore {
    fn get_artist_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }

    fn get_album_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }

    fn get_track_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }

    fn get_resolved_album_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }

    fn get_resolved_track_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }

    fn get_artist_discography_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }

    fn get_image_path(&self, _id: &str) -> PathBuf {
        PathBuf::new()
    }

    fn get_track_audio_path(&self, _track_id: &str) -> Option<PathBuf> {
        None
    }

    fn get_track_album_id(&self, _track_id: &str) -> Option<String> {
        None
    }

    fn get_artists_count(&self) -> usize {
        0
    }

    fn get_albums_count(&self) -> usize {
        0
    }

    fn get_tracks_count(&self) -> usize {
        0
    }

    fn get_searchable_content(&self) -> Result<Vec<SearchableItem>> {
        Ok(Vec::new())
    }

    fn create_artist(&self, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn update_artist(&self, _id: &str, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn delete_artist(&self, _id: &str) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn create_album(&self, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn update_album(&self, _id: &str, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn delete_album(&self, _id: &str) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn create_track(&self, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn update_track(&self, _id: &str, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn delete_track(&self, _id: &str) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn create_image(&self, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn update_image(&self, _id: &str, _data: serde_json::Value) -> Result<serde_json::Value> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn delete_image(&self, _id: &str) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }
}
