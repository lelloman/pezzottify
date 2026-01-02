//! Search API routes
//! Note: Functions are feature-gated and used as route handlers

#![allow(dead_code)] // Feature-gated search functionality

#[cfg(not(feature = "no_search"))]
use crate::catalog_store::CatalogStore;
#[cfg(not(feature = "no_search"))]
use crate::search::streaming::StreamingSearchPipeline;
#[cfg(not(feature = "no_search"))]
use crate::search::{
    HashedItemType, RelevanceFilterConfig, ResolvedSearchResult, SearchResult, SearchedAlbum,
    SearchedArtist, SearchedTrack,
};

#[cfg(feature = "no_search")]
use axum::Router;
#[cfg(not(feature = "no_search"))]
use axum::{
    extract::{Query, State},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
#[cfg(not(feature = "no_search"))]
use chrono::Datelike;
#[cfg(not(feature = "no_search"))]
use futures::stream::{self, Stream};
#[cfg(not(feature = "no_search"))]
use std::convert::Infallible;
#[cfg(not(feature = "no_search"))]
use std::sync::Arc;
#[cfg(not(feature = "no_search"))]
use std::time::Duration;

#[cfg(not(feature = "no_search"))]
use super::session::Session;
use super::state::ServerState;
#[cfg(not(feature = "no_search"))]
use serde::Deserialize;

/// Key for storing relevance filter configuration in server_store
#[cfg(not(feature = "no_search"))]
pub const RELEVANCE_FILTER_CONFIG_KEY: &str = "search.relevance_filter";

#[cfg(not(feature = "no_search"))]
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchFilter {
    Album,
    Artist,
    Track,
}

#[cfg(not(feature = "no_search"))]
#[derive(Deserialize)]
struct SearchBody {
    pub query: String,

    #[serde(default)]
    pub resolve: bool,

    pub filters: Option<Vec<SearchFilter>>,
}

#[cfg(not(feature = "no_search"))]
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

    // Get release date/year from the album (field is renamed to "date" in JSON)
    let year = album_json
        .get("album")
        .and_then(|a| a.get("date"))
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

#[cfg(not(feature = "no_search"))]
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

#[cfg(not(feature = "no_search"))]
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

    // Get availability (defaults to "available" if not present)
    let availability = track_json
        .get("availability")
        .and_then(|a| a.as_str())
        .unwrap_or("available")
        .to_string();

    let resolved_track = SearchedTrack {
        id,
        name,
        duration,
        image_id,
        artists_ids_names,
        album_id,
        availability,
    };

    Some(ResolvedSearchResult::Track(resolved_track))
}

#[cfg(not(feature = "no_search"))]
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

#[cfg(not(feature = "no_search"))]
enum SearchResponse {
    Resolved(Json<Vec<ResolvedSearchResult>>),
    Raw(Json<Vec<SearchResult>>),
}

#[cfg(not(feature = "no_search"))]
impl IntoResponse for SearchResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            SearchResponse::Raw(t) => t.into_response(),
            SearchResponse::Resolved(t) => t.into_response(),
        }
    }
}

#[cfg(not(feature = "no_search"))]
fn get_relevance_filter(server_state: &ServerState) -> RelevanceFilterConfig {
    server_state
        .server_store
        .get_state(RELEVANCE_FILTER_CONFIG_KEY)
        .ok()
        .flatten()
        .and_then(|json| RelevanceFilterConfig::from_json(&json).ok())
        .unwrap_or_default()
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

    // Apply relevance filtering as post-processing step
    let relevance_filter = get_relevance_filter(&server_state);
    let filtered_results = relevance_filter.filter(search_results);

    if payload.resolve {
        SearchResponse::Resolved(Json(resolve_search_results(
            &server_state.catalog_store,
            filtered_results,
        )))
    } else {
        SearchResponse::Raw(Json(filtered_results))
    }
}

