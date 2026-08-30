//! Search API routes

use crate::db_executor::{DbPriority, DbRunError};
use crate::search::resolve;
use crate::search::streaming::{SearchSection, StreamingSearchPipeline};
use crate::search::{
    HashedItemType, RelevanceFilterConfig, ResolvedSearchResult, SearchResult, SearchVault,
};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post, put},
    Json, Router,
};
use futures::stream;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::convert::Infallible;
use std::time::{Duration, Instant};

use super::api_error::ApiError;
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

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SearchMode {
    #[default]
    Strict,
    Expanded,
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

    /// Search strategy. Defaults to strict for backwards-compatible API behavior.
    #[serde(default)]
    pub search_mode: SearchMode,
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

fn run_search(
    search_vault: &dyn SearchVault,
    query: &str,
    limit: usize,
    filters: Option<Vec<HashedItemType>>,
    mode: SearchMode,
) -> Vec<SearchResult> {
    match mode {
        SearchMode::Strict => search_vault.search(query, limit, filters),
        SearchMode::Expanded => search_vault.search_expanded(query, limit, filters),
    }
}

fn run_available_search(
    search_vault: &dyn SearchVault,
    query: &str,
    limit: usize,
    filters: Option<Vec<HashedItemType>>,
    mode: SearchMode,
) -> Vec<SearchResult> {
    match mode {
        SearchMode::Strict => search_vault.search_with_availability(query, limit, filters, true),
        SearchMode::Expanded => {
            search_vault.search_expanded_with_availability(query, limit, filters, true)
        }
    }
}

async fn get_relevance_filter(
    server_state: &ServerState,
) -> Result<RelevanceFilterConfig, DbRunError> {
    match server_state
        .database
        .server
        .run(DbPriority::Interactive, |server_store| {
            server_store.get_state(RELEVANCE_FILTER_CONFIG_KEY)
        })
        .await
    {
        Ok(json) => Ok(json
            .and_then(|json| RelevanceFilterConfig::from_json(&json).ok())
            .unwrap_or_default()),
        Err(DbRunError::Store(_)) => Ok(RelevanceFilterConfig::default()),
        Err(err) => Err(err),
    }
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
) -> Response {
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

    let relevance_filter = match get_relevance_filter(&server_state).await {
        Ok(filter) => filter,
        Err(err) => return ApiError::from(err).into_response(),
    };

    if payload.resolve {
        // For resolved results, fetch more upfront since we need to resolve anyway
        let query = payload.query;
        let mode = payload.search_mode;
        let available_only = payload.exclude_unavailable;
        let search_results = match server_state
            .database
            .search_read
            .run(DbPriority::Interactive, move |search_vault| {
                Ok(if available_only {
                    run_available_search(search_vault, &query, limit, filters, mode)
                } else {
                    run_search(search_vault, &query, limit, filters, mode)
                })
            })
            .await
        {
            Ok(results) => results,
            Err(err) => return ApiError::from(err).into_response(),
        };
        let filtered_results = relevance_filter.filter(search_results);

        let mut resolved = match server_state
            .database
            .catalog_read
            .run(DbPriority::Interactive, move |catalog_store| {
                Ok(filtered_results
                    .iter()
                    .filter_map(|result| {
                        resolve::resolve_to_result(catalog_store, &result.item_id, result.item_type)
                    })
                    .collect())
            })
            .await
        {
            Ok(results) => results,
            Err(err) => return ApiError::from(err).into_response(),
        };

        // Apply availability filter if requested
        if payload.exclude_unavailable {
            resolved = filter_by_availability(resolved);
        }

        SearchResponse::Resolved(Json(resolved)).into_response()
    } else if payload.exclude_unavailable {
        // Use streaming approach to find enough available results
        let query = payload.query;
        let mode = payload.search_mode;
        let results = match server_state
            .database
            .search_read
            .run(DbPriority::Interactive, move |search_vault| {
                Ok(match mode {
                    SearchMode::Strict => {
                        search_vault.search_with_availability(&query, limit, filters, true)
                    }
                    SearchMode::Expanded => {
                        search_vault.search_expanded_with_availability(&query, limit, filters, true)
                    }
                })
            })
            .await
        {
            Ok(results) => relevance_filter.filter(results),
            Err(err) => return ApiError::from(err).into_response(),
        };
        SearchResponse::Raw(Json(results)).into_response()
    } else {
        // No availability filter - simple search
        let query = payload.query;
        let mode = payload.search_mode;
        let search_results = match server_state
            .database
            .search_read
            .run(DbPriority::Interactive, move |search_vault| {
                Ok(run_search(search_vault, &query, limit, filters, mode))
            })
            .await
        {
            Ok(results) => results,
            Err(err) => return ApiError::from(err).into_response(),
        };
        let filtered_results = relevance_filter.filter(search_results);
        SearchResponse::Raw(Json(filtered_results)).into_response()
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
    /// Search strategy. Defaults to strict for backwards-compatible API behavior.
    #[serde(default)]
    search_mode: SearchMode,
}

async fn streaming_search(
    _session: Session,
    State(server_state): State<ServerState>,
    Query(params): Query<StreamingSearchQuery>,
) -> Response {
    let desired_results = server_state.config.streaming_search.top_results_limit
        + server_state.config.streaming_search.other_results_limit;
    let max_results = desired_results + 50;
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(32);
    tokio::spawn(async move {
        let started = Instant::now();
        let query = params.q.clone();
        let mode = params.search_mode;
        let available_results = match server_state
            .database
            .search_read
            .run(DbPriority::Interactive, move |search_vault| {
                Ok(run_available_search(
                    search_vault,
                    &query,
                    max_results,
                    None,
                    mode,
                ))
            })
            .await
        {
            Ok(results) => results,
            Err(_) => return,
        };

        let available_count = available_results.len();
        let seen: HashSet<_> = available_results
            .iter()
            .map(|result| (result.item_type, result.item_id.clone()))
            .collect();
        let pipeline_config = server_state.config.streaming_search.clone();
        let user_manager = server_state.user_manager.clone();
        let query = params.q.clone();
        let available_sections = match server_state
            .database
            .catalog_read
            .run(DbPriority::Interactive, move |catalog_store| {
                let pipeline = StreamingSearchPipeline::new(
                    catalog_store,
                    user_manager.as_ref(),
                    pipeline_config,
                );
                Ok(pipeline.execute(&query, available_results))
            })
            .await
        {
            Ok(sections) => filter_sections_by_availability(sections),
            Err(_) => return,
        };

        // Send the playable phase immediately. Its Done marker is replaced by
        // one final marker after the optional full-catalog phase.
        for section in available_sections
            .into_iter()
            .filter(|section| !matches!(section, SearchSection::Done { .. }))
        {
            let json = serde_json::to_string(&section).unwrap_or_else(|_| "{}".to_string());
            if sender.send(Ok(Event::default().data(json))).await.is_err() {
                return;
            }
        }

        if !params.exclude_unavailable && available_count < desired_results {
            let query = params.q.clone();
            let mode = params.search_mode;
            if let Ok(full_results) = server_state
                .database
                .search_read
                .run(DbPriority::Interactive, move |search_vault| {
                    Ok(run_search(search_vault, &query, max_results, None, mode))
                })
                .await
            {
                let missing = desired_results.saturating_sub(available_count);
                let supplemental: Vec<_> = full_results
                    .into_iter()
                    .filter(|result| !seen.contains(&(result.item_type, result.item_id.clone())))
                    .collect();
                if !supplemental.is_empty() {
                    if let Ok(items) = server_state
                        .database
                        .catalog_read
                        .run(DbPriority::Interactive, move |catalog_store| {
                            Ok(supplemental
                                .into_iter()
                                .filter_map(|result| {
                                    resolve::resolve_to_result(
                                        catalog_store,
                                        &result.item_id,
                                        result.item_type,
                                    )
                                })
                                .filter(|item| !is_result_available(item))
                                .take(missing)
                                .collect::<Vec<_>>())
                        })
                        .await
                    {
                        if !items.is_empty() {
                            let section = SearchSection::MoreResults { items };
                            let json = serde_json::to_string(&section)
                                .unwrap_or_else(|_| "{}".to_string());
                            if sender.send(Ok(Event::default().data(json))).await.is_err() {
                                return;
                            }
                        }
                    }
                }
            }
        }

        let done = SearchSection::Done {
            total_time_ms: started.elapsed().as_millis() as u64,
        };
        let json = serde_json::to_string(&done).unwrap_or_else(|_| "{}".to_string());
        let _ = sender.send(Ok(Event::default().data(json))).await;
    });

    let stream = stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|event| (event, receiver))
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
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
) -> Response {
    let config = match get_relevance_filter(&server_state).await {
        Ok(config) => config,
        Err(err) => return ApiError::from(err).into_response(),
    };
    let config_json = config.to_json();
    Json(RelevanceFilterResponse {
        config,
        config_json,
    })
    .into_response()
}

/// PUT /admin/search/relevance-filter - Update relevance filter configuration
async fn admin_set_relevance_filter(
    _session: Session,
    State(server_state): State<ServerState>,
    Json(new_config): Json<RelevanceFilterConfig>,
) -> Response {
    let json = new_config.to_json();
    let stored_json = json.clone();
    match server_state
        .database
        .server
        .run(DbPriority::Interactive, move |server_store| {
            server_store.set_state(RELEVANCE_FILTER_CONFIG_KEY, &stored_json)
        })
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(RelevanceFilterResponse {
                config: new_config,
                config_json: json,
            }),
        )
            .into_response(),
        Err(DbRunError::Store(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to save config: {}", e)})),
        )
            .into_response(),
        Err(err) => ApiError::from(err).into_response(),
    }
}

/// Creates admin routes for search configuration (requires ServerAdmin permission)
pub fn make_search_admin_routes(state: ServerState) -> Router {
    Router::new()
        .route("/search/relevance-filter", get(admin_get_relevance_filter))
        .route("/search/relevance-filter", put(admin_set_relevance_filter))
        .with_state(state)
}
