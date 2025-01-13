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

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Response, IntoResponse},
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

async fn get_artist(
    State(state): State<Arc<ServerState>>,
    Path(id): Path<String>,
) -> Response {
    match state.catalog.get_artist(&id) {
        Some(artist) => Json(artist).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_track(State(state): State<Arc<ServerState>>, track_id: String) -> impl IntoResponse {
    todo!("get track not implemented yet")
}

async fn get_album(album_id: String) -> impl IntoResponse {
    todo!("get album not implemented yet")
}

async fn get_image(image_id: String) -> impl IntoResponse {
    todo!("get image not implemented yet")
}

fn load_catalog(path: PathBuf) -> Result<Catalog> {
    let catalog_result = Catalog::build(&path);
    let problems = catalog_result.problems;
    let catalog = catalog_result.catalog;

    if !problems.is_empty() {
        info!("Found {} problems:", problems.len());
        for problem in problems.iter() {
            info!("- {:?}", problem);
        }
        info!("");
    }

    match (&catalog, problems.is_empty()) {
        (Some(_), true) => info!("Catalog checked, no issues found."),
        (Some(_), false) => info!("Catalog was built, but check the issues above."),
        (None, _) => {
            info!("Check the problems above, the catalog could not be initialized.")
        }
    }
    if let Some(catalog) = catalog {
        info!(
            "Catalog has:\n{} artists\n{} albums\n{} tracks",
            catalog.get_artists_count(),
            catalog.get_albums_count(),
            catalog.get_tracks_count()
        );
        return Ok(catalog);
    }

    bail!("Could not load catalog");
}

struct ServerState {
    start_time: Instant,
    catalog: Catalog,
    hash: String,
}

impl ServerState {
    fn new(catalog: Catalog) -> ServerState {
        ServerState {
            start_time: Instant::now(),
            catalog,
            hash: "123456".to_owned(),
        }
    }
}

async fn run_server(catalog: Catalog, port: u16) -> Result<()> {
    let state = Arc::new(ServerState::new(catalog));

    let app: Router = Router::new()
        .route("/", get(home))
        .route("/artist/{id}", get(get_artist))
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
    let catalog = load_catalog(cli_args.path.clone())?;

    if cli_args.check_only {
        return Ok(());
    }

    run_server(catalog, cli_args.port).await
}
