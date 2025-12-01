//! HTTP server implementation with route handlers
//! Note: Many functions appear unused but are registered as route handlers

#![allow(dead_code)] // Route handlers registered dynamically

use anyhow::Result;
use std::{
    fs::File,
    io::Read,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tracing::{debug, error, info, warn};

use crate::catalog_store::CatalogStore;
use crate::{
    server::stream_track::stream_track,
    user::{user_models::LikedContentType, FullUserStore, Permission},
};
use crate::{search::SearchVault, user::UserManager};
use axum_extra::extract::cookie::{Cookie, SameSite};
use tower_http::services::ServeDir;

use axum::{
    body::Body,
    extract::{ConnectInfo, Path, Query, State},
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
    extract_user_id_for_rate_limit, http_cache, log_requests, make_search_routes, state::*,
    IpKeyExtractor, RequestsLoggingLevel, ServerConfig, UserOrIpKeyExtractor,
    CONTENT_READ_PER_MINUTE, GLOBAL_PER_MINUTE, LOGIN_PER_MINUTE, SEARCH_PER_MINUTE,
    STREAM_PER_MINUTE, WRITE_PER_MINUTE,
};
use tower_governor::governor::GovernorConfigBuilder;
use crate::server::session::Session;
use crate::user::auth::AuthTokenValue;
use axum::extract::Request;
use axum::middleware::Next;
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
        debug!("require_access_catalog: FORBIDDEN - user_id={} lacks AccessCatalog permission", session.user_id);
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!("require_access_catalog: ALLOWED - user_id={}", session.user_id);
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
        debug!("require_like_content: FORBIDDEN - user_id={} lacks LikeContent permission", session.user_id);
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!("require_like_content: ALLOWED - user_id={}", session.user_id);
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
        debug!("require_own_playlists: FORBIDDEN - user_id={} lacks OwnPlaylists permission", session.user_id);
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!("require_own_playlists: ALLOWED - user_id={}", session.user_id);
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
        debug!("require_edit_catalog: FORBIDDEN - user_id={} lacks EditCatalog permission", session.user_id);
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!("require_edit_catalog: ALLOWED - user_id={}", session.user_id);
    next.run(request).await
}

async fn require_reboot_server(
    session: Session,
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    debug!(
        "require_reboot_server: user_id={}, has_permission={}, permissions={:?}",
        session.user_id,
        session.has_permission(Permission::RebootServer),
        session.permissions
    );
    if !session.has_permission(Permission::RebootServer) {
        debug!("require_reboot_server: FORBIDDEN - user_id={} lacks RebootServer permission", session.user_id);
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!("require_reboot_server: ALLOWED - user_id={}", session.user_id);
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
        debug!("require_manage_permissions: FORBIDDEN - user_id={} lacks ManagePermissions permission", session.user_id);
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!("require_manage_permissions: ALLOWED - user_id={}", session.user_id);
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
        debug!("require_view_analytics: FORBIDDEN - user_id={} lacks ViewAnalytics permission", session.user_id);
        return StatusCode::FORBIDDEN.into_response();
    }
    debug!("require_view_analytics: ALLOWED - user_id={}", session.user_id);
    next.run(request).await
}

#[derive(Deserialize, Debug)]
struct LoginBody {
    pub user_handle: String,
    pub password: String,
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
}

