//! Skeleton sync API endpoints.
//!
//! These endpoints provide catalog skeleton data for efficient client sync.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use tracing::error;

use super::session::Session;
use super::state::GuardedCatalogStore;
use crate::skeleton::{
    AlbumAddedPayload, FullSkeletonResponse, SkeletonChange, SkeletonDeltaResponse,
    SkeletonEventType, SkeletonVersionResponse, TrackAddedPayload, VersionTooOldError,
};

/// Query parameters for delta endpoint.
#[derive(Deserialize)]
pub struct DeltaQuery {
    pub since: i64,
}

/// GET /v1/catalog/skeleton/version - Get current skeleton version and checksum
pub async fn get_skeleton_version(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
) -> Response {
    let version = match catalog_store.get_skeleton_version() {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to get skeleton version: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let checksum = match catalog_store.get_skeleton_checksum() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to get skeleton checksum: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Json(SkeletonVersionResponse { version, checksum }).into_response()
}

/// GET /v1/catalog/skeleton - Get full skeleton data
pub async fn get_full_skeleton(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
) -> Response {
    let version = match catalog_store.get_skeleton_version() {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to get skeleton version: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let checksum = match catalog_store.get_skeleton_checksum() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to get skeleton checksum: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let artists = match catalog_store.get_all_artist_ids() {
        Ok(ids) => ids,
        Err(e) => {
            error!("Failed to get artist IDs: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let albums = match catalog_store.get_all_albums_skeleton() {
        Ok(a) => a,
        Err(e) => {
            error!("Failed to get albums skeleton: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let tracks = match catalog_store.get_all_tracks_skeleton() {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to get tracks skeleton: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Json(FullSkeletonResponse {
        version,
        checksum,
        artists,
        albums,
        tracks,
    })
    .into_response()
}

/// GET /v1/catalog/skeleton/delta?since=N - Get skeleton changes since version N
pub async fn get_skeleton_delta(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Query(params): Query<DeltaQuery>,
) -> Response {
    let current_version = match catalog_store.get_skeleton_version() {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to get skeleton version: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let earliest = match catalog_store.get_skeleton_earliest_seq() {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to get earliest seq: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // If there are no events yet (earliest == 0) and they're requesting since 0,
    // that's fine - just return empty changes
    // But if they're requesting a version older than our earliest event, return 404
    if earliest > 0 && params.since > 0 && params.since < earliest {
        return (
            StatusCode::NOT_FOUND,
            Json(VersionTooOldError {
                error: "version_too_old".to_string(),
                message: format!(
                    "Version {} is no longer available. Earliest available: {}",
                    params.since, earliest
                ),
                earliest_available: earliest,
                current_version,
            }),
        )
            .into_response();
    }

    let events = match catalog_store.get_skeleton_events_since(params.since) {
        Ok(e) => e,
        Err(e) => {
            error!("Failed to get skeleton events: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Convert events to changes
    let changes: Vec<SkeletonChange> = events
        .into_iter()
        .map(|e| {
            let (artist_ids, album_id) = match e.event_type {
                SkeletonEventType::AlbumAdded => {
                    let payload: Option<AlbumAddedPayload> =
                        e.payload.and_then(|p| serde_json::from_str(&p).ok());
                    (payload.map(|p| p.artist_ids), None)
                }
                SkeletonEventType::TrackAdded => {
                    let payload: Option<TrackAddedPayload> =
                        e.payload.and_then(|p| serde_json::from_str(&p).ok());
                    (None, payload.map(|p| p.album_id))
                }
                _ => (None, None),
            };

            SkeletonChange {
                event_type: e.event_type.as_str().to_string(),
                id: e.entity_id,
                artist_ids,
                album_id,
            }
        })
        .collect();

    let checksum = match catalog_store.get_skeleton_checksum() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to get skeleton checksum: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Json(SkeletonDeltaResponse {
        from_version: params.since,
        to_version: current_version,
        checksum,
        changes,
    })
    .into_response()
}
