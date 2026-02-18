use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use clap::Parser;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fmt::Debug, path::PathBuf};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

// Import modules from the library crate
use pezzottify_server::background_jobs::jobs::{
    AudioAnalysisJob, CatalogAvailabilityStatsJob, DevicePruningJob, IngestionCleanupJob,
    PopularContentJob, RelatedArtistsEnrichmentJob, WhatsNewBatchJob,
};
use pezzottify_server::background_jobs::{create_scheduler, GuardedSearchVault, JobContext};
use pezzottify_server::catalog_store::{CatalogStore, SqliteCatalogStore};
use pezzottify_server::enrichment_store::SqliteEnrichmentStore;
use pezzottify_server::config;
use pezzottify_server::ingestion::{IngestionStore, SqliteIngestionStore};
use pezzottify_server::search::{Fts5LevenshteinSearchVault, NoopSearchVault};
use pezzottify_server::server::{metrics, run_server, RequestsLoggingLevel};
use pezzottify_server::server_store::{self, ServerStore, SqliteServerStore};
use pezzottify_server::user::{self, SqliteUserStore, UserManager};

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

    // Extract OIDC config before consuming file_config
    let oidc_config = file_config.as_ref().and_then(|f| f.oidc.clone());

    // Resolve final configuration (TOML overrides CLI)
    let cli_config: config::CliConfig = (&cli_args).into();
    let app_config = config::AppConfig::resolve(&cli_config, file_config)?;

    info!("Configuration loaded:");
    info!("  db_dir: {:?}", app_config.db_dir);
    info!("  media_path: {:?}", app_config.media_path);
    info!("  port: {}", app_config.port);

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
        app_config.catalog_store.read_pool_size,
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

    // Create UserManager early so it can be shared with job scheduler
    let user_manager = Arc::new(std::sync::Mutex::new(UserManager::new(
        catalog_store.clone() as Arc<dyn CatalogStore>,
        user_store.clone() as Arc<dyn user::FullUserStore>,
    )));

    // Create search vault early so it can be shared with job scheduler
    // Use lazy initialization for fast startup, then build index in background
    let search_vault: Box<dyn pezzottify_server::search::SearchVault> =
        match app_config.search.engine.as_str() {
            "noop" => {
                info!("Search disabled (noop engine)");
                Box::new(NoopSearchVault)
            }
            _ => {
                info!(
                    "Initializing search vault (engine: {})...",
                    app_config.search.engine
                );
                let vault = Arc::new(Fts5LevenshteinSearchVault::new_lazy(
                    &app_config.search_db_path(),
                )?);

                // No background build - search index grows organically via OrganicIndexer
                // when users browse content (artists, albums, tracks)
                info!(
                "Search vault ready (organic indexing mode - index grows as content is accessed)"
            );

                // Box the Arc directly - SearchVault is implemented for Arc<T>
                Box::new(vault)
            }
        };
    // SearchVault is internally thread-safe - no external Mutex needed
    let guarded_search_vault: GuardedSearchVault = std::sync::Arc::from(search_vault);

    // Set up background job scheduler
    let shutdown_token = CancellationToken::new();
    let (hook_sender, hook_receiver) = tokio::sync::mpsc::channel(100);

    // Create enrichment store if audio analysis is enabled
    let enrichment_store = if app_config.audio_analysis.is_some() {
        info!(
            "Initializing enrichment store at {:?}",
            app_config.enrichment_db_path()
        );
        match SqliteEnrichmentStore::new(app_config.enrichment_db_path()) {
            Ok(store) => Some(Arc::new(store)),
            Err(e) => {
                error!("Failed to create enrichment store: {:?}", e);
                None
            }
        }
    } else {
        None
    };

    let mut job_context = JobContext::with_search_vault(
        shutdown_token.child_token(),
        catalog_store.clone() as Arc<dyn CatalogStore>,
        user_store.clone() as Arc<dyn user::FullUserStore>,
        server_store.clone() as Arc<dyn server_store::ServerStore>,
        user_manager.clone(),
        guarded_search_vault.clone(),
    );

    if let Some(ref store) = enrichment_store {
        job_context = job_context.with_enrichment_store(
            store.clone() as Arc<dyn pezzottify_server::enrichment_store::EnrichmentStore>,
        );
    }

    let (mut scheduler, scheduler_handle) = create_scheduler(
        server_store.clone(),
        hook_receiver,
        shutdown_token.clone(),
        job_context,
    );

    // Register jobs
    scheduler
        .register_job(Arc::new(PopularContentJob::from_settings(
            &app_config.background_jobs.popular_content,
        )))
        .await;
    scheduler
        .register_job(Arc::new(CatalogAvailabilityStatsJob::from_settings(
            &app_config.background_jobs.catalog_availability_stats,
        )))
        .await;
    scheduler
        .register_job(Arc::new(WhatsNewBatchJob::from_settings(
            &app_config.background_jobs.whatsnew_batch,
        )))
        .await;
    scheduler
        .register_job(Arc::new(DevicePruningJob::from_settings(
            &app_config.background_jobs.device_pruning,
        )))
        .await;

    // Register ingestion cleanup job if ingestion is enabled
    if app_config.ingestion.enabled {
        let ingestion_db_path = app_config.ingestion_db_path();
        match SqliteIngestionStore::open(&ingestion_db_path) {
            Ok(store) => {
                let ingestion_store: Arc<dyn IngestionStore> = Arc::new(store);
                let temp_dir = app_config.ingestion_temp_dir();
                let ic_settings = app_config
                    .background_jobs
                    .ingestion_cleanup
                    .as_ref()
                    .cloned()
                    .unwrap_or_default();
                scheduler
                    .register_job(Arc::new(IngestionCleanupJob::from_settings(
                        ingestion_store,
                        temp_dir,
                        &ic_settings,
                    )))
                    .await;
                info!("Registered ingestion cleanup job");
            }
            Err(e) => {
                error!("Failed to open ingestion database for cleanup job: {:?}", e);
            }
        }
    }

    // Register related artists enrichment job if configured
    if let Some(ref ra_settings) = app_config.related_artists {
        match RelatedArtistsEnrichmentJob::new(ra_settings.clone()) {
            Ok(job) => {
                scheduler.register_job(Arc::new(job)).await;
                info!("Registered related artists enrichment job");
            }
            Err(e) => {
                error!("Failed to create related artists enrichment job: {}", e);
            }
        }
    }

    // Register audio analysis job if configured (requires enrichment store in context)
    if let Some(ref aa_settings) = app_config.audio_analysis {
        if enrichment_store.is_some() {
            scheduler
                .register_job(Arc::new(AudioAnalysisJob::new(aa_settings.clone())))
                .await;
            info!("Registered audio analysis job");
        } else {
            error!("Audio analysis enabled but enrichment store failed to initialize");
        }
    }

    // Delay the first catalog availability stats run after each startup.
    // This avoids expensive filesystem scans during initial server warm-up.
    let first_stats_run_at = Utc::now()
        + ChronoDuration::minutes(
            app_config
                .background_jobs
                .catalog_availability_stats
                .startup_delay_minutes as i64,
        );
    if let Err(e) = server_store.update_schedule_state(&server_store::JobScheduleState {
        job_id: "catalog_availability_stats".to_string(),
        next_run_at: first_stats_run_at,
        last_run_at: None,
    }) {
        error!(
            "Failed to set initial schedule for catalog availability stats job: {}",
            e
        );
    }

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

    // NOTE: Download manager disabled for Spotify schema (catalog is read-only)

    // Spawn background task for storage metrics updates
    let db_dir_for_metrics = app_config.db_dir.clone();
    let media_path_for_metrics = app_config.media_path.clone();
    tokio::spawn(async move {
        // Update storage metrics immediately at startup
        metrics::update_storage_metrics(&db_dir_for_metrics, &media_path_for_metrics);
        info!("Storage metrics initialized");

        // Then update periodically (every 15 minutes)
        let mut interval = tokio::time::interval(Duration::from_secs(15 * 60));
        loop {
            interval.tick().await;
            metrics::update_storage_metrics(&db_dir_for_metrics, &media_path_for_metrics);
        }
    });

    info!("Ready to serve at port {}!", app_config.port);
    info!("Metrics available at port {}!", app_config.metrics_port);

    // Run HTTP server and job scheduler concurrently
    tokio::select! {
        result = run_server(
            catalog_store,
            guarded_search_vault,
            user_store,
            user_manager,
            app_config.logging_level.clone(),
            app_config.port,
            app_config.metrics_port,
            app_config.content_cache_age_sec,
            app_config.frontend_dir_path.clone(),
            Some(scheduler_handle),
            server_store,
            oidc_config,
            app_config.search.streaming.clone(),
            app_config.download_manager.clone(),
            app_config.db_dir.clone(),
            app_config.media_path.clone(),
            app_config.agent.clone(),
            app_config.ingestion.clone(),
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
