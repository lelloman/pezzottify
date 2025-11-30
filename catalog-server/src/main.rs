use anyhow::{Context, Result};
use clap::Parser;
use std::sync::Arc;
use std::{fmt::Debug, path::PathBuf};
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod catalog;
use catalog::Catalog;

mod catalog_store;
use catalog_store::{LegacyCatalogAdapter, SqliteCatalogStore};

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

    /// The port for the metrics server (Prometheus scraping).
    #[clap(long, default_value_t = 9091)]
    pub metrics_port: u16,

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

    /// Path to the SQLite catalog database file. When provided, uses the new
    /// SQLite-backed catalog store instead of the filesystem-based catalog.
    /// The catalog database must be populated using the catalog-import tool first.
    #[clap(long, value_parser = parse_path)]
    pub catalog_db: Option<PathBuf>,
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

    // Determine catalog store based on whether --catalog-db is provided
    let catalog_store: Arc<dyn catalog_store::CatalogStore> = if let Some(catalog_db_path) =
        cli_args.catalog_db
    {
        // Use the new SQLite-backed catalog store
        let catalog_path = match cli_args.catalog_path {
            Some(path) => path,
            None => Catalog::infer_path().with_context(|| {
                "Could not infer catalog directory for media paths, please specify it explicitly."
            })?,
        };

        info!(
            "Opening SQLite catalog database at {:?}...",
            catalog_db_path
        );
        Arc::new(SqliteCatalogStore::new(&catalog_db_path, &catalog_path)?)
    } else {
        // Use the legacy filesystem-based catalog
        let catalog_path = match cli_args.catalog_path {
            Some(path) => path,
            None => Catalog::infer_path().with_context(|| {
                "Could not infer catalog directory, please specify it explicitly."
            })?,
        };

        info!("Loading catalog from filesystem...");
        let catalog = catalog::load_catalog(catalog_path, cli_args.check_all)?;

        if cli_args.check_only {
            return Ok(());
        }

        Arc::new(LegacyCatalogAdapter::new(catalog))
    };

    // Initialize metrics system
    info!("Initializing metrics...");
    server::metrics::init_metrics();
    server::metrics::init_catalog_metrics(
        catalog_store.get_artists_count(),
        catalog_store.get_albums_count(),
        catalog_store.get_tracks_count(),
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
    let search_vault: Box<dyn SearchVault> =
        Box::new(PezzotHashSearchVault::new(catalog_store.clone()));

    #[cfg(feature = "no_search")]
    let search_vault: Box<dyn SearchVault> = Box::new(NoOpSearchVault {});

    info!("Ready to serve at port {}!", cli_args.port);
    info!("Metrics available at port {}!", cli_args.metrics_port);
    run_server(
        catalog_store,
        search_vault,
        user_store,
        cli_args.logging_level,
        cli_args.port,
        cli_args.metrics_port,
        cli_args.content_cache_age_sec,
        cli_args.frontend_dir_path,
    )
    .await
}