#[derive(Deserialize, Debug)]
struct AddTracksToPlaylistBody {
    pub tracks_ids: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct RemoveTracksFromPlaylist {
    pub tracks_positions: Vec<usize>,
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
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(proxy): State<super::state::OptionalProxy>,
    Path(id): Path<String>,
) -> Response {
    // If proxy is available, ensure artist has complete data
    if let Some(ref proxy) = proxy {
        if let Err(e) = proxy.ensure_artist_complete(&id).await {
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
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(proxy): State<super::state::OptionalProxy>,
    Path(id): Path<String>,
) -> Response {
    // If proxy is available, ensure album has complete data
    if let Some(ref proxy) = proxy {
        if let Err(e) = proxy.ensure_album_complete(&id).await {
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
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(proxy): State<super::state::OptionalProxy>,
    Path(id): Path<String>,
) -> Response {
    // If proxy is available, ensure album has complete data
    if let Some(ref proxy) = proxy {
        if let Err(e) = proxy.ensure_album_complete(&id).await {
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
    _session: Session,
    State(catalog_store): State<GuardedCatalogStore>,
    State(proxy): State<super::state::OptionalProxy>,
    Path(id): Path<String>,
) -> Response {
    // If proxy is available, ensure artist has complete data
    if let Some(ref proxy) = proxy {
        if let Err(e) = proxy.ensure_artist_complete(&id).await {
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

fn update_user_liked_content(
    user_manager: GuardedUserManager,
    user_id: usize,
    content_type: LikedContentType,
    content_id: &str,
    liked: bool,
) -> Response {
    match user_manager
        .lock()
        .unwrap()
        .set_user_liked_content(user_id, content_id, content_type, liked)
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
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
    Path((content_type_str, content_id)): Path<(String, String)>,
) -> Response {
    let Some(content_type) = parse_content_type(&content_type_str) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    update_user_liked_content(user_manager, session.user_id, content_type, &content_id, true)
}

async fn delete_user_liked_content(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path((content_type_str, content_id)): Path<(String, String)>,
) -> Response {
    let Some(content_type) = parse_content_type(&content_type_str) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    update_user_liked_content(user_manager, session.user_id, content_type, &content_id, false)
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
    Json(body): Json<CreatePlaylistBody>,
) -> Response {
    match user_manager.lock().unwrap().create_user_playlist(
        session.user_id,
        &body.name,
        session.user_id,
        body.track_ids.clone(),
    ) {
        Ok(id) => Json(id).into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn put_playlist(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(id): Path<String>,
    Json(body): Json<UpdatePlaylistBody>,
) -> Response {
    debug!("Updating playlist with id {}", id);
    match user_manager.lock().unwrap().update_user_playlist(
        &id,
        session.user_id,
        body.name,
        body.track_ids,
    ) {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => {
            debug!("Error updating playlist: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn delete_playlist(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(id): Path<String>,
) -> Response {
    match user_manager
        .lock()
        .unwrap()
        .delete_user_playlist(&id, session.user_id)
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
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
    Path(id): Path<String>,
    Json(body): Json<AddTracksToPlaylistBody>,
) -> Response {
    match user_manager
        .lock()
        .unwrap()
        .add_playlist_tracks(&id, session.user_id, body.tracks_ids)
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn remove_tracks_from_playlist(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(id): Path<String>,
    Json(body): Json<RemoveTracksFromPlaylist>,
) -> Response {
    match user_manager.lock().unwrap().remove_tracks_from_playlist(
        &id,
        session.user_id,
        body.tracks_positions,
    ) {
        Ok(_) => StatusCode::OK.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
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
        let datetime = chrono::DateTime::from_timestamp(started_at as i64, 0)
            .unwrap_or_else(|| chrono::Utc::now());
        datetime.format("%Y%m%d").to_string().parse::<u32>().unwrap_or(0)
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

    match user_manager
        .lock()
        .unwrap()
        .get_user_listening_summary(session.user_id, start_date, end_date)
    {
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
        let datetime = chrono::DateTime::from_timestamp(now_secs as i64, 0)
            .unwrap_or_else(|| chrono::Utc::now());
        datetime.format("%Y%m%d").to_string().parse::<u32>().unwrap_or(0)
    });

    let start = start_date.unwrap_or_else(|| {
        let thirty_days_ago = now_secs - (30 * 24 * 60 * 60);
        let datetime = chrono::DateTime::from_timestamp(thirty_days_ago as i64, 0)
            .unwrap_or_else(|| chrono::Utc::now());
        datetime.format("%Y%m%d").to_string().parse::<u32>().unwrap_or(0)
    });

    (start, end)
}

async fn login(
    State(user_manager): State<GuardedUserManager>,
    Json(body): Json<LoginBody>,
) -> Response {
    let start = Instant::now();
    debug!("login() called with {:?}", body);
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
                return match locked_manager.generate_auth_token(&credentials) {
                    Ok(auth_token) => {
                        super::metrics::record_login_attempt("success", start.elapsed());
                        let response_body = LoginSuccessResponse {
                            token: auth_token.value.0.clone(),
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

async fn admin_get_users(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
) -> Response {
    match user_manager.lock().unwrap().get_all_user_handles() {
        Ok(handles) => {
            let mut users: Vec<UserInfo> = vec![];
            let manager = user_manager.lock().unwrap();
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
            let role_strings: Vec<String> = roles.iter().map(|r| r.to_string().to_owned()).collect();
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
    Path(user_handle): Path<String>,
    Json(body): Json<AddRoleBody>,
) -> Response {
    let role = match crate::user::UserRole::from_str(&body.role) {
        Some(r) => r,
        None => return (StatusCode::BAD_REQUEST, "Invalid role").into_response(),
    };

    let manager = user_manager.lock().unwrap();
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match manager.add_user_role(user_id, role) {
        Ok(()) => StatusCode::CREATED.into_response(),
        Err(err) => {
            error!("Error adding user role: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_remove_user_role(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path((user_handle, role_name)): Path<(String, String)>,
) -> Response {
    let role = match crate::user::UserRole::from_str(&role_name) {
        Some(r) => r,
        None => return (StatusCode::BAD_REQUEST, "Invalid role").into_response(),
    };

    let manager = user_manager.lock().unwrap();
    let user_id = match manager.get_user_id(&user_handle) {
        Ok(Some(id)) => id,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!("Error getting user id: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match manager.remove_user_role(user_id, role) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(err) => {
            error!("Error removing user role: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
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
            let perm_strings: Vec<String> = permissions.iter().map(|p| format!("{:?}", p)).collect();
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
    Path(user_handle): Path<String>,
    Json(body): Json<AddExtraPermissionBody>,
) -> Response {
    use crate::user::{Permission, PermissionGrant};
    use std::time::{Duration, SystemTime};

    let permission = match body.permission.as_str() {
        "AccessCatalog" => Permission::AccessCatalog,
        "LikeContent" => Permission::LikeContent,
        "OwnPlaylists" => Permission::OwnPlaylists,
        "EditCatalog" => Permission::EditCatalog,
        "ManagePermissions" => Permission::ManagePermissions,
        "IssueContentDownload" => Permission::IssueContentDownload,
        "RebootServer" => Permission::RebootServer,
        _ => return (StatusCode::BAD_REQUEST, "Invalid permission").into_response(),
    };

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
    let end_time = body.duration_seconds.map(|secs| start_time + Duration::from_secs(secs));

    let grant = PermissionGrant::Extra {
        start_time,
        end_time,
        permission,
        countdown: body.countdown,
    };

    match manager.add_user_extra_permission(user_id, grant) {
        Ok(permission_id) => {
            (StatusCode::CREATED, Json(AddExtraPermissionResponse { permission_id })).into_response()
        }
        Err(err) => {
            error!("Error adding extra permission: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn admin_remove_extra_permission(
    _session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(permission_id): Path<usize>,
) -> Response {
    match user_manager
        .lock()
        .unwrap()
        .remove_user_extra_permission(permission_id)
    {
        Ok(()) => StatusCode::OK.into_response(),
        Err(err) => {
            error!("Error removing extra permission: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
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
    fn new(
        config: ServerConfig,
        catalog_store: Arc<dyn CatalogStore>,
        search_vault: Box<dyn SearchVault>,
        user_manager: UserManager,
        downloader: Option<Arc<dyn crate::downloader::Downloader>>,
        media_base_path: Option<std::path::PathBuf>,
    ) -> ServerState {
        // Create proxy if downloader and media_base_path are available
        let proxy = match (&downloader, media_base_path) {
            (Some(dl), Some(path)) => Some(Arc::new(super::proxy::CatalogProxy::new(
                dl.clone(),
                catalog_store.clone(),
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
            hash: "123456".to_owned(),
        }
    }
}

pub fn make_app(
    config: ServerConfig,
    catalog_store: Arc<dyn CatalogStore>,
    search_vault: Box<dyn SearchVault>,
    user_store: Box<dyn FullUserStore>,
    downloader: Option<Arc<dyn crate::downloader::Downloader>>,
    media_base_path: Option<std::path::PathBuf>,
) -> Result<Router> {
    let user_manager = UserManager::new(catalog_store.clone(), user_store);
    let state = ServerState::new(config.clone(), catalog_store, search_vault, user_manager, downloader, media_base_path);

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
        .route("/liked/{content_type}/{content_id}", post(add_user_liked_content))
        .route("/liked/{content_type}/{content_id}", delete(delete_user_liked_content))
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
        .layer(GovernorLayer::new(user_content_read_rate_limit))
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

    let user_routes = liked_content_routes
        .merge(playlist_routes)
        .merge(listening_stats_routes);

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

    // Admin reboot route (requires RebootServer permission)
    let admin_reboot_routes: Router = Router::new()
        .route("/reboot", post(reboot_server))
        .layer(GovernorLayer::new(write_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_reboot_server,
        ))
        .with_state(state.clone());

    // Admin user management routes (requires ManagePermissions permission)
    let admin_user_routes: Router = Router::new()
        .route("/users", get(admin_get_users))
        .route("/users/{user_handle}/roles", get(admin_get_user_roles))
        .route("/users/{user_handle}/roles", post(admin_add_user_role))
        .route("/users/{user_handle}/roles/{role}", delete(admin_remove_user_role))
        .route("/users/{user_handle}/permissions", get(admin_get_user_permissions))
        .route("/users/{user_handle}/permissions", post(admin_add_user_extra_permission))
        .route("/permissions/{permission_id}", delete(admin_remove_extra_permission))
        // Bandwidth statistics routes
        .route("/bandwidth/summary", get(admin_get_bandwidth_summary))
        .route("/bandwidth/usage", get(admin_get_bandwidth_usage))
        .route("/bandwidth/users/{user_handle}/summary", get(admin_get_user_bandwidth_summary))
        .route("/bandwidth/users/{user_handle}/usage", get(admin_get_user_bandwidth_usage))
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
        .route("/listening/track/{track_id}", get(admin_get_track_listening_stats))
        .route("/listening/users/{user_handle}/summary", get(admin_get_user_listening_summary))
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
        .route("/changelog/batch/{batch_id}", get(admin_get_changelog_batch))
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

    let admin_routes = admin_reboot_routes
        .merge(admin_user_routes)
        .merge(admin_listening_routes)
        .merge(admin_changelog_routes);

    let home_router: Router = match config.frontend_dir_path {
        Some(frontend_path) => {
            let static_files_service =
                ServeDir::new(frontend_path).append_index_html_on_directories(true);
            Router::new().fallback_service(static_files_service)
        }
        None => Router::new()
            .route("/", get(home))
            .with_state(state.clone()),
    };

    let mut app: Router = home_router
        .nest("/v1/auth", auth_routes)
        .nest("/v1/content", content_routes)
        .nest("/v1/user", user_routes)
        .nest("/v1/admin", admin_routes);

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

pub async fn run_server(
    catalog_store: Arc<dyn CatalogStore>,
    search_vault: Box<dyn SearchVault>,
    user_store: Box<dyn FullUserStore>,
    requests_logging_level: RequestsLoggingLevel,
    port: u16,
    metrics_port: u16,
    content_cache_age_sec: usize,
    frontend_dir_path: Option<String>,
    downloader: Option<Arc<dyn crate::downloader::Downloader>>,
    media_base_path: Option<std::path::PathBuf>,
) -> Result<()> {
    let config = ServerConfig {
        port,
        requests_logging_level,
        content_cache_age_sec,
        frontend_dir_path,
    };
    let app = make_app(config, catalog_store.clone(), search_vault, user_store, downloader, media_base_path)?;

    let main_listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    // Create a minimal metrics-only server
    let metrics_app = Router::new().route("/metrics", get(super::metrics::metrics_handler));
    let metrics_listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", metrics_port))
        .await
        .unwrap();

    // Spawn the stale batch auto-close background task
    let catalog_store_for_bg = catalog_store.clone();
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(STALE_BATCH_CHECK_INTERVAL_SECS));
        loop {
            interval.tick().await;
            check_and_close_stale_batches(&catalog_store_for_bg);
        }
    });

    // Run both servers concurrently
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
    use crate::user::{UserAuthCredentialsStore, UserAuthTokenStore, UserBandwidthStore, UserStore};
    use axum::{body::Body, http::Request};
    use tower::ServiceExt; // for `call`, `oneshot`, and `ready

    #[tokio::test]
    async fn responds_forbidden_on_protected_routes() {
        let user_store = Box::new(InMemoryUserStore::default());
        let catalog_store: Arc<dyn CatalogStore> = Arc::new(NullCatalogStore);
        let app = &mut make_app(
            ServerConfig::default(),
            catalog_store,
            Box::new(NoOpSearchVault {}),
            user_store,
            None, // no downloader
            None, // no media_base_path
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

        fn get_user_handle(&self, _user_id: usize) -> Result<Option<String>> {
            todo!()
        }

        fn get_user_id(&self, _user_handle: &str) -> Result<Option<usize>> {
            todo!()
        }

        fn get_user_playlists(&self, _user_id: usize) -> Result<Vec<String>> {
            todo!()
        }

        fn is_user_liked_content(&self, _user_id: usize, _content_id: &str) -> Result<Option<bool>> {
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

        fn remove_user_extra_permission(&self, _permission_id: usize) -> Result<()> {
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
        fn get_user_auth_credentials(&self, _user_handle: &str) -> Result<Option<UserAuthCredentials>> {
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
            store.add_user_role(user_id, crate::user::UserRole::Admin).unwrap();
            (store, user_id, temp_dir)
        }

        #[allow(dead_code)]
        fn create_test_store_with_regular_user() -> (SqliteUserStore, usize, TempDir) {
            let (store, temp_dir) = create_test_store();
            let user_id = store.create_user("regular_user").unwrap();
            store.add_user_role(user_id, crate::user::UserRole::Regular).unwrap();
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
            store.add_user_role(user_id, crate::user::UserRole::Admin).unwrap();
            let roles = store.get_user_roles(user_id).unwrap();
            assert!(roles.contains(&crate::user::UserRole::Admin));

            // Add Regular role
            store.add_user_role(user_id, crate::user::UserRole::Regular).unwrap();
            let roles = store.get_user_roles(user_id).unwrap();
            assert_eq!(roles.len(), 2);
            assert!(roles.contains(&crate::user::UserRole::Admin));
            assert!(roles.contains(&crate::user::UserRole::Regular));

            // Remove Admin role
            store.remove_user_role(user_id, crate::user::UserRole::Admin).unwrap();
            let roles = store.get_user_roles(user_id).unwrap();
            assert_eq!(roles.len(), 1);
            assert!(roles.contains(&crate::user::UserRole::Regular));
        }

        #[test]
        fn test_add_duplicate_role_is_idempotent() {
            let (store, _temp_dir) = create_test_store();
            let user_id = store.create_user("testuser").unwrap();

            store.add_user_role(user_id, crate::user::UserRole::Admin).unwrap();
            store.add_user_role(user_id, crate::user::UserRole::Admin).unwrap();

            let roles = store.get_user_roles(user_id).unwrap();
            // Should still only have one Admin role
            assert_eq!(roles.iter().filter(|r| **r == crate::user::UserRole::Admin).count(), 1);
        }

        #[test]
        fn test_resolve_user_permissions_from_role() {
            let (store, user_id, _temp_dir) = create_test_store_with_admin_user();

            let permissions = store.resolve_user_permissions(user_id).unwrap();
            // Admin should have: AccessCatalog, EditCatalog, ManagePermissions, IssueContentDownload, RebootServer
            assert!(permissions.contains(&crate::user::Permission::AccessCatalog));
            assert!(permissions.contains(&crate::user::Permission::EditCatalog));
            assert!(permissions.contains(&crate::user::Permission::ManagePermissions));
            assert!(permissions.contains(&crate::user::Permission::IssueContentDownload));
            assert!(permissions.contains(&crate::user::Permission::RebootServer));
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
                permission: crate::user::Permission::RebootServer,
                countdown: None,
            };

            let permission_id = store.add_user_extra_permission(user_id, grant).unwrap();
            assert!(permission_id > 0);

            // Verify permission is resolved (still within time limit)
            let permissions = store.resolve_user_permissions(user_id).unwrap();
            assert!(permissions.contains(&crate::user::Permission::RebootServer));
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
            let user_manager = crate::user::UserManager::new(catalog_store, Box::new(store));

            let found_id = user_manager.get_user_id("testuser").unwrap();
            assert_eq!(found_id, Some(user_id));

            let not_found = user_manager.get_user_id("nonexistent").unwrap();
            assert_eq!(not_found, None);
        }

    }
}