// =============================================================================
// Streaming Search (SSE)
// =============================================================================

#[cfg(not(feature = "no_search"))]
#[derive(Deserialize)]
struct StreamingSearchQuery {
    /// The search query string
    q: String,
}

#[cfg(not(feature = "no_search"))]
async fn streaming_search(
    _session: Session,
    State(server_state): State<ServerState>,
    Query(params): Query<StreamingSearchQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Run organic search first
    let max_results = server_state.config.streaming_search.top_results_limit
        + server_state.config.streaming_search.other_results_limit
        + 50;
    let search_results =
        server_state
            .search_vault
            .lock()
            .unwrap()
            .search(&params.q, max_results, None);

    // Get the user_manager from state
    let user_manager = server_state.user_manager.lock().unwrap();

    // Build the pipeline with config from server state
    let pipeline = StreamingSearchPipeline::new(
        server_state.catalog_store.as_ref(),
        &user_manager,
        server_state.config.streaming_search.clone(),
    );

    // Execute the pipeline with search results
    let sections = pipeline.execute(&params.q, search_results);
    drop(user_manager);

    // Convert sections to SSE events
    let events: Vec<_> = sections
        .into_iter()
        .map(|section| {
            let json = serde_json::to_string(&section).unwrap_or_else(|_| "{}".to_string());
            Event::default().data(json)
        })
        .collect();

    // Create a stream from the collected events
    let stream = stream::iter(events.into_iter().map(Ok));

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

#[cfg(not(feature = "no_search"))]
pub fn make_search_routes(state: ServerState) -> Option<Router> {
    Some(
        Router::new()
            .route("/search", post(search))
            .route("/search/stream", get(streaming_search))
            .with_state(state),
    )
}

#[cfg(feature = "no_search")]
pub fn make_search_routes(_state: ServerState) -> Option<Router> {
    None
}

// =============================================================================
// Admin endpoints for relevance filter configuration
// =============================================================================

#[cfg(not(feature = "no_search"))]
use axum::http::StatusCode;
#[cfg(not(feature = "no_search"))]
use serde::Serialize;

#[cfg(not(feature = "no_search"))]
#[derive(Serialize)]
struct RelevanceFilterResponse {
    config: RelevanceFilterConfig,
    config_json: String,
}

/// GET /admin/search/relevance-filter - Get current relevance filter configuration
#[cfg(not(feature = "no_search"))]
async fn admin_get_relevance_filter(
    _session: Session,
    State(server_state): State<ServerState>,
) -> impl IntoResponse {
    let config = get_relevance_filter(&server_state);
    let config_json = config.to_json();
    Json(RelevanceFilterResponse {
        config,
        config_json,
    })
}

/// PUT /admin/search/relevance-filter - Update relevance filter configuration
#[cfg(not(feature = "no_search"))]
async fn admin_set_relevance_filter(
    _session: Session,
    State(server_state): State<ServerState>,
    Json(new_config): Json<RelevanceFilterConfig>,
) -> impl IntoResponse {
    let json = new_config.to_json();
    match server_state
        .server_store
        .set_state(RELEVANCE_FILTER_CONFIG_KEY, &json)
    {
        Ok(()) => (
            StatusCode::OK,
            Json(RelevanceFilterResponse {
                config: new_config,
                config_json: json,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to save config: {}", e)})),
        )
            .into_response(),
    }
}

/// Creates admin routes for search configuration (requires ServerAdmin permission)
#[cfg(not(feature = "no_search"))]
pub fn make_search_admin_routes(state: ServerState) -> Option<Router> {
    use axum::routing::put;
    Some(
        Router::new()
            .route("/search/relevance-filter", get(admin_get_relevance_filter))
            .route("/search/relevance-filter", put(admin_set_relevance_filter))
            .with_state(state),
    )
}

#[cfg(feature = "no_search")]
pub fn make_search_admin_routes(_state: ServerState) -> Option<Router> {
    None
}
