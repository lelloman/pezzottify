use anyhow::Result;
use std::{
    fs::File,
    io::Read,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tracing::error;

use crate::catalog::Catalog;
use crate::search::{SearchVault, SearchedAlbum};

use axum_extra::extract::cookie::{Cookie, SameSite};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, response, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower::{Service, ServiceExt}; // for `call`, `oneshot`, and `ready

use super::{auth, make_search_routes, state::*};
use super::{stream_track, AuthStore, UserId};
use crate::server::{auth::AuthManager, session::Session};

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

#[derive(Deserialize)]
struct LoginBody {
    pub user_id: UserId,
    pub password: String,
}

#[derive(Serialize)]
struct LoginSuccessResponse {
    token: String,
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
    session: Session,
    State(catalog): State<GuardedCatalog>,
    Path(id): Path<String>,
) -> Response {
    match catalog.lock().unwrap().get_resolved_album(&id) {
        Ok(Some(album)) => Json(album).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response(),
    }
}

async fn get_artist_albums(
    session: Session,
    State(catalog): State<GuardedCatalog>,
    Path(id): Path<String>,
) -> Response {
    match catalog.lock().unwrap().get_artist_albums(id) {
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

async fn get_image(
    session: Session,
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

async fn login(
    State(auth_manager): State<GuardedAuthManager>,
    Json(body): Json<LoginBody>,
) -> Response {
    let mut locked_manager = auth_manager.lock().unwrap();
    if let Some(credentials) = locked_manager.get_user_credentials(&body.user_id) {
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

async fn logout(State(auth_manager): State<GuardedAuthManager>, session: Session) -> Response {
    let mut locked_manager = auth_manager.lock().unwrap();
    match locked_manager.delete_auth_token(&session.user_id, &auth::AuthTokenValue(session.token)) {
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
        catalog: Catalog,
        search_vault: Box<dyn SearchVault>,
        auth_manager: AuthManager,
    ) -> ServerState {
        ServerState {
            start_time: Instant::now(),
            catalog: Arc::new(Mutex::new(catalog)),
            search_vault: Arc::new(Mutex::new(search_vault)),
            auth_manager: Arc::new(Mutex::new(auth_manager)),
            hash: "123456".to_owned(),
        }
    }
}

fn make_app(
    catalog: Catalog,
    search_vault: Box<dyn SearchVault>,
    auth_store: Box<dyn AuthStore>,
) -> Result<Router> {
    let auth_manager = AuthManager::initialize(auth_store)?;
    let state = ServerState::new(catalog, search_vault, auth_manager);

    let auth_routes: Router = Router::new()
        .route("/login", post(login))
        .route("/logout", get(logout))
        .route("/challenge", get(get_challenge))
        .route("/challenge", post(post_challenge))
        .with_state(state.clone());

    let mut content_routes: Router = Router::new()
        .route("/artist/{id}", get(get_artist))
        .route("/album/{id}", get(get_album))
        .route("/album/{id}/resolved", get(get_resolved_album))
        .route("/artist/{id}/albums", get(get_artist_albums))
        .route("/track/{id}", get(get_track))
        .route("/image/{id}", get(get_image))
        .route("/stream/{id}", get(stream_track))
        .with_state(state.clone());

    if let Some(search_routes) = make_search_routes(state.clone()) {
        content_routes = content_routes.merge(search_routes);
    }

    let app: Router = Router::new()
        .route("/", get(home))
        .with_state(state)
        .nest("/v1/auth", auth_routes)
        .nest("/v1/content", content_routes);

    Ok(app)
}

pub async fn run_server(
    catalog: Catalog,
    search_vault: Box<dyn SearchVault>,
    auth_store: Box<dyn AuthStore>,
    port: u16,
) -> Result<()> {
    let app = make_app(catalog, search_vault, auth_store)?;

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();

    Ok(axum::serve(listener, app).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        search::NoOpSearchVault,
        server::{ActiveChallenge, AuthToken, AuthTokenValue, UserAuthCredentials},
    };
    use axum::{body::Body, http::Request};
    use std::collections::HashMap;

    #[tokio::test]
    async fn responds_forbidden_on_protected_routes() {
        let auth_store = Box::new(InMemoryAuthStore::default());
        let app =
            &mut make_app(Catalog::dummy(), Box::new(NoOpSearchVault {}), auth_store).unwrap();

        let protected_routes = vec![
            "/v1/content/artist/123",
            "/v1/content/album/123",
            "/v1/content/artist/123/albums",
            "/v1/content/album/123/resolved",
            "/v1/content/track/123",
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
    struct InMemoryAuthStore {
        auth_credentials: HashMap<UserId, UserAuthCredentials>,
        active_challenges: Vec<ActiveChallenge>,
        auth_tokens: HashMap<AuthTokenValue, AuthToken>,
    }

    impl AuthStore for InMemoryAuthStore {
        fn load_auth_credentials(&self) -> Result<HashMap<UserId, UserAuthCredentials>> {
            Ok(self.auth_credentials.clone())
        }

        fn update_auth_credentials(&self, credentials: auth::UserAuthCredentials) -> Result<()> {
            todo!()
        }

        fn load_challenges(&self) -> Result<Vec<ActiveChallenge>> {
            Ok(self.active_challenges.clone())
        }

        fn delete_challenge(&self, challenge: auth::ActiveChallenge) -> Result<()> {
            todo!()
        }

        fn flag_sent_challenge(&self, challenge: &auth::ActiveChallenge) -> Result<()> {
            todo!()
        }

        fn add_challenges(&self, challenges: Vec<auth::ActiveChallenge>) -> Result<()> {
            todo!()
        }

        fn load_auth_tokens(&self) -> Result<HashMap<AuthTokenValue, AuthToken>> {
            Ok(self.auth_tokens.clone())
        }

        fn delete_auth_token(&self, value: auth::AuthTokenValue) -> Result<()> {
            todo!()
        }

        fn update_auth_token(&self, token: &auth::AuthToken) -> Result<()> {
            todo!()
        }

        fn add_auth_token(&self, token: &auth::AuthToken) -> Result<()> {
            todo!()
        }
    }
}
