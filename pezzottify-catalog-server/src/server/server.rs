use anyhow::Result;
use std::{
    fs::File,
    io::Read,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::catalog::Catalog;
use crate::search::{SearchResult, SearchVault};

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use super::session::Session;
use super::state::ServerState;

type GuardedCatalog = Arc<Mutex<Catalog>>;
type GuardedSearchVault = Arc<Mutex<SearchVault>>;

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
struct SearchBody {
    pub query: String,
}

async fn home(session: Option<Session>, State(state): State<ServerState>) -> impl IntoResponse {
    let stats = ServerStats {
        uptime: format_uptime(state.start_time.elapsed()),
        hash: state.hash.clone(),
        session_token: session.map(|s| s.token),
    };
    Json(stats)
}

async fn get_artist(State(catalog): State<GuardedCatalog>, Path(id): Path<String>) -> Response {
    match catalog.lock().unwrap().get_artist(&id) {
        Some(artist) => Json(artist).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_track(State(catalog): State<GuardedCatalog>, Path(id): Path<String>) -> Response {
    match catalog.lock().unwrap().get_track(&id) {
        Some(track) => Json(track).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_album(State(catalog): State<GuardedCatalog>, Path(id): Path<String>) -> Response {
    match catalog.lock().unwrap().get_album(&id) {
        Some(album) => Json(album).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn search(
    State(search_vault): State<GuardedSearchVault>,
    Json(payload): Json<SearchBody>,
) -> impl IntoResponse {
    let search_results: Vec<SearchResult> = search_vault.lock().unwrap().search(payload.query, 30).collect();
    Json(search_results)
}

async fn get_image(State(catalog): State<GuardedCatalog>, Path(id): Path<String>) -> Response {
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

impl ServerState {
    fn new(catalog: Catalog, search_vault: SearchVault) -> ServerState {
        ServerState {
            start_time: Instant::now(),
            catalog: Arc::new(Mutex::new(catalog)),
            search_vault: Arc::new(Mutex::new(search_vault)),
            hash: "123456".to_owned(),
        }
    }
}

pub async fn run_server(catalog: Catalog, search_vault: SearchVault, port: u16) -> Result<()> {
    let state = ServerState::new(catalog, search_vault);

    let app: Router = Router::new()
        .route("/", get(home))
        .route("/artist/{id}", get(get_artist))
        .route("/album/{id}", get(get_album))
        .route("/track/{id}", get(get_track))
        .route("/search", post(search))
        .route("/image/{id}", get(get_image))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();

    Ok(axum::serve(listener, app).await?)
}
