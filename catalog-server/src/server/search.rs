//! Search API routes

use crate::catalog_store::CatalogStore;
use crate::search::streaming::{SearchSection, StreamingSearchPipeline};
use crate::search::{
    HashedItemType, RelevanceFilterConfig, ResolvedSearchResult, SearchResult, SearchVault,
    SearchedAlbum, SearchedArtist, SearchedTrack,
};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post, put},
    Json, Router,
};
use chrono::Datelike;
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use super::session::Session;
use super::state::ServerState;

/// Key for storing relevance filter configuration in server_store
pub const RELEVANCE_FILTER_CONFIG_KEY: &str = "search.relevance_filter";

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

    /// Maximum number of results to return (default: 30)
    pub limit: Option<usize>,

    pub filters: Option<Vec<SearchFilter>>,

    /// If true, exclude unavailable content from results
    #[serde(default)]
    pub exclude_unavailable: bool,
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

    // Get album availability from the album data
    let availability = album_json
        .get("album")
        .and_then(|a| a.get("album_availability"))
        .and_then(|av| av.as_str())
        .unwrap_or("missing")
        .to_string();

    let resolved_album = SearchedAlbum {
        id: album_id.to_owned(),
        name,
        artists_ids_names,
        image_id,
        year,
        availability,
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

    // Get availability (defaults to false if not present)
    let available = artist_json
        .get("artist")
        .and_then(|a| a.get("available"))
        .and_then(|a| a.as_bool())
        .unwrap_or(false);

    let resolved_artist = SearchedArtist {
        name,
        id: artist_id.to_owned(),
        image_id,
        available,
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

fn get_relevance_filter(server_state: &ServerState) -> RelevanceFilterConfig {
    server_state
        .server_store
        .get_state(RELEVANCE_FILTER_CONFIG_KEY)
        .ok()
        .flatten()
        .and_then(|json| RelevanceFilterConfig::from_json(&json).ok())
        .unwrap_or_default()
}

/// Filter resolved search results by availability
fn filter_by_availability(results: Vec<ResolvedSearchResult>) -> Vec<ResolvedSearchResult> {
    results
        .into_iter()
        .filter(|result| match result {
            ResolvedSearchResult::Track(track) => track.availability == "available",
            ResolvedSearchResult::Album(album) => album.availability != "missing",
            ResolvedSearchResult::Artist(artist) => artist.available,
        })
        .collect()
}

/// Check if a resolved search result is available
fn is_result_available(result: &ResolvedSearchResult) -> bool {
    match result {
        ResolvedSearchResult::Track(track) => track.availability == "available",
        ResolvedSearchResult::Album(album) => album.availability != "missing",
        ResolvedSearchResult::Artist(artist) => artist.available,
    }
}

/// Search with availability filtering using the denormalized availability index.
/// This is much more efficient than the old batch post-filtering approach.
fn search_with_availability_filter(
    search_vault: &dyn SearchVault,
    _catalog_store: &Arc<dyn CatalogStore>, // No longer needed for filtering
    relevance_filter: &RelevanceFilterConfig,
    query: &str,
    limit: usize,
    filters: Option<Vec<HashedItemType>>,
) -> Vec<SearchResult> {
    // Use the new availability-aware search that does filtering at query time
    let results = search_vault.search_with_availability(query, limit, filters, true);
    relevance_filter.filter(results)
}

/// Filter streaming search sections by availability
fn filter_sections_by_availability(sections: Vec<SearchSection>) -> Vec<SearchSection> {
    sections
        .into_iter()
        .filter_map(|section| match section {
            // Filter primary matches - skip if unavailable
            SearchSection::PrimaryArtist { item, confidence } => {
                if is_result_available(&item) {
                    Some(SearchSection::PrimaryArtist { item, confidence })
                } else {
                    None
                }
            }
            SearchSection::PrimaryAlbum { item, confidence } => {
                if is_result_available(&item) {
                    Some(SearchSection::PrimaryAlbum { item, confidence })
                } else {
                    None
                }
            }
            SearchSection::PrimaryTrack { item, confidence } => {
                if is_result_available(&item) {
                    Some(SearchSection::PrimaryTrack { item, confidence })
                } else {
                    None
                }
            }
            // Filter enrichment sections - keep only available items
            SearchSection::PopularBy {
                target_id,
                target_type,
                items,
            } => {
                let filtered: Vec<_> = items.into_iter().filter(|t| t.available).collect();
                if filtered.is_empty() {
                    None
                } else {
                    Some(SearchSection::PopularBy {
                        target_id,
                        target_type,
                        items: filtered,
                    })
                }
            }
            SearchSection::AlbumsBy { target_id, items } => {
                let filtered: Vec<_> = items
                    .into_iter()
                    .filter(|a| a.availability != "missing")
                    .collect();
                if filtered.is_empty() {
                    None
                } else {
                    Some(SearchSection::AlbumsBy {
                        target_id,
                        items: filtered,
                    })
                }
            }
            SearchSection::TracksFrom { target_id, items } => {
                let filtered: Vec<_> = items.into_iter().filter(|t| t.available).collect();
                if filtered.is_empty() {
                    None
                } else {
                    Some(SearchSection::TracksFrom {
                        target_id,
                        items: filtered,
                    })
                }
            }
            SearchSection::RelatedArtists { target_id, items } => {
                let filtered: Vec<_> = items.into_iter().filter(|a| a.available).collect();
                if filtered.is_empty() {
                    None
                } else {
                    Some(SearchSection::RelatedArtists {
                        target_id,
                        items: filtered,
                    })
                }
            }
            // Filter result sections
            SearchSection::MoreResults { items } => {
                let filtered = filter_by_availability(items);
                if filtered.is_empty() {
                    None
                } else {
                    Some(SearchSection::MoreResults { items: filtered })
                }
            }
            SearchSection::Results { items } => {
                let filtered = filter_by_availability(items);
                if filtered.is_empty() {
                    None
                } else {
                    Some(SearchSection::Results { items: filtered })
                }
            }
            // Always keep Done
            SearchSection::Done { total_time_ms } => Some(SearchSection::Done { total_time_ms }),
        })
        .collect()
}

async fn search(
    _session: Session,
    State(server_state): State<ServerState>,
    Json(payload): Json<SearchBody>,
) -> impl IntoResponse {
    let limit = payload.limit.unwrap_or(30).min(100); // Cap at 100 max
    let filters: Option<Vec<HashedItemType>> = payload.filters.map(|v| {
        v.iter()
            .map(|i| match i {
                SearchFilter::Album => HashedItemType::Album,
                SearchFilter::Artist => HashedItemType::Artist,
                SearchFilter::Track => HashedItemType::Track,
            })
            .collect()
    });

    let relevance_filter = get_relevance_filter(&server_state);

    if payload.resolve {
        // For resolved results, fetch more upfront since we need to resolve anyway
        let search_results =
            server_state
                .search_vault
                .search(payload.query.as_str(), limit, filters);
        let filtered_results = relevance_filter.filter(search_results);

        let mut resolved = resolve_search_results(&server_state.catalog_store, filtered_results);

        // Apply availability filter if requested
        if payload.exclude_unavailable {
            resolved = filter_by_availability(resolved);
        }

        SearchResponse::Resolved(Json(resolved))
    } else if payload.exclude_unavailable {
        // Use streaming approach to find enough available results
        let results = search_with_availability_filter(
            server_state.search_vault.as_ref(),
            &server_state.catalog_store,
            &relevance_filter,
            &payload.query,
            limit,
            filters,
        );
        SearchResponse::Raw(Json(results))
    } else {
        // No availability filter - simple search
        let search_results =
            server_state
                .search_vault
                .search(payload.query.as_str(), limit, filters);
        let filtered_results = relevance_filter.filter(search_results);
        SearchResponse::Raw(Json(filtered_results))
    }
}

// =============================================================================
// Streaming Search (SSE)
// =============================================================================

#[derive(Deserialize)]
struct StreamingSearchQuery {
    /// The search query string
    q: String,
    /// If true, exclude unavailable content from results
    #[serde(default)]
    exclude_unavailable: bool,
}

async fn streaming_search(
    _session: Session,
    State(server_state): State<ServerState>,
    Query(params): Query<StreamingSearchQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Run organic search first
    let max_results = server_state.config.streaming_search.top_results_limit
        + server_state.config.streaming_search.other_results_limit
        + 50;
    let search_results = server_state
        .search_vault
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

    // Apply availability filter if requested
    let sections = if params.exclude_unavailable {
        filter_sections_by_availability(sections)
    } else {
        sections
    };

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

pub fn make_search_routes(state: ServerState) -> Router {
    Router::new()
        .route("/search", post(search))
        .route("/search/stream", get(streaming_search))
        .with_state(state)
}

// =============================================================================
// Admin endpoints for relevance filter configuration
// =============================================================================

#[derive(Serialize)]
struct RelevanceFilterResponse {
    config: RelevanceFilterConfig,
    config_json: String,
}

/// GET /admin/search/relevance-filter - Get current relevance filter configuration
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
pub fn make_search_admin_routes(state: ServerState) -> Router {
    Router::new()
        .route("/search/relevance-filter", get(admin_get_relevance_filter))
        .route("/search/relevance-filter", put(admin_set_relevance_filter))
        .with_state(state)
}
