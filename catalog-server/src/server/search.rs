use crate::search::{
    HashedItemType, ResolvedSearchResult, SearchResult, SearchedAlbum, SearchedArtist,
    SearchedTrack,
};

use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};
use chrono::Datelike;

use super::{session::Session, state::ServerState};
use crate::catalog::{Album, Artist, Catalog, Track};
use crate::search::SearchVault;
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

fn resolve_album(catalog: &Catalog, album_id: &str) -> Option<ResolvedSearchResult> {
    let album = catalog.get_album(album_id)?;

    let year = chrono::DateTime::from_timestamp(album.date, 0)
        .map(|d| d.year())
        .map(|y| y as i64);

    let resolved_album = SearchedAlbum {
        id: album_id.to_owned(),
        name: album.name,
        artists_ids_names: album
            .artists_ids
            .iter()
            .filter_map(|a_id| catalog.get_artist(a_id))
            .map(|a| (a.id, a.name))
            .collect(),
        image_id: album
            .covers
            .first()
            .or_else(|| album.cover_group.first())
            .map(|i| i.id.to_owned()),
        year,
    };

    Some(ResolvedSearchResult::Album(resolved_album))
}

fn resolve_artist(catalog: &Catalog, artist_id: &str) -> Option<ResolvedSearchResult> {
    let artist = catalog.get_artist(artist_id)?;

    let image_id = artist
        .portraits
        .first()
        .or_else(|| artist.portrait_group.first())
        .map(|i| i.id.to_owned());

    let resolved_artist = SearchedArtist {
        name: artist.name,
        id: artist_id.to_owned(),
        image_id,
    };

    Some(ResolvedSearchResult::Artist(resolved_artist))
}

fn resolve_track(catalog: &Catalog, track_id: &str) -> Option<ResolvedSearchResult> {
    let track = catalog.get_track(track_id)?;

    let artists: Vec<Artist> = track
        .artists_ids
        .iter()
        .filter_map(|artist_id| catalog.get_artist(artist_id))
        .collect();

    let artists_ids_names = artists
        .iter()
        .map(|a| (a.id.clone(), a.name.clone()))
        .collect();

    let image_id = catalog
        .get_album(&track.album_id)
        .map(|a| {
            a.covers
                .first()
                .cloned()
                .or_else(|| a.cover_group.first().cloned())
        })
        .or_else(|| {
            let artist = artists
                .iter()
                .find(|a| !a.portraits.is_empty() || !a.portrait_group.is_empty());
            artist
                .map(|a| a.portraits.first().cloned())
                .or_else(|| artist.map(|a| a.portrait_group.first().cloned()))
        })
        .flatten()
        .map(|i| i.id);

    let resolved_track = SearchedTrack {
        id: track.id,
        name: track.name,
        duration: track.duration as u32,
        image_id,
        artists_ids_names,
        album_id: track.album_id,
    };

    Some(ResolvedSearchResult::Track(resolved_track))
}

fn resolve_search_results(
    catalog: &Catalog,
    results: Vec<SearchResult>,
) -> Vec<ResolvedSearchResult> {
    results
        .iter()
        .filter_map(|result| match result.item_type {
            HashedItemType::Album => resolve_album(catalog, &result.item_id),
            HashedItemType::Artist => resolve_artist(catalog, &result.item_id),
            HashedItemType::Track => resolve_track(catalog, &result.item_id),
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
    session: Session,
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
            &server_state.catalog.lock().unwrap(),
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
