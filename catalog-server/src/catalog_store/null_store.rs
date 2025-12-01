//! Null catalog store implementation.
//!
//! A no-op implementation of CatalogStore for use cases where catalog
//! functionality is not needed (e.g., CLI tools that only manage users).

use super::changelog::{CatalogBatch, ChangeEntityType, ChangeEntry, WhatsNewBatch};
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

    fn get_resolved_artist_json(&self, _id: &str) -> Result<Option<serde_json::Value>> {
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

    fn create_changelog_batch(
        &self,
        _name: &str,
        _description: Option<&str>,
    ) -> Result<CatalogBatch> {
        anyhow::bail!("NullCatalogStore does not support changelog operations")
    }

    fn get_changelog_batch(&self, _id: &str) -> Result<Option<CatalogBatch>> {
        Ok(None)
    }

    fn get_active_changelog_batch(&self) -> Result<Option<CatalogBatch>> {
        Ok(None)
    }

    fn close_changelog_batch(&self, _id: &str) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support changelog operations")
    }

    fn list_changelog_batches(&self, _is_open: Option<bool>) -> Result<Vec<CatalogBatch>> {
        Ok(Vec::new())
    }

    fn delete_changelog_batch(&self, _id: &str) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support changelog operations")
    }

    fn get_changelog_batch_changes(&self, _batch_id: &str) -> Result<Vec<ChangeEntry>> {
        Ok(Vec::new())
    }

    fn get_changelog_entity_history(
        &self,
        _entity_type: ChangeEntityType,
        _entity_id: &str,
    ) -> Result<Vec<ChangeEntry>> {
        Ok(Vec::new())
    }

    fn get_whats_new_batches(&self, _limit: usize) -> Result<Vec<WhatsNewBatch>> {
        Ok(Vec::new())
    }

    fn get_stale_batches(&self, _stale_threshold_hours: u64) -> Result<Vec<CatalogBatch>> {
        Ok(Vec::new())
    }
}
