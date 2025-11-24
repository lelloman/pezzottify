use anyhow::Result;
use std::{
    fs::File,
    io::Read,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tracing::{debug, error};

use crate::{
    catalog::Catalog,
    user::{user_models::LikedContentType, UserStore},
};
use crate::{search::SearchVault, user::UserManager};
use axum_extra::extract::cookie::{Cookie, SameSite};
use tower_http::services::ServeDir;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, response, HeaderValue, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower::{Service, ServiceExt}; // for `call`, `oneshot`, and `ready

use super::{
    http_cache, log_requests, make_search_routes, slowdown_request, state::*, RequestsLoggingLevel,
    ServerConfig,
};
use super::{session, stream_track};
use crate::server::session::Session;
use crate::user::auth::{AuthTokenValue, UserAuthCredentials};
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
    session: Session,
    State(catalog): State<GuardedCatalog>,
    Path(id): Path<String>,
) -> Response {
    match catalog.lock().unwrap().get_artist(&id) {
        Some(artist) => Json(artist).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_album(
    session: Session,
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
    if let Some(credentials) = locked_manager.get_user_credentials(&body.user_handle) {
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
    }
    StatusCode::FORBIDDEN.into_response()
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

async fn get_challenge(State(state): State<ServerState>) -> Response {
    todo!()
}

async fn post_challenge(State(state): State<ServerState>) -> Response {
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

fn make_app(
    config: ServerConfig,
    catalog: Catalog,
    search_vault: Box<dyn SearchVault>,
    user_store: Box<dyn UserStore>,
) -> Result<Router> {
    let guarded_catalog = Arc::new(Mutex::new(catalog));
    let user_manager = UserManager::new(guarded_catalog.clone(), user_store);
    let state = ServerState::new(config.clone(), guarded_catalog, search_vault, user_manager);

    let auth_routes: Router = Router::new()
        .route("/login", post(login))
        .route("/logout", get(logout))
        .route("/challenge", get(get_challenge))
        .route("/challenge", post(post_challenge))
        .with_state(state.clone());

    let mut content_routes: Router = Router::new()
        .route("/album/{id}", get(get_album))
        .route("/album/{id}/resolved", get(get_resolved_album))
        .route("/artist/{id}", get(get_artist))
        .route("/artist/{id}/discography", get(get_artist_discography))
        .route("/track/{id}", get(get_track))
        .route("/track/{id}/resolved", get(get_resolved_track))
        .route("/image/{id}", get(get_image))
        .route("/stream/{id}", get(stream_track))
        .layer(middleware::from_fn_with_state(
            config.content_cache_age_sec,
            http_cache,
        ))
        .with_state(state.clone());

    let user_routes: Router = Router::new()
        .route("/liked/{content_id}", post(add_user_liked_content))
        .route("/liked/{content_id}", delete(delete_user_liked_content))
        .route("/liked/{content_id}", get(get_user_liked_content))
        .route("/playlist", post(post_playlist))
        .route("/playlist/{id}", put(put_playlist))
        .route("/playlist/{id}", delete(delete_playlist))
        .route("/playlist/{id}", get(get_playlist))
        .route("/playlist/{id}/add", put(add_playlist_tracks))
        .route("/playlist/{id}/remove", put(remove_tracks_from_playlist))
        .route("/playlists", get(get_user_playlists))
        .with_state(state.clone());

    if let Some(search_routes) = make_search_routes(state.clone()) {
        content_routes = content_routes.merge(search_routes);
    }

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
        .nest("/v1/user", user_routes);

    #[cfg(feature = "slowdown")]
    {
        app = app.layer(middleware::from_fn(slowdown_request));
    }
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

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();

    Ok(axum::serve(listener, app).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::NoOpSearchVault;
    use crate::user::auth::{ActiveChallenge, AuthToken, AuthTokenValue};
    use crate::user::user_models::LikedContentType;
    use crate::user::{UserAuthCredentialsStore, UserAuthTokenStore};
    use axum::{body::Body, http::Request};
    use std::collections::HashMap;

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
            let request = Request::builder().uri(route).body(Body::empty()).unwrap();
            let response = app.oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }

        let request = Request::builder()
            .method("POST")
            .uri("/v1/content/search")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[derive(Default)]
    struct InMemoryUserStore {}

    impl UserStore for InMemoryUserStore {
        fn create_user(&self, user_handle: &str) -> Result<usize> {
            todo!()
        }

        fn get_user_handle(&self, user_id: usize) -> Option<String> {
            todo!()
        }

        fn get_user_id(&self, user_handle: &str) -> Option<usize> {
            todo!()
        }

        fn get_user_playlists(&self, user_id: usize) -> Result<Vec<String>> {
            todo!()
        }

        fn is_user_liked_content(&self, user_id: usize, content_id: &str) -> Option<bool> {
            todo!()
        }

        fn set_user_liked_content(
            &self,
            user_id: usize,
            content_id: &str,
            content_type: LikedContentType,
            liked: bool,
        ) -> Result<()> {
            todo!()
        }

        fn get_all_user_handles(&self) -> Vec<String> {
            todo!()
        }

        fn get_user_liked_content(
            &self,
            user_id: usize,
            content_type: LikedContentType,
        ) -> Result<Vec<String>> {
            todo!()
        }

        fn create_user_playlist(
            &self,
            user_id: usize,
            playlist_name: &str,
            creator_id: usize,
            track_ids: Vec<String>,
        ) -> Result<String> {
            todo!()
        }

        fn delete_user_playlist(&self, playlist_id: &str, user_id: usize) -> Result<()> {
            todo!()
        }

        fn update_user_playlist(
            &self,
            playlist_id: &str,
            user_id: usize,
            playlist_name: Option<String>,
            track_ids: Option<Vec<String>>,
        ) -> Result<()> {
            todo!()
        }

        fn get_user_playlist(
            &self,
            playlist_id: &str,
            user_id: usize,
        ) -> Result<crate::user::UserPlaylist> {
            todo!()
        }

        fn get_user_roles(&self, user_id: usize) -> Result<Vec<crate::user::UserRole>> {
            todo!()
        }

        fn add_user_role(&self, user_id: usize, role: crate::user::UserRole) -> Result<()> {
            todo!()
        }

        fn remove_user_role(&self, user_id: usize, role: crate::user::UserRole) -> Result<()> {
            todo!()
        }

        fn add_user_extra_permission(&self, user_id: usize, grant: crate::user::PermissionGrant) -> Result<usize> {
            todo!()
        }

        fn remove_user_extra_permission(&self, permission_id: usize) -> Result<()> {
            todo!()
        }

        fn decrement_permission_countdown(&self, permission_id: usize) -> Result<bool> {
            todo!()
        }

        fn resolve_user_permissions(&self, user_id: usize) -> Result<Vec<crate::user::Permission>> {
            Ok(vec![])
        }
    }

    impl UserAuthTokenStore for InMemoryUserStore {
        fn get_user_auth_token(&self, token: &AuthTokenValue) -> Option<AuthToken> {
            todo!()
        }

        fn delete_user_auth_token(&self, token: &AuthTokenValue) -> Option<AuthToken> {
            todo!()
        }

        fn update_user_auth_token_last_used_timestamp(&self, token: &AuthTokenValue) -> Result<()> {
            todo!()
        }

        fn add_user_auth_token(&self, token: AuthToken) -> Result<()> {
            todo!()
        }

        fn get_all_user_auth_tokens(&self, user_handle: &str) -> Vec<AuthToken> {
            todo!()
        }
    }

    impl UserAuthCredentialsStore for InMemoryUserStore {
        fn get_user_auth_credentials(&self, user_handle: &str) -> Option<UserAuthCredentials> {
            todo!()
        }

        fn update_user_auth_credentials(&self, credentials: UserAuthCredentials) -> Result<()> {
            todo!()
        }
    }

    #[derive(Default)]
    struct InMemoryAuthStore {
        auth_credentials: HashMap<String, UserAuthCredentials>,
        active_challenges: Vec<ActiveChallenge>,
        auth_tokens: HashMap<AuthTokenValue, AuthToken>,
    }
}
