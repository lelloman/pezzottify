//! HTTP server implementation with route handlers
//! Note: Many functions appear unused but are registered as route handlers

#![allow(dead_code)] // Route handlers registered dynamically

use anyhow::Result;
use std::{
    fs::File,
    io::{BufReader, Read},
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::config::SslSettings;

use tracing::{debug, error, info, warn};

use crate::background_jobs::{JobError, JobInfo, SchedulerHandle};
use crate::catalog_store::CatalogStore;
use crate::{search::SearchVault, user::UserManager};
use crate::{
    server::stream_track::stream_track,
    user::{
        device::DeviceRegistration, settings::UserSetting, sync_events::UserEvent,
        user_models::LikedContentType, FullUserStore, Permission,
    },
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use tower_http::services::{ServeDir, ServeFile};

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, response, HeaderValue, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_governor::GovernorLayer;

#[cfg(feature = "slowdown")]
use super::slowdown_request;
use super::{
    http_cache, log_requests, make_search_routes, state::*, IpKeyExtractor, RequestsLoggingLevel,
    ServerConfig, UserOrIpKeyExtractor, CONTENT_READ_PER_MINUTE, GLOBAL_PER_MINUTE,
    LOGIN_PER_MINUTE, SEARCH_PER_MINUTE, STREAM_PER_MINUTE, WRITE_PER_MINUTE,
};
use crate::server::session::Session;
use crate::user::auth::AuthTokenValue;
use axum::extract::Request;
use axum::middleware::Next;
use tower_governor::governor::GovernorConfigBuilder;

const MAX_DEVICES_PER_USER: usize = 50;

#[derive(Serialize)]
struct ServerStats {
    pub uptime: String,
    pub hash: String,
    pub session_token: Option<String>,
}

fn format_uptime(duration: Duration) -> String {
    let total_seconds = duration.as_secs();

    let days = total_seconds / 86_400;
    let hours = (total_seconds % 86_400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    format!("{}d {:02}:{:02}:{:02}", days, hours, minutes, seconds)
}

async fn require_access_catalog(
    session: Session,
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    debug!(
        "require_access_catalog: user_id={}, has_permission={}, permissions={:?}",
        session.user_id,
        session.has_permission(Permission::AccessCatalog),
        session.permissions
    );
    if !session.has_permission(Permission::AccessCatalog) {
        debug!(
            "require_access_catalog: FORBIDDEN - user_id={} lacks AccessCatalog permission",
            session.user_id
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!(
        "require_access_catalog: ALLOWED - user_id={}",
        session.user_id
    );
    next.run(request).await
}

async fn require_like_content(
    session: Session,
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    debug!(
        "require_like_content: user_id={}, has_permission={}, permissions={:?}",
        session.user_id,
        session.has_permission(Permission::LikeContent),
        session.permissions
    );
    if !session.has_permission(Permission::LikeContent) {
        debug!(
            "require_like_content: FORBIDDEN - user_id={} lacks LikeContent permission",
            session.user_id
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!(
        "require_like_content: ALLOWED - user_id={}",
        session.user_id
    );
    next.run(request).await
}

async fn require_own_playlists(
    session: Session,
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    debug!(
        "require_own_playlists: user_id={}, has_permission={}, permissions={:?}",
        session.user_id,
        session.has_permission(Permission::OwnPlaylists),
        session.permissions
    );
    if !session.has_permission(Permission::OwnPlaylists) {
        debug!(
            "require_own_playlists: FORBIDDEN - user_id={} lacks OwnPlaylists permission",
            session.user_id
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!(
        "require_own_playlists: ALLOWED - user_id={}",
        session.user_id
    );
    next.run(request).await
}

async fn require_edit_catalog(
    session: Session,
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    debug!(
        "require_edit_catalog: user_id={}, has_permission={}, permissions={:?}",
        session.user_id,
        session.has_permission(Permission::EditCatalog),
        session.permissions
    );
    if !session.has_permission(Permission::EditCatalog) {
        debug!(
            "require_edit_catalog: FORBIDDEN - user_id={} lacks EditCatalog permission",
            session.user_id
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!(
        "require_edit_catalog: ALLOWED - user_id={}",
        session.user_id
    );
    next.run(request).await
}

async fn require_server_admin(
    session: Session,
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    debug!(
        "require_server_admin: user_id={}, has_permission={}, permissions={:?}",
        session.user_id,
        session.has_permission(Permission::ServerAdmin),
        session.permissions
    );
    if !session.has_permission(Permission::ServerAdmin) {
        debug!(
            "require_server_admin: FORBIDDEN - user_id={} lacks ServerAdmin permission",
            session.user_id
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!(
        "require_server_admin: ALLOWED - user_id={}",
        session.user_id
    );
    next.run(request).await
}

async fn require_manage_permissions(
    session: Session,
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    debug!(
        "require_manage_permissions: user_id={}, has_permission={}, permissions={:?}",
        session.user_id,
        session.has_permission(Permission::ManagePermissions),
        session.permissions
    );
    if !session.has_permission(Permission::ManagePermissions) {
        debug!(
            "require_manage_permissions: FORBIDDEN - user_id={} lacks ManagePermissions permission",
            session.user_id
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!(
        "require_manage_permissions: ALLOWED - user_id={}",
        session.user_id
    );
    next.run(request).await
}

async fn require_view_analytics(
    session: Session,
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    debug!(
        "require_view_analytics: user_id={}, has_permission={}, permissions={:?}",
        session.user_id,
        session.has_permission(Permission::ViewAnalytics),
        session.permissions
    );
    if !session.has_permission(Permission::ViewAnalytics) {
        debug!(
            "require_view_analytics: FORBIDDEN - user_id={} lacks ViewAnalytics permission",
            session.user_id
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!(
        "require_view_analytics: ALLOWED - user_id={}",
        session.user_id
    );
    next.run(request).await
}

#[derive(Deserialize, Debug)]
struct LoginBody {
    pub user_handle: String,
    pub password: String,
    pub device_uuid: String,
    pub device_type: String,
    pub device_name: Option<String>,
    pub os_info: Option<String>,
}

#[derive(Deserialize, Debug)]
struct CreatePlaylistBody {
    pub name: String,
    pub track_ids: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct UpdatePlaylistBody {
    pub name: Option<String>,
    pub track_ids: Option<Vec<String>>,
}

#[derive(Serialize)]
struct LoginSuccessResponse {
    token: String,
    user_handle: String,
    permissions: Vec<Permission>,
}

#[derive(Serialize)]
struct SessionResponse {
    user_handle: String,
    permissions: Vec<Permission>,
}

#[derive(Deserialize, Debug)]
struct AddTracksToPlaylistBody {
    pub tracks_ids: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct RemoveTracksFromPlaylist {
    pub tracks_positions: Vec<usize>,
}

// ========================================================================
// Sync API Types
// ========================================================================

#[derive(Serialize)]
struct SyncStateResponse {
    seq: i64,
    likes: LikesState,
    settings: Vec<UserSetting>,
    playlists: Vec<PlaylistState>,
    permissions: Vec<Permission>,
}

#[derive(Serialize)]
struct LikesState {
    albums: Vec<String>,
    artists: Vec<String>,
    tracks: Vec<String>,
}

#[derive(Serialize)]
struct PlaylistState {
    id: String,
    name: String,
    tracks: Vec<String>,
}

#[derive(Serialize)]
struct SyncEventsResponse {
    events: Vec<crate::user::sync_events::StoredEvent>,
    current_seq: i64,
}

#[derive(Deserialize)]
struct SyncEventsQuery {
    since: i64,
}

// ========================================================================
// Sync API Handlers
// ========================================================================

/// GET /v1/sync/state - Returns full user state for initial sync
async fn get_sync_state(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
) -> Response {
    let um = user_manager.lock().unwrap();

    // Get current sequence number
    let seq = match um.get_current_seq(session.user_id) {
        Ok(seq) => seq,
        Err(err) => {
            error!("Error getting current seq: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Get likes for all content types
    let albums = match um.get_user_liked_content(session.user_id, LikedContentType::Album) {
        Ok(v) => v,
        Err(err) => {
            error!("Error getting liked albums: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let artists = match um.get_user_liked_content(session.user_id, LikedContentType::Artist) {
        Ok(v) => v,
        Err(err) => {
            error!("Error getting liked artists: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let tracks = match um.get_user_liked_content(session.user_id, LikedContentType::Track) {
        Ok(v) => v,
        Err(err) => {
            error!("Error getting liked tracks: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Get all user settings
    let settings = match um.get_all_user_settings(session.user_id) {
        Ok(s) => s,
        Err(err) => {
            error!("Error getting user settings: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Get all playlists with their full data
    let playlist_ids = match um.get_user_playlists(session.user_id) {
        Ok(p) => p,
        Err(err) => {
            error!("Error getting user playlists: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut playlists = Vec::new();
    for playlist_id in playlist_ids {
        match um.get_user_playlist(&playlist_id, session.user_id) {
            Ok(playlist) => {
                playlists.push(PlaylistState {
                    id: playlist.id,
                    name: playlist.name,
                    tracks: playlist.tracks,
                });
            }
            Err(err) => {
                warn!("Error getting playlist {}: {}", playlist_id, err);
                // Continue with other playlists
            }
        }
    }

    // Get permissions
    let permissions = match um.get_user_permissions(session.user_id) {
        Ok(p) => p,
        Err(err) => {
            error!("Error getting user permissions: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Json(SyncStateResponse {
        seq,
        likes: LikesState {
            albums,
            artists,
            tracks,
        },
        settings,
        playlists,
        permissions,
    })
    .into_response()
}

/// GET /v1/sync/events - Returns events since a given sequence number
async fn get_sync_events(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<SyncEventsQuery>,
) -> Response {
    let um = user_manager.lock().unwrap();

    // Get current sequence number first (needed for pruning check)
    let current_seq = match um.get_current_seq(session.user_id) {
        Ok(seq) => seq,
        Err(err) => {
            error!("Error getting current seq: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Check if requested sequence has been pruned
    // Return 410 GONE if the event *after* the requested sequence has been pruned.
    // For example, if since=5 and min_seq=10, events 6-9 have been pruned,
    // so we can't provide a continuous stream from seq 5.
    // However, if since=0 and min_seq=1, that's fine - we return event 1.
    if query.since > 0 {
        match um.get_min_seq(session.user_id) {
            Ok(Some(min_seq)) if query.since + 1 < min_seq => {
                // Requested sequence is no longer available
                return StatusCode::GONE.into_response();
            }
            Ok(None) => {
                // No events exist but client is asking for events after some sequence.
                // Either all events were pruned or the client has invalid state.
                // Return 410 to signal the client should reset their sync state.
                return StatusCode::GONE.into_response();
            }
            Err(err) => {
                error!("Error getting min seq: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            _ => {}
        }
    }

    // Get events since the requested sequence
    let events = match um.get_events_since(session.user_id, query.since) {
        Ok(e) => e,
        Err(err) => {
            error!("Error getting events since {}: {}", query.since, err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Json(SyncEventsResponse {
        events,
        current_seq,
    })
    .into_response()
}

async fn home(session: Option<Session>, State(state): State<ServerState>) -> impl IntoResponse {
    let stats = ServerStats {
        uptime: format_uptime(state.start_time.elapsed()),
        hash: state.hash.clone(),
        session_token: session.map(|s| s.token),
    };
    Json(stats)
}

async fn get_artist(
    session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(proxy): State<super::state::OptionalProxy>,
    Path(id): Path<String>,
) -> Response {
    // If proxy is available, ensure artist has complete data
    if let Some(ref proxy) = proxy {
        if let Err(e) = proxy
            .ensure_artist_complete(&id, session.user_id, &session.permissions)
            .await
        {
            warn!("Proxy fetch failed for artist {}: {}", id, e);
            // Continue serving what we have
        }
    }

    match catalog_store.get_artist_json(&id) {
        Ok(Some(artist)) => Json(artist).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response(),
    }
}

async fn get_album(
    session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(proxy): State<super::state::OptionalProxy>,
    Path(id): Path<String>,
) -> Response {
    // If proxy is available, ensure album has complete data
    if let Some(ref proxy) = proxy {
        if let Err(e) = proxy
            .ensure_album_complete(&id, session.user_id, &session.permissions)
            .await
        {
            warn!("Proxy fetch failed for album {}: {}", id, e);
            // Continue serving what we have
        }
    }

    match catalog_store.get_album_json(&id) {
        Ok(Some(album)) => Json(album).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response(),
    }
}

async fn get_resolved_album(
    session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(proxy): State<super::state::OptionalProxy>,
    Path(id): Path<String>,
) -> Response {
    // If proxy is available, ensure album has complete data
    if let Some(ref proxy) = proxy {
        if let Err(e) = proxy
            .ensure_album_complete(&id, session.user_id, &session.permissions)
            .await
        {
            warn!("Proxy fetch failed for album {}: {}", id, e);
        }
    }

    match catalog_store.get_resolved_album_json(&id) {
        Ok(Some(album)) => Json(album).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response(),
    }
}

async fn get_artist_discography(
    session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(proxy): State<super::state::OptionalProxy>,
    Path(id): Path<String>,
) -> Response {
    // If proxy is available, ensure artist has complete data
    if let Some(ref proxy) = proxy {
        if let Err(e) = proxy
            .ensure_artist_complete(&id, session.user_id, &session.permissions)
            .await
        {
            warn!("Proxy fetch failed for artist discography {}: {}", id, e);
        }
    }

    match catalog_store.get_artist_discography_json(&id) {
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Ok(Some(discography)) => Json(discography).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response(),
    }
}

pub async fn get_track(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
) -> Response {
    match catalog_store.get_track_json(&id) {
        Ok(Some(track)) => Json(track).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response(),
    }
}

pub async fn get_resolved_track(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
) -> Response {
    match catalog_store.get_resolved_track_json(&id) {
        Ok(Some(track)) => Json(track).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response(),
    }
}

async fn get_image(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
) -> Response {
    let file_path = catalog_store.get_image_path(&id);
    if !file_path.exists() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let mut file = File::open(file_path).unwrap();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).unwrap();

    if let Some(kind) = infer::get(&buffer) {
        if kind.mime_type().starts_with("image/") {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, kind.mime_type().to_string())
                .body(buffer.to_vec().into())
                .unwrap();
        }
    }
    StatusCode::NOT_FOUND.into_response()
}

// =============================================================================
// Catalog Editing Handlers
// =============================================================================

async fn create_artist(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Json(data): Json<serde_json::Value>,
) -> Response {
    match catalog_store.create_artist(data) {
        Ok(artist) => (StatusCode::CREATED, Json(artist)).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, format!("{}", err)).into_response(),
    }
}

async fn update_artist(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
    Json(data): Json<serde_json::Value>,
) -> Response {
    match catalog_store.update_artist(&id, data) {
        Ok(artist) => Json(artist).into_response(),
        Err(err) => {
            if err.to_string().contains("not found") {
                StatusCode::NOT_FOUND.into_response()
            } else {
                (StatusCode::BAD_REQUEST, format!("{}", err)).into_response()
            }
        }
    }
}

async fn delete_artist(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
) -> Response {
    match catalog_store.delete_artist(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            if err.to_string().contains("not found") {
                StatusCode::NOT_FOUND.into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response()
            }
        }
    }
}

async fn create_album(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Json(data): Json<serde_json::Value>,
) -> Response {
    match catalog_store.create_album(data) {
        Ok(album) => (StatusCode::CREATED, Json(album)).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, format!("{}", err)).into_response(),
    }
}

async fn update_album(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
    Json(data): Json<serde_json::Value>,
) -> Response {
    match catalog_store.update_album(&id, data) {
        Ok(album) => Json(album).into_response(),
        Err(err) => {
            if err.to_string().contains("not found") {
                StatusCode::NOT_FOUND.into_response()
            } else {
                (StatusCode::BAD_REQUEST, format!("{}", err)).into_response()
            }
        }
    }
}

async fn delete_album(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
) -> Response {
    match catalog_store.delete_album(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            if err.to_string().contains("not found") {
                StatusCode::NOT_FOUND.into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response()
            }
        }
    }
}

async fn create_track(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Json(data): Json<serde_json::Value>,
) -> Response {
    match catalog_store.create_track(data) {
        Ok(track) => (StatusCode::CREATED, Json(track)).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, format!("{}", err)).into_response(),
    }
}

async fn update_track(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
    Json(data): Json<serde_json::Value>,
) -> Response {
    match catalog_store.update_track(&id, data) {
        Ok(track) => Json(track).into_response(),
        Err(err) => {
            if err.to_string().contains("not found") {
                StatusCode::NOT_FOUND.into_response()
            } else {
                (StatusCode::BAD_REQUEST, format!("{}", err)).into_response()
            }
        }
    }
}

async fn delete_track(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
) -> Response {
    match catalog_store.delete_track(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            if err.to_string().contains("not found") {
                StatusCode::NOT_FOUND.into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response()
            }
        }
    }
}

async fn create_image(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Json(data): Json<serde_json::Value>,
) -> Response {
    match catalog_store.create_image(data) {
        Ok(image) => (StatusCode::CREATED, Json(image)).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, format!("{}", err)).into_response(),
    }
}

async fn update_image(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
    Json(data): Json<serde_json::Value>,
) -> Response {
    match catalog_store.update_image(&id, data) {
        Ok(image) => Json(image).into_response(),
        Err(err) => {
            if err.to_string().contains("not found") {
                StatusCode::NOT_FOUND.into_response()
            } else {
                (StatusCode::BAD_REQUEST, format!("{}", err)).into_response()
            }
        }
    }
}

async fn delete_image(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
) -> Response {
    match catalog_store.delete_image(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            if err.to_string().contains("not found") {
                StatusCode::NOT_FOUND.into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response()
            }
        }
    }
}

// ============================================================================
// What's New Endpoint (user-facing changelog)
// ============================================================================

#[derive(Deserialize)]
struct WhatsNewQuery {
    #[serde(default = "default_whats_new_limit")]
    limit: usize,
}

fn default_whats_new_limit() -> usize {
    10
}

/// GET /v1/content/whatsnew - List recent catalog updates
async fn get_whats_new(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Query(query): Query<WhatsNewQuery>,
) -> Response {
    // Cap limit at 50 for performance
    let limit = query.limit.min(50);

    match catalog_store.get_whats_new_batches(limit) {
        Ok(batches) => Json(serde_json::json!({ "batches": batches })).into_response(),
        Err(err) => {
            error!("Error getting what's new batches: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get popular albums and artists based on listening data from the last 365 days.
/// Uses a large window so that low-traffic instances still return meaningful results.
/// Results are cached for 24 hours in UserManager.
async fn get_popular_content(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<PopularContentQuery>,
) -> Response {
    use std::time::SystemTime;

    // Use 365-day window - on busy instances the query limit caps results,
    // on quiet instances we get meaningful data from further back
    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let end_date = {
        let datetime =
            chrono::DateTime::from_timestamp(now_secs as i64, 0).unwrap_or_else(chrono::Utc::now);
        datetime
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap_or(0)
    };

    let start_date = {
        let one_year_ago = now_secs - (365 * 24 * 60 * 60);
        let datetime = chrono::DateTime::from_timestamp(one_year_ago as i64, 0)
            .unwrap_or_else(chrono::Utc::now);
        datetime
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap_or(0)
    };

    // Default and cap limits
    let albums_limit = query.albums_limit.unwrap_or(10).min(20);
    let artists_limit = query.artists_limit.unwrap_or(10).min(20);

    info!(
        "get_popular_content: date range {} - {}, limits albums={} artists={}",
        start_date, end_date, albums_limit, artists_limit
    );

    match user_manager.lock().unwrap().get_popular_content(
        start_date,
        end_date,
        albums_limit,
        artists_limit,
    ) {
        Ok(content) => {
            info!(
                "get_popular_content: returning {} albums, {} artists",
                content.albums.len(),
                content.artists.len()
            );
            Json(content).into_response()
        }
        Err(err) => {
            error!("Error getting popular content: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn parse_content_type(content_type_str: &str) -> Option<LikedContentType> {
    match content_type_str {
        "artist" => Some(LikedContentType::Artist),
        "album" => Some(LikedContentType::Album),
        "track" => Some(LikedContentType::Track),
        _ => None,
    }
}

async fn add_user_liked_content(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path((content_type_str, content_id)): Path<(String, String)>,
) -> Response {
    let Some(content_type) = parse_content_type(&content_type_str) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let stored_event = {
        let um = user_manager.lock().unwrap();
        match um.set_user_liked_content(session.user_id, &content_id, content_type, true) {
            Ok(_) => {
                let event = UserEvent::ContentLiked {
                    content_type,
                    content_id: content_id.to_string(),
                };
                match um.append_event(session.user_id, &event) {
                    Ok(stored) => Some(stored),
                    Err(e) => {
                        warn!("Failed to log sync event: {}", e);
                        None
                    }
                }
            }
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    };

    // Broadcast to other devices if we have a stored event and device_id
    if let (Some(stored_event), Some(device_id)) = (stored_event, session.device_id) {
        let ws_msg = super::websocket::messages::ServerMessage::new(
            super::websocket::messages::msg_types::SYNC,
            super::websocket::messages::sync::SyncEventMessage {
                event: stored_event,
            },
        );
        connection_manager
            .send_to_other_devices(session.user_id, device_id, ws_msg)
            .await;
    }

    StatusCode::OK.into_response()
}

async fn delete_user_liked_content(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path((content_type_str, content_id)): Path<(String, String)>,
) -> Response {
    let Some(content_type) = parse_content_type(&content_type_str) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let stored_event = {
        let um = user_manager.lock().unwrap();
        match um.set_user_liked_content(session.user_id, &content_id, content_type, false) {
            Ok(_) => {
                let event = UserEvent::ContentUnliked {
                    content_type,
                    content_id: content_id.to_string(),
                };
                match um.append_event(session.user_id, &event) {
                    Ok(stored) => Some(stored),
                    Err(e) => {
                        warn!("Failed to log sync event: {}", e);
                        None
                    }
                }
            }
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    };

    // Broadcast to other devices if we have a stored event and device_id
    if let (Some(stored_event), Some(device_id)) = (stored_event, session.device_id) {
        let ws_msg = super::websocket::messages::ServerMessage::new(
            super::websocket::messages::msg_types::SYNC,
            super::websocket::messages::sync::SyncEventMessage {
                event: stored_event,
            },
        );
        connection_manager
            .send_to_other_devices(session.user_id, device_id, ws_msg)
            .await;
    }

    StatusCode::OK.into_response()
}

async fn get_user_liked_content(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(content_type_str): Path<String>,
) -> Response {
    let Some(content_type) = parse_content_type(&content_type_str) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let user_manager = user_manager.lock().unwrap();
    let liked_content = user_manager.get_user_liked_content(session.user_id, content_type);
    match liked_content {
        Ok(liked_content) => Json(liked_content).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn post_playlist(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Json(body): Json<CreatePlaylistBody>,
) -> Response {
    let (id, stored_event) = {
        let um = user_manager.lock().unwrap();
        match um.create_user_playlist(
            session.user_id,
            &body.name,
            session.user_id,
            body.track_ids.clone(),
        ) {
            Ok(id) => {
                // Log sync event
                let event = UserEvent::PlaylistCreated {
                    playlist_id: id.clone(),
                    name: body.name.clone(),
                };
                let stored_event = match um.append_event(session.user_id, &event) {
                    Ok(stored) => Some(stored),
                    Err(e) => {
                        warn!("Failed to log sync event: {}", e);
                        None
                    }
                };
                (Some(id), stored_event)
            }
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    };

    // Broadcast to other devices
    if let (Some(stored_event), Some(device_id)) = (stored_event, session.device_id) {
        let ws_msg = super::websocket::messages::ServerMessage::new(
            super::websocket::messages::msg_types::SYNC,
            super::websocket::messages::sync::SyncEventMessage {
                event: stored_event,
            },
        );
        connection_manager
            .send_to_other_devices(session.user_id, device_id, ws_msg)
            .await;
    }

    Json(id).into_response()
}

async fn put_playlist(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(id): Path<String>,
    Json(body): Json<UpdatePlaylistBody>,
) -> Response {
    debug!("Updating playlist with id {}", id);
    let stored_events = {
        let um = user_manager.lock().unwrap();
        match um.update_user_playlist(
            &id,
            session.user_id,
            body.name.clone(),
            body.track_ids.clone(),
        ) {
            Ok(_) => {
                // Log sync events for name and/or tracks changes
                let mut events = Vec::new();
                if let Some(name) = body.name {
                    let event = UserEvent::PlaylistRenamed {
                        playlist_id: id.clone(),
                        name,
                    };
                    match um.append_event(session.user_id, &event) {
                        Ok(stored) => events.push(stored),
                        Err(e) => warn!("Failed to log sync event: {}", e),
                    }
                }
                if let Some(track_ids) = body.track_ids {
                    let event = UserEvent::PlaylistTracksUpdated {
                        playlist_id: id.clone(),
                        track_ids,
                    };
                    match um.append_event(session.user_id, &event) {
                        Ok(stored) => events.push(stored),
                        Err(e) => warn!("Failed to log sync event: {}", e),
                    }
                }
                events
            }
            Err(err) => {
                debug!("Error updating playlist: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    };

    // Broadcast to other devices
    if let Some(device_id) = session.device_id {
        for stored_event in stored_events {
            let ws_msg = super::websocket::messages::ServerMessage::new(
                super::websocket::messages::msg_types::SYNC,
                super::websocket::messages::sync::SyncEventMessage {
                    event: stored_event,
                },
            );
            connection_manager
                .send_to_other_devices(session.user_id, device_id, ws_msg)
                .await;
        }
    }

    StatusCode::OK.into_response()
}

async fn delete_playlist(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(id): Path<String>,
) -> Response {
    let stored_event = {
        let um = user_manager.lock().unwrap();
        match um.delete_user_playlist(&id, session.user_id) {
            Ok(_) => {
                // Log sync event
                let event = UserEvent::PlaylistDeleted {
                    playlist_id: id.clone(),
                };
                match um.append_event(session.user_id, &event) {
                    Ok(stored) => Some(stored),
                    Err(e) => {
                        warn!("Failed to log sync event: {}", e);
                        None
                    }
                }
            }
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    };

    // Broadcast to other devices
    if let (Some(stored_event), Some(device_id)) = (stored_event, session.device_id) {
        let ws_msg = super::websocket::messages::ServerMessage::new(
            super::websocket::messages::msg_types::SYNC,
            super::websocket::messages::sync::SyncEventMessage {
                event: stored_event,
            },
        );
        connection_manager
            .send_to_other_devices(session.user_id, device_id, ws_msg)
            .await;
    }

    StatusCode::OK.into_response()
}

async fn get_playlist(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(id): Path<String>,
) -> Response {
    match user_manager
        .lock()
        .unwrap()
        .get_user_playlist(&id, session.user_id)
    {
        Ok(playlist) => {
            if playlist.user_id == session.user_id {
                Json(playlist).into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn add_playlist_tracks(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(id): Path<String>,
    Json(body): Json<AddTracksToPlaylistBody>,
) -> Response {
    let stored_event = {
        let um = user_manager.lock().unwrap();
        match um.add_playlist_tracks(&id, session.user_id, body.tracks_ids) {
            Ok(_) => {
                // Fetch updated track list and log sync event
                if let Ok(playlist) = um.get_user_playlist(&id, session.user_id) {
                    let event = UserEvent::PlaylistTracksUpdated {
                        playlist_id: id.clone(),
                        track_ids: playlist.tracks,
                    };
                    match um.append_event(session.user_id, &event) {
                        Ok(stored) => Some(stored),
                        Err(e) => {
                            warn!("Failed to log sync event: {}", e);
                            None
                        }
                    }
                } else {
                    None
                }
            }
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    };

    // Broadcast to other devices
    if let (Some(stored_event), Some(device_id)) = (stored_event, session.device_id) {
        let ws_msg = super::websocket::messages::ServerMessage::new(
            super::websocket::messages::msg_types::SYNC,
            super::websocket::messages::sync::SyncEventMessage {
                event: stored_event,
            },
        );
        connection_manager
            .send_to_other_devices(session.user_id, device_id, ws_msg)
            .await;
    }

    StatusCode::OK.into_response()
}

async fn remove_tracks_from_playlist(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(id): Path<String>,
    Json(body): Json<RemoveTracksFromPlaylist>,
) -> Response {
    let stored_event = {
        let um = user_manager.lock().unwrap();
        match um.remove_tracks_from_playlist(&id, session.user_id, body.tracks_positions) {
            Ok(_) => {
                // Fetch updated track list and log sync event
                if let Ok(playlist) = um.get_user_playlist(&id, session.user_id) {
                    let event = UserEvent::PlaylistTracksUpdated {
                        playlist_id: id.clone(),
                        track_ids: playlist.tracks,
                    };
                    match um.append_event(session.user_id, &event) {
                        Ok(stored) => Some(stored),
                        Err(e) => {
                            warn!("Failed to log sync event: {}", e);
                            None
                        }
                    }
                } else {
                    None
                }
            }
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    };

    // Broadcast to other devices
    if let (Some(stored_event), Some(device_id)) = (stored_event, session.device_id) {
        let ws_msg = super::websocket::messages::ServerMessage::new(
            super::websocket::messages::msg_types::SYNC,
            super::websocket::messages::sync::SyncEventMessage {
                event: stored_event,
            },
        );
        connection_manager
            .send_to_other_devices(session.user_id, device_id, ws_msg)
            .await;
    }

    StatusCode::OK.into_response()
}

async fn get_user_playlists(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
) -> Response {
    match user_manager
        .lock()
        .unwrap()
        .get_user_playlists(session.user_id)
    {
        Ok(playlists) => Json(playlists).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// User listening stats endpoints

async fn post_listening_event(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Json(body): Json<ListeningEventRequest>,
) -> Response {
    use std::time::SystemTime;

    // Calculate date in YYYYMMDD format
    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let started_at = body.started_at.unwrap_or(now_secs);
    let date = {
        let datetime =
            chrono::DateTime::from_timestamp(started_at as i64, 0).unwrap_or_else(chrono::Utc::now);
        datetime
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap_or(0)
    };

    // Calculate completion (>90% = complete)
    let completed = body.duration_seconds as f64 / body.track_duration_seconds as f64 >= 0.90;

    // Capture values for metrics before moving into event
    let client_type_for_metrics = body.client_type.clone();
    let duration_for_metrics = body.duration_seconds;

    let event = crate::user::ListeningEvent {
        id: None,
        user_id: session.user_id,
        track_id: body.track_id,
        session_id: body.session_id,
        started_at,
        ended_at: body.ended_at,
        duration_seconds: body.duration_seconds,
        track_duration_seconds: body.track_duration_seconds,
        completed,
        seek_count: body.seek_count.unwrap_or(0),
        pause_count: body.pause_count.unwrap_or(0),
        playback_context: body.playback_context,
        client_type: body.client_type,
        date,
    };

    match user_manager.lock().unwrap().record_listening_event(event) {
        Ok((id, created)) => {
            // Record metrics only for newly created events
            if created {
                super::metrics::record_listening_event(
                    client_type_for_metrics.as_deref(),
                    completed,
                    duration_for_metrics,
                );
            }
            Json(ListeningEventResponse { id, created }).into_response()
        }
        Err(err) => {
            error!("Error recording listening event: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_user_listening_summary(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);

    match user_manager.lock().unwrap().get_user_listening_summary(
        session.user_id,
        start_date,
        end_date,
    ) {
        Ok(summary) => Json(summary).into_response(),
        Err(err) => {
            error!("Error getting listening summary: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_user_listening_history(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<ListeningHistoryQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(50).min(500);

    match user_manager
        .lock()
        .unwrap()
        .get_user_listening_history(session.user_id, limit)
    {
        Ok(history) => Json(history).into_response(),
        Err(err) => {
            error!("Error getting listening history: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_user_listening_events(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<ListeningEventsQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);

    match user_manager.lock().unwrap().get_user_listening_events(
        session.user_id,
        start_date,
        end_date,
        query.limit,
        query.offset,
    ) {
        Ok(events) => Json(events).into_response(),
        Err(err) => {
            error!("Error getting listening events: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Helper to get default date range (last 30 days if not specified)
fn get_default_date_range(start_date: Option<u32>, end_date: Option<u32>) -> (u32, u32) {
    use std::time::SystemTime;

    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let end = end_date.unwrap_or_else(|| {
        let datetime =
            chrono::DateTime::from_timestamp(now_secs as i64, 0).unwrap_or_else(chrono::Utc::now);
        datetime
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap_or(0)
    });

    let start = start_date.unwrap_or_else(|| {
        let thirty_days_ago = now_secs - (30 * 24 * 60 * 60);
        let datetime = chrono::DateTime::from_timestamp(thirty_days_ago as i64, 0)
            .unwrap_or_else(chrono::Utc::now);
        datetime
            .format("%Y%m%d")
            .to_string()
            .parse::<u32>()
            .unwrap_or(0)
    });

    (start, end)
}

// User settings endpoints

#[derive(Deserialize)]
struct UpdateSettingsBody {
    settings: Vec<UserSetting>,
}

#[derive(Serialize)]
struct UserSettingsResponse {
    settings: Vec<UserSetting>,
}

async fn get_user_settings(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
) -> Response {
    match user_manager
        .lock()
        .unwrap()
        .get_all_user_settings(session.user_id)
    {
        Ok(settings) => Json(UserSettingsResponse { settings }).into_response(),
        Err(err) => {
            error!("Error getting user settings: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn update_user_settings(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Json(body): Json<UpdateSettingsBody>,
) -> Response {
    let stored_events = {
        let locked_manager = user_manager.lock().unwrap();
        let mut events = Vec::new();
        for setting in body.settings {
            if let Err(err) = locked_manager.set_user_setting(session.user_id, setting.clone()) {
                error!("Error setting user setting {:?}: {}", setting, err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            // Log sync event
            let event = UserEvent::SettingChanged {
                setting: setting.clone(),
            };
            match locked_manager.append_event(session.user_id, &event) {
                Ok(stored) => events.push(stored),
                Err(e) => warn!("Failed to log sync event: {}", e),
            }
        }
        events
    };

    // Broadcast all events to other devices
    if let Some(device_id) = session.device_id {
        for stored_event in stored_events {
            let ws_msg = super::websocket::messages::ServerMessage::new(
                super::websocket::messages::msg_types::SYNC,
                super::websocket::messages::sync::SyncEventMessage {
                    event: stored_event,
                },
            );
            connection_manager
                .send_to_other_devices(session.user_id, device_id, ws_msg)
                .await;
        }
    }

    StatusCode::OK.into_response()
}

async fn login(
    State(user_manager): State<GuardedUserManager>,
    Json(body): Json<LoginBody>,
) -> Response {
    let start = Instant::now();
    debug!("login() called with {:?}", body);

    // 1. Validate device info first (fail fast)
    let device_registration = match DeviceRegistration::validate_and_sanitize(
        &body.device_uuid,
        &body.device_type,
        body.device_name.as_deref(),
        body.os_info.as_deref(),
    ) {
        Ok(reg) => reg,
        Err(e) => {
            warn!("Invalid device info in login request: {}", e);
            super::metrics::record_login_attempt("failure", start.elapsed());
            return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
        }
    };

    let mut locked_manager = user_manager.lock().unwrap();
    let credentials = match locked_manager.get_user_credentials(&body.user_handle) {
        Ok(Some(creds)) => creds,
        Ok(None) => {
            super::metrics::record_login_attempt("failure", start.elapsed());
            return StatusCode::UNAUTHORIZED.into_response();
        }
        Err(_) => {
            super::metrics::record_login_attempt("error", start.elapsed());
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let Some(password_credentials) = &credentials.username_password {
        if let Ok(true) = password_credentials.hasher.verify(
            &body.password,
            &password_credentials.hash,
            &password_credentials.salt,
        ) {
            // Fetch user permissions
            let permissions = match locked_manager.get_user_permissions(credentials.user_id) {
                Ok(perms) => perms,
                Err(err) => {
                    error!("Error fetching user permissions: {}", err);
                    super::metrics::record_login_attempt("error", start.elapsed());
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };

            // 2. Register/update device
            let device_id = match locked_manager.register_or_update_device(&device_registration) {
                Ok(id) => id,
                Err(e) => {
                    error!("Device registration failed: {}", e);
                    super::metrics::record_login_attempt("error", start.elapsed());
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };

            // 3. Associate device with user
            if let Err(e) =
                locked_manager.associate_device_with_user(device_id, credentials.user_id)
            {
                error!("Device association failed: {}", e);
                // Non-fatal, continue with login
            }

            // 4. Enforce per-user device limit
            if let Err(e) =
                locked_manager.enforce_user_device_limit(credentials.user_id, MAX_DEVICES_PER_USER)
            {
                error!("Device limit enforcement failed: {}", e);
                // Non-fatal, continue with login
            }

            // 5. Generate auth token with device_id
            return match locked_manager.generate_auth_token(&credentials, device_id) {
                Ok(auth_token) => {
                    super::metrics::record_login_attempt("success", start.elapsed());
                    let response_body = LoginSuccessResponse {
                        token: auth_token.value.0.clone(),
                        user_handle: body.user_handle.clone(),
                        permissions,
                    };
                    let response_body = serde_json::to_string(&response_body).unwrap();

                    let cookie_value = HeaderValue::from_str(&format!(
                        "session_token={}; Path=/; HttpOnly",
                        auth_token.value.0.clone()
                    ))
                    .unwrap();
                    response::Builder::new()
                        .status(StatusCode::CREATED)
                        .header(axum::http::header::SET_COOKIE, cookie_value)
                        .body(Body::from(response_body))
                        .unwrap()
                }
                Err(err) => {
                    error!("Error with auth token generation: {}", err);
                    super::metrics::record_login_attempt("error", start.elapsed());
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            };
        }
    }
    super::metrics::record_login_attempt("failure", start.elapsed());
    StatusCode::UNAUTHORIZED.into_response()
}

async fn logout(State(user_manager): State<GuardedUserManager>, session: Session) -> Response {
    let mut locked_manager = user_manager.lock().unwrap();
    match locked_manager.delete_auth_token(&session.user_id, &AuthTokenValue(session.token)) {
        Ok(()) => {
            let cookie_value = Cookie::build(Cookie::new("session_token", ""))
                .path("/")
                .expires(time::OffsetDateTime::now_utc() - time::Duration::days(1)) // Expire it in the past
                .same_site(SameSite::Lax)
                .build();

            response::Builder::new()
                .status(StatusCode::OK)
                .header(axum::http::header::SET_COOKIE, cookie_value.to_string())
                .body(Body::empty())
                .unwrap()
        }
        Err(_) => StatusCode::BAD_REQUEST.into_response(),
    }
}

async fn get_session(State(user_manager): State<GuardedUserManager>, session: Session) -> Response {
    let locked_manager = user_manager.lock().unwrap();

    // Get the user handle from user_id
    let user_handle = match locked_manager.get_user_handle(session.user_id) {
        Ok(Some(handle)) => handle,
        Ok(None) => {
            error!("User handle not found for user_id={}", session.user_id);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Err(err) => {
            error!(
                "Failed to get user handle for user_id={}: {}",
                session.user_id, err
            );
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response_body = SessionResponse {
        user_handle,
        permissions: session.permissions.clone(),
    };

    Json(response_body).into_response()
}

async fn reboot_server(session: Session) -> Response {
    info!(
        "Server reboot requested by user_id={}, initiating shutdown...",
        session.user_id
    );

    // Spawn a task to exit the process after responding
    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        info!("Server shutting down for reboot");
        std::process::exit(0);
    });

    (StatusCode::ACCEPTED, "Server reboot initiated").into_response()
}

// ============================================================================
// Admin Job API handlers
// ============================================================================

#[derive(Serialize)]
struct ListJobsResponse {
    jobs: Vec<JobInfo>,
}

async fn admin_list_jobs(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
) -> Response {
    let handle = match scheduler_handle {
        Some(h) => h,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Job scheduler not available"})),
            )
                .into_response();
        }
    };

    match handle.list_jobs().await {
        Ok(jobs) => {
            debug!("User {} listed {} jobs", session.user_id, jobs.len());
            (StatusCode::OK, Json(ListJobsResponse { jobs })).into_response()
        }
        Err(e) => {
            error!("Failed to list jobs: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to list jobs"})),
            )
                .into_response()
        }
    }
}

async fn admin_get_job(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
    Path(job_id): Path<String>,
) -> Response {
    let handle = match scheduler_handle {
        Some(h) => h,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Job scheduler not available"})),
            )
                .into_response();
        }
    };

    match handle.get_job(&job_id).await {
        Ok(Some(job)) => {
            debug!("User {} retrieved job {}", session.user_id, job_id);
            (StatusCode::OK, Json(job)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Job not found"})),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to get job {}: {}", job_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to get job"})),
            )
                .into_response()
        }
    }
}

async fn admin_trigger_job(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
    Path(job_id): Path<String>,
) -> Response {
    let handle = match scheduler_handle {
        Some(h) => h,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Job scheduler not available"})),
            )
                .into_response();
        }
    };

    info!("User {} triggering job {}", session.user_id, job_id);

    match handle.trigger_job(&job_id).await {
        Ok(()) => {
            info!(
                "Job {} triggered successfully by user {}",
                job_id, session.user_id
            );
            (
                StatusCode::ACCEPTED,
                Json(serde_json::json!({"status": "triggered", "job_id": job_id})),
            )
                .into_response()
        }
        Err(JobError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Job not found"})),
        )
            .into_response(),
        Err(JobError::AlreadyRunning) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "Job is already running"})),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to trigger job {}: {}", job_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to trigger job: {}", e)})),
            )
                .into_response()
        }
    }
}

async fn admin_get_job_history(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
    Path(job_id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let handle = match scheduler_handle {
        Some(h) => h,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Job scheduler not available"})),
            )
                .into_response();
        }
    };

    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    match handle.get_job_history(&job_id, limit) {
        Ok(history) => {
            debug!(
                "User {} retrieved {} history entries for job {}",
                session.user_id,
                history.len(),
                job_id
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({"history": history})),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to get job history for {}: {}", job_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to get job history"})),
            )
                .into_response()
        }
    }
}

async fn get_challenge(State(_state): State<ServerState>) -> Response {
    todo!()
}

async fn post_challenge(State(_state): State<ServerState>) -> Response {
    todo!()
}

// Admin endpoint types and handlers

#[derive(Serialize)]
struct UserInfo {
    pub user_handle: String,
    pub user_id: usize,
}

#[derive(Serialize)]
struct UserRolesResponse {
    pub user_handle: String,
    pub roles: Vec<String>,
}

#[derive(Serialize)]
struct UserPermissionsResponse {
    pub user_handle: String,
    pub permissions: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct AddRoleBody {
    pub role: String,
}

#[derive(Deserialize, Debug)]
struct AddExtraPermissionBody {
    pub permission: String,
    pub duration_seconds: Option<u64>,
    pub countdown: Option<u64>,
}

#[derive(Serialize)]
struct AddExtraPermissionResponse {
    pub permission_id: usize,
}

// Listening stats request/response structs

#[derive(Deserialize, Debug)]
struct ListeningEventRequest {
    pub track_id: String,
    pub session_id: Option<String>,
    pub started_at: Option<u64>,
    pub ended_at: Option<u64>,
    pub duration_seconds: u32,
    pub track_duration_seconds: u32,
    pub seek_count: Option<u32>,
    pub pause_count: Option<u32>,
    pub playback_context: Option<String>,
    pub client_type: Option<String>,
}

#[derive(Serialize)]
struct ListeningEventResponse {
    pub id: usize,
    pub created: bool,
}

#[derive(Deserialize, Debug)]
struct DateRangeQuery {
    pub start_date: Option<u32>,
    pub end_date: Option<u32>,
}

#[derive(Deserialize, Debug)]
struct ListeningEventsQuery {
    pub start_date: Option<u32>,
    pub end_date: Option<u32>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Deserialize, Debug)]
struct ListeningHistoryQuery {
    pub limit: Option<usize>,
}

#[derive(Deserialize, Debug)]
struct TopTracksQuery {
    pub start_date: Option<u32>,
    pub end_date: Option<u32>,
    pub limit: Option<usize>,
}

#[derive(Deserialize, Debug)]
struct PopularContentQuery {
    pub albums_limit: Option<usize>,
    pub artists_limit: Option<usize>,
}

async fn admin_get_users(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
) -> Response {
    let manager = user_manager.lock().unwrap();
    match manager.get_all_user_handles() {
        Ok(handles) => {
            let mut users: Vec<UserInfo> = vec![];
            for handle in handles {
                if let Ok(Some(user_id)) = manager.get_user_id(&handle) {
                    users.push(UserInfo {
                        user_handle: handle,
                        user_id,
                    });
                }
            }
            Json(users).into_response()
        }
        Err(err) => {
            error!("Error getting users: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
struct CreateUserBody {
    user_handle: String,
}

#[derive(Serialize)]
struct CreateUserResponse {
    user_id: usize,
    user_handle: String,
}

async fn admin_create_user(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Json(body): Json<CreateUserBody>,
) -> Response {
    let manager = user_manager.lock().unwrap();

    // Create user (add_user validates handle is not empty and not duplicate)
    let user_id = match manager.add_user(&body.user_handle) {
        Ok(id) => id,
        Err(err) => {
            let err_str = err.to_string();
            if err_str.contains("already exists") {
                return (StatusCode::CONFLICT, "User handle already exists").into_response();
            }
            if err_str.contains("cannot be empty") {
                return (StatusCode::BAD_REQUEST, "User handle cannot be empty").into_response();
            }
            error!("Error creating user: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    (
        StatusCode::CREATED,
        Json(CreateUserResponse {
            user_id,
            user_handle: body.user_handle,
        }),
    )
        .into_response()
}

async fn admin_delete_user(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
) -> Response {
    let manager = user_manager.lock().unwrap();

    // Get user id first
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Prevent self-deletion
    if user_id == session.user_id {
        return (StatusCode::BAD_REQUEST, "Cannot delete your own account").into_response();
    }

    match manager.delete_user(user_id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error deleting user: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Serialize)]
struct UserCredentialsStatusResponse {
    user_handle: String,
    has_password: bool,
}

async fn admin_get_user_credentials_status(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
) -> Response {
    let manager = user_manager.lock().unwrap();

    match manager.get_user_credentials(&user_handle) {
        Ok(Some(creds)) => Json(UserCredentialsStatusResponse {
            user_handle,
            has_password: creds.username_password.is_some(),
        })
        .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user credentials: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
struct SetPasswordBody {
    password: String,
}

async fn admin_set_user_password(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
    Json(body): Json<SetPasswordBody>,
) -> Response {
    let mut manager = user_manager.lock().unwrap();

    // Check if user exists and has password already
    let has_password = match manager.get_user_credentials(&user_handle) {
        Ok(Some(creds)) => creds.username_password.is_some(),
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user credentials: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let result = if has_password {
        manager.update_password_credentials(&user_handle, body.password)
    } else {
        manager.create_password_credentials(&user_handle, body.password)
    };

    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            error!("Error setting user password: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_delete_user_password(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
) -> Response {
    let mut manager = user_manager.lock().unwrap();

    // Check if user exists
    match manager.get_user_credentials(&user_handle) {
        Ok(Some(_)) => {}
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user credentials: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match manager.delete_password_credentials(&user_handle) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            error!("Error deleting user password: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_get_user_roles(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
) -> Response {
    let manager = user_manager.lock().unwrap();
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match manager.get_user_roles(user_id) {
        Ok(roles) => {
            let role_strings: Vec<String> = roles.iter().map(|r| r.as_str().to_owned()).collect();
            Json(UserRolesResponse {
                user_handle,
                roles: role_strings,
            })
            .into_response()
        }
        Err(err) => {
            error!("Error getting user roles: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_add_user_role(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(user_handle): Path<String>,
    Json(body): Json<AddRoleBody>,
) -> Response {
    let role = match crate::user::UserRole::from_str(&body.role) {
        Some(r) => r,
        None => return (StatusCode::BAD_REQUEST, "Invalid role").into_response(),
    };

    let (user_id, stored_event) = {
        let manager = user_manager.lock().unwrap();
        let user_id = match manager.get_user_id(&user_handle) {
            Ok(Some(id)) => id,
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                error!("Error getting user id: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        if let Err(err) = manager.add_user_role(user_id, role) {
            error!("Error adding user role: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }

        // Get new permissions and emit PermissionsReset event
        let permissions = match manager.get_user_permissions(user_id) {
            Ok(perms) => perms,
            Err(err) => {
                error!("Error getting user permissions after role change: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        let event = UserEvent::PermissionsReset { permissions };
        let stored_event = match manager.append_event(user_id, &event) {
            Ok(stored) => stored,
            Err(e) => {
                warn!("Failed to log sync event for permission change: {}", e);
                return StatusCode::CREATED.into_response();
            }
        };

        (user_id, stored_event)
    };

    // Broadcast to all user's devices
    let ws_msg = super::websocket::messages::ServerMessage::new(
        super::websocket::messages::msg_types::SYNC,
        super::websocket::messages::sync::SyncEventMessage {
            event: stored_event,
        },
    );
    connection_manager.broadcast_to_user(user_id, ws_msg).await;

    StatusCode::CREATED.into_response()
}

async fn admin_remove_user_role(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path((user_handle, role_name)): Path<(String, String)>,
) -> Response {
    let role = match crate::user::UserRole::from_str(&role_name) {
        Some(r) => r,
        None => return (StatusCode::BAD_REQUEST, "Invalid role").into_response(),
    };

    let (user_id, stored_event) = {
        let manager = user_manager.lock().unwrap();
        let user_id = match manager.get_user_id(&user_handle) {
            Ok(Some(id)) => id,
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                error!("Error getting user id: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        if let Err(err) = manager.remove_user_role(user_id, role) {
            error!("Error removing user role: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }

        // Get new permissions and emit PermissionsReset event
        let permissions = match manager.get_user_permissions(user_id) {
            Ok(perms) => perms,
            Err(err) => {
                error!("Error getting user permissions after role change: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        let event = UserEvent::PermissionsReset { permissions };
        let stored_event = match manager.append_event(user_id, &event) {
            Ok(stored) => stored,
            Err(e) => {
                warn!("Failed to log sync event for permission change: {}", e);
                return StatusCode::OK.into_response();
            }
        };

        (user_id, stored_event)
    };

    // Broadcast to all user's devices
    let ws_msg = super::websocket::messages::ServerMessage::new(
        super::websocket::messages::msg_types::SYNC,
        super::websocket::messages::sync::SyncEventMessage {
            event: stored_event,
        },
    );
    connection_manager.broadcast_to_user(user_id, ws_msg).await;

    StatusCode::OK.into_response()
}

async fn admin_get_user_permissions(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
) -> Response {
    let manager = user_manager.lock().unwrap();
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match manager.get_user_permissions(user_id) {
        Ok(permissions) => {
            let perm_strings: Vec<String> =
                permissions.iter().map(|p| format!("{:?}", p)).collect();
            Json(UserPermissionsResponse {
                user_handle,
                permissions: perm_strings,
            })
            .into_response()
        }
        Err(err) => {
            error!("Error getting user permissions: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_add_user_extra_permission(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(user_handle): Path<String>,
    Json(body): Json<AddExtraPermissionBody>,
) -> Response {
    use crate::user::PermissionGrant;
    use std::time::{Duration, SystemTime};

    let permission = match body.permission.as_str() {
        "AccessCatalog" => Permission::AccessCatalog,
        "LikeContent" => Permission::LikeContent,
        "OwnPlaylists" => Permission::OwnPlaylists,
        "EditCatalog" => Permission::EditCatalog,
        "ManagePermissions" => Permission::ManagePermissions,
        "IssueContentDownload" => Permission::IssueContentDownload,
        "ServerAdmin" => Permission::ServerAdmin,
        "ViewAnalytics" => Permission::ViewAnalytics,
        _ => return (StatusCode::BAD_REQUEST, "Invalid permission").into_response(),
    };

    let (user_id, permission_id, stored_event) = {
        let manager = user_manager.lock().unwrap();
        let user_id = match manager.get_user_id(&user_handle) {
            Ok(Some(id)) => id,
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                error!("Error getting user id: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        let start_time = SystemTime::now();
        let end_time = body
            .duration_seconds
            .map(|secs| start_time + Duration::from_secs(secs));

        let grant = PermissionGrant::Extra {
            start_time,
            end_time,
            permission,
            countdown: body.countdown,
        };

        let permission_id = match manager.add_user_extra_permission(user_id, grant) {
            Ok(id) => id,
            Err(err) => {
                error!("Error adding extra permission: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        // Emit PermissionGranted event
        let event = UserEvent::PermissionGranted { permission };
        let stored_event = match manager.append_event(user_id, &event) {
            Ok(stored) => stored,
            Err(e) => {
                warn!("Failed to log sync event for permission grant: {}", e);
                return (
                    StatusCode::CREATED,
                    Json(AddExtraPermissionResponse { permission_id }),
                )
                    .into_response();
            }
        };

        (user_id, permission_id, stored_event)
    };

    // Broadcast to all user's devices
    let ws_msg = super::websocket::messages::ServerMessage::new(
        super::websocket::messages::msg_types::SYNC,
        super::websocket::messages::sync::SyncEventMessage {
            event: stored_event,
        },
    );
    connection_manager.broadcast_to_user(user_id, ws_msg).await;

    (
        StatusCode::CREATED,
        Json(AddExtraPermissionResponse { permission_id }),
    )
        .into_response()
}

async fn admin_remove_extra_permission(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(permission_id): Path<usize>,
) -> Response {
    let (user_id, permission) = {
        let manager = user_manager.lock().unwrap();
        match manager.remove_user_extra_permission(permission_id) {
            Ok(Some((user_id, permission))) => (user_id, permission),
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                error!("Error removing extra permission: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    };

    // Emit sync event for the permission revocation
    let event = UserEvent::PermissionRevoked { permission };
    let stored_event = {
        let manager = user_manager.lock().unwrap();
        match manager.append_event(user_id, &event) {
            Ok(event) => event,
            Err(err) => {
                error!("Error appending permission revoked event: {}", err);
                // Permission was removed but sync failed - still return success
                return StatusCode::OK.into_response();
            }
        }
    };

    // Broadcast the sync event to user's connected devices
    let ws_msg = super::websocket::messages::ServerMessage::new(
        super::websocket::messages::msg_types::SYNC,
        super::websocket::messages::sync::SyncEventMessage {
            event: stored_event,
        },
    );
    connection_manager.broadcast_to_user(user_id, ws_msg).await;

    StatusCode::OK.into_response()
}

// Bandwidth statistics endpoints

#[derive(Deserialize, Debug)]
struct BandwidthQueryParams {
    /// Start date in YYYYMMDD format
    start_date: u32,
    /// End date in YYYYMMDD format
    end_date: u32,
}

/// Get bandwidth summary for all users (admin only)
async fn admin_get_bandwidth_summary(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(params): Query<BandwidthQueryParams>,
) -> Response {
    match user_manager
        .lock()
        .unwrap()
        .get_total_bandwidth_summary(params.start_date, params.end_date)
    {
        Ok(summary) => Json(summary).into_response(),
        Err(err) => {
            error!("Error getting bandwidth summary: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get detailed bandwidth usage for all users (admin only)
async fn admin_get_bandwidth_usage(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(params): Query<BandwidthQueryParams>,
) -> Response {
    match user_manager
        .lock()
        .unwrap()
        .get_all_bandwidth_usage(params.start_date, params.end_date)
    {
        Ok(usage) => Json(usage).into_response(),
        Err(err) => {
            error!("Error getting bandwidth usage: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get bandwidth summary for a specific user (admin only)
async fn admin_get_user_bandwidth_summary(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
    Query(params): Query<BandwidthQueryParams>,
) -> Response {
    let manager = user_manager.lock().unwrap();
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match manager.get_user_bandwidth_summary(user_id, params.start_date, params.end_date) {
        Ok(summary) => Json(summary).into_response(),
        Err(err) => {
            error!("Error getting user bandwidth summary: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get detailed bandwidth usage for a specific user (admin only)
async fn admin_get_user_bandwidth_usage(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
    Query(params): Query<BandwidthQueryParams>,
) -> Response {
    let manager = user_manager.lock().unwrap();
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match manager.get_user_bandwidth_usage(user_id, params.start_date, params.end_date) {
        Ok(usage) => Json(usage).into_response(),
        Err(err) => {
            error!("Error getting user bandwidth usage: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// Listening statistics admin endpoints (requires ViewAnalytics permission)

/// Get daily listening stats for the platform (admin only)
async fn admin_get_daily_listening_stats(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);

    match user_manager
        .lock()
        .unwrap()
        .get_daily_listening_stats(start_date, end_date)
    {
        Ok(stats) => Json(stats).into_response(),
        Err(err) => {
            error!("Error getting daily listening stats: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get top tracks by play count (admin only)
async fn admin_get_top_tracks(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Query(query): Query<TopTracksQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);
    let limit = query.limit.unwrap_or(50).min(500);

    match user_manager
        .lock()
        .unwrap()
        .get_top_tracks(start_date, end_date, limit)
    {
        Ok(tracks) => Json(tracks).into_response(),
        Err(err) => {
            error!("Error getting top tracks: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get listening stats for a specific track (admin only)
async fn admin_get_track_listening_stats(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(track_id): Path<String>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);

    match user_manager
        .lock()
        .unwrap()
        .get_track_listening_stats(&track_id, start_date, end_date)
    {
        Ok(stats) => Json(stats).into_response(),
        Err(err) => {
            error!("Error getting track listening stats: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get listening summary for a specific user (admin only)
async fn admin_get_user_listening_summary(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(user_handle): Path<String>,
    Query(query): Query<DateRangeQuery>,
) -> Response {
    let manager = user_manager.lock().unwrap();
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let (start_date, end_date) = get_default_date_range(query.start_date, query.end_date);

    match manager.get_user_listening_summary(user_id, start_date, end_date) {
        Ok(summary) => Json(summary).into_response(),
        Err(err) => {
            error!("Error getting user listening summary: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Response for online users endpoint
#[derive(Serialize)]
struct OnlineUsersResponse {
    /// Total count of unique users connected via WebSocket
    count: usize,
    /// Handles of first few connected users (up to 3)
    handles: Vec<String>,
}

/// Get count and handles of currently connected users
async fn admin_get_online_users(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
) -> Response {
    // Get connected user IDs from WebSocket connection manager
    let user_ids = connection_manager.get_connected_user_ids().await;
    let count = user_ids.len();

    // Get handles for first 3 users
    let manager = user_manager.lock().unwrap();
    let handles: Vec<String> = user_ids
        .into_iter()
        .take(3)
        .filter_map(|user_id| manager.get_user_handle(user_id).ok())
        .flatten()
        .collect();

    Json(OnlineUsersResponse { count, handles }).into_response()
}

// ============================================================================
// Changelog admin endpoints (requires EditCatalog permission)
// ============================================================================

#[derive(Deserialize)]
struct CreateBatchBody {
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct ListBatchesQuery {
    is_open: Option<bool>,
}

/// Create a new changelog batch
async fn admin_create_changelog_batch(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Json(body): Json<CreateBatchBody>,
) -> Response {
    match catalog_store.create_changelog_batch(&body.name, body.description.as_deref()) {
        Ok(batch) => (StatusCode::CREATED, Json(batch)).into_response(),
        Err(err) => {
            let err_msg = err.to_string();
            if err_msg.contains("already active") || err_msg.contains("already open") {
                (StatusCode::CONFLICT, err_msg).into_response()
            } else {
                error!("Error creating changelog batch: {}", err);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

/// List changelog batches with optional filter
async fn admin_list_changelog_batches(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Query(query): Query<ListBatchesQuery>,
) -> Response {
    match catalog_store.list_changelog_batches(query.is_open) {
        Ok(batches) => Json(batches).into_response(),
        Err(err) => {
            error!("Error listing changelog batches: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get a specific changelog batch by ID
async fn admin_get_changelog_batch(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(batch_id): Path<String>,
) -> Response {
    match catalog_store.get_changelog_batch(&batch_id) {
        Ok(Some(batch)) => Json(batch).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting changelog batch: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Close a changelog batch
async fn admin_close_changelog_batch(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(batch_id): Path<String>,
) -> Response {
    match catalog_store.close_changelog_batch(&batch_id) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(err) => {
            let err_msg = err.to_string();
            if err_msg.contains("not found") {
                StatusCode::NOT_FOUND.into_response()
            } else if err_msg.contains("already closed") {
                (StatusCode::CONFLICT, err_msg).into_response()
            } else {
                error!("Error closing changelog batch: {}", err);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

/// Delete a changelog batch (only if empty)
async fn admin_delete_changelog_batch(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(batch_id): Path<String>,
) -> Response {
    match catalog_store.delete_changelog_batch(&batch_id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            let err_msg = err.to_string();
            if err_msg.contains("not found") {
                StatusCode::NOT_FOUND.into_response()
            } else if err_msg.contains("recorded changes") || err_msg.contains("closed") {
                (StatusCode::BAD_REQUEST, err_msg).into_response()
            } else {
                error!("Error deleting changelog batch: {}", err);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

/// Get all changes in a changelog batch
async fn admin_get_changelog_batch_changes(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(batch_id): Path<String>,
) -> Response {
    // First check if batch exists
    match catalog_store.get_changelog_batch(&batch_id) {
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error checking changelog batch: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Ok(Some(_)) => {}
    }

    match catalog_store.get_changelog_batch_changes(&batch_id) {
        Ok(changes) => Json(changes).into_response(),
        Err(err) => {
            error!("Error getting changelog batch changes: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Get change history for a specific entity
async fn admin_get_changelog_entity_history(
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    Path((entity_type, entity_id)): Path<(String, String)>,
) -> Response {
    use crate::catalog_store::ChangeEntityType;

    let entity_type = match entity_type.to_lowercase().as_str() {
        "artist" => ChangeEntityType::Artist,
        "album" => ChangeEntityType::Album,
        "track" => ChangeEntityType::Track,
        "image" => ChangeEntityType::Image,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                "Invalid entity type. Must be one of: artist, album, track, image",
            )
                .into_response()
        }
    };

    match catalog_store.get_changelog_entity_history(entity_type, &entity_id) {
        Ok(changes) => Json(changes).into_response(),
        Err(err) => {
            error!("Error getting changelog entity history: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

impl ServerState {
    #[allow(clippy::arc_with_non_send_sync, clippy::too_many_arguments)]
    fn new(
        config: ServerConfig,
        catalog_store: Arc<dyn CatalogStore>,
        search_vault: Box<dyn SearchVault>,
        user_manager: UserManager,
        user_store: Arc<dyn FullUserStore>,
        downloader: Option<Arc<dyn crate::downloader::Downloader>>,
        media_base_path: Option<std::path::PathBuf>,
        scheduler_handle: Option<SchedulerHandle>,
    ) -> ServerState {
        // Create proxy if downloader and media_base_path are available
        let proxy = match (&downloader, media_base_path) {
            (Some(dl), Some(path)) => Some(Arc::new(super::proxy::CatalogProxy::new(
                dl.clone(),
                catalog_store.clone(),
                user_store,
                path,
            ))),
            _ => None,
        };

        ServerState {
            config,
            start_time: Instant::now(),
            catalog_store,
            search_vault: Arc::new(Mutex::new(search_vault)),
            user_manager: Arc::new(Mutex::new(user_manager)),
            downloader,
            proxy,
            ws_connection_manager: Arc::new(super::websocket::ConnectionManager::new()),
            scheduler_handle,
            hash: "123456".to_owned(),
        }
    }
}

pub fn make_app(
    config: ServerConfig,
    catalog_store: Arc<dyn CatalogStore>,
    search_vault: Box<dyn SearchVault>,
    user_store: Arc<dyn FullUserStore>,
    downloader: Option<Arc<dyn crate::downloader::Downloader>>,
    media_base_path: Option<std::path::PathBuf>,
    scheduler_handle: Option<SchedulerHandle>,
) -> Result<Router> {
    let user_manager = UserManager::new(catalog_store.clone(), user_store.clone());
    let state = ServerState::new(
        config.clone(),
        catalog_store,
        search_vault,
        user_manager,
        user_store,
        downloader,
        media_base_path,
        scheduler_handle,
    );

    // Login route with strict IP-based rate limiting
    // For rates < 60/min, we use per_second(1) and rely on burst_size to enforce the limit
    let login_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(std::cmp::max(1, (LOGIN_PER_MINUTE / 60) as u64))
            .burst_size(LOGIN_PER_MINUTE)
            .key_extractor(IpKeyExtractor)
            .finish()
            .unwrap(),
    );

    let login_routes: Router = Router::new()
        .route("/login", post(login))
        .layer(GovernorLayer::new(login_rate_limit))
        .with_state(state.clone());

    // Other auth routes without rate limiting (already authenticated)
    let other_auth_routes: Router = Router::new()
        .route("/logout", get(logout))
        .route("/session", get(get_session))
        .route("/challenge", get(get_challenge))
        .route("/challenge", post(post_challenge))
        .with_state(state.clone());

    let auth_routes = login_routes.merge(other_auth_routes);

    // Separate stream routes for different rate limiting
    let stream_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(std::cmp::max(1, (STREAM_PER_MINUTE / 60) as u64))
            .burst_size(STREAM_PER_MINUTE)
            .key_extractor(UserOrIpKeyExtractor)
            .finish()
            .unwrap(),
    );

    let stream_routes: Router = Router::new()
        .route("/stream/{id}", get(stream_track))
        .layer(GovernorLayer::new(stream_rate_limit))
        .with_state(state.clone());

    // Content read routes (album, artist, track, image)
    let content_read_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(std::cmp::max(1, (CONTENT_READ_PER_MINUTE / 60) as u64))
            .burst_size(CONTENT_READ_PER_MINUTE)
            .key_extractor(UserOrIpKeyExtractor)
            .finish()
            .unwrap(),
    );

    let content_read_routes: Router = Router::new()
        .route("/album/{id}", get(get_album))
        .route("/album/{id}/resolved", get(get_resolved_album))
        .route("/artist/{id}", get(get_artist))
        .route("/artist/{id}/discography", get(get_artist_discography))
        .route("/track/{id}", get(get_track))
        .route("/track/{id}/resolved", get(get_resolved_track))
        .route("/image/{id}", get(get_image))
        .route("/whatsnew", get(get_whats_new))
        .route("/popular", get(get_popular_content))
        .layer(GovernorLayer::new(content_read_rate_limit))
        .with_state(state.clone());

    // Merge content routes and apply common middleware
    let mut content_routes: Router = stream_routes
        .merge(content_read_routes)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .layer(middleware::from_fn_with_state(
            config.content_cache_age_sec,
            http_cache,
        ));

    // Write rate limiting for user content modifications
    let write_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(std::cmp::max(1, (WRITE_PER_MINUTE / 60) as u64))
            .burst_size(WRITE_PER_MINUTE)
            .key_extractor(UserOrIpKeyExtractor)
            .finish()
            .unwrap(),
    );

    // Create a separate rate limit config for user content reads (same as general content reads)
    let user_content_read_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(std::cmp::max(1, (CONTENT_READ_PER_MINUTE / 60) as u64))
            .burst_size(CONTENT_READ_PER_MINUTE)
            .key_extractor(UserOrIpKeyExtractor)
            .finish()
            .unwrap(),
    );

    // Liked content READ routes (higher limit)
    // Route pattern: /liked/{content_type} - returns list of liked content IDs
    let liked_content_read_routes: Router = Router::new()
        .route("/liked/{content_type}", get(get_user_liked_content))
        .layer(GovernorLayer::new(user_content_read_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_like_content,
        ))
        .with_state(state.clone());

    // Liked content WRITE routes (stricter limit)
    // Route pattern: /liked/{content_type}/{content_id}
    let liked_content_write_routes: Router = Router::new()
        .route(
            "/liked/{content_type}/{content_id}",
            post(add_user_liked_content),
        )
        .route(
            "/liked/{content_type}/{content_id}",
            delete(delete_user_liked_content),
        )
        .layer(GovernorLayer::new(write_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_like_content,
        ))
        .with_state(state.clone());

    let liked_content_routes = liked_content_read_routes.merge(liked_content_write_routes);

    // Playlist READ routes (higher limit)
    let playlist_read_routes: Router = Router::new()
        .route("/playlist/{id}", get(get_playlist))
        .route("/playlists", get(get_user_playlists))
        .layer(GovernorLayer::new(user_content_read_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_own_playlists,
        ))
        .with_state(state.clone());

    // Playlist WRITE routes (stricter limit)
    let playlist_write_routes: Router = Router::new()
        .route("/playlist", post(post_playlist))
        .route("/playlist/{id}", put(put_playlist))
        .route("/playlist/{id}", delete(delete_playlist))
        .route("/playlist/{id}/add", put(add_playlist_tracks))
        .route("/playlist/{id}/remove", put(remove_tracks_from_playlist))
        .layer(GovernorLayer::new(write_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_own_playlists,
        ))
        .with_state(state.clone());

    let playlist_routes = playlist_read_routes.merge(playlist_write_routes);

    // Apply search rate limiting to search routes if they exist
    if let Some(search_routes) = make_search_routes(state.clone()) {
        let search_rate_limit = Arc::new(
            GovernorConfigBuilder::default()
                .per_second(std::cmp::max(1, (SEARCH_PER_MINUTE / 60) as u64))
                .burst_size(SEARCH_PER_MINUTE)
                .key_extractor(UserOrIpKeyExtractor)
                .finish()
                .unwrap(),
        );

        let rate_limited_search_routes = search_routes.layer(GovernorLayer::new(search_rate_limit));
        content_routes = content_routes.merge(rate_limited_search_routes);
    }

    // Listening stats routes (requires AccessCatalog permission)
    let listening_stats_routes: Router = Router::new()
        .route("/listening", post(post_listening_event))
        .route("/listening/summary", get(get_user_listening_summary))
        .route("/listening/history", get(get_user_listening_history))
        .route("/listening/events", get(get_user_listening_events))
        .layer(GovernorLayer::new(write_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .with_state(state.clone());

    // User settings routes (requires AccessCatalog permission)
    let settings_routes: Router = Router::new()
        .route("/settings", get(get_user_settings))
        .route("/settings", put(update_user_settings))
        .layer(GovernorLayer::new(write_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .with_state(state.clone());

    let user_routes = liked_content_routes
        .merge(playlist_routes)
        .merge(listening_stats_routes)
        .merge(settings_routes);

    // Sync routes (requires AccessCatalog permission)
    // Rate limiting: uses content read limit since these are read operations
    let sync_routes: Router = Router::new()
        .route("/state", get(get_sync_state))
        .route("/events", get(get_sync_events))
        .layer(GovernorLayer::new(user_content_read_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .with_state(state.clone());

    // Catalog editing routes (requires EditCatalog permission)
    let catalog_edit_routes: Router = Router::new()
        // Artist CRUD
        .route("/artist", post(create_artist))
        .route("/artist/{id}", put(update_artist))
        .route("/artist/{id}", delete(delete_artist))
        // Album CRUD
        .route("/album", post(create_album))
        .route("/album/{id}", put(update_album))
        .route("/album/{id}", delete(delete_album))
        // Track CRUD
        .route("/track", post(create_track))
        .route("/track/{id}", put(update_track))
        .route("/track/{id}", delete(delete_track))
        // Image CRUD
        .route("/image", post(create_image))
        .route("/image/{id}", put(update_image))
        .route("/image/{id}", delete(delete_image))
        .layer(GovernorLayer::new(write_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_edit_catalog,
        ))
        .with_state(state.clone());

    // Merge catalog edit routes into content routes
    content_routes = content_routes.merge(catalog_edit_routes);

    // Admin server routes (requires ServerAdmin permission)
    let admin_server_routes: Router = Router::new()
        .route("/reboot", post(reboot_server))
        .route("/jobs", get(admin_list_jobs))
        .route("/jobs/{job_id}", get(admin_get_job))
        .route("/jobs/{job_id}/trigger", post(admin_trigger_job))
        .route("/jobs/{job_id}/history", get(admin_get_job_history))
        .layer(GovernorLayer::new(write_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_server_admin,
        ))
        .with_state(state.clone());

    // Admin user management routes (requires ManagePermissions permission)
    let admin_user_routes: Router = Router::new()
        .route("/users", get(admin_get_users))
        .route("/users", post(admin_create_user))
        .route("/users/{user_handle}", delete(admin_delete_user))
        .route("/users/{user_handle}/roles", get(admin_get_user_roles))
        .route("/users/{user_handle}/roles", post(admin_add_user_role))
        .route(
            "/users/{user_handle}/roles/{role}",
            delete(admin_remove_user_role),
        )
        .route(
            "/users/{user_handle}/permissions",
            get(admin_get_user_permissions),
        )
        .route(
            "/users/{user_handle}/permissions",
            post(admin_add_user_extra_permission),
        )
        .route(
            "/permissions/{permission_id}",
            delete(admin_remove_extra_permission),
        )
        .route(
            "/users/{user_handle}/credentials",
            get(admin_get_user_credentials_status),
        )
        .route(
            "/users/{user_handle}/password",
            put(admin_set_user_password),
        )
        .route(
            "/users/{user_handle}/password",
            delete(admin_delete_user_password),
        )
        // Bandwidth statistics routes
        .route("/bandwidth/summary", get(admin_get_bandwidth_summary))
        .route("/bandwidth/usage", get(admin_get_bandwidth_usage))
        .route(
            "/bandwidth/users/{user_handle}/summary",
            get(admin_get_user_bandwidth_summary),
        )
        .route(
            "/bandwidth/users/{user_handle}/usage",
            get(admin_get_user_bandwidth_usage),
        )
        .layer(GovernorLayer::new(write_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_manage_permissions,
        ))
        .with_state(state.clone());

    // Admin listening stats routes (requires ViewAnalytics permission)
    let admin_listening_routes: Router = Router::new()
        .route("/listening/daily", get(admin_get_daily_listening_stats))
        .route("/listening/top-tracks", get(admin_get_top_tracks))
        .route(
            "/listening/track/{track_id}",
            get(admin_get_track_listening_stats),
        )
        .route(
            "/listening/users/{user_handle}/summary",
            get(admin_get_user_listening_summary),
        )
        .route("/online-users", get(admin_get_online_users))
        .layer(GovernorLayer::new(write_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_view_analytics,
        ))
        .with_state(state.clone());

    // Admin changelog routes (requires EditCatalog permission)
    let admin_changelog_routes: Router = Router::new()
        .route("/changelog/batch", post(admin_create_changelog_batch))
        .route("/changelog/batches", get(admin_list_changelog_batches))
        .route(
            "/changelog/batch/{batch_id}",
            get(admin_get_changelog_batch),
        )
        .route(
            "/changelog/batch/{batch_id}/close",
            post(admin_close_changelog_batch),
        )
        .route(
            "/changelog/batch/{batch_id}",
            delete(admin_delete_changelog_batch),
        )
        .route(
            "/changelog/batch/{batch_id}/changes",
            get(admin_get_changelog_batch_changes),
        )
        .route(
            "/changelog/entity/{entity_type}/{entity_id}",
            get(admin_get_changelog_entity_history),
        )
        .layer(GovernorLayer::new(write_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_edit_catalog,
        ))
        .with_state(state.clone());

    let admin_routes = admin_server_routes
        .merge(admin_user_routes)
        .merge(admin_listening_routes)
        .merge(admin_changelog_routes);

    let home_router: Router = match config.frontend_dir_path {
        Some(ref frontend_path) => {
            let index_path = std::path::Path::new(frontend_path).join("index.html");
            let static_files_service = ServeDir::new(frontend_path)
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new(index_path));
            Router::new().fallback_service(static_files_service)
        }
        None => Router::new()
            .route("/", get(home))
            .with_state(state.clone()),
    };

    // WebSocket route - requires authentication (Session extractor will validate)
    let ws_routes: Router = Router::new()
        .route("/ws", get(super::websocket::ws_handler))
        .with_state(state.clone());

    let mut app: Router = home_router
        .nest("/v1/auth", auth_routes)
        .nest("/v1/content", content_routes)
        .nest("/v1/user", user_routes)
        .nest("/v1/admin", admin_routes)
        .nest("/v1/sync", sync_routes)
        .nest("/v1", ws_routes);

    #[cfg(feature = "slowdown")]
    {
        app = app.layer(middleware::from_fn(slowdown_request));
    }

    // Apply global rate limit to entire app (protects against overall abuse)
    let global_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(std::cmp::max(1, (GLOBAL_PER_MINUTE / 60) as u64))
            .burst_size(GLOBAL_PER_MINUTE)
            .key_extractor(UserOrIpKeyExtractor)
            .finish()
            .unwrap(),
    );

    app = app.layer(GovernorLayer::new(global_rate_limit));

    app = app.layer(middleware::from_fn_with_state(state.clone(), log_requests));

    Ok(app)
}

/// Interval between stale batch checks (10 minutes in seconds)
/// The actual staleness threshold is configured in ChangeLogStore (default 1 hour).
const STALE_BATCH_CHECK_INTERVAL_SECS: u64 = 600;

#[allow(clippy::too_many_arguments)]
pub async fn run_server(
    catalog_store: Arc<dyn CatalogStore>,
    search_vault: Box<dyn SearchVault>,
    user_store: Arc<dyn FullUserStore>,
    requests_logging_level: RequestsLoggingLevel,
    port: u16,
    metrics_port: u16,
    content_cache_age_sec: usize,
    frontend_dir_path: Option<String>,
    downloader: Option<Arc<dyn crate::downloader::Downloader>>,
    media_base_path: Option<std::path::PathBuf>,
    scheduler_handle: Option<SchedulerHandle>,
    ssl_settings: Option<SslSettings>,
) -> Result<()> {
    let config = ServerConfig {
        port,
        requests_logging_level,
        content_cache_age_sec,
        frontend_dir_path,
    };
    let app = make_app(
        config,
        catalog_store.clone(),
        search_vault,
        user_store,
        downloader,
        media_base_path,
        scheduler_handle,
    )?;

    // Create a minimal metrics-only server (always HTTP, internal use)
    let metrics_app = Router::new().route("/metrics", get(super::metrics::metrics_handler));
    let metrics_listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", metrics_port))
        .await
        .unwrap();

    // Spawn the stale batch auto-close background task
    let catalog_store_for_bg = catalog_store.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            STALE_BATCH_CHECK_INTERVAL_SECS,
        ));
        loop {
            interval.tick().await;
            check_and_close_stale_batches(&catalog_store_for_bg);
        }
    });

    // Run main server with or without TLS
    if let Some(ssl) = ssl_settings {
        info!("Starting HTTPS server with TLS on port {}", port);
        let rustls_config = load_rustls_config(&ssl)?;
        let addr = SocketAddr::from(([0, 0, 0, 0], port));

        tokio::select! {
            result = axum_server::bind_rustls(addr, rustls_config)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>()) => {
                result?;
            }
            result = axum::serve(metrics_listener, metrics_app) => {
                result?;
            }
        }
    } else {
        info!("Starting HTTP server on port {}", port);
        let main_listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
            .await
            .unwrap();

        tokio::select! {
            result = axum::serve(
                main_listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            ) => {
                result?;
            }
            result = axum::serve(metrics_listener, metrics_app) => {
                result?;
            }
        }
    }

    Ok(())
}

fn load_rustls_config(ssl: &SslSettings) -> Result<axum_server::tls_rustls::RustlsConfig> {
    use axum_server::tls_rustls::RustlsConfig;

    // Load certificate chain
    let cert_file = File::open(&ssl.cert_path)
        .map_err(|e| anyhow::anyhow!("Failed to open certificate file: {}", e))?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<_> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("Failed to parse certificate: {}", e))?;

    if certs.is_empty() {
        anyhow::bail!("No certificates found in {:?}", ssl.cert_path);
    }

    // Load private key
    let key_file = File::open(&ssl.key_path)
        .map_err(|e| anyhow::anyhow!("Failed to open key file: {}", e))?;
    let mut key_reader = BufReader::new(key_file);
    let key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| anyhow::anyhow!("Failed to parse private key: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("No private key found in {:?}", ssl.key_path))?;

    // Create RustlsConfig from the loaded cert and key
    let config = RustlsConfig::from_der(
        certs.into_iter().map(|c| c.to_vec()).collect(),
        key.secret_der().to_vec(),
    );

    Ok(futures::executor::block_on(config)?)
}

/// Close stale changelog batches automatically.
fn check_and_close_stale_batches(catalog_store: &Arc<dyn CatalogStore>) {
    super::metrics::CHANGELOG_STALE_BATCH_CHECKS_TOTAL.inc();

    match catalog_store.close_stale_batches() {
        Ok(closed_count) => {
            if closed_count > 0 {
                info!(
                    "Background task closed {} stale changelog batch(es)",
                    closed_count
                );
            } else {
                debug!("No stale changelog batches to close");
            }
            super::metrics::CHANGELOG_STALE_BATCHES.set(0.0);
        }
        Err(e) => {
            error!("Failed to close stale batches: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_store::NullCatalogStore;
    use crate::search::NoOpSearchVault;
    use crate::user::auth::UserAuthCredentials;
    use crate::user::auth::{AuthToken, AuthTokenValue};
    use crate::user::user_models::{BandwidthSummary, BandwidthUsage, LikedContentType};
    use crate::user::{
        UserAuthCredentialsStore, UserAuthTokenStore, UserBandwidthStore, UserStore,
    };
    use axum::extract::ConnectInfo;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt; // for `call`, `oneshot`, and `ready

    #[tokio::test]
    async fn responds_forbidden_on_protected_routes() {
        let user_store = Arc::new(InMemoryUserStore::default());
        let catalog_store: Arc<dyn CatalogStore> = Arc::new(NullCatalogStore);
        let app = &mut make_app(
            ServerConfig::default(),
            catalog_store,
            Box::new(NoOpSearchVault {}),
            user_store,
            None, // no downloader
            None, // no media_base_path
            None, // no scheduler_handle
        )
        .unwrap();

        // Create a test socket address for rate limiting
        let test_addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();

        let protected_routes = vec![
            "/v1/content/artist/123",
            "/v1/content/album/123",
            "/v1/content/album/123/resolved",
            "/v1/content/artist/123/discography",
            "/v1/content/track/123",
            "/v1/content/track/123/resolved",
            "/v1/content/image/123",
            "/v1/content/stream/123",
            "/v1/auth/logout",
            // Admin routes (require ManagePermissions)
            "/v1/admin/users",
            "/v1/admin/users/testuser/roles",
        ];

        for route in protected_routes.into_iter() {
            println!("Trying route {}", route);
            let mut request = Request::builder().uri(route).body(Body::empty()).unwrap();
            // Add ConnectInfo extension for rate limiting
            request.extensions_mut().insert(ConnectInfo(test_addr));
            let response = app.oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }

        // Only test search route if search feature is enabled
        #[cfg(not(feature = "no_search"))]
        {
            let mut request = Request::builder()
                .method("POST")
                .uri("/v1/content/search")
                .body(Body::empty())
                .unwrap();
            // Add ConnectInfo extension for rate limiting
            request.extensions_mut().insert(ConnectInfo(test_addr));
            let response = app.oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
    }

    #[derive(Default)]
    struct InMemoryUserStore {}

    impl UserStore for InMemoryUserStore {
        fn create_user(&self, _user_handle: &str) -> Result<usize> {
            todo!()
        }

        fn delete_user(&self, _user_id: usize) -> Result<bool> {
            todo!()
        }

        fn get_user_handle(&self, _user_id: usize) -> Result<Option<String>> {
            todo!()
        }

        fn get_user_id(&self, _user_handle: &str) -> Result<Option<usize>> {
            todo!()
        }

        fn get_user_playlists(&self, _user_id: usize) -> Result<Vec<String>> {
            todo!()
        }

        fn is_user_liked_content(
            &self,
            _user_id: usize,
            _content_id: &str,
        ) -> Result<Option<bool>> {
            todo!()
        }

        fn set_user_liked_content(
            &self,
            _user_id: usize,
            _content_id: &str,
            _content_type: LikedContentType,
            _liked: bool,
        ) -> Result<()> {
            todo!()
        }

        fn get_all_user_handles(&self) -> Result<Vec<String>> {
            todo!()
        }

        fn get_user_liked_content(
            &self,
            _user_id: usize,
            _content_type: LikedContentType,
        ) -> Result<Vec<String>> {
            todo!()
        }

        fn create_user_playlist(
            &self,
            _user_id: usize,
            _playlist_name: &str,
            _creator_id: usize,
            _track_ids: Vec<String>,
        ) -> Result<String> {
            todo!()
        }

        fn delete_user_playlist(&self, _playlist_id: &str, _user_id: usize) -> Result<()> {
            todo!()
        }

        fn update_user_playlist(
            &self,
            _playlist_id: &str,
            _user_id: usize,
            _playlist_name: Option<String>,
            _track_ids: Option<Vec<String>>,
        ) -> Result<()> {
            todo!()
        }

        fn get_user_playlist(
            &self,
            _playlist_id: &str,
            _user_id: usize,
        ) -> Result<crate::user::UserPlaylist> {
            todo!()
        }

        fn get_user_roles(&self, _user_id: usize) -> Result<Vec<crate::user::UserRole>> {
            todo!()
        }

        fn add_user_role(&self, _user_id: usize, _role: crate::user::UserRole) -> Result<()> {
            todo!()
        }

        fn remove_user_role(&self, _user_id: usize, _role: crate::user::UserRole) -> Result<()> {
            todo!()
        }

        fn add_user_extra_permission(
            &self,
            _user_id: usize,
            _grant: crate::user::PermissionGrant,
        ) -> Result<usize> {
            todo!()
        }

        fn remove_user_extra_permission(
            &self,
            _permission_id: usize,
        ) -> Result<Option<(usize, Permission)>> {
            todo!()
        }

        fn decrement_permission_countdown(&self, _permission_id: usize) -> Result<bool> {
            todo!()
        }

        fn resolve_user_permissions(
            &self,
            _user_id: usize,
        ) -> Result<Vec<crate::user::Permission>> {
            Ok(vec![])
        }
    }

    impl UserAuthTokenStore for InMemoryUserStore {
        fn get_user_auth_token(&self, _token: &AuthTokenValue) -> Result<Option<AuthToken>> {
            todo!()
        }

        fn delete_user_auth_token(&self, _token: &AuthTokenValue) -> Result<Option<AuthToken>> {
            todo!()
        }

        fn update_user_auth_token_last_used_timestamp(
            &self,
            _token: &AuthTokenValue,
        ) -> Result<()> {
            todo!()
        }

        fn add_user_auth_token(&self, _token: AuthToken) -> Result<()> {
            todo!()
        }

        fn get_all_user_auth_tokens(&self, _user_handle: &str) -> Result<Vec<AuthToken>> {
            todo!()
        }

        fn prune_unused_auth_tokens(&self, _unused_for_days: u64) -> Result<usize> {
            todo!()
        }
    }

    impl UserAuthCredentialsStore for InMemoryUserStore {
        fn get_user_auth_credentials(
            &self,
            _user_handle: &str,
        ) -> Result<Option<UserAuthCredentials>> {
            todo!()
        }

        fn update_user_auth_credentials(&self, _credentials: UserAuthCredentials) -> Result<()> {
            todo!()
        }
    }

    impl UserBandwidthStore for InMemoryUserStore {
        fn record_bandwidth_usage(
            &self,
            _user_id: usize,
            _date: u32,
            _endpoint_category: &str,
            _bytes_sent: u64,
            _request_count: u64,
        ) -> Result<()> {
            Ok(()) // No-op for tests
        }

        fn get_user_bandwidth_usage(
            &self,
            _user_id: usize,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<Vec<BandwidthUsage>> {
            Ok(vec![])
        }

        fn get_user_bandwidth_summary(
            &self,
            _user_id: usize,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<BandwidthSummary> {
            Ok(BandwidthSummary {
                user_id: Some(_user_id),
                total_bytes_sent: 0,
                total_requests: 0,
                by_category: std::collections::HashMap::new(),
            })
        }

        fn get_all_bandwidth_usage(
            &self,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<Vec<BandwidthUsage>> {
            Ok(vec![])
        }

        fn get_total_bandwidth_summary(
            &self,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<BandwidthSummary> {
            Ok(BandwidthSummary {
                user_id: None,
                total_bytes_sent: 0,
                total_requests: 0,
                by_category: std::collections::HashMap::new(),
            })
        }

        fn prune_bandwidth_usage(&self, _older_than_days: u32) -> Result<usize> {
            Ok(0)
        }
    }

    impl crate::user::UserListeningStore for InMemoryUserStore {
        fn record_listening_event(
            &self,
            _event: crate::user::ListeningEvent,
        ) -> Result<(usize, bool)> {
            Ok((1, true))
        }

        fn get_user_listening_events(
            &self,
            _user_id: usize,
            _start_date: u32,
            _end_date: u32,
            _limit: Option<usize>,
            _offset: Option<usize>,
        ) -> Result<Vec<crate::user::ListeningEvent>> {
            Ok(vec![])
        }

        fn get_user_listening_summary(
            &self,
            user_id: usize,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<crate::user::ListeningSummary> {
            Ok(crate::user::ListeningSummary {
                user_id: Some(user_id),
                total_plays: 0,
                total_duration_seconds: 0,
                completed_plays: 0,
                unique_tracks: 0,
            })
        }

        fn get_user_listening_history(
            &self,
            _user_id: usize,
            _limit: usize,
        ) -> Result<Vec<crate::user::UserListeningHistoryEntry>> {
            Ok(vec![])
        }

        fn get_track_listening_stats(
            &self,
            track_id: &str,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<crate::user::TrackListeningStats> {
            Ok(crate::user::TrackListeningStats {
                track_id: track_id.to_string(),
                play_count: 0,
                total_duration_seconds: 0,
                completed_count: 0,
                unique_listeners: 0,
            })
        }

        fn get_daily_listening_stats(
            &self,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<Vec<crate::user::DailyListeningStats>> {
            Ok(vec![])
        }

        fn get_top_tracks(
            &self,
            _start_date: u32,
            _end_date: u32,
            _limit: usize,
        ) -> Result<Vec<crate::user::TrackListeningStats>> {
            Ok(vec![])
        }

        fn prune_listening_events(&self, _older_than_days: u32) -> Result<usize> {
            Ok(0)
        }
    }

    impl crate::user::UserSettingsStore for InMemoryUserStore {
        fn get_user_setting(
            &self,
            _user_id: usize,
            _key: &str,
        ) -> Result<Option<crate::user::UserSetting>> {
            Ok(None)
        }

        fn set_user_setting(
            &self,
            _user_id: usize,
            _setting: crate::user::UserSetting,
        ) -> Result<()> {
            Ok(())
        }

        fn get_all_user_settings(&self, _user_id: usize) -> Result<Vec<crate::user::UserSetting>> {
            Ok(vec![])
        }
    }

    impl crate::user::DeviceStore for InMemoryUserStore {
        fn register_or_update_device(
            &self,
            _registration: &crate::user::device::DeviceRegistration,
        ) -> Result<usize> {
            Ok(1)
        }
        fn get_device(&self, _device_id: usize) -> Result<Option<crate::user::device::Device>> {
            Ok(None)
        }
        fn get_device_by_uuid(
            &self,
            _device_uuid: &str,
        ) -> Result<Option<crate::user::device::Device>> {
            Ok(None)
        }
        fn get_user_devices(&self, _user_id: usize) -> Result<Vec<crate::user::device::Device>> {
            Ok(vec![])
        }
        fn associate_device_with_user(&self, _device_id: usize, _user_id: usize) -> Result<()> {
            Ok(())
        }
        fn touch_device(&self, _device_id: usize) -> Result<()> {
            Ok(())
        }
        fn prune_orphaned_devices(&self, _inactive_for_days: u32) -> Result<usize> {
            Ok(0)
        }
        fn enforce_user_device_limit(&self, _user_id: usize, _max_devices: usize) -> Result<usize> {
            Ok(0)
        }
    }

    impl crate::user::UserEventStore for InMemoryUserStore {
        fn append_event(
            &self,
            _user_id: usize,
            event: &crate::user::sync_events::UserEvent,
        ) -> Result<crate::user::sync_events::StoredEvent> {
            Ok(crate::user::sync_events::StoredEvent {
                seq: 1,
                event: event.clone(),
                server_timestamp: 0,
            })
        }

        fn get_events_since(
            &self,
            _user_id: usize,
            _since_seq: i64,
        ) -> Result<Vec<crate::user::sync_events::StoredEvent>> {
            Ok(vec![])
        }

        fn get_current_seq(&self, _user_id: usize) -> Result<i64> {
            Ok(0)
        }

        fn get_min_seq(&self, _user_id: usize) -> Result<Option<i64>> {
            Ok(None)
        }

        fn prune_events_older_than(&self, _before_timestamp: i64) -> Result<u64> {
            Ok(0)
        }
    }

    // Tests for admin endpoints using SqliteUserStore
    mod admin_endpoint_tests {
        use super::*;
        use crate::user::SqliteUserStore;
        use std::time::SystemTime;
        use tempfile::TempDir;

        fn create_test_store() -> (SqliteUserStore, TempDir) {
            let temp_dir = TempDir::new().unwrap();
            let temp_file_path = temp_dir.path().join("test.db");
            let store = SqliteUserStore::new(&temp_file_path).unwrap();
            (store, temp_dir)
        }

        fn create_test_store_with_admin_user() -> (SqliteUserStore, usize, TempDir) {
            let (store, temp_dir) = create_test_store();
            let user_id = store.create_user("admin_user").unwrap();
            store
                .add_user_role(user_id, crate::user::UserRole::Admin)
                .unwrap();
            (store, user_id, temp_dir)
        }

        #[allow(dead_code)]
        fn create_test_store_with_regular_user() -> (SqliteUserStore, usize, TempDir) {
            let (store, temp_dir) = create_test_store();
            let user_id = store.create_user("regular_user").unwrap();
            store
                .add_user_role(user_id, crate::user::UserRole::Regular)
                .unwrap();
            (store, user_id, temp_dir)
        }

        #[test]
        fn test_get_all_user_handles() {
            let (store, _temp_dir) = create_test_store();
            store.create_user("user1").unwrap();
            store.create_user("user2").unwrap();
            store.create_user("user3").unwrap();

            let handles = store.get_all_user_handles().unwrap();
            assert_eq!(handles.len(), 3);
            assert!(handles.contains(&"user1".to_string()));
            assert!(handles.contains(&"user2".to_string()));
            assert!(handles.contains(&"user3".to_string()));
        }

        #[test]
        fn test_get_user_id() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            let found_id = store.get_user_id("testuser").unwrap();
            assert_eq!(found_id, Some(user_id));

            let not_found = store.get_user_id("nonexistent").unwrap();
            assert_eq!(not_found, None);
        }

        #[test]
        fn test_get_user_roles() {
            let (store, user_id, _temp_dir) = create_test_store_with_admin_user();

            let roles = store.get_user_roles(user_id).unwrap();
            assert_eq!(roles.len(), 1);
            assert_eq!(roles[0], crate::user::UserRole::Admin);
        }

        #[test]
        fn test_add_and_remove_user_role() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            // Add Admin role
            store
                .add_user_role(user_id, crate::user::UserRole::Admin)
                .unwrap();
            let roles = store.get_user_roles(user_id).unwrap();
            assert!(roles.contains(&crate::user::UserRole::Admin));

            // Add Regular role
            store
                .add_user_role(user_id, crate::user::UserRole::Regular)
                .unwrap();
            let roles = store.get_user_roles(user_id).unwrap();
            assert_eq!(roles.len(), 2);
            assert!(roles.contains(&crate::user::UserRole::Admin));
            assert!(roles.contains(&crate::user::UserRole::Regular));

            // Remove Admin role
            store
                .remove_user_role(user_id, crate::user::UserRole::Admin)
                .unwrap();
            let roles = store.get_user_roles(user_id).unwrap();
            assert_eq!(roles.len(), 1);
            assert!(roles.contains(&crate::user::UserRole::Regular));
        }

        #[test]
        fn test_add_duplicate_role_is_idempotent() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            store
                .add_user_role(user_id, crate::user::UserRole::Admin)
                .unwrap();
            store
                .add_user_role(user_id, crate::user::UserRole::Admin)
                .unwrap();

            let roles = store.get_user_roles(user_id).unwrap();
            // Should still only have one Admin role
            assert_eq!(
                roles
                    .iter()
                    .filter(|r| **r == crate::user::UserRole::Admin)
                    .count(),
                1
            );
        }

        #[test]
        fn test_resolve_user_permissions_from_role() {
            let (store, user_id, _temp_dir) = create_test_store_with_admin_user();

            let permissions = store.resolve_user_permissions(user_id).unwrap();
            // Admin should have: AccessCatalog, EditCatalog, ManagePermissions, IssueContentDownload, ServerAdmin
            assert!(permissions.contains(&crate::user::Permission::AccessCatalog));
            assert!(permissions.contains(&crate::user::Permission::EditCatalog));
            assert!(permissions.contains(&crate::user::Permission::ManagePermissions));
            assert!(permissions.contains(&crate::user::Permission::IssueContentDownload));
            assert!(permissions.contains(&crate::user::Permission::ServerAdmin));
        }

        #[test]
        fn test_add_extra_permission_with_countdown() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            let grant = crate::user::PermissionGrant::Extra {
                start_time: SystemTime::now(),
                end_time: None,
                permission: crate::user::Permission::EditCatalog,
                countdown: Some(5),
            };

            let permission_id = store.add_user_extra_permission(user_id, grant).unwrap();
            assert!(permission_id > 0);

            // Verify permission is resolved
            let permissions = store.resolve_user_permissions(user_id).unwrap();
            assert!(permissions.contains(&crate::user::Permission::EditCatalog));
        }

        #[test]
        fn test_add_extra_permission_with_time_limit() {
            use std::time::Duration;

            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            let start_time = SystemTime::now();
            let end_time = start_time + Duration::from_secs(3600); // 1 hour from now

            let grant = crate::user::PermissionGrant::Extra {
                start_time,
                end_time: Some(end_time),
                permission: crate::user::Permission::ServerAdmin,
                countdown: None,
            };

            let permission_id = store.add_user_extra_permission(user_id, grant).unwrap();
            assert!(permission_id > 0);

            // Verify permission is resolved (still within time limit)
            let permissions = store.resolve_user_permissions(user_id).unwrap();
            assert!(permissions.contains(&crate::user::Permission::ServerAdmin));
        }

        #[test]
        fn test_remove_extra_permission() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            let grant = crate::user::PermissionGrant::Extra {
                start_time: SystemTime::now(),
                end_time: None,
                permission: crate::user::Permission::EditCatalog,
                countdown: None,
            };

            let permission_id = store.add_user_extra_permission(user_id, grant).unwrap();

            // Verify permission exists
            let permissions = store.resolve_user_permissions(user_id).unwrap();
            assert!(permissions.contains(&crate::user::Permission::EditCatalog));

            // Remove it
            store.remove_user_extra_permission(permission_id).unwrap();

            // Verify permission is gone
            let permissions = store.resolve_user_permissions(user_id).unwrap();
            assert!(!permissions.contains(&crate::user::Permission::EditCatalog));
        }

        #[test]
        fn test_countdown_decrements_and_removes_permission() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            let grant = crate::user::PermissionGrant::Extra {
                start_time: SystemTime::now(),
                end_time: None,
                permission: crate::user::Permission::EditCatalog,
                countdown: Some(2),
            };

            let permission_id = store.add_user_extra_permission(user_id, grant).unwrap();

            // First decrement - should still have uses remaining
            let has_remaining = store.decrement_permission_countdown(permission_id).unwrap();
            assert!(has_remaining);

            // Second decrement - should be last use, permission removed
            let has_remaining = store.decrement_permission_countdown(permission_id).unwrap();
            assert!(!has_remaining);

            // Verify permission is gone
            let permissions = store.resolve_user_permissions(user_id).unwrap();
            assert!(!permissions.contains(&crate::user::Permission::EditCatalog));
        }

        #[test]
        fn test_user_manager_get_user_id() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            let catalog_store: std::sync::Arc<dyn crate::catalog_store::CatalogStore> =
                std::sync::Arc::new(crate::catalog_store::NullCatalogStore);
            let user_manager = crate::user::UserManager::new(catalog_store, Arc::new(store));

            let found_id = user_manager.get_user_id("testuser").unwrap();
            assert_eq!(found_id, Some(user_id));

            let not_found = user_manager.get_user_id("nonexistent").unwrap();
            assert_eq!(not_found, None);
        }
    }
}
