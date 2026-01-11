//! Shared entity resolution for search results.
//!
//! This module provides a single source of truth for resolving search result items
//! (artists, albums, tracks) to their full representations. Both organic search
//! and streaming search use these functions to ensure consistent data.

use crate::catalog_store::CatalogStore;
use crate::search::{
    HashedItemType, ResolvedSearchResult, SearchedAlbum, SearchedArtist, SearchedTrack,
};

/// Resolve an artist by ID to its search representation.
pub fn resolve_artist(catalog_store: &dyn CatalogStore, id: &str) -> Option<SearchedArtist> {
    let resolved = catalog_store.get_resolved_artist(id).ok()??;
    Some(SearchedArtist {
        id: resolved.artist.id.clone(),
        name: resolved.artist.name,
        image_id: Some(resolved.artist.id), // Use artist ID for image lookup
        available: resolved.artist.available,
    })
}

/// Resolve an album by ID to its search representation.
pub fn resolve_album(catalog_store: &dyn CatalogStore, id: &str) -> Option<SearchedAlbum> {
    let resolved = catalog_store.get_resolved_album(id).ok()??;
    // Extract year from string date (e.g., "2023-05-15", "2023-05", "2023")
    let year = resolved
        .album
        .release_date
        .as_ref()
        .and_then(|d| d.split('-').next())
        .and_then(|y| y.parse::<i64>().ok());
    Some(SearchedAlbum {
        id: resolved.album.id.clone(),
        name: resolved.album.name,
        artists_ids_names: resolved
            .artists
            .into_iter()
            .map(|a| (a.id, a.name))
            .collect(),
        image_id: Some(resolved.album.id), // Use album ID for image lookup
        year,
        availability: resolved.album.album_availability.to_db_str().to_string(),
    })
}

/// Resolve a track by ID to its search representation.
pub fn resolve_track(catalog_store: &dyn CatalogStore, id: &str) -> Option<SearchedTrack> {
    let resolved = catalog_store.get_resolved_track(id).ok()??;
    Some(SearchedTrack {
        id: resolved.track.id.clone(),
        name: resolved.track.name,
        // Convert from ms to seconds for display
        duration: (resolved.track.duration_ms / 1000) as u32,
        artists_ids_names: resolved
            .artists
            .into_iter()
            .map(|ta| (ta.artist.id, ta.artist.name))
            .collect(),
        image_id: Some(resolved.album.id.clone()), // Use album ID for image lookup
        album_id: resolved.album.id,
        availability: resolved.track.availability.as_str().to_string(),
    })
}

/// Resolve a search result item to its full representation.
///
/// This is the main entry point for resolving search results. It dispatches
/// to the appropriate type-specific resolver based on item_type.
pub fn resolve_to_result(
    catalog_store: &dyn CatalogStore,
    id: &str,
    item_type: HashedItemType,
) -> Option<ResolvedSearchResult> {
    match item_type {
        HashedItemType::Artist => {
            resolve_artist(catalog_store, id).map(ResolvedSearchResult::Artist)
        }
        HashedItemType::Album => resolve_album(catalog_store, id).map(ResolvedSearchResult::Album),
        HashedItemType::Track => resolve_track(catalog_store, id).map(ResolvedSearchResult::Track),
    }
}
