//! HTTP server implementation with route handlers
//! Note: Many functions appear unused but are registered as route handlers

#![allow(dead_code)] // Route handlers registered dynamically

use anyhow::Result;
use std::{
    fs::File,
    io::Read,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use tracing::{debug, error, info, warn};

use crate::background_jobs::{JobError, JobInfo, SchedulerHandle};
use crate::catalog_store::CatalogStore;
use crate::notifications::NotificationService;
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
    extract_user_id_for_rate_limit, http_cache, log_requests, make_search_admin_routes,
    make_search_routes, metrics, state::*, IpKeyExtractor, RequestsLoggingLevel, ServerConfig,
    UserOrIpKeyExtractor, CONTENT_READ_PER_MINUTE, GLOBAL_PER_MINUTE, LOGIN_PER_MINUTE,
    SEARCH_PER_MINUTE, STREAM_PER_MINUTE, WRITE_PER_MINUTE,
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

async fn require_request_content(
    session: Session,
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    debug!(
        "require_request_content: user_id={}, has_permission={}, permissions={:?}",
        session.user_id,
        session.has_permission(Permission::RequestContent),
        session.permissions
    );
    if !session.has_permission(Permission::RequestContent) {
        debug!(
            "require_request_content: FORBIDDEN - user_id={} lacks RequestContent permission",
            session.user_id
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!(
        "require_request_content: ALLOWED - user_id={}",
        session.user_id
    );
    next.run(request).await
}

async fn require_download_manager_admin(
    session: Session,
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    debug!(
        "require_download_manager_admin: user_id={} permissions={:?}",
        session.user_id, session.permissions
    );
    if !session.has_permission(Permission::DownloadManagerAdmin) {
        debug!(
            "require_download_manager_admin: FORBIDDEN - user_id={} lacks DownloadManagerAdmin permission",
            session.user_id
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!(
        "require_download_manager_admin: ALLOWED - user_id={}",
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
    notifications: Vec<crate::notifications::Notification>,
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

    // Get notifications
    let notifications = match um.get_user_notifications(session.user_id) {
        Ok(n) => n,
        Err(err) => {
            error!("Error getting user notifications: {}", err);
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
        notifications,
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

/// POST /v1/user/notifications/{id}/read - Mark notification as read
async fn mark_notification_read(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    State(connection_manager): State<GuardedConnectionManager>,
    Path(notification_id): Path<String>,
) -> Response {
    let (notification, stored_event) = {
        let um = user_manager.lock().unwrap();

        // Mark as read
        let notification = match um.mark_notification_read(&notification_id, session.user_id) {
            Ok(Some(n)) => n,
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                error!("Error marking notification read: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        // Log sync event
        let read_at = notification
            .read_at
            .unwrap_or_else(|| chrono::Utc::now().timestamp());
        let event = UserEvent::NotificationRead {
            notification_id: notification_id.clone(),
            read_at,
        };

        let stored_event = match um.append_event(session.user_id, &event) {
            Ok(e) => Some(e),
            Err(err) => {
                warn!("Failed to log notification_read event: {}", err);
                None
            }
        };

        (notification, stored_event)
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

    // Return the notification (useful for knowing read_at timestamp)
    Json(notification).into_response()
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
    debug!(
        "get_artist: id={}, user_id={}, proxy_available={}",
        id,
        session.user_id,
        proxy.is_some()
    );

    // If proxy is available, ensure artist has complete data
    if let Some(ref proxy) = proxy {
        debug!(
            "get_artist: calling proxy.ensure_artist_complete for {}",
            id
        );
        if let Err(e) = proxy
            .ensure_artist_complete(&id, session.user_id, &session.permissions)
            .await
        {
            warn!("Proxy fetch failed for artist {}: {}", id, e);
            // Continue serving what we have
        }
    } else {
        debug!("get_artist: proxy not available for {}", id);
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
    // Try to delete auth token from database (for legacy sessions)
    // For OIDC sessions, this will fail since JWT isn't stored in DB - that's OK
    let mut locked_manager = user_manager.lock().unwrap();
    let _ = locked_manager.delete_auth_token(&session.user_id, &AuthTokenValue(session.token));

    // Always clear the session cookie
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

// ============================================================================
// OIDC Authentication Handlers
// ============================================================================

/// Query parameters for OIDC login initiation
#[derive(Deserialize, Debug, Default)]
struct OidcLoginQuery {
    /// Device ID for multi-device tracking
    device_id: Option<String>,
    /// Device type (web, android, ios, desktop)
    device_type: Option<String>,
    /// Human-readable device name
    device_name: Option<String>,
}

/// OIDC login initiation - redirects to the OIDC provider
async fn oidc_login(
    Query(params): Query<OidcLoginQuery>,
    State(oidc_client): State<OptionalOidcClient>,
    State(auth_state_store): State<GuardedAuthStateStore>,
) -> Response {
    let oidc_client = match oidc_client {
        Some(client) => client,
        None => {
            error!("OIDC login attempted but OIDC is not configured");
            return (StatusCode::SERVICE_UNAVAILABLE, "OIDC is not configured").into_response();
        }
    };

    // Build device info from query params
    let device_info = if params.device_id.is_some()
        || params.device_type.is_some()
        || params.device_name.is_some()
    {
        Some(crate::oidc::DeviceInfo {
            device_id: params.device_id,
            device_type: params.device_type,
            device_name: params.device_name,
        })
    } else {
        None
    };

    match oidc_client.authorize_url(device_info.as_ref()) {
        Ok((auth_url, state)) => {
            // Store the state for validation in callback
            auth_state_store.store(state.clone()).await;
            debug!(
                "Initiating OIDC login, redirecting to provider with state={}, device_id={:?}",
                state.csrf_token, state.device_id
            );

            // Return redirect response
            response::Builder::new()
                .status(StatusCode::FOUND)
                .header(header::LOCATION, auth_url)
                .body(Body::empty())
                .unwrap()
        }
        Err(e) => {
            error!("Failed to generate OIDC authorization URL: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Query parameters for OIDC callback
#[derive(Deserialize, Debug)]
struct OidcCallbackQuery {
    code: String,
    state: String,
}

/// OIDC callback - exchanges authorization code for tokens
async fn oidc_callback(
    Query(params): Query<OidcCallbackQuery>,
    State(oidc_client): State<OptionalOidcClient>,
    State(auth_state_store): State<GuardedAuthStateStore>,
    State(user_manager): State<GuardedUserManager>,
) -> Response {
    let start = Instant::now();
    debug!("OIDC callback received with state={}", params.state);

    let oidc_client = match oidc_client {
        Some(client) => client,
        None => {
            error!("OIDC callback received but OIDC is not configured");
            super::metrics::record_login_attempt("error", start.elapsed());
            return (StatusCode::SERVICE_UNAVAILABLE, "OIDC is not configured").into_response();
        }
    };

    // Retrieve and validate stored state
    let stored_state = match auth_state_store.take(&params.state).await {
        Some(state) => state,
        None => {
            warn!("OIDC callback with unknown or expired state");
            super::metrics::record_login_attempt("failure", start.elapsed());
            return (StatusCode::BAD_REQUEST, "Invalid or expired state").into_response();
        }
    };

    // Exchange code for tokens and validate
    let auth_result = match oidc_client
        .exchange_code(&params.code, &params.state, &stored_state)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            error!("OIDC token exchange failed: {}", e);
            super::metrics::record_login_attempt("failure", start.elapsed());
            return (StatusCode::UNAUTHORIZED, "Authentication failed").into_response();
        }
    };

    debug!(
        "OIDC authentication successful for subject={}",
        auth_result.subject
    );

    // Look up local user by OIDC subject to verify they exist
    let user_id = {
        let locked_manager = user_manager.lock().unwrap();

        match locked_manager.get_user_id_by_oidc_subject(&auth_result.subject) {
            Ok(Some(id)) => id,
            Ok(None) => {
                warn!(
                    "No local user found for OIDC subject={} (email={:?})",
                    auth_result.subject, auth_result.email
                );
                super::metrics::record_login_attempt("failure", start.elapsed());
                return (
                    StatusCode::FORBIDDEN,
                    "User not registered. Contact administrator.",
                )
                    .into_response();
            }
            Err(e) => {
                error!("Failed to look up user by OIDC subject: {}", e);
                super::metrics::record_login_attempt("error", start.elapsed());
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    };

    super::metrics::record_login_attempt("success", start.elapsed());
    info!("OIDC login successful for user_id={}", user_id);

    // Set the ID token as a cookie for web clients
    let cookie_value = HeaderValue::from_str(&format!(
        "session_token={}; Path=/; HttpOnly; SameSite=Lax",
        auth_result.id_token
    ))
    .unwrap();

    // Redirect to the app after successful authentication
    response::Builder::new()
        .status(StatusCode::FOUND)
        .header(axum::http::header::SET_COOKIE, cookie_value)
        .header(axum::http::header::LOCATION, "/")
        .body(Body::empty())
        .unwrap()
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

/// Request body for triggering a job with optional parameters.
#[derive(Debug, Deserialize, Default)]
struct TriggerJobRequest {
    /// Optional parameters to pass to the job's execute_with_params() method.
    #[serde(default)]
    params: Option<serde_json::Value>,
}

async fn admin_trigger_job(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
    Path(job_id): Path<String>,
    body: Result<Json<TriggerJobRequest>, axum::extract::rejection::JsonRejection>,
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

    // Accept either valid JSON body or no body at all (treat as empty params)
    let params = match body {
        Ok(Json(req)) => req.params,
        Err(_) => None, // No body or invalid JSON = no params
    };

    info!(
        "User {} triggering job {} with params: {:?}",
        session.user_id, job_id, params
    );

    match handle.trigger_job(&job_id, params).await {
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

/// Get job audit log entries (all jobs).
async fn admin_get_job_audit_log(
    session: Session,
    State(scheduler_handle): State<super::state::OptionalSchedulerHandle>,
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
        .unwrap_or(50);
    let offset = params
        .get("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    match handle.get_job_audit_log(limit, offset) {
        Ok(entries) => {
            debug!(
                "User {} retrieved {} job audit log entries",
                session.user_id,
                entries.len()
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({"entries": entries})),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to get job audit log: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to get job audit log"})),
            )
                .into_response()
        }
    }
}

/// Get job audit log entries for a specific job.
async fn admin_get_job_audit_log_by_job(
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
        .unwrap_or(50);
    let offset = params
        .get("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    match handle.get_job_audit_log_by_job(&job_id, limit, offset) {
        Ok(entries) => {
            debug!(
                "User {} retrieved {} job audit log entries for job {}",
                session.user_id,
                entries.len(),
                job_id
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({"entries": entries})),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to get job audit log for {}: {}", job_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to get job audit log"})),
            )
                .into_response()
        }
    }
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
        "ServerAdmin" => Permission::ServerAdmin,
        "ViewAnalytics" => Permission::ViewAnalytics,
        "RequestContent" => Permission::RequestContent,
        "DownloadManagerAdmin" => Permission::DownloadManagerAdmin,
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
    State(search_vault): State<super::state::GuardedSearchVault>,
    State(whatsnew_notifier): State<GuardedWhatsNewNotifier>,
    Path(batch_id): Path<String>,
) -> Response {
    // Get the batch info before closing (we need this for the notification)
    let batch = match catalog_store.get_changelog_batch(&batch_id) {
        Ok(Some(batch)) => batch,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting changelog batch: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Get the batch summary (counts of added items)
    let summary = match catalog_store.get_changelog_batch_summary(&batch_id) {
        Ok(summary) => summary,
        Err(err) => {
            error!("Error getting changelog batch summary: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match catalog_store.close_changelog_batch(&batch_id) {
        Ok(()) => {
            // Rebuild search index after batch close
            if let Err(err) = search_vault.lock().unwrap().rebuild_index() {
                error!("Failed to rebuild search index after batch close: {}", err);
                // Don't fail the request - batch is already closed
            }

            // Notify users who have opted in to WhatsNew notifications
            whatsnew_notifier
                .notify_batch_closed(&batch, &summary)
                .await;
            StatusCode::OK.into_response()
        }
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

// ============================================================================
// Download Manager Endpoints
// ============================================================================

/// Query parameters for download search.
#[derive(Deserialize)]
struct DownloadSearchParams {
    /// Search query string
    q: String,
    /// Content type to search for: "album" (default) or "artist"
    #[serde(rename = "type")]
    content_type: Option<String>,
}

/// GET /v1/download/search - Search for downloadable content
async fn search_download_content(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Query(params): Query<DownloadSearchParams>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let search_type = match params.content_type.as_deref() {
        Some("artist") => crate::download_manager::SearchType::Artist,
        _ => crate::download_manager::SearchType::Album,
    };

    match dm.search(&params.q, search_type).await {
        Ok(results) => Json(results).into_response(),
        Err(err) => {
            error!("Error searching download content: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// GET /v1/download/search/discography/:artist_id - Get artist discography
async fn search_download_discography(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Path(artist_id): Path<String>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    match dm.search_discography(&artist_id).await {
        Ok(result) => Json(result).into_response(),
        Err(err) => {
            let err_msg = err.to_string();
            error!(
                "Error fetching discography for artist {}: {}",
                artist_id, err_msg
            );
            if err_msg.contains("404") || err_msg.contains("not found") {
                (StatusCode::NOT_FOUND, "Artist not found").into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response()
            }
        }
    }
}

/// GET /v1/download/album/:album_id - Get detailed external album information
///
/// Fetches album metadata and tracks from the external downloader service,
/// enriched with catalog status and request status if in download queue.
async fn get_external_album_details(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Path(album_id): Path<String>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    match dm.get_external_album_details(&album_id).await {
        Ok(result) => Json(result).into_response(),
        Err(err) => {
            let err_msg = err.to_string();
            if err_msg.contains("404") || err_msg.contains("not found") {
                (StatusCode::NOT_FOUND, "Album not found").into_response()
            } else {
                error!("Error fetching external album details: {}", err);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

/// POST /v1/download/request/album - Request download of an album
async fn request_album_download(
    session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Json(request): Json<crate::download_manager::AlbumRequest>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let user_id = session.user_id.to_string();
    match dm.request_album(&user_id, request).await {
        Ok(result) => {
            metrics::record_download_user_request("album");
            Json(result).into_response()
        }
        Err(err) => {
            let err_msg = err.to_string();
            if err_msg.contains("Rate limit exceeded") {
                (StatusCode::TOO_MANY_REQUESTS, err_msg).into_response()
            } else if err_msg.contains("already in queue") {
                (StatusCode::CONFLICT, err_msg).into_response()
            } else {
                error!("Error requesting album download: {}", err);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

/// POST /v1/download/request/discography - Request download of artist discography
async fn request_discography_download(
    session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Json(request): Json<crate::download_manager::DiscographyRequest>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let user_id = session.user_id.to_string();
    match dm.request_discography(&user_id, request) {
        Ok(result) => {
            metrics::record_download_user_request("discography");
            Json(result).into_response()
        }
        Err(err) => {
            let err_msg = err.to_string();
            if err_msg.contains("Rate limit exceeded") {
                (StatusCode::TOO_MANY_REQUESTS, err_msg).into_response()
            } else if err_msg.contains("not yet implemented") {
                (StatusCode::NOT_IMPLEMENTED, err_msg).into_response()
            } else {
                error!("Error requesting discography download: {}", err);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

/// Query parameters for my-requests endpoint
#[derive(Deserialize)]
struct MyRequestsParams {
    #[serde(default = "default_my_requests_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

fn default_my_requests_limit() -> usize {
    50
}

/// GET /v1/download/my-requests - Get user's download requests
async fn get_my_download_requests(
    session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Query(params): Query<MyRequestsParams>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let user_id = session.user_id.to_string();
    match dm.get_user_requests_with_progress(&user_id, params.limit, params.offset) {
        Ok(requests) => {
            let limits = dm.check_user_limits(&user_id).unwrap_or_else(|_| {
                crate::download_manager::UserLimitStatus::available(0, 0, 0, 0)
            });
            Json(serde_json::json!({
                "requests": requests,
                "stats": limits
            }))
            .into_response()
        }
        Err(err) => {
            error!("Error getting user requests: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// GET /v1/download/request/:id - Get status of a specific request
async fn get_download_request_status(
    session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Path(request_id): Path<String>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let user_id = session.user_id.to_string();
    match dm.get_request_status(&user_id, &request_id) {
        Ok(Some(item)) => Json(item).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting request status: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// GET /v1/download/limits - Get user's rate limit status
async fn get_download_limits(
    session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let user_id = session.user_id.to_string();
    match dm.check_user_limits(&user_id) {
        Ok(limits) => Json(limits).into_response(),
        Err(err) => {
            error!("Error checking user limits: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ============================================================================
// Download Admin Handlers
// ============================================================================

/// Response for download queue statistics
#[derive(serde::Serialize)]
struct DownloadStatsResponse {
    downloader: DownloaderStatusResponse,
    queue: QueueStatsResponse,
    capacity: CapacityStatsResponse,
}

#[derive(serde::Serialize)]
struct DownloaderStatusResponse {
    online: bool,
    state: Option<String>,
    uptime_secs: Option<u64>,
    last_error: Option<String>,
}

#[derive(serde::Serialize)]
struct QueueStatsResponse {
    pending: i64,
    in_progress: i64,
    retry_waiting: i64,
    completed_today: i64,
    failed_today: i64,
}

#[derive(serde::Serialize)]
struct CapacityStatsResponse {
    albums_this_hour: i32,
    max_per_hour: i32,
    albums_today: i32,
    max_per_day: i32,
}

/// GET /v1/download/admin/stats - Get download queue statistics
async fn admin_get_download_stats(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let queue_stats = match dm.get_queue_stats() {
        Ok(stats) => stats,
        Err(err) => {
            error!("Error getting queue stats: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let capacity = match dm.check_global_capacity() {
        Ok(cap) => cap,
        Err(err) => {
            error!("Error getting capacity: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Get downloader service status
    let downloader_status = dm.get_downloader_status().await;
    let downloader = match downloader_status {
        Some(status) => DownloaderStatusResponse {
            online: status.state == "Healthy",
            state: Some(status.state),
            uptime_secs: Some(status.uptime_secs),
            last_error: status.last_error,
        },
        None => DownloaderStatusResponse {
            online: false,
            state: None,
            uptime_secs: None,
            last_error: Some("Unable to connect to downloader service".to_string()),
        },
    };

    let response = DownloadStatsResponse {
        downloader,
        queue: QueueStatsResponse {
            pending: queue_stats.pending,
            in_progress: queue_stats.in_progress,
            retry_waiting: queue_stats.retry_waiting,
            completed_today: queue_stats.completed_today,
            failed_today: queue_stats.failed_today,
        },
        capacity: CapacityStatsResponse {
            albums_this_hour: capacity.albums_this_hour,
            max_per_hour: capacity.max_per_hour,
            albums_today: capacity.albums_today,
            max_per_day: capacity.max_per_day,
        },
    };

    Json(response).into_response()
}

/// Query parameters for failed items endpoint
#[derive(serde::Deserialize)]
struct FailedItemsQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

/// GET /v1/download/admin/failed - Get failed download items
async fn admin_get_download_failed(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Query(query): Query<FailedItemsQuery>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);

    match dm.get_failed_items(limit, offset) {
        Ok(items) => Json(items).into_response(),
        Err(err) => {
            error!("Error getting failed items: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Query parameters for retry endpoint
#[derive(serde::Deserialize)]
struct RetryQuery {
    force: Option<bool>,
}

/// POST /v1/download/admin/retry/:id - Retry a failed download
async fn admin_retry_download(
    session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Path(request_id): Path<String>,
    Query(query): Query<RetryQuery>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let force = query.force.unwrap_or(false);
    let user_id = session.user_id.to_string();
    match dm.retry_failed(&user_id, &request_id, force) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(err) => {
            error!("Error retrying download {}: {}", request_id, err);
            (StatusCode::BAD_REQUEST, err.to_string()).into_response()
        }
    }
}

/// DELETE /v1/download/admin/request/:id - Delete a download request
async fn admin_delete_download(
    session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Path(request_id): Path<String>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let user_id = session.user_id.to_string();
    match dm.delete_request(&user_id, &request_id) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(err) => {
            error!("Error deleting download {}: {}", request_id, err);
            (StatusCode::BAD_REQUEST, err.to_string()).into_response()
        }
    }
}

/// Query parameters for activity endpoint
#[derive(serde::Deserialize)]
struct ActivityQuery {
    hours: Option<usize>,
}

/// Response for activity endpoint
#[derive(serde::Serialize)]
struct ActivityResponse {
    hourly: Vec<HourlyActivity>,
    totals: ActivityTotals,
}

#[derive(serde::Serialize)]
struct HourlyActivity {
    hour: String,
    albums: i64,
    tracks: i64,
    bytes: i64,
}

#[derive(serde::Serialize)]
struct ActivityTotals {
    albums: i64,
    tracks: i64,
    bytes: i64,
}

/// GET /v1/download/admin/activity - Get download activity
async fn admin_get_download_activity(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Query(query): Query<ActivityQuery>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let hours = query.hours.unwrap_or(24).min(168); // Max 7 days

    match dm.get_activity(hours) {
        Ok(entries) => {
            let mut total_albums = 0i64;
            let mut total_tracks = 0i64;
            let mut total_bytes = 0i64;

            let hourly: Vec<HourlyActivity> = entries
                .iter()
                .map(|e| {
                    total_albums += e.albums_downloaded;
                    total_tracks += e.tracks_downloaded;
                    total_bytes += e.bytes_downloaded;

                    HourlyActivity {
                        hour: chrono::DateTime::from_timestamp(e.hour_bucket, 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_default(),
                        albums: e.albums_downloaded,
                        tracks: e.tracks_downloaded,
                        bytes: e.bytes_downloaded,
                    }
                })
                .collect();

            let response = ActivityResponse {
                hourly,
                totals: ActivityTotals {
                    albums: total_albums,
                    tracks: total_tracks,
                    bytes: total_bytes,
                },
            };

            Json(response).into_response()
        }
        Err(err) => {
            error!("Error getting download activity: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Query parameters for stats history endpoint
#[derive(serde::Deserialize)]
struct StatsHistoryQuery {
    /// Period: "hourly" (48h), "daily" (30d), or "weekly" (12w). Default: daily
    /// Used for aggregation granularity
    period: Option<String>,
    /// Custom start time (unix timestamp). If provided, overrides period default.
    since: Option<i64>,
    /// Custom end time (unix timestamp). If provided, limits results to before this time.
    until: Option<i64>,
}

/// GET /v1/download/admin/stats/history - Get aggregated download statistics over time
///
/// Query params:
/// - `period`: "hourly", "daily", or "weekly" (default: daily) - sets aggregation granularity
/// - `since`: Unix timestamp for custom start time (optional)
/// - `until`: Unix timestamp for custom end time (optional)
async fn admin_get_stats_history(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Query(query): Query<StatsHistoryQuery>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let period = query
        .period
        .as_deref()
        .and_then(crate::download_manager::StatsPeriod::from_str)
        .unwrap_or(crate::download_manager::StatsPeriod::Daily);

    match dm.get_stats_history(period, query.since, query.until) {
        Ok(history) => Json(history).into_response(),
        Err(err) => {
            error!("Error getting stats history: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Query parameters for requests endpoint
#[derive(serde::Deserialize)]
struct RequestsQuery {
    status: Option<String>,
    user_id: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    exclude_completed: Option<bool>,
    top_level_only: Option<bool>,
}

/// Response for requests endpoint
#[derive(serde::Serialize)]
struct DownloadRequestsResponse {
    items: Vec<crate::download_manager::QueueItem>,
}

/// A request item with optional progress info (for top-level requests with children)
#[derive(serde::Serialize)]
struct RequestWithProgress {
    #[serde(flatten)]
    item: crate::download_manager::QueueItem,
    progress: Option<crate::download_manager::DownloadProgress>,
}

/// Response for requests endpoint with progress info
#[derive(serde::Serialize)]
struct RequestsWithProgressResponse {
    items: Vec<RequestWithProgress>,
}

/// GET /v1/download/admin/requests - Get all download requests
async fn admin_get_download_requests(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Query(query): Query<RequestsQuery>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);
    let exclude_completed = query.exclude_completed.unwrap_or(false);
    let top_level_only = query.top_level_only.unwrap_or(false);

    let status = query
        .status
        .as_deref()
        .and_then(|s| match s.to_lowercase().as_str() {
            "pending" => Some(crate::download_manager::QueueStatus::Pending),
            "in_progress" => Some(crate::download_manager::QueueStatus::InProgress),
            "retry_waiting" => Some(crate::download_manager::QueueStatus::RetryWaiting),
            "completed" => Some(crate::download_manager::QueueStatus::Completed),
            "failed" => Some(crate::download_manager::QueueStatus::Failed),
            _ => None,
        });

    match dm.get_all_requests(
        status,
        exclude_completed,
        top_level_only,
        query.user_id.as_deref(),
        limit,
        offset,
    ) {
        Ok(items) => {
            // If top_level_only, enrich with progress info
            if top_level_only {
                let items_with_progress: Vec<_> = items
                    .into_iter()
                    .map(|item| {
                        let progress = dm.get_request_progress(&item.id).ok().flatten();
                        RequestWithProgress { item, progress }
                    })
                    .collect();
                Json(RequestsWithProgressResponse {
                    items: items_with_progress,
                })
                .into_response()
            } else {
                Json(DownloadRequestsResponse { items }).into_response()
            }
        }
        Err(err) => {
            error!("Error getting download requests: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Query parameters for audit log endpoint
#[derive(serde::Deserialize)]
struct AuditLogQuery {
    queue_item_id: Option<String>,
    user_id: Option<String>,
    event_type: Option<String>,
    content_type: Option<String>,
    content_id: Option<String>,
    since: Option<i64>,
    until: Option<i64>,
    limit: Option<usize>,
    offset: Option<usize>,
}

/// Response for audit log endpoint
#[derive(serde::Serialize)]
struct AuditLogResponse {
    entries: Vec<crate::download_manager::AuditLogEntry>,
    total_count: usize,
    has_more: bool,
}

/// GET /v1/download/admin/audit - Get audit log entries
async fn admin_get_audit_log(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Query(query): Query<AuditLogQuery>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let limit = query.limit.unwrap_or(100).min(500);
    let offset = query.offset.unwrap_or(0);

    // Build filter from query params
    let mut filter = crate::download_manager::AuditLogFilter::new().paginate(limit, offset);

    if let Some(queue_item_id) = query.queue_item_id {
        filter = filter.for_queue_item(queue_item_id);
    }

    if let Some(user_id) = query.user_id {
        filter = filter.for_user(user_id);
    }

    if let Some(event_type_str) = query.event_type {
        if let Some(event_type) = crate::download_manager::AuditEventType::from_str(&event_type_str)
        {
            filter = filter.with_event_types(vec![event_type]);
        }
    }

    if let Some(content_type_str) = query.content_type {
        if let Some(content_type) =
            crate::download_manager::DownloadContentType::from_str(&content_type_str)
        {
            filter = filter.for_content_type(content_type);
        }
    }

    if let Some(content_id) = query.content_id {
        filter = filter.for_content_id(content_id);
    }

    // Apply time range filters
    let since = query.since.or(filter.since);
    let until = query.until.or(filter.until);
    if since.is_some() || until.is_some() {
        filter = filter.in_range(since, until);
    }

    match dm.get_audit_log(filter) {
        Ok((entries, total_count)) => {
            let has_more = offset + entries.len() < total_count;
            let response = AuditLogResponse {
                entries,
                total_count,
                has_more,
            };
            Json(response).into_response()
        }
        Err(err) => {
            error!("Error getting audit log: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Response for audit item endpoint
#[derive(serde::Serialize)]
struct AuditItemResponse {
    queue_item: crate::download_manager::QueueItem,
    events: Vec<crate::download_manager::AuditLogEntry>,
}

/// GET /v1/download/admin/audit/item/:id - Get audit history for a specific queue item
async fn admin_get_audit_for_item(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Path(queue_item_id): Path<String>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    match dm.get_audit_for_item(&queue_item_id) {
        Ok(Some((queue_item, events))) => {
            let response = AuditItemResponse { queue_item, events };
            Json(response).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting audit for item: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Query parameters for user audit endpoint
#[derive(serde::Deserialize)]
struct UserAuditQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

/// Response for user audit endpoint
#[derive(serde::Serialize)]
struct UserAuditResponse {
    entries: Vec<crate::download_manager::AuditLogEntry>,
    total_count: usize,
    has_more: bool,
}

/// GET /v1/download/admin/audit/user/:id - Get audit history for a specific user
async fn admin_get_audit_for_user(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
    Path(user_id): Path<String>,
    Query(query): Query<UserAuditQuery>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let limit = query.limit.unwrap_or(100).min(500);
    let offset = query.offset.unwrap_or(0);

    match dm.get_audit_for_user(&user_id, limit, offset) {
        Ok((entries, total_count)) => {
            let has_more = offset + entries.len() < total_count;
            let response = UserAuditResponse {
                entries,
                total_count,
                has_more,
            };
            Json(response).into_response()
        }
        Err(err) => {
            error!("Error getting audit for user: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// =============================================================================
// Download Admin - Throttle & Corruption Handler
// =============================================================================

/// Response for GET /admin/throttle
#[derive(Debug, Serialize)]
struct ThrottleStateResponse {
    enabled: bool,
    max_mb_per_minute: f64,
    max_mb_per_hour: f64,
    current_mb_last_minute: f64,
    current_mb_last_hour: f64,
    is_throttled: bool,
}

/// Get current throttle state
async fn admin_get_throttle_state(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let stats = dm.get_throttle_stats().await;
    let response = ThrottleStateResponse {
        enabled: dm.config().throttle_enabled,
        max_mb_per_minute: stats.max_bytes_per_minute as f64 / (1024.0 * 1024.0),
        max_mb_per_hour: stats.max_bytes_per_hour as f64 / (1024.0 * 1024.0),
        current_mb_last_minute: stats.bytes_last_minute as f64 / (1024.0 * 1024.0),
        current_mb_last_hour: stats.bytes_last_hour as f64 / (1024.0 * 1024.0),
        is_throttled: stats.is_throttled,
    };

    Json(response).into_response()
}

/// Response for GET /admin/corruption-handler
#[derive(Debug, Serialize)]
struct CorruptionHandlerStateResponse {
    current_level: u32,
    successes_since_last_level_change: u32,
    successes_to_deescalate: u32,
    in_cooldown: bool,
    cooldown_remaining_secs: Option<u64>,
    current_cooldown_duration_secs: u64,
    recent_results: Vec<bool>,
    window_size: usize,
    failure_threshold: usize,
}

/// Get current corruption handler state
async fn admin_get_corruption_handler_state(
    _session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    let state = dm.get_corruption_handler_state().await;
    let config = dm.config();

    let response = CorruptionHandlerStateResponse {
        current_level: state.current_level,
        successes_since_last_level_change: state.successes_since_last_level_change,
        successes_to_deescalate: config.corruption_successes_to_deescalate,
        in_cooldown: state.in_cooldown,
        cooldown_remaining_secs: state.cooldown_remaining_secs,
        current_cooldown_duration_secs: state.current_cooldown_duration_secs,
        recent_results: state.recent_results,
        window_size: config.corruption_window_size,
        failure_threshold: config.corruption_failure_threshold,
    };

    Json(response).into_response()
}

/// Reset corruption handler state (admin action)
async fn admin_reset_corruption_handler(
    session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    info!(
        "Admin {:?} resetting corruption handler state",
        session.user_id
    );

    dm.reset_corruption_handler().await;

    // Return updated state
    let state = dm.get_corruption_handler_state().await;
    let config = dm.config();

    let response = CorruptionHandlerStateResponse {
        current_level: state.current_level,
        successes_since_last_level_change: state.successes_since_last_level_change,
        successes_to_deescalate: config.corruption_successes_to_deescalate,
        in_cooldown: state.in_cooldown,
        cooldown_remaining_secs: state.cooldown_remaining_secs,
        current_cooldown_duration_secs: state.current_cooldown_duration_secs,
        recent_results: state.recent_results,
        window_size: config.corruption_window_size,
        failure_threshold: config.corruption_failure_threshold,
    };

    Json(response).into_response()
}

/// Reset throttle state (admin action)
async fn admin_reset_throttle(
    session: Session,
    State(download_manager): State<super::state::OptionalDownloadManager>,
) -> Response {
    let dm = match download_manager {
        Some(dm) => dm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Download manager not enabled",
            )
                .into_response()
        }
    };

    info!("Admin {:?} resetting throttle state", session.user_id);

    dm.reset_throttle().await;

    // Return updated stats
    let stats = dm.get_throttle_stats().await;
    let response = ThrottleStateResponse {
        enabled: dm.config().throttle_enabled,
        max_mb_per_minute: stats.max_bytes_per_minute as f64 / (1024.0 * 1024.0),
        max_mb_per_hour: stats.max_bytes_per_hour as f64 / (1024.0 * 1024.0),
        current_mb_last_minute: stats.bytes_last_minute as f64 / (1024.0 * 1024.0),
        current_mb_last_hour: stats.bytes_last_hour as f64 / (1024.0 * 1024.0),
        is_throttled: stats.is_throttled,
    };

    Json(response).into_response()
}

impl ServerState {
    /// Create a new ServerState with an already-guarded search vault.
    /// This allows sharing the search vault with background tasks.
    #[allow(clippy::arc_with_non_send_sync, clippy::too_many_arguments)]
    fn new_with_guarded_search_vault(
        config: ServerConfig,
        catalog_store: Arc<dyn CatalogStore>,
        search_vault: super::state::GuardedSearchVault,
        user_manager: GuardedUserManager,
        user_store: Arc<dyn FullUserStore>,
        downloader: Option<Arc<dyn crate::downloader::Downloader>>,
        media_base_path: Option<std::path::PathBuf>,
        scheduler_handle: Option<SchedulerHandle>,
        download_manager: Option<Arc<crate::download_manager::DownloadManager>>,
        server_store: Arc<dyn crate::server_store::ServerStore>,
    ) -> ServerState {
        // Create connection manager first since it's needed by proxy and whatsnew notifier
        let ws_connection_manager = Arc::new(super::websocket::ConnectionManager::new());

        // Create proxy if downloader and media_base_path are available
        let proxy = match (&downloader, media_base_path) {
            (Some(dl), Some(path)) => Some(Arc::new(super::proxy::CatalogProxy::new(
                dl.clone(),
                catalog_store.clone(),
                user_store.clone(),
                path,
            ))),
            _ => None,
        };

        // Create WhatsNew notifier
        let whatsnew_notifier = Arc::new(crate::whatsnew::WhatsNewNotifier::new(
            user_store,
            ws_connection_manager.clone(),
        ));

        // Create auth state store for OIDC flow (always created, even if OIDC is disabled)
        let auth_state_store = Arc::new(crate::oidc::AuthStateStore::new());

        ServerState {
            config,
            start_time: Instant::now(),
            catalog_store,
            search_vault,
            user_manager,
            downloader,
            proxy,
            ws_connection_manager,
            scheduler_handle,
            download_manager,
            whatsnew_notifier,
            server_store,
            hash: "123456".to_owned(),
            oidc_client: None, // Will be set by make_app if OIDC is configured
            auth_state_store,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn make_app(
    config: ServerConfig,
    catalog_store: Arc<dyn CatalogStore>,
    search_vault: super::state::GuardedSearchVault,
    user_store: Arc<dyn FullUserStore>,
    user_manager: GuardedUserManager,
    downloader: Option<Arc<dyn crate::downloader::Downloader>>,
    media_base_path: Option<std::path::PathBuf>,
    scheduler_handle: Option<SchedulerHandle>,
    download_manager: Option<Arc<crate::download_manager::DownloadManager>>,
    server_store: Arc<dyn crate::server_store::ServerStore>,
    oidc_config: Option<crate::config::OidcConfig>,
) -> Result<Router> {
    // Initialize OIDC client if configured
    let oidc_client = match oidc_config {
        Some(cfg) => {
            info!(
                "Initializing OIDC client for provider: {}",
                cfg.provider_url
            );
            match crate::oidc::OidcClient::new(cfg).await {
                Ok(client) => {
                    info!("OIDC client initialized successfully");
                    Some(Arc::new(client))
                }
                Err(e) => {
                    error!(
                        "Failed to initialize OIDC client: {:?}. OIDC login will be disabled.",
                        e
                    );
                    None
                }
            }
        }
        None => {
            info!("OIDC not configured, password-based login only");
            None
        }
    };

    let mut state = ServerState::new_with_guarded_search_vault(
        config.clone(),
        catalog_store,
        search_vault,
        user_manager,
        user_store.clone(),
        downloader,
        media_base_path,
        scheduler_handle,
        download_manager.clone(),
        server_store,
    );
    state.oidc_client = oidc_client;

    // Set up sync notifier and notification service for download manager if enabled
    if let Some(ref dm) = download_manager {
        let sync_notifier = Arc::new(crate::download_manager::DownloadSyncNotifier::new(
            user_store.clone(),
            state.ws_connection_manager.clone(),
        ));
        dm.set_sync_notifier(sync_notifier).await;
        tracing::info!("Download sync notifier initialized");

        // Set up notification service for user notifications on download completion
        let notification_service = Arc::new(NotificationService::new(
            user_store,
            state.ws_connection_manager.clone(),
        ));
        dm.set_notification_service(notification_service).await;
        tracing::info!("Notification service initialized for download manager");
    }

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

    // Password-based login (legacy, will be removed after OIDC migration)
    let password_login_routes: Router = Router::new()
        .route("/login", post(login))
        .layer(GovernorLayer::new(login_rate_limit.clone()))
        .with_state(state.clone());

    // OIDC login routes (also rate-limited)
    let oidc_login_routes: Router = Router::new()
        .route("/oidc/login", get(oidc_login))
        .route("/oidc/callback", get(oidc_callback))
        .layer(GovernorLayer::new(login_rate_limit))
        .with_state(state.clone());

    // Other auth routes without rate limiting (already authenticated)
    let other_auth_routes: Router = Router::new()
        .route("/logout", get(logout))
        .route("/session", get(get_session))
        .route("/challenge", get(get_challenge))
        .route("/challenge", post(post_challenge))
        .with_state(state.clone());

    let auth_routes = password_login_routes
        .merge(oidc_login_routes)
        .merge(other_auth_routes);

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
        .layer(GovernorLayer::new(content_read_rate_limit.clone()))
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

    // Notifications routes (requires AccessCatalog permission)
    let notifications_routes: Router = Router::new()
        .route("/notifications/{id}/read", post(mark_notification_read))
        .layer(GovernorLayer::new(write_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .with_state(state.clone());

    let user_routes = liked_content_routes
        .merge(playlist_routes)
        .merge(listening_stats_routes)
        .merge(settings_routes)
        .merge(notifications_routes);

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
        .route("/jobs/audit", get(admin_get_job_audit_log))
        .route("/jobs/{job_id}", get(admin_get_job))
        .route("/jobs/{job_id}/trigger", post(admin_trigger_job))
        .route("/jobs/{job_id}/history", get(admin_get_job_history))
        .route("/jobs/{job_id}/audit", get(admin_get_job_audit_log_by_job))
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

    // Search admin routes (requires ServerAdmin permission)
    let admin_search_routes: Router = if let Some(routes) = make_search_admin_routes(state.clone())
    {
        routes.route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_server_admin,
        ))
    } else {
        Router::new()
    };

    let admin_routes = admin_server_routes
        .merge(admin_user_routes)
        .merge(admin_listening_routes)
        .merge(admin_changelog_routes)
        .merge(admin_search_routes);

    // Download manager user routes (requires RequestContent permission)
    let download_user_routes: Router = Router::new()
        .route("/search", get(search_download_content))
        .route(
            "/search/discography/{artist_id}",
            get(search_download_discography),
        )
        .route("/album/{album_id}", get(get_external_album_details))
        .route("/request/album", post(request_album_download))
        .route("/request/discography", post(request_discography_download))
        .route("/my-requests", get(get_my_download_requests))
        .route("/request/{id}", get(get_download_request_status))
        .route("/limits", get(get_download_limits))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_request_content,
        ))
        .with_state(state.clone());

    // Download manager admin read routes (requires DownloadManagerAdmin permission)
    let download_admin_read_routes: Router = Router::new()
        .route("/admin/stats", get(admin_get_download_stats))
        .route("/admin/stats/history", get(admin_get_stats_history))
        .route("/admin/failed", get(admin_get_download_failed))
        .route("/admin/activity", get(admin_get_download_activity))
        .route("/admin/requests", get(admin_get_download_requests))
        .route("/admin/audit", get(admin_get_audit_log))
        .route("/admin/audit/item/{id}", get(admin_get_audit_for_item))
        .route("/admin/audit/user/{id}", get(admin_get_audit_for_user))
        .route("/admin/throttle", get(admin_get_throttle_state))
        .route(
            "/admin/corruption-handler",
            get(admin_get_corruption_handler_state),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_download_manager_admin,
        ))
        .with_state(state.clone());

    // Download manager admin write routes (requires DownloadManagerAdmin permission)
    let download_admin_write_routes: Router = Router::new()
        .route("/admin/retry/{id}", post(admin_retry_download))
        .route("/admin/request/{id}", delete(admin_delete_download))
        .route("/admin/throttle/reset", post(admin_reset_throttle))
        .route(
            "/admin/corruption-handler/reset",
            post(admin_reset_corruption_handler),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_download_manager_admin,
        ))
        .with_state(state.clone());

    // Combine all download routes
    let download_routes: Router = download_user_routes
        .merge(download_admin_read_routes)
        .merge(download_admin_write_routes);

    // Skeleton sync routes (requires AccessCatalog permission)
    let skeleton_routes: Router = Router::new()
        .route("/", get(super::skeleton::get_full_skeleton))
        .route("/version", get(super::skeleton::get_skeleton_version))
        .route("/delta", get(super::skeleton::get_skeleton_delta))
        .layer(GovernorLayer::new(content_read_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .with_state(state.clone());

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
        .nest("/v1/download", download_routes)
        .nest("/v1/catalog/skeleton", skeleton_routes)
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

    // Extract user ID from session for rate limiting (must run before rate limiters)
    app = app.layer(middleware::from_fn_with_state(
        state.clone(),
        extract_user_id_for_rate_limit,
    ));

    app = app.layer(middleware::from_fn_with_state(state.clone(), log_requests));

    Ok(app)
}

/// Interval between stale batch checks (10 minutes in seconds)
/// The actual staleness threshold is configured in ChangeLogStore (default 1 hour).
const STALE_BATCH_CHECK_INTERVAL_SECS: u64 = 600;

#[allow(clippy::too_many_arguments)]
pub async fn run_server(
    catalog_store: Arc<dyn CatalogStore>,
    guarded_search_vault: super::state::GuardedSearchVault,
    user_store: Arc<dyn FullUserStore>,
    user_manager: GuardedUserManager,
    requests_logging_level: RequestsLoggingLevel,
    port: u16,
    metrics_port: u16,
    content_cache_age_sec: usize,
    frontend_dir_path: Option<String>,
    downloader: Option<Arc<dyn crate::downloader::Downloader>>,
    media_base_path: Option<std::path::PathBuf>,
    scheduler_handle: Option<SchedulerHandle>,
    download_manager: Option<Arc<crate::download_manager::DownloadManager>>,
    server_store: Arc<dyn crate::server_store::ServerStore>,
    oidc_config: Option<crate::config::OidcConfig>,
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
        guarded_search_vault.clone(),
        user_store,
        user_manager,
        downloader,
        media_base_path,
        scheduler_handle,
        download_manager,
        server_store,
        oidc_config,
    )
    .await?;

    // Create a minimal metrics-only server (always HTTP, internal use)
    let metrics_app = Router::new().route("/metrics", get(super::metrics::metrics_handler));
    let metrics_listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", metrics_port))
        .await
        .unwrap();

    // Spawn the stale batch auto-close background task
    let catalog_store_for_bg = catalog_store.clone();
    let search_vault_for_bg = guarded_search_vault.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            STALE_BATCH_CHECK_INTERVAL_SECS,
        ));
        loop {
            interval.tick().await;
            check_and_close_stale_batches(&catalog_store_for_bg, &search_vault_for_bg);
        }
    });

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

    Ok(())
}

/// Close stale changelog batches automatically and rebuild search index if any were closed.
fn check_and_close_stale_batches(
    catalog_store: &Arc<dyn CatalogStore>,
    search_vault: &super::state::GuardedSearchVault,
) {
    super::metrics::CHANGELOG_STALE_BATCH_CHECKS_TOTAL.inc();

    match catalog_store.close_stale_batches() {
        Ok(closed_count) => {
            if closed_count > 0 {
                info!(
                    "Background task closed {} stale changelog batch(es)",
                    closed_count
                );

                // Rebuild search index after closing stale batches
                if let Err(err) = search_vault.lock().unwrap().rebuild_index() {
                    error!(
                        "Failed to rebuild search index after closing stale batches: {}",
                        err
                    );
                }
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
    use crate::server_store::{
        JobAuditEntry, JobAuditEventType, JobRun, JobRunStatus, JobScheduleState, ServerStore,
    };
    use crate::user::auth::UserAuthCredentials;
    use crate::user::auth::{AuthToken, AuthTokenValue};
    use crate::user::user_models::{BandwidthSummary, BandwidthUsage, LikedContentType};
    use crate::user::{
        UserAuthCredentialsStore, UserAuthTokenStore, UserBandwidthStore, UserStore,
    };
    use axum::extract::ConnectInfo;
    use axum::{body::Body, http::Request};
    use std::collections::HashMap;
    use std::sync::RwLock;
    use tower::ServiceExt; // for `call`, `oneshot`, and `ready

    /// A minimal in-memory ServerStore for testing
    #[derive(Default)]
    struct MockServerStore {
        state: RwLock<HashMap<String, String>>,
    }

    impl ServerStore for MockServerStore {
        fn record_job_start(&self, _job_id: &str, _triggered_by: &str) -> anyhow::Result<i64> {
            Ok(1)
        }
        fn record_job_finish(
            &self,
            _run_id: i64,
            _status: JobRunStatus,
            _error_message: Option<String>,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        fn get_running_jobs(&self) -> anyhow::Result<Vec<JobRun>> {
            Ok(vec![])
        }
        fn get_job_history(&self, _job_id: &str, _limit: usize) -> anyhow::Result<Vec<JobRun>> {
            Ok(vec![])
        }
        fn get_last_run(&self, _job_id: &str) -> anyhow::Result<Option<JobRun>> {
            Ok(None)
        }
        fn mark_stale_jobs_failed(&self) -> anyhow::Result<usize> {
            Ok(0)
        }
        fn get_schedule_state(&self, _job_id: &str) -> anyhow::Result<Option<JobScheduleState>> {
            Ok(None)
        }
        fn update_schedule_state(&self, _state: &JobScheduleState) -> anyhow::Result<()> {
            Ok(())
        }
        fn get_all_schedule_states(&self) -> anyhow::Result<Vec<JobScheduleState>> {
            Ok(vec![])
        }
        fn get_state(&self, key: &str) -> anyhow::Result<Option<String>> {
            Ok(self.state.read().unwrap().get(key).cloned())
        }
        fn set_state(&self, key: &str, value: &str) -> anyhow::Result<()> {
            self.state
                .write()
                .unwrap()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }
        fn delete_state(&self, key: &str) -> anyhow::Result<()> {
            self.state.write().unwrap().remove(key);
            Ok(())
        }
        fn log_job_audit(
            &self,
            _job_id: &str,
            _event_type: JobAuditEventType,
            _duration_ms: Option<i64>,
            _details: Option<&serde_json::Value>,
            _error: Option<&str>,
        ) -> anyhow::Result<i64> {
            Ok(1)
        }
        fn get_job_audit_log(
            &self,
            _limit: usize,
            _offset: usize,
        ) -> anyhow::Result<Vec<JobAuditEntry>> {
            Ok(vec![])
        }
        fn get_job_audit_log_by_job(
            &self,
            _job_id: &str,
            _limit: usize,
            _offset: usize,
        ) -> anyhow::Result<Vec<JobAuditEntry>> {
            Ok(vec![])
        }
        fn cleanup_old_job_audit_entries(&self, _before_timestamp: i64) -> anyhow::Result<usize> {
            Ok(0)
        }
    }

    #[tokio::test]
    async fn responds_forbidden_on_protected_routes() {
        let user_store: Arc<dyn FullUserStore> = Arc::new(InMemoryUserStore::default());
        let catalog_store: Arc<dyn CatalogStore> = Arc::new(NullCatalogStore);
        let user_manager = Arc::new(std::sync::Mutex::new(crate::user::UserManager::new(
            catalog_store.clone(),
            user_store.clone(),
        )));
        let guarded_search_vault: crate::server::state::GuardedSearchVault =
            Arc::new(std::sync::Mutex::new(Box::new(NoOpSearchVault {})));
        let server_store: Arc<dyn ServerStore> = Arc::new(MockServerStore::default());
        let app = &mut make_app(
            ServerConfig::default(),
            catalog_store,
            guarded_search_vault,
            user_store,
            user_manager,
            None, // no downloader
            None, // no media_base_path
            None, // no scheduler_handle
            None, // no download_manager
            server_store,
            None, // no oidc_config
        )
        .await
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

        fn get_user_id_by_oidc_subject(&self, _oidc_subject: &str) -> Result<Option<usize>> {
            Ok(None)
        }

        fn set_user_oidc_subject(&self, _user_id: usize, _oidc_subject: &str) -> Result<()> {
            Ok(())
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

        fn get_all_track_play_counts(
            &self,
            _start_date: u32,
            _end_date: u32,
        ) -> Result<Vec<crate::user::user_models::TrackPlayCount>> {
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

        fn get_user_ids_with_setting(&self, _key: &str, _value: &str) -> Result<Vec<usize>> {
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

    impl crate::notifications::NotificationStore for InMemoryUserStore {
        fn create_notification(
            &self,
            _user_id: usize,
            notification_type: crate::notifications::NotificationType,
            title: String,
            body: Option<String>,
            data: serde_json::Value,
        ) -> Result<crate::notifications::Notification> {
            Ok(crate::notifications::Notification {
                id: "test-notif-1".to_string(),
                notification_type,
                title,
                body,
                data,
                read_at: None,
                created_at: 0,
            })
        }

        fn get_user_notifications(
            &self,
            _user_id: usize,
        ) -> Result<Vec<crate::notifications::Notification>> {
            Ok(vec![])
        }

        fn get_notification(
            &self,
            _notification_id: &str,
            _user_id: usize,
        ) -> Result<Option<crate::notifications::Notification>> {
            Ok(None)
        }

        fn mark_notification_read(
            &self,
            _notification_id: &str,
            _user_id: usize,
        ) -> Result<Option<crate::notifications::Notification>> {
            Ok(None)
        }

        fn get_unread_count(&self, _user_id: usize) -> Result<usize> {
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
            // Admin should have: AccessCatalog, EditCatalog, ManagePermissions, ServerAdmin, ViewAnalytics, RequestContent, DownloadManagerAdmin
            assert!(permissions.contains(&crate::user::Permission::AccessCatalog));
            assert!(permissions.contains(&crate::user::Permission::EditCatalog));
            assert!(permissions.contains(&crate::user::Permission::ManagePermissions));
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
