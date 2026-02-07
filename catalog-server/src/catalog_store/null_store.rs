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

    fn get_track(&self, _id: &str) -> Result<Option<super::Track>> {
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

    fn get_resolved_artist(&self, _id: &str) -> Result<Option<super::ResolvedArtist>> {
        Ok(None)
    }

    fn get_resolved_album(&self, _id: &str) -> Result<Option<super::ResolvedAlbum>> {
        Ok(None)
    }

    fn get_resolved_track(&self, _id: &str) -> Result<Option<super::ResolvedTrack>> {
        Ok(None)
    }

    fn get_discography(
        &self,
        _id: &str,
        _limit: usize,
        _offset: usize,
        _sort: super::DiscographySort,
    ) -> Result<Option<super::ArtistDiscography>> {
        Ok(None)
    }

    fn get_album_image_url(&self, _album_id: &str) -> Result<Option<super::ImageUrl>> {
        Ok(None)
    }

    fn get_artist_image_url(&self, _artist_id: &str) -> Result<Option<super::ImageUrl>> {
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

    fn list_all_track_ids(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn create_artist(&self, _artist: &super::Artist) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn update_artist(&self, _artist: &super::Artist) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn delete_artist(&self, _id: &str) -> Result<bool> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn create_album(&self, _album: &super::Album, _artist_ids: &[String]) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn update_album(&self, _album: &super::Album, _artist_ids: Option<&[String]>) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn delete_album(&self, _id: &str) -> Result<bool> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn create_track(&self, _track: &super::Track, _artist_ids: &[String]) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn update_track(&self, _track: &super::Track, _artist_ids: Option<&[String]>) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn delete_track(&self, _id: &str) -> Result<bool> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn set_track_audio_uri(&self, _track_id: &str, _audio_uri: &str) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn recompute_album_availability(&self, _album_id: &str) -> Result<super::AlbumAvailability> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn recompute_artist_availability(&self, _artist_id: &str) -> Result<bool> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn get_album_artist_ids(&self, _album_id: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn get_items_popularity(
        &self,
        _items: &[(String, super::SearchableContentType)],
    ) -> Result<std::collections::HashMap<(String, super::SearchableContentType), i32>> {
        Ok(std::collections::HashMap::new())
    }

    fn get_genres_with_counts(&self) -> Result<Vec<super::GenreInfo>> {
        Ok(Vec::new())
    }

    fn get_tracks_by_genre(
        &self,
        _genre: &str,
        _limit: usize,
        _offset: usize,
    ) -> Result<super::GenreTracksResult> {
        Ok(super::GenreTracksResult {
            track_ids: Vec::new(),
            total: 0,
            has_more: false,
        })
    }

    fn get_random_tracks_by_genre(&self, _genre: &str, _limit: usize) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn get_album_track_durations(&self, _album_id: &str) -> Result<Vec<i64>> {
        Ok(vec![])
    }

    fn find_albums_by_fingerprint(
        &self,
        _track_count: i32,
        _total_duration_ms: i64,
    ) -> Result<Vec<super::AlbumFingerprintCandidate>> {
        Ok(Vec::new())
    }

    fn update_album_fingerprint(&self, _album_id: &str) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn get_artists_needing_mbid(&self, _limit: usize) -> Result<Vec<(String, i64)>> {
        Ok(Vec::new())
    }

    fn get_artists_needing_related(&self, _limit: usize) -> Result<Vec<(String, String, i64)>> {
        Ok(Vec::new())
    }

    fn set_artist_mbid(&self, _artist_id: &str, _mbid: &str) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn mark_artist_mbid_not_found(&self, _artist_id: &str) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn set_related_artists(&self, _artist_rowid: i64, _related: &[(i64, f64)]) -> Result<()> {
        anyhow::bail!("NullCatalogStore does not support write operations")
    }

    fn get_related_artists(&self, _artist_id: &str) -> Result<Vec<super::Artist>> {
        Ok(Vec::new())
    }

    fn get_artist_rowid_by_mbid(&self, _mbid: &str) -> Result<Option<i64>> {
        Ok(None)
    }
}
