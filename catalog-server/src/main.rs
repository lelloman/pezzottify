use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fmt::Debug, path::PathBuf};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

// Import modules from the library crate
use pezzottify_catalog_server::background_jobs::jobs::PopularContentJob;
use pezzottify_catalog_server::background_jobs::{create_scheduler, JobContext};
use pezzottify_catalog_server::catalog_store::{CatalogStore, SqliteCatalogStore};
use pezzottify_catalog_server::config;
use pezzottify_catalog_server::download_manager::{
    DownloadManager, DownloaderClient, QueueProcessor, SqliteDownloadQueueStore,
};
use pezzottify_catalog_server::downloader;
#[cfg(feature = "no_search")]
use pezzottify_catalog_server::search::NoOpSearchVault;
#[cfg(not(feature = "no_search"))]
use pezzottify_catalog_server::search::PezzotHashSearchVault;
use pezzottify_catalog_server::search::SearchVault;
use pezzottify_catalog_server::server::{metrics, run_server, RequestsLoggingLevel};
use pezzottify_catalog_server::server_store::{self, SqliteServerStore};
use pezzottify_catalog_server::user::{self, SqliteUserStore};

fn parse_path(s: &str) -> Result<PathBuf, String> {
    let path_buf = PathBuf::from(s);
    let original_path = match path_buf.canonicalize() {
        Ok(path) => path,
        Err(msg) => {
            if msg.kind() == std::io::ErrorKind::NotFound {
                path_buf
            } else {
                return Err(format!("Error resolving path '{}': {}", s, msg));
            }
        }
    };
    if original_path.is_absolute() {
        return Ok(original_path);
    }
    let cwd = std::env::current_dir().map_err(|e| format!("Failed to get current dir: {}", e))?;
    Ok(cwd.join(original_path))
}

fn parse_dir(s: &str) -> Result<PathBuf, String> {
    let path = parse_path(s)?;
    if !path.exists() {
        return Err(format!("Directory does not exist: {}", s));
    }
    if !path.is_dir() {
        return Err(format!("Path is not a directory: {}", s));
    }
    Ok(path)
}

#[derive(Parser, Debug)]
struct CliArgs {
    /// Path to TOML configuration file. Values in the file override CLI arguments.
    #[clap(long, value_parser = parse_path)]
    pub config: Option<PathBuf>,

    /// Directory containing database files (catalog.db, user.db, server.db).
    /// Can also be specified in config file.
    #[clap(long, value_parser = parse_dir)]
    pub db_dir: Option<PathBuf>,

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

    /// Path to SSL certificate file (PEM format). Requires --ssl-key.
    #[clap(long, value_parser = parse_path)]
    pub ssl_cert: Option<PathBuf>,

    /// Path to SSL private key file (PEM format). Requires --ssl-cert.
    #[clap(long, value_parser = parse_path)]
    pub ssl_key: Option<PathBuf>,
}

/// Convert CLI args to CliConfig for config resolution
impl From<&CliArgs> for config::CliConfig {
    fn from(args: &CliArgs) -> Self {
        config::CliConfig {
            db_dir: args.db_dir.clone(),
            media_path: args.media_path.clone(),
            port: args.port,
            metrics_port: args.metrics_port,
            logging_level: args.logging_level.clone(),
            content_cache_age_sec: args.content_cache_age_sec,
            frontend_dir_path: args.frontend_dir_path.clone(),
            downloader_url: args.downloader_url.clone(),
            downloader_timeout_sec: args.downloader_timeout_sec,
            event_retention_days: args.event_retention_days,
            prune_interval_hours: args.prune_interval_hours,
            ssl_cert: args.ssl_cert.clone(),
            ssl_key: args.ssl_key.clone(),
        }
    }
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

    // Load TOML config if provided
    let file_config = match &cli_args.config {
        Some(path) => {
            info!("Loading configuration from {:?}", path);
            Some(config::FileConfig::load(path)?)
        }
        None => None,
    };

    // Resolve final configuration (TOML overrides CLI)
    let cli_config: config::CliConfig = (&cli_args).into();
    let app_config = config::AppConfig::resolve(&cli_config, file_config)?;

    info!("Configuration loaded:");
    info!("  db_dir: {:?}", app_config.db_dir);
    info!("  media_path: {:?}", app_config.media_path);
    info!("  port: {}", app_config.port);
    info!(
        "  download_manager.enabled: {}",
        app_config.download_manager.enabled
    );
    info!("  ssl.enabled: {}", app_config.ssl.is_some());

    // Create catalog store (will create DB if not exists)
    if !app_config.catalog_db_path().exists() {
        info!(
            "Creating new catalog database at {:?}",
            app_config.catalog_db_path()
        );
    }
    let catalog_store = Arc::new(SqliteCatalogStore::new(
        app_config.catalog_db_path(),
        &app_config.media_path,
    )?);

