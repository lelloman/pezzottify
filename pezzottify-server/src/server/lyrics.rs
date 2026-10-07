use super::{api_error::ApiError, session::Session, state::ServerState};
use crate::{catalog_store::TrackAvailability, db_executor::DbPriority, lyrics::LyricsFetcher};
use simple_server::{
    extract::Extract,
    web::{
        extract::{Path, State},
        http::{header, StatusCode},
        response::{IntoResponse, Response},
        routing::{get, post},
        Json, Router,
    },
};
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;

// Bound detached user work independently of the daily batch and HTTP read limits.
static DOWNLOAD_SLOTS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(4)));

pub fn read_routes() -> Router<ServerState> {
    Router::new().route("/track/{id}/lyrics", get(get_lyrics))
}
pub fn write_routes() -> Router<ServerState> {
    Router::new().route("/lyrics/{entity_type}/{id}/download", post(download))
}
async fn get_lyrics(
    Extract(_session): Extract<Session>,
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> Response {
    match state
        .database
        .catalog_read
        .run(DbPriority::Interactive, move |store| {
            if store.get_track(&id)?.is_none() {
                return Ok(None);
            }
            Ok(Some(store.get_track_lyrics(&id)?))
        })
        .await
    {
        Ok(Some(lyrics)) => ([(header::CACHE_CONTROL, "no-store")], Json(lyrics)).into_response(),
        Ok(None) => ApiError::not_found("track_not_found", "Track not found").into_response(),
        Err(error) => ApiError::from(error).into_response(),
    }
}
async fn download(
    Extract(_session): Extract<Session>,
    State(state): State<ServerState>,
    Path((entity_type, id)): Path<(String, String)>,
) -> Response {
    if !matches!(entity_type.as_str(), "track" | "album") {
        return ApiError::bad_request(
            "invalid_entity_type",
            "Lyrics downloads support tracks and albums",
        )
        .into_response();
    }
    let ids = state
        .database
        .catalog_read
        .run(DbPriority::Interactive, move |store| {
            if entity_type == "track" {
                Ok(store.get_track(&id)?.map(|track| {
                    if track.availability == TrackAvailability::Available {
                        vec![id]
                    } else {
                        vec![]
                    }
                }))
            } else {
                if store.get_album_json(&id)?.is_none() {
                    return Ok(None);
                }
                store.get_available_album_track_ids(&id).map(Some)
            }
        })
        .await;
    let ids = match ids {
        Ok(Some(ids)) => ids,
        Ok(None) => {
            return ApiError::not_found("catalog_item_not_found", "Track or album not found")
                .into_response()
        }
        Err(error) => return ApiError::from(error).into_response(),
    };
    if ids.is_empty() {
        return ApiError::bad_request(
            "no_available_tracks",
            "No available tracks to fetch lyrics for",
        )
        .into_response();
    }
    if ids.len() > 500 {
        return ApiError::bad_request(
            "album_too_large",
            "Request lyrics for individual tracks in albums with more than 500 tracks",
        )
        .into_response();
    }
    let permit =
        match DOWNLOAD_SLOTS.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => return (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, "30")],
                Json(
                    serde_json::json!({"message": "Lyrics downloads are busy. Try again shortly."}),
                ),
            )
                .into_response(),
        };
    let guard = match state.runtime_tasks.tasks.token() {
        Ok(guard) => guard,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let fetcher = match LyricsFetcher::new() {
        Ok(fetcher) => fetcher,
        Err(error) => return ApiError::internal("Create lyrics client", error).into_response(),
    };
    let count = ids.len();
    tokio::spawn(async move {
        let (_permit, _guard) = (permit, guard);
        tokio::select! {
            _ = state.runtime_tasks.shutdown.requested() => {},
            result = fetcher.download(&state.database.catalog_write, ids, DbPriority::Interactive, true) => {
                match result {
                    Ok(summary) => tracing::info!(?summary, "Requested lyrics download completed"),
                    Err(error) => tracing::warn!(%error, "Requested lyrics download failed"),
                }
            }
        }
    });
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"status": "accepted", "tracks": count})),
    )
        .into_response()
}
