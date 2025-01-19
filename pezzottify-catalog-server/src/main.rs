use anyhow::{bail, Result};
use clap::Parser;
use std::{
    fmt::{Debug, Write},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::{debug, info};

mod catalog;
use catalog::Catalog;

mod search;
use search::{SearchResult, SearchVault};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::main;

fn parse_root_dir(s: &str) -> Result<PathBuf> {
    let original_path = PathBuf::from(s).canonicalize()?;
    if original_path.is_absolute() {
        return Ok(original_path);
    }
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(original_path))
}

#[derive(Parser, Debug)]
struct CliArgs {
    #[clap(value_parser = parse_root_dir)]
    pub path: PathBuf,

    #[clap(long)]
    pub check_only: bool,

    #[clap(short, long, default_value_t = 3001)]
    pub port: u16,
}

#[derive(Serialize)]
struct ServerStats {
    pub uptime: String,
    pub hash: String,
}

#[derive(Deserialize)]
struct SearchBody {
    pub query: String,
}

fn format_uptime(duration: Duration) -> String {
    let total_seconds = duration.as_secs();

    let days = total_seconds / 86_400;
    let hours = (total_seconds % 86_400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    format!("{}d {:02}:{:02}:{:02}", days, hours, minutes, seconds)
}

async fn home(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let stats = ServerStats {
        uptime: format_uptime(state.start_time.elapsed()),
        hash: state.hash.clone(),
    };
    Json(stats)
}

async fn get_artist(State(state): State<Arc<ServerState>>, Path(id): Path<String>) -> Response {
    match state.catalog.get_artist(&id) {
        Some(artist) => Json(artist).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_track(State(state): State<Arc<ServerState>>, Path(id): Path<String>) -> Response {
    match state.catalog.get_track(&id) {
        Some(track) => Json(track).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_album(State(state): State<Arc<ServerState>>, Path(id): Path<String>) -> Response {
    match state.catalog.get_album(&id) {
        Some(album) => Json(album).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn search(State(state): State<Arc<ServerState>>, Json(payload): Json<SearchBody>) -> impl IntoResponse {
    let search_results: Vec<SearchResult> = state.search_vault.search(payload.query).collect();
    Json(search_results)
}


async fn get_image(State(state): State<Arc<ServerState>>, Path(id): Path<String>) -> impl IntoResponse {
    todo!("get image not implemented yet")
}

struct ServerState {
    start_time: Instant,
    catalog: Catalog,
    search_vault: SearchVault,
    hash: String,
}

impl ServerState {
    fn new(catalog: Catalog, search_vault: SearchVault) -> ServerState {
        ServerState {
            start_time: Instant::now(),
            catalog,
            search_vault,
            hash: "123456".to_owned(),
        }
    }
}

async fn run_server(catalog: Catalog, search_vault: SearchVault, port: u16) -> Result<()> {
    let state = Arc::new(ServerState::new(catalog, search_vault));

    let app: Router = Router::new()
        .route("/", get(home))
        .route("/artist/{id}", get(get_artist))
        .route("/album/{id}", get(get_album))
        .route("/track/{id}", get(get_track))
        .route("/search", post(search))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();

    Ok(axum::serve(listener, app).await?)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli_args = CliArgs::parse();

    tracing_subscriber::fmt::init();
    let catalog = catalog::load_catalog(&cli_args.path)?;

    if cli_args.check_only {
        return Ok(());
    }

    let search_vault = SearchVault::new(&catalog);
    run_server(catalog, search_vault, cli_args.port).await
}