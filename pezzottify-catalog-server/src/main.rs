use anyhow::{bail, Result};
use clap::Parser;
use std::path::PathBuf;
use tracing::{debug, info};

mod catalog;

use catalog::Catalog;

use axum::{
    http::StatusCode,
    response::IntoResponse,
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

async fn home() -> impl IntoResponse {
    todo!("HELLO")
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

async fn run_server(catalog: Catalog, port: u16) -> Result<()> {
    let app: Router = Router::new().route("/", get(home));

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
