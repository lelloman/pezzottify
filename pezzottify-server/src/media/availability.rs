//! Live local observations are composed after catalog reads have released their locks.
use crate::catalog_store::*;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaPresence {
    Present,
    Missing,
    Unknown(String),
}

pub fn probe(root: &Path, uri: Option<&str>) -> MediaPresence {
    let Some(uri) = uri else {
        return MediaPresence::Missing;
    };
    match super::local::open_media_file_beneath(root, uri) {
        Ok(_) => MediaPresence::Present,
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
        {
            MediaPresence::Missing
        }
        Err(error) => MediaPresence::Unknown(error.to_string()),
    }
}
fn enrich(root: &Path, track: &mut Track) {
    track.availability = if probe(root, track.audio_uri.as_deref()) == MediaPresence::Present {
        TrackAvailability::Available
    } else {
        TrackAvailability::Unavailable
    };
}
/// Compatibility read projection. SQL operations remain in the underlying store;
/// physical checks live here in media and never execute while holding its transaction.
pub struct MediaCatalogView {
    inner: Arc<dyn CatalogStore>,
    root: PathBuf,
}
impl MediaCatalogView {
    pub fn wrap(inner: Arc<dyn CatalogStore>) -> Arc<dyn CatalogStore> {
        Arc::new(Self {
            root: inner.media_root(),
            inner,
        })
    }
}
impl CatalogStore for MediaCatalogView {
    fn get_artist_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.inner.get_artist_json(id)
    }
    fn get_album_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.inner.get_album_json(id)
    }
    fn get_track_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.get_track(id)
            .map(|track| track.map(|track| serde_json::to_value(track).unwrap()))
    }
    fn get_track(&self, id: &str) -> Result<Option<crate::catalog_store::Track>> {
        let mut track = self.inner.get_track(id)?;
        if let Some(track) = &mut track {
            enrich(&self.root, track);
        }
        Ok(track)
    }
    fn get_resolved_artist_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.inner.get_resolved_artist_json(id)
    }
    fn get_resolved_album_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.get_resolved_album(id)
            .map(|value| value.map(|value| serde_json::to_value(value).unwrap()))
    }
    fn get_resolved_track_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.get_resolved_track(id)
            .map(|value| value.map(|value| serde_json::to_value(value).unwrap()))
    }
    fn get_resolved_artist(
        &self,
        id: &str,
    ) -> Result<Option<crate::catalog_store::ResolvedArtist>> {
        self.inner.get_resolved_artist(id)
    }
    fn get_resolved_album(&self, id: &str) -> Result<Option<crate::catalog_store::ResolvedAlbum>> {
        let mut album = self.inner.get_resolved_album(id)?;
        if let Some(album) = &mut album {
            for disc in &mut album.discs {
                for track in &mut disc.tracks {
                    enrich(&self.root, track);
                }
            }
        }
        Ok(album)
    }
    fn get_resolved_track(&self, id: &str) -> Result<Option<crate::catalog_store::ResolvedTrack>> {
        let mut track = self.inner.get_resolved_track(id)?;
        if let Some(track) = &mut track {
            enrich(&self.root, &mut track.track);
        }
        Ok(track)
    }
    fn get_discography(
        &self,
        id: &str,
        limit: usize,
        offset: usize,
        sort: crate::catalog_store::DiscographySort,
        appears_on: bool,
    ) -> Result<Option<crate::catalog_store::ArtistDiscography>> {
        self.inner
            .get_discography(id, limit, offset, sort, appears_on)
    }
    fn get_album_image_url(
        &self,
        album_id: &str,
    ) -> Result<Option<crate::catalog_store::ImageUrl>> {
        self.inner.get_album_image_url(album_id)
    }
    fn get_artist_image_url(
        &self,
        artist_id: &str,
    ) -> Result<Option<crate::catalog_store::ImageUrl>> {
        self.inner.get_artist_image_url(artist_id)
    }
    fn get_item_image_url(&self, item_id: &str) -> Result<Option<crate::catalog_store::ImageUrl>> {
        self.inner.get_item_image_url(item_id)
    }
    fn media_root(&self) -> PathBuf {
        self.inner.media_root()
    }
    fn media_presence_page(
        &self,
        _after: i64,
        _limit: usize,
    ) -> Result<Vec<(i64, String, Option<String>)>> {
        self.inner.media_presence_page(_after, _limit)
    }
    fn compare_exchange_audio(
        &self,
        _id: &str,
        _expected: Option<&str>,
        _new: Option<&str>,
    ) -> Result<bool> {
        self.inner.compare_exchange_audio(_id, _expected, _new)
    }
    fn apply_media_observations(
        &self,
        _observations: &[(String, Option<String>, bool)],
        cancelled: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<crate::catalog_store::AvailabilityRefreshResult> {
        self.inner
            .apply_media_observations(_observations, cancelled)
    }
    fn get_track_album_id(&self, track_id: &str) -> Option<String> {
        self.inner.get_track_album_id(track_id)
    }
    fn get_track_availability(&self, track_id: &str) -> crate::catalog_store::TrackAvailability {
        self.get_track(track_id)
            .ok()
            .flatten()
            .map(|track| track.availability)
            .unwrap_or_default()
    }
    fn get_artists_count(&self) -> usize {
        self.inner.get_artists_count()
    }
    fn get_albums_count(&self) -> usize {
        self.inner.get_albums_count()
    }
    fn get_tracks_count(&self) -> usize {
        self.inner.get_tracks_count()
    }
    fn get_catalog_cardinality_stats(
        &self,
    ) -> Result<Option<crate::catalog_store::CatalogCardinalityStats>> {
        self.inner.get_catalog_cardinality_stats()
    }
    fn rebuild_catalog_cardinality_stats(
        &self,
        _is_cancelled: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<crate::catalog_store::CatalogCardinalityStats> {
        self.inner.rebuild_catalog_cardinality_stats(_is_cancelled)
    }
    fn refresh_availability_and_stats(
        &self,
    ) -> Result<crate::catalog_store::AvailabilityRefreshResult> {
        self.inner.refresh_availability_and_stats()
    }
    fn refresh_availability_and_stats_with_cancel(
        &self,
        _is_cancelled: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<crate::catalog_store::AvailabilityRefreshResult> {
        self.inner
            .refresh_availability_and_stats_with_cancel(_is_cancelled)
    }
    fn get_searchable_content(&self) -> Result<Vec<SearchableItem>> {
        self.inner.get_searchable_content()
    }
    fn get_searchable_content_page(
        &self,
        content_type: SearchableContentType,
        after_rowid: i64,
        limit: usize,
    ) -> Result<Vec<(i64, SearchableItem)>> {
        self.inner
            .get_searchable_content_page(content_type, after_rowid, limit)
    }
    fn get_available_searchable_content_page(
        &self,
        content_type: SearchableContentType,
        after_rowid: i64,
        limit: usize,
    ) -> Result<Vec<(i64, SearchableItem)>> {
        self.inner
            .get_available_searchable_content_page(content_type, after_rowid, limit)
    }
    fn list_all_track_ids(&self) -> Result<Vec<String>> {
        self.inner.list_all_track_ids()
    }
    fn list_available_track_ids_with_audio_uri(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(String, String)>> {
        self.inner
            .list_available_track_ids_with_audio_uri(limit, offset)
    }
    fn list_available_tracks_missing_embeddings(
        &self,
        _namespaces: &[String],
        _limit: usize,
    ) -> Result<Vec<(String, String)>> {
        self.inner
            .list_available_tracks_missing_embeddings(_namespaces, _limit)
    }
    fn get_track_embedding_coverage(
        &self,
        _namespaces: &[String],
    ) -> Result<TrackEmbeddingCoverage> {
        self.inner.get_track_embedding_coverage(_namespaces)
    }
    fn list_complete_album_tracklists_page(
        &self,
        _after_album_rowid: Option<i64>,
        _limit: usize,
    ) -> Result<Vec<AlbumTracklist>> {
        self.inner
            .list_complete_album_tracklists_page(_after_album_rowid, _limit)
    }
    fn get_album_embedding_coverage(
        &self,
        _namespaces: &[String],
        _media_path: &Path,
    ) -> Result<AlbumEmbeddingCoverage> {
        self.inner
            .get_album_embedding_coverage(_namespaces, _media_path)
    }
    fn create_artist(&self, artist: &crate::catalog_store::Artist) -> Result<()> {
        self.inner.create_artist(artist)
    }
    fn update_artist(&self, artist: &crate::catalog_store::Artist) -> Result<()> {
        self.inner.update_artist(artist)
    }
    fn delete_artist(&self, id: &str) -> Result<bool> {
        self.inner.delete_artist(id)
    }
    fn create_album(
        &self,
        album: &crate::catalog_store::Album,
        artist_ids: &[String],
    ) -> Result<()> {
        self.inner.create_album(album, artist_ids)
    }
    fn update_album_metadata(
        &self,
        album_id: &str,
        metadata: &crate::catalog_store::AlbumMetadataUpdate,
        artist_ids: Option<&[String]>,
    ) -> Result<()> {
        self.inner
            .update_album_metadata(album_id, metadata, artist_ids)
    }
    fn delete_album(&self, id: &str) -> Result<bool> {
        self.inner.delete_album(id)
    }
    fn create_track(
        &self,
        track: &crate::catalog_store::Track,
        artist_ids: &[String],
    ) -> Result<()> {
        self.inner.create_track(track, artist_ids)
    }
    fn update_track_metadata(
        &self,
        track_id: &str,
        metadata: &crate::catalog_store::TrackMetadataUpdate,
        artist_ids: Option<&[String]>,
    ) -> Result<()> {
        self.inner
            .update_track_metadata(track_id, metadata, artist_ids)
    }
    fn delete_track(&self, id: &str) -> Result<bool> {
        self.inner.delete_track(id)
    }
    fn set_track_audio_uri(&self, track_id: &str, audio_uri: &str) -> Result<()> {
        self.inner.set_track_audio_uri(track_id, audio_uri)
    }
    fn clear_track_audio_uri(&self, track_id: &str) -> Result<()> {
        self.inner.clear_track_audio_uri(track_id)
    }
    fn recompute_album_availability(
        &self,
        album_id: &str,
    ) -> Result<crate::catalog_store::AlbumAvailability> {
        self.inner.recompute_album_availability(album_id)
    }
    fn recompute_artist_availability(&self, artist_id: &str) -> Result<bool> {
        self.inner.recompute_artist_availability(artist_id)
    }
    fn get_album_artist_ids(&self, album_id: &str) -> Result<Vec<String>> {
        self.inner.get_album_artist_ids(album_id)
    }
    fn upsert_entity_embedding(
        &self,
        _embedding: &crate::catalog_store::EntityEmbeddingUpsert,
    ) -> Result<crate::catalog_store::EntityEmbedding> {
        self.inner.upsert_entity_embedding(_embedding)
    }
    fn get_entity_embedding(
        &self,
        _entity_type: &str,
        _entity_id: &str,
        _namespace: &str,
        _include_vector: bool,
    ) -> Result<Option<crate::catalog_store::EntityEmbedding>> {
        self.inner
            .get_entity_embedding(_entity_type, _entity_id, _namespace, _include_vector)
    }
    fn list_entity_embeddings(
        &self,
        _entity_type: &str,
        _entity_id: &str,
        _include_vector: bool,
    ) -> Result<Vec<crate::catalog_store::EntityEmbedding>> {
        self.inner
            .list_entity_embeddings(_entity_type, _entity_id, _include_vector)
    }
    fn delete_entity_embedding(
        &self,
        _entity_type: &str,
        _entity_id: &str,
        _namespace: &str,
    ) -> Result<bool> {
        self.inner
            .delete_entity_embedding(_entity_type, _entity_id, _namespace)
    }
    fn search_entity_embeddings(
        &self,
        _namespace: &str,
        _query: &[f32],
        _entity_type: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<crate::catalog_store::EntityEmbeddingSearchResult>> {
        self.inner
            .search_entity_embeddings(_namespace, _query, _entity_type, _limit)
    }
    fn get_items_popularity(
        &self,
        items: &[(String, SearchableContentType)],
    ) -> Result<std::collections::HashMap<(String, SearchableContentType), i32>> {
        self.inner.get_items_popularity(items)
    }
    fn get_genres_with_counts(&self) -> Result<Vec<crate::catalog_store::GenreInfo>> {
        self.inner.get_genres_with_counts()
    }
    fn get_tracks_by_genre(
        &self,
        genre: &str,
        limit: usize,
        offset: usize,
    ) -> Result<crate::catalog_store::GenreTracksResult> {
        self.inner.get_tracks_by_genre(genre, limit, offset)
    }
    fn get_random_tracks_by_genre(&self, genre: &str, limit: usize) -> Result<Vec<String>> {
        self.inner.get_random_tracks_by_genre(genre, limit)
    }
    fn get_available_album_track_ids(&self, _album_id: &str) -> Result<Vec<String>> {
        self.inner.get_available_album_track_ids(_album_id)
    }
    fn get_artist_top_track_ids(&self, _artist_id: &str, _limit: usize) -> Result<Vec<String>> {
        self.inner.get_artist_top_track_ids(_artist_id, _limit)
    }
    fn find_albums_by_fingerprint(
        &self,
        track_count: i32,
        total_duration_ms: i64,
    ) -> Result<Vec<crate::catalog_store::AlbumFingerprintCandidate>> {
        self.inner
            .find_albums_by_fingerprint(track_count, total_duration_ms)
    }
    fn get_album_track_durations(&self, album_id: &str) -> Result<Vec<i64>> {
        self.inner.get_album_track_durations(album_id)
    }
    fn update_album_fingerprint(&self, album_id: &str) -> Result<()> {
        self.inner.update_album_fingerprint(album_id)
    }
    fn get_artists_needing_mbid(&self, limit: usize) -> Result<Vec<(String, i64)>> {
        self.inner.get_artists_needing_mbid(limit)
    }
    fn get_artists_needing_related(&self, limit: usize) -> Result<Vec<(String, String, i64)>> {
        self.inner.get_artists_needing_related(limit)
    }
    fn get_artist_mbid(&self, artist_id: &str) -> Result<Option<String>> {
        self.inner.get_artist_mbid(artist_id)
    }
    fn set_artist_mbid(&self, artist_id: &str, mbid: &str) -> Result<()> {
        self.inner.set_artist_mbid(artist_id, mbid)
    }
    fn mark_artist_mbid_not_found(&self, artist_id: &str) -> Result<()> {
        self.inner.mark_artist_mbid_not_found(artist_id)
    }
    fn record_artist_mbid_failure(&self, artist_rowid: i64, error: &str) -> Result<()> {
        self.inner.record_artist_mbid_failure(artist_rowid, error)
    }
    fn record_artist_related_failure(&self, artist_rowid: i64, error: &str) -> Result<()> {
        self.inner
            .record_artist_related_failure(artist_rowid, error)
    }
    fn release_artist_enrichment_claims(&self) -> Result<()> {
        self.inner.release_artist_enrichment_claims()
    }
    fn set_related_artists(&self, artist_rowid: i64, related: &[(i64, f64)]) -> Result<()> {
        self.inner.set_related_artists(artist_rowid, related)
    }
    fn get_related_artists(&self, artist_id: &str) -> Result<Vec<crate::catalog_store::Artist>> {
        self.inner.get_related_artists(artist_id)
    }
    fn get_artist_rowid_by_mbid(&self, mbid: &str) -> Result<Option<i64>> {
        self.inner.get_artist_rowid_by_mbid(mbid)
    }
    fn get_artist_rowids_by_mbids(&self, mbids: &[String]) -> Result<Vec<(String, i64)>> {
        self.inner.get_artist_rowids_by_mbids(mbids)
    }
}

impl super::MediaManager {
    pub fn reconcile_presence(
        &self,
        cancelled: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<AvailabilityRefreshResult> {
        self.recover(cancelled)?;
        let mut after = 0;
        let mut result = AvailabilityRefreshResult::default();
        loop {
            anyhow::ensure!(!cancelled(), "cancelled");
            let _guard = self.mutations.lock().unwrap();
            let page = self.catalog.media_presence_page(after, 1000)?;
            if page.is_empty() {
                break;
            }
            after = page.last().unwrap().0;
            let mut observations = Vec::new();
            for (_, id, uri) in page {
                anyhow::ensure!(!cancelled(), "cancelled");
                if probe(&self.root, uri.as_deref()) == MediaPresence::Missing {
                    observations.push((id, uri, false));
                }
            }
            if !observations.is_empty() {
                let refreshed = self.observe_missing_batch(&observations, cancelled)?;
                result.track_updates.extend(refreshed.track_updates);
                result.album_updates.extend(refreshed.album_updates);
                result.artist_updates.extend(refreshed.artist_updates);
                result.repaired.tracks_updated += refreshed.repaired.tracks_updated;
                result.repaired.albums_updated += refreshed.repaired.albums_updated;
                result.repaired.artists_updated += refreshed.repaired.artists_updated;
            }
        }
        result.stats = self.catalog.refresh_availability_and_stats()?.stats;
        Ok(result)
    }
}

/// Shared accounting backend; callers keep their existing reporting schemas.
pub fn directory_size(path: &Path) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| entry.metadata().ok())
        .map(|metadata| metadata.len())
        .sum()
}
