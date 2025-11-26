use anyhow::{Context, Result};
use clap::Parser;
use std::{fmt::Debug, path::PathBuf};
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod catalog;
use catalog::Catalog;

mod search;
use search::{NoOpSearchVault, PezzotHashSearchVault, SearchVault};

mod server;
use server::{run_server, RequestsLoggingLevel};

mod sqlite_persistence;

mod user;
use user::SqliteUserStore;

fn parse_path(s: &str) -> Result<PathBuf> {
    let path_buf = PathBuf::from(s);
    let original_path = match path_buf.canonicalize() {
        Ok(path) => path,
        Err(msg) => {
            if msg.kind() == std::io::ErrorKind::NotFound {
                path_buf
            } else {
                return Err(msg).with_context(|| format!("Error resolving path: {}", s));
            }
        }
    };
    if original_path.is_absolute() {
        return Ok(original_path);
    }
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(original_path))
}

#[derive(Parser, Debug)]
struct CliArgs {
    /// Path to the catalog directory.
    #[clap(value_parser = parse_path)]
    pub catalog_path: Option<PathBuf>,

    /// Path to the SQLite database file to use for user storage.
    #[clap(value_parser = parse_path)]
    pub user_store_file_path: Option<PathBuf>,

    /// Only check the catalog for errors, do not start the server, might want to run with check_all too.
    #[clap(long)]
    pub check_only: bool,

    /// The port to listen on.
    #[clap(short, long, default_value_t = 3001)]
    pub port: u16,

    /// The level of logging to perform on each request.
    #[clap(long, default_value = "path")]
    pub logging_level: RequestsLoggingLevel,

    /// The maximum age of content in the cache in seconds.
    #[clap(long, default_value_t = 3600)]
    pub content_cache_age_sec: usize,

    /// Path to the frontend directory to be statically served.
    #[clap(long)]
    pub frontend_dir_path: Option<String>,

    /// Perform a full check of the catalog, including all files.
    #[clap(long)]
    pub check_all: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli_args = CliArgs::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .with_env_var("LOG_LEVEL")
                .from_env_lossy(),
        )
        .try_init()
        .unwrap();

    let catalog_path = match cli_args.catalog_path {
        Some(path) => path,
        None => Catalog::infer_path().with_context(|| {
            "Could not infer catalog directory, please specifiy it explicityly."
        })?,
    };

    info!("Loading catalog...");
    let catalog = catalog::load_catalog(catalog_path, cli_args.check_all)?;

    if cli_args.check_only {
        return Ok(());
    }

    // Initialize metrics system
    info!("Initializing metrics...");
    server::metrics::init_metrics();
    server::metrics::init_catalog_metrics(
        catalog.get_artists_count(),
        catalog.get_albums_count(),
        catalog.get_tracks_count(),
    );

    let user_store_file_path = match cli_args.user_store_file_path {
        Some(path) => path,
        None => SqliteUserStore::infer_path().with_context(|| {
            "Could not infer UserStore DB file path, please specify it explicitly."
        })?,
    };
    let user_store = Box::new(SqliteUserStore::new(&user_store_file_path)?);
    info!("Indexing content for search...");

    #[cfg(not(feature = "no_search"))]
    let search_vault: Box<dyn SearchVault> = Box::new(PezzotHashSearchVault::new(&catalog));

    #[cfg(feature = "no_search")]
    let search_vault: Box<dyn SearchVault> = Box::new(NoOpSearchVault {});

    info!("Ready to serve at port {}!", cli_args.port);
    run_server(
        catalog,
        search_vault,
        user_store,
        cli_args.logging_level,
        cli_args.port,
        cli_args.content_cache_age_sec,
        cli_args.frontend_dir_path,
    )
    .await
}
