//! Recommendation routes for smart continuation and radio playback.

use std::collections::HashSet;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::catalog_store::{CatalogStore, TrackAvailability};
use crate::config::AudioEmbeddingsSettings;

use super::session::Session;
use super::state::ServerState;

const DEFAULT_TRACK_NAMESPACE: &str = "musicfm.mean.v1";
const DEFAULT_ALBUM_NAMESPACE: &str = "album.musicfm.median.v1";
const CONTINUATION_CONTEXT_LIMIT: usize = 10;
const ARTIST_SEED_TRACK_LIMIT: usize = 50;

#[derive(Debug, Deserialize)]
struct ContinuationRequest {
    #[serde(default)]
    context_track_ids: Vec<String>,
    #[serde(default)]
    exclude_track_ids: Vec<String>,
    count: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RadioQuery {
    count: Option<usize>,
}

#[derive(Debug, Serialize)]
struct TrackIdsResponse {
    track_ids: Vec<String>,
}

pub fn recommendation_routes() -> Router<ServerState> {
    Router::new()
        .route(
            "/recommendations/continuation",
            post(post_continuation_recommendations),
        )
        .route("/radio/{entity_type}/{entity_id}", get(get_radio))
}

async fn post_continuation_recommendations(
    _session: Session,
    State(state): State<ServerState>,
    Json(body): Json<ContinuationRequest>,
) -> Response {
    let count = body.count.unwrap_or(1).clamp(1, 10);
    let catalog_store = Arc::clone(&state.catalog_store);
    let namespace = track_namespace(state.config.audio_embeddings.as_ref());

    match tokio::task::spawn_blocking(move || {
        let seed = weighted_track_vector(
            catalog_store.as_ref(),
            &namespace,
            &body.context_track_ids,
            CONTINUATION_CONTEXT_LIMIT,
        )?;
        let Some(seed) = seed else {
            return Ok(Vec::new());
        };

        let exclude = body.exclude_track_ids.into_iter().collect::<HashSet<_>>();
        recommend_tracks(catalog_store.as_ref(), &namespace, &seed, count, &exclude)
    })
    .await
    {
        Ok(Ok(track_ids)) => no_store_json(TrackIdsResponse { track_ids }),
        Ok(Err(err)) => {
            error!("Error generating continuation recommendations: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate recommendations".to_string(),
            )
                .into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        )
            .into_response(),
    }
}

async fn get_radio(
    _session: Session,
    State(state): State<ServerState>,
    Path((entity_type, entity_id)): Path<(String, String)>,
    Query(query): Query<RadioQuery>,
) -> Response {
    let count = query.count.unwrap_or(50).clamp(1, 200);
    let catalog_store = Arc::clone(&state.catalog_store);
    let namespace = track_namespace(state.config.audio_embeddings.as_ref());
    let album_namespace = album_namespace_for_track_namespace(&namespace);

    match tokio::task::spawn_blocking(move || match entity_type.as_str() {
        "track" => track_radio(catalog_store.as_ref(), &namespace, &entity_id, count),
        "album" => album_radio(
            catalog_store.as_ref(),
            &namespace,
            &album_namespace,
            &entity_id,
            count,
        ),
        "artist" => artist_radio(catalog_store.as_ref(), &namespace, &entity_id, count),
        _ => Err(anyhow::anyhow!(
            "unsupported radio entity_type '{}'",
            entity_type
        )),
    })
    .await
    {
        Ok(Ok(track_ids)) => no_store_json(TrackIdsResponse { track_ids }),
        Ok(Err(err)) => {
            let status = if err.to_string().contains("unsupported radio entity_type") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            error!("Error generating radio: {}", err);
            (status, err.to_string()).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        )
            .into_response(),
    }
}

fn no_store_json<T: Serialize>(value: T) -> Response {
    let mut response = Json(value).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
}

fn track_namespace(settings: Option<&AudioEmbeddingsSettings>) -> String {
    let Some(settings) = settings else {
        return DEFAULT_TRACK_NAMESPACE.to_string();
    };
    settings
        .specs
        .iter()
        .find(|spec| spec.namespace == DEFAULT_TRACK_NAMESPACE)
        .or_else(|| settings.specs.first())
        .map(|spec| spec.namespace.clone())
        .unwrap_or_else(|| DEFAULT_TRACK_NAMESPACE.to_string())
}

fn album_namespace_for_track_namespace(namespace: &str) -> String {
    match namespace {
        "musicfm.mean.v1" => DEFAULT_ALBUM_NAMESPACE.to_string(),
        "ast.audioset.v1" => "album.ast.median.v1".to_string(),
        "ast.instruments.v1" => "album.ast_instruments.median.v1".to_string(),
        _ => format!("album.{namespace}.median"),
    }
}

fn track_radio(
    catalog_store: &dyn CatalogStore,
    namespace: &str,
    track_id: &str,
    count: usize,
) -> anyhow::Result<Vec<String>> {
    let mut result = Vec::new();
    let mut exclude = HashSet::new();

    if track_is_available(catalog_store, track_id)? {
        result.push(track_id.to_string());
        exclude.insert(track_id.to_string());
    }

    if result.len() >= count {
        return Ok(result);
    }

    let Some(seed) = get_vector(catalog_store, "track", track_id, namespace)? else {
        return Ok(result);
    };
    let mut recommendations = recommend_tracks(
        catalog_store,
        namespace,
        &seed,
        count.saturating_sub(result.len()),
        &exclude,
    )?;
    result.append(&mut recommendations);
    Ok(result)
}

fn album_radio(
    catalog_store: &dyn CatalogStore,
    track_namespace: &str,
    album_namespace: &str,
    album_id: &str,
    count: usize,
) -> anyhow::Result<Vec<String>> {
    let album_track_ids = catalog_store.get_available_album_track_ids(album_id)?;
    let exclude = album_track_ids.iter().cloned().collect::<HashSet<_>>();

    let seed = match get_vector(catalog_store, "album", album_id, album_namespace)? {
        Some(vector) => Some(vector),
        None => mean_track_vector(catalog_store, track_namespace, &album_track_ids)?,
    };
    let Some(seed) = seed else {
        return Ok(Vec::new());
    };

    recommend_tracks(catalog_store, track_namespace, &seed, count, &exclude)
}

fn artist_radio(
    catalog_store: &dyn CatalogStore,
    namespace: &str,
    artist_id: &str,
    count: usize,
) -> anyhow::Result<Vec<String>> {
    let top_tracks = catalog_store.get_artist_top_track_ids(artist_id, ARTIST_SEED_TRACK_LIMIT)?;
    let mut result = Vec::new();
    let mut exclude = HashSet::new();

    if let Some(first) = top_tracks.first() {
        result.push(first.clone());
        exclude.insert(first.clone());
    }
    if result.len() >= count {
        return Ok(result);
    }

    let Some(seed) = mean_track_vector(catalog_store, namespace, &top_tracks)? else {
        return Ok(result);
    };
    let mut recommendations = recommend_tracks(
        catalog_store,
        namespace,
        &seed,
        count.saturating_sub(result.len()),
        &exclude,
    )?;
    result.append(&mut recommendations);
    Ok(result)
}

fn weighted_track_vector(
    catalog_store: &dyn CatalogStore,
    namespace: &str,
    track_ids: &[String],
    max_tracks: usize,
) -> anyhow::Result<Option<Vec<f32>>> {
    let recent = track_ids.iter().rev().take(max_tracks);
    let mut weighted_vectors = Vec::new();
    let mut weight = 1.0_f32;

    for track_id in recent {
        if let Some(vector) = get_vector(catalog_store, "track", track_id, namespace)? {
            weighted_vectors.push((vector, weight));
        }
        weight *= 0.85;
    }

    weighted_mean(weighted_vectors)
}

fn mean_track_vector(
    catalog_store: &dyn CatalogStore,
    namespace: &str,
    track_ids: &[String],
) -> anyhow::Result<Option<Vec<f32>>> {
    let mut vectors = Vec::new();
    for track_id in track_ids {
        if let Some(vector) = get_vector(catalog_store, "track", track_id, namespace)? {
            vectors.push((vector, 1.0));
        }
    }
    weighted_mean(vectors)
}

fn weighted_mean(vectors: Vec<(Vec<f32>, f32)>) -> anyhow::Result<Option<Vec<f32>>> {
    let Some((first, _)) = vectors.first() else {
        return Ok(None);
    };
    let dim = first.len();
    let mut out = vec![0.0_f32; dim];
    let mut total_weight = 0.0_f32;

    for (vector, weight) in vectors {
        if vector.len() != dim {
            continue;
        }
        for (idx, value) in vector.iter().enumerate() {
            out[idx] += *value * weight;
        }
        total_weight += weight;
    }

    if total_weight <= f32::EPSILON {
        return Ok(None);
    }
    for value in &mut out {
        *value /= total_weight;
    }
    Ok(Some(out))
}

fn get_vector(
    catalog_store: &dyn CatalogStore,
    entity_type: &str,
    entity_id: &str,
    namespace: &str,
) -> anyhow::Result<Option<Vec<f32>>> {
    Ok(catalog_store
        .get_entity_embedding(entity_type, entity_id, namespace, true)?
        .and_then(|embedding| embedding.vector))
}

fn recommend_tracks(
    catalog_store: &dyn CatalogStore,
    namespace: &str,
    seed: &[f32],
    count: usize,
    exclude: &HashSet<String>,
) -> anyhow::Result<Vec<String>> {
    if count == 0 {
        return Ok(Vec::new());
    }

    let oversample = (count * 16).clamp(100, 1000);
    let results =
        catalog_store.search_entity_embeddings(namespace, seed, Some("track"), oversample)?;

    let mut rng = rand::rng();
    let mut scored = results
        .into_iter()
        .map(|result| (result.entity_id, result.score + rng.random_range(0.0..0.03)))
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut selected = Vec::with_capacity(count);
    let mut seen = exclude.clone();
    for (track_id, _) in scored {
        if selected.len() >= count {
            break;
        }
        if !seen.insert(track_id.clone()) {
            continue;
        }
        if !track_is_available(catalog_store, &track_id)? {
            continue;
        }
        selected.push(track_id);
    }

    Ok(selected)
}

fn track_is_available(catalog_store: &dyn CatalogStore, track_id: &str) -> anyhow::Result<bool> {
    Ok(catalog_store
        .get_track(track_id)?
        .map(|track| track.availability == TrackAvailability::Available)
        .unwrap_or(false))
}