    // Initialize metrics system
    info!("Initializing metrics...");
    metrics::init_metrics();
    metrics::init_catalog_metrics(
        catalog_store.get_artists_count(),
        catalog_store.get_albums_count(),
        catalog_store.get_tracks_count(),
    );

    // Create user store (will create DB if not exists)
    if !app_config.user_db_path().exists() {
        info!(
            "Creating new user database at {:?}",
            app_config.user_db_path()
        );
    }
    let user_store = Arc::new(SqliteUserStore::new(app_config.user_db_path())?);

    // Create server store for background job history
    info!(
        "Initializing server store at {:?}",
        app_config.server_db_path()
    );
    let server_store = Arc::new(SqliteServerStore::new(app_config.server_db_path())?);

    // Set up background job scheduler
    let shutdown_token = CancellationToken::new();
    let (hook_sender, hook_receiver) = tokio::sync::mpsc::channel(100);

    let job_context = JobContext::new(
        shutdown_token.child_token(),
        catalog_store.clone() as Arc<dyn CatalogStore>,
        user_store.clone() as Arc<dyn user::FullUserStore>,
        server_store.clone() as Arc<dyn server_store::ServerStore>,
    );

    let (mut scheduler, scheduler_handle) = create_scheduler(
        server_store.clone(),
        hook_receiver,
        shutdown_token.clone(),
        job_context,
    );

    // Register jobs
    scheduler
        .register_job(Arc::new(PopularContentJob::new()))
        .await;
    info!(
        "Job scheduler initialized with {} job(s)",
        scheduler.job_count().await
    );

    // Note: The hook_sender is currently unused but will be used by the HTTP server
    // to notify the scheduler of events like catalog changes
    let _ = hook_sender;

    // Spawn background task for event pruning if enabled
    if app_config.event_retention_days > 0 {
        let retention_days = app_config.event_retention_days;
        let interval_hours = app_config.prune_interval_hours;
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
    let downloader: Option<Arc<dyn downloader::Downloader>> =
        app_config.downloader_url.clone().map(|url| {
            info!("Downloader service configured at {}", url);
            Arc::new(downloader::DownloaderClient::new(
                url,
                app_config.downloader_timeout_sec,
            )) as Arc<dyn downloader::Downloader>
        });

    // Pass media_base_path for proxy if downloader is configured
    let media_base_path_for_proxy = if downloader.is_some() {
        Some(app_config.media_path.clone())
    } else {
        None
    };

    // Initialize download manager if enabled
    let download_manager = if app_config.download_manager.enabled {
        let queue_store = Arc::new(SqliteDownloadQueueStore::new(
            app_config.download_queue_db_path(),
        )?);

        let dm_downloader_client = DownloaderClient::new(
            app_config.downloader_url.clone().unwrap(),
            app_config.downloader_timeout_sec,
        )?;

        let manager = Arc::new(DownloadManager::new(
            queue_store,
            dm_downloader_client,
            catalog_store.clone() as Arc<dyn CatalogStore>,
            app_config.media_path.clone(),
            app_config.download_manager.clone(),
        ));

        info!(
            "Download manager initialized (process_interval={}s)",
            app_config.download_manager.process_interval_secs
        );
        Some(manager)
    } else {
        info!("Download manager disabled");
        None
    };

    // Spawn queue processor task if download manager is enabled
    if let Some(ref dm) = download_manager {
        let processor = QueueProcessor::new(
            dm.clone(),
            app_config.download_manager.process_interval_secs,
        );
        let shutdown = shutdown_token.child_token();
        tokio::spawn(async move {
            processor.run(shutdown).await;
        });
        info!("Queue processor started");
    }

    info!("Ready to serve at port {}!", app_config.port);
    info!("Metrics available at port {}!", app_config.metrics_port);

    // Run HTTP server and job scheduler concurrently
    tokio::select! {
        result = run_server(
            catalog_store,
            search_vault,
            user_store,
            app_config.logging_level.clone(),
            app_config.port,
            app_config.metrics_port,
            app_config.content_cache_age_sec,
            app_config.frontend_dir_path.clone(),
            downloader,
            media_base_path_for_proxy,
            Some(scheduler_handle),
            app_config.ssl.clone(),
            download_manager,
        ) => {
            info!("HTTP server stopped: {:?}", result);
            shutdown_token.cancel();
            result
        },
        _ = scheduler.run() => {
            info!("Scheduler stopped");
            Ok(())
        },
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, initiating graceful shutdown");
            shutdown_token.cancel();
            // Give the scheduler a moment to shut down gracefully
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(())
        }
    }
}
