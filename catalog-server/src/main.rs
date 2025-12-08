use anyhow::{Context, Result};
use clap::Parser;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fmt::Debug, path::PathBuf};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod catalog_store;
use catalog_store::{CatalogStore, SqliteCatalogStore};

mod downloader;

mod search;
#[cfg(feature = "no_search")]
use search::NoOpSearchVault;
#[cfg(not(feature = "no_search"))]
use search::PezzotHashSearchVault;
use search::SearchVault;

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
    /// Path to the SQLite catalog database file.
    #[clap(value_parser = parse_path)]
    pub catalog_db: PathBuf,

    /// Path to the SQLite database file to use for user storage.
    #[clap(value_parser = parse_path)]
    pub user_store_file_path: PathBuf,

    /// Path to the catalog media directory (for audio files and images).
    #[clap(long, value_parser = parse_path)]
    pub media_path: Option<PathBuf>,

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

    /// URL of the downloader service for fetching missing content.
    #[clap(long)]
    pub downloader_url: Option<String>,

    /// Timeout in seconds for downloader requests.
    #[clap(long, default_value_t = 300)]
    pub downloader_timeout_sec: u64,

    /// Number of days to retain sync events before pruning. Set to 0 to disable pruning.
    #[clap(long, default_value_t = 30)]
    pub event_retention_days: u64,

    /// Interval in hours between pruning runs. Only used if event_retention_days > 0.
    #[clap(long, default_value_t = 24)]
    pub prune_interval_hours: u64,
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

    // Default media path to parent of catalog db if not specified
    let media_path = match cli_args.media_path {
        Some(path) => path,
        None => cli_args
            .catalog_db
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".")),
    };

    info!(
        "Opening SQLite catalog database at {:?}...",
        cli_args.catalog_db
    );
    let catalog_store = Arc::new(SqliteCatalogStore::new(&cli_args.catalog_db, &media_path)?);

    // Initialize metrics system
    info!("Initializing metrics...");
    server::metrics::init_metrics();
    server::metrics::init_catalog_metrics(
        catalog_store.get_artists_count(),
        catalog_store.get_albums_count(),
        catalog_store.get_tracks_count(),
    );

    let user_store = Arc::new(SqliteUserStore::new(&cli_args.user_store_file_path)?);

    // Spawn background task for event pruning if enabled
    if cli_args.event_retention_days > 0 {
        let retention_days = cli_args.event_retention_days;
        let interval_hours = cli_args.prune_interval_hours;
        let pruning_user_store = user_store.clone();

        info!(
            "Event pruning enabled: retaining {} days, pruning every {} hours",
            retention_days, interval_hours
        );

        tokio::spawn(async move {
            let interval = Duration::from_secs(interval_hours * 60 * 60);
            let mut ticker = tokio::time::interval(interval);

            // Skip the first immediate tick, wait for the first interval
            ticker.tick().await;

            loop {
                ticker.tick().await;

                let cutoff = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
                    - (retention_days as i64 * 24 * 60 * 60);

                match pruning_user_store.prune_events_older_than(cutoff) {
                    Ok(count) => {
                        if count > 0 {
                            info!("Pruned {} old sync events", count);
                        }
                    }
                    Err(e) => {
                        error!("Failed to prune sync events: {}", e);
                    }
                }
            }
        });
    }

    info!("Indexing content for search...");

    #[cfg(not(feature = "no_search"))]
    let search_vault: Box<dyn SearchVault> =
        Box::new(PezzotHashSearchVault::new(catalog_store.clone()));

    #[cfg(feature = "no_search")]
    let search_vault: Box<dyn SearchVault> = Box::new(NoOpSearchVault {});

    // Create downloader client if URL is configured
    let downloader: Option<Arc<dyn downloader::Downloader>> = cli_args.downloader_url.map(|url| {
        info!("Downloader service configured at {}", url);
        Arc::new(downloader::DownloaderClient::new(
            url,
            cli_args.downloader_timeout_sec,
        )) as Arc<dyn downloader::Downloader>
    });

    // Pass media_base_path for proxy if downloader is configured
    let media_base_path_for_proxy = if downloader.is_some() {
        Some(media_path.clone())
    } else {
        None
    };

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
        downloader,
        media_base_path_for_proxy,
    )
    .await
}
