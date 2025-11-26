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

use tracing::{debug, error, info};

use crate::{
    catalog::Catalog,
    server::stream_track::stream_track,
    user::{user_models::LikedContentType, Permission, UserStore},
};
use crate::{search::SearchVault, user::UserManager};
use axum_extra::extract::cookie::{Cookie, SameSite};
use tower_http::services::ServeDir;

use axum::{
    body::Body,
    extract::{ConnectInfo, Path, State},
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
    State(catalog): State<GuardedCatalog>,
    Path(id): Path<String>,
) -> Response {
    match catalog.lock().unwrap().get_artist(&id) {
        Some(artist) => Json(artist).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_album(
    _session: Session,
    State(catalog): State<GuardedCatalog>,
    Path(id): Path<String>,
) -> Response {
    match catalog.lock().unwrap().get_album(&id) {
        Some(album) => Json(album).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_resolved_album(
    _session: Session,
    State(catalog): State<GuardedCatalog>,
    Path(id): Path<String>,
) -> Response {
    match catalog.lock().unwrap().get_resolved_album(&id) {
        Ok(Some(album)) => Json(album).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response(),
    }
}

async fn get_artist_discography(
    _session: Session,
    State(catalog): State<GuardedCatalog>,
    Path(id): Path<String>,
) -> Response {
    match catalog.lock().unwrap().get_artist_discography(id) {
        None => StatusCode::NOT_FOUND.into_response(),
        Some(albums_ids) => Json(albums_ids).into_response(),
    }
}

pub async fn get_track(
    _session: Session,
    State(catalog): State<GuardedCatalog>,
    Path(id): Path<String>,
) -> Response {
    match catalog.lock().unwrap().get_track(&id) {
        Some(track) => Json(track).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn get_resolved_track(
    _session: Session,
    State(catalog): State<GuardedCatalog>,
    Path(id): Path<String>,
) -> Response {
    match catalog.lock().unwrap().get_resolved_track(&id) {
        Ok(Some(track)) => Json(track).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response(),
    }
}

async fn get_image(
    _session: Session,
    State(catalog): State<GuardedCatalog>,
    Path(id): Path<String>,
) -> Response {
    let file_path = catalog.lock().unwrap().get_image_path(id);
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

fn update_user_liked_content(
    user_manager: GuardedUserManager,
    user_id: usize,
    content_id: &str,
    liked: bool,
) -> Response {
    match user_manager
        .lock()
        .unwrap()
        .set_user_liked_content(user_id, content_id, liked)
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn add_user_liked_content(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(content_id): Path<String>,
) -> Response {
    update_user_liked_content(user_manager, session.user_id, &content_id, true)
}

async fn delete_user_liked_content(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(content_id): Path<String>,
) -> Response {
    update_user_liked_content(user_manager, session.user_id, &content_id, false)
}

async fn get_user_liked_content(
    session: Session,
    State(user_manager): State<GuardedUserManager>,
    Path(content_id): Path<String>,
) -> Response {
    let content_type = content_id; // Other route methods are already registered with content_id.
    let content_type = match content_type.as_str() {
        "album" => LikedContentType::Album,
        "artist" => LikedContentType::Artist,
        "track" => LikedContentType::Track,
        _ => return StatusCode::BAD_REQUEST.into_response(),
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

async fn login(
    State(user_manager): State<GuardedUserManager>,
    Json(body): Json<LoginBody>,
) -> Response {
    debug!("login() called with {:?}", body);
    let mut locked_manager = user_manager.lock().unwrap();
    let credentials = match locked_manager.get_user_credentials(&body.user_handle) {
        Ok(Some(creds)) => creds,
        Ok(None) => return StatusCode::UNAUTHORIZED.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if let Some(password_credentials) = &credentials.username_password {
            if let Ok(true) = password_credentials.hasher.verify(
                &body.password,
                &password_credentials.hash,
                &password_credentials.salt,
            ) {
                return match locked_manager.generate_auth_token(&credentials) {
                    Ok(auth_token) => {
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
                        StatusCode::INTERNAL_SERVER_ERROR.into_response()
                    }
                };
            }
        }
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

impl ServerState {
    fn new(
        config: ServerConfig,
        catalog: Arc<Mutex<Catalog>>,
        search_vault: Box<dyn SearchVault>,
        user_manager: UserManager,
    ) -> ServerState {
        ServerState {
            config,
            start_time: Instant::now(),
            catalog: catalog,
            search_vault: Arc::new(Mutex::new(search_vault)),
            user_manager: Arc::new(Mutex::new(user_manager)),
            hash: "123456".to_owned(),
        }
    }
}

pub fn make_app(
    config: ServerConfig,
    catalog: Catalog,
    search_vault: Box<dyn SearchVault>,
    user_store: Box<dyn UserStore>,
) -> Result<Router> {
    let guarded_catalog = Arc::new(Mutex::new(catalog));
    let user_manager = UserManager::new(guarded_catalog.clone(), user_store);
    let state = ServerState::new(config.clone(), guarded_catalog, search_vault, user_manager);

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
    let liked_content_read_routes: Router = Router::new()
        .route("/liked/{content_id}", get(get_user_liked_content))
        .layer(GovernorLayer::new(user_content_read_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_like_content,
        ))
        .with_state(state.clone());

    // Liked content WRITE routes (stricter limit)
    let liked_content_write_routes: Router = Router::new()
        .route("/liked/{content_id}", post(add_user_liked_content))
        .route("/liked/{content_id}", delete(delete_user_liked_content))
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

    let user_routes = liked_content_routes.merge(playlist_routes);

    // Admin routes with write rate limiting
    let admin_routes: Router = Router::new()
        .route("/reboot", post(reboot_server))
        .layer(GovernorLayer::new(write_rate_limit.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_reboot_server,
        ))
        .with_state(state.clone());

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

pub async fn run_server(
    catalog: Catalog,
    search_vault: Box<dyn SearchVault>,
    user_store: Box<dyn UserStore>,
    requests_logging_level: RequestsLoggingLevel,
    port: u16,
    content_cache_age_sec: usize,
    frontend_dir_path: Option<String>,
) -> Result<()> {
    let config = ServerConfig {
        port,
        requests_logging_level,
        content_cache_age_sec,
        frontend_dir_path,
    };
    let app = make_app(config, catalog, search_vault, user_store)?;

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    // Use into_make_service_with_connect_info to provide IP address for rate limiting
    Ok(axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::NoOpSearchVault;
    use crate::user::auth::UserAuthCredentials;
    use crate::user::auth::{AuthToken, AuthTokenValue};
    use crate::user::user_models::LikedContentType;
    use crate::user::{UserAuthCredentialsStore, UserAuthTokenStore};
    use axum::{body::Body, http::Request};
    use tower::ServiceExt; // for `call`, `oneshot`, and `ready

    #[tokio::test]
    async fn responds_forbidden_on_protected_routes() {
        let user_store = Box::new(InMemoryUserStore::default());
        let app = &mut make_app(
            ServerConfig::default(),
            Catalog::dummy(),
            Box::new(NoOpSearchVault {}),
            user_store,
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
        ];

        for route in protected_routes.into_iter() {
            println!("Trying route {}", route);
            let mut request = Request::builder().uri(route).body(Body::empty()).unwrap();
            // Add ConnectInfo extension for rate limiting
            request.extensions_mut().insert(ConnectInfo(test_addr));
            let response = app.oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }

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

        fn get_all_user_handles(&self) -> Vec<String> {
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

        fn get_all_user_auth_tokens(&self, _user_handle: &str) -> Vec<AuthToken> {
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
}
