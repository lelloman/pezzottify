//! Search API routes
//! Note: Functions are feature-gated and used as route handlers

#![allow(dead_code)] // Feature-gated search functionality

use crate::catalog_store::CatalogStore;
use crate::search::{
    HashedItemType, ResolvedSearchResult, SearchResult, SearchedAlbum, SearchedArtist,
    SearchedTrack,
};

use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};
use chrono::Datelike;
use std::sync::Arc;

use super::{session::Session, state::ServerState};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]

pub enum SearchFilter {
    Album,
    Artist,
    Track,
}
#[derive(Deserialize)]
struct SearchBody {
    pub query: String,

    #[serde(default)]
    pub resolve: bool,

    pub filters: Option<Vec<SearchFilter>>,
}

fn resolve_album(
    catalog_store: &Arc<dyn CatalogStore>,
    album_id: &str,
) -> Option<ResolvedSearchResult> {
    // Use get_resolved_album_json to get display_image
    let album_json = catalog_store.get_resolved_album_json(album_id).ok()??;

    // get_resolved_album_json returns a ResolvedAlbum, so we need to access nested fields
    let name = album_json
        .get("album")
        .and_then(|a| a.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();

    // Get release date/year from the album
    let year = album_json
        .get("album")
        .and_then(|a| a.get("release_date"))
        .and_then(|d| d.as_i64())
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
        .map(|d| d.year() as i64);

    // Get artists from the resolved album (already includes artist names)
    let artists_ids_names: Vec<(String, String)> = album_json
        .get("artists")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|artist| {
                    let id = artist.get("id")?.as_str()?.to_string();
                    let name = artist.get("name")?.as_str()?.to_string();
                    Some((id, name))
                })
                .collect()
        })
        .unwrap_or_default();

    // Get image ID from display_image object
    let image_id = album_json
        .get("display_image")
        .and_then(|img| img.get("id"))
        .and_then(|id| id.as_str())
        .map(String::from);

    let resolved_album = SearchedAlbum {
        id: album_id.to_owned(),
        name,
        artists_ids_names,
        image_id,
        year,
    };

    Some(ResolvedSearchResult::Album(resolved_album))
}

fn resolve_artist(
    catalog_store: &Arc<dyn CatalogStore>,
    artist_id: &str,
) -> Option<ResolvedSearchResult> {
    let artist_json = catalog_store.get_artist_json(artist_id).ok()??;

    // get_artist_json returns a ResolvedArtist, so we need to access nested fields
    let name = artist_json
        .get("artist")
        .and_then(|a| a.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();

    // Get image ID from display_image object
    let image_id = artist_json
        .get("display_image")
        .and_then(|img| img.get("id"))
        .and_then(|id| id.as_str())
        .map(String::from);

    let resolved_artist = SearchedArtist {
        name,
        id: artist_id.to_owned(),
        image_id,
    };

    Some(ResolvedSearchResult::Artist(resolved_artist))
}

fn resolve_track(
    catalog_store: &Arc<dyn CatalogStore>,
    track_id: &str,
) -> Option<ResolvedSearchResult> {
    let track_json = catalog_store.get_track_json(track_id).ok()??;

    let id = track_json.get("id")?.as_str()?.to_string();
    let name = track_json.get("name")?.as_str()?.to_string();
    let duration = track_json
        .get("duration_secs")
        .and_then(|d| d.as_u64())
        .unwrap_or(0) as u32;
    let album_id = track_json.get("album_id")?.as_str()?.to_string();

    // Get artist IDs and resolve their names
    let artists_ids: Vec<String> = track_json
        .get("artists_ids")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let artists_ids_names: Vec<(String, String)> = artists_ids
        .iter()
        .filter_map(|a_id| {
            let artist_json = catalog_store.get_artist_json(a_id).ok()??;
            // get_artist_json returns ResolvedArtist, so access nested name
            let artist_name = artist_json
                .get("artist")
                .and_then(|a| a.get("name"))
                .and_then(|n| n.as_str())?
                .to_string();
            Some((a_id.clone(), artist_name))
        })
        .collect();

    // Try to get image from album first, then from artist
    let image_id = catalog_store
        .get_resolved_album_json(&album_id)
        .ok()
        .flatten()
        .and_then(|album| {
            // get_resolved_album_json returns ResolvedAlbum with display_image object
            album
                .get("display_image")
                .and_then(|img| img.get("id"))
                .and_then(|id| id.as_str())
                .map(String::from)
        })
        .or_else(|| {
            // Try to get image from first artist with an image
            artists_ids.iter().find_map(|a_id| {
                let artist_json = catalog_store.get_artist_json(a_id).ok()??;
                // get_artist_json returns ResolvedArtist with display_image object
                artist_json
                    .get("display_image")
                    .and_then(|img| img.get("id"))
                    .and_then(|id| id.as_str())
                    .map(String::from)
            })
        });

    let resolved_track = SearchedTrack {
        id,
        name,
        duration,
        image_id,
        artists_ids_names,
        album_id,
    };

    Some(ResolvedSearchResult::Track(resolved_track))
}

fn resolve_search_results(
    catalog_store: &Arc<dyn CatalogStore>,
    results: Vec<SearchResult>,
) -> Vec<ResolvedSearchResult> {
    results
        .iter()
        .filter_map(|result| match result.item_type {
            HashedItemType::Album => resolve_album(catalog_store, &result.item_id),
            HashedItemType::Artist => resolve_artist(catalog_store, &result.item_id),
            HashedItemType::Track => resolve_track(catalog_store, &result.item_id),
        })
        .collect()
}

enum SearchResponse {
    Resolved(Json<Vec<ResolvedSearchResult>>),
    Raw(Json<Vec<SearchResult>>),
}

impl IntoResponse for SearchResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            SearchResponse::Raw(t) => t.into_response(),
            SearchResponse::Resolved(t) => t.into_response(),
        }
    }
}

#[cfg(not(feature = "no_search"))]
async fn search(
    _session: Session,
    State(server_state): State<ServerState>,
    Json(payload): Json<SearchBody>,
) -> impl IntoResponse {
    let filters = payload.filters.map(|v| {
        v.iter()
            .map(|i| match i {
                SearchFilter::Album => HashedItemType::Album,
                SearchFilter::Artist => HashedItemType::Artist,
                SearchFilter::Track => HashedItemType::Track,
            })
            .collect()
    });
    let search_results =
        server_state
            .search_vault
            .lock()
            .unwrap()
            .search(payload.query.as_str(), 30, filters);
    if payload.resolve {
        SearchResponse::Resolved(Json(resolve_search_results(
            &server_state.catalog_store,
            search_results,
        )))
    } else {
        SearchResponse::Raw(Json(search_results))
    }
}

pub fn make_search_routes(state: ServerState) -> Option<Router> {
    #[cfg(not(feature = "no_search"))]
    {
        Some(
            Router::new()
                .route("/search", post(search))
                .with_state(state),
        )
    }

    #[cfg(feature = "no_search")]
    None
}
