#[allow(clippy::too_many_arguments)]
pub async fn make_app(
    config: ServerConfig,
    catalog_store: Arc<dyn CatalogStore>,
    search_vault: super::state::GuardedSearchVault,
    user_store: Arc<dyn FullUserStore>,
    user_manager: GuardedUserManager,
    scheduler_handle: Option<SchedulerHandle>,
    server_store: Arc<dyn crate::server_store::ServerStore>,
    oidc_config: Option<crate::config::OidcConfig>,
    db_registry: Arc<crate::backup::DbRegistry>,
    enrichment_store: OptionalEnrichmentStore,
) -> Result<Router> {
    // Initialize OIDC client if configured
    let oidc_client = match oidc_config {
        Some(cfg) => {
            info!(
                "Initializing OIDC client for provider: {}",
                cfg.provider_url
            );
            match crate::oidc::OidcClient::new(cfg).await {
                Ok(client) => {
                    info!("OIDC client initialized successfully");
                    Some(Arc::new(client))
                }
                Err(e) => {
                    error!(
                        "Failed to initialize OIDC client: {:?}. OIDC login will be disabled.",
                        e
                    );
                    None
                }
            }
        }
        None => {
            info!("OIDC not configured, password-based login only");
            None
        }
    };

    let show_store: Arc<dyn crate::shows::ShowStore> = Arc::new(
        SqliteShowStore::open(config.shows_db_path(), &db_registry)
            .context("Failed to open shows database")?,
    );

    let mut state = ServerState::new_with_guarded_search_vault(
        config.clone(),
        catalog_store.clone(),
        search_vault.clone(),
        user_manager,
        user_store.clone(),
        scheduler_handle,
        server_store,
        show_store,
        db_registry,
        enrichment_store,
    );
    state.oidc_client = oidc_client;

    // Spawn orphaned session checker task
    {
        let playback_manager = state.playback_session_manager.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                playback_manager.check_stale_devices().await;
            }
        });
    }

    // Create organic indexer for on-demand search index growth
    state.organic_indexer = Some(crate::search::OrganicIndexer::new(
        search_vault.clone(),
        catalog_store.clone(),
    ));

    // Initialize download manager if enabled
    if config.download_manager.enabled {
        info!("Initializing download manager...");
        let queue_db_path = config.db_dir.join("download_queue.db");
        match crate::download_manager::SqliteDownloadQueueStore::new(
            &queue_db_path,
            &state.db_registry,
        ) {
            Ok(queue_store) => {
                let manager = Arc::new(crate::download_manager::DownloadManager::new(
                    Arc::new(queue_store),
                    catalog_store.clone(),
                    config.media_path.clone(),
                    config.download_manager.clone(),
                ));

                // Set the search vault for availability updates
                {
                    let manager_clone = manager.clone();
                    let search_vault_clone = search_vault.clone();
                    tokio::spawn(async move {
                        manager_clone.set_search_vault(search_vault_clone).await;
                    });
                }

                // Wire up sync notifier for WebSocket download status updates
                {
                    let sync_notifier =
                        Arc::new(crate::download_manager::DownloadSyncNotifier::new(
                            user_store.clone(),
                            state.ws_connection_manager.clone(),
                            state.server_store.clone(),
                        ));
                    let manager_for_notifier = manager.clone();
                    tokio::spawn(async move {
                        manager_for_notifier.set_sync_notifier(sync_notifier).await;
                    });
                }

                state.download_manager = Some(manager);
                info!("Download manager initialized successfully");
            }
            Err(e) => {
                error!("Failed to initialize download queue store: {:?}", e);
            }
        }
    } else {
        info!("Download manager disabled in config");
    }

    // Initialize Ingestion Manager if enabled
    if config.ingestion.enabled {
        info!("Initializing ingestion manager...");
        let ingestion_db_path = config.ingestion_db_path();
        match crate::ingestion::SqliteIngestionStore::open(&ingestion_db_path, &state.db_registry) {
            Ok(store) => {
                // Parse bitrate from string like "320k" to u32
                let target_bitrate = config
                    .ingestion
                    .output_bitrate
                    .trim_end_matches('k')
                    .trim_end_matches('K')
                    .parse::<u32>()
                    .unwrap_or(320);

                let ingestion_config = crate::ingestion::IngestionManagerConfig {
                    temp_dir: config.ingestion_temp_dir(),
                    media_dir: config.media_path.clone(),
                    max_file_size: config.ingestion.max_upload_size_mb * 1024 * 1024,
                    target_bitrate,
                    auto_match_threshold: config.ingestion.auto_approve_threshold,
                    ..Default::default()
                };

                // Get download manager from state (if available) and cast to trait for ingestion
                let manager_traits = state.download_manager.clone().map(|dm| {
                    let dm_traits: Arc<dyn crate::ingestion::DownloadManagerTrait> =
                        dm as Arc<dyn crate::ingestion::DownloadManagerTrait>;
                    dm_traits
                });

                // Create notifier for WebSocket updates
                let notifier = Arc::new(
                    crate::ingestion::IngestionNotifier::new(state.ws_connection_manager.clone())
                        .with_server_store(state.server_store.clone()),
                );

                // Create notification service for download completion notifications
                let notification_service =
                    Arc::new(crate::notifications::NotificationService::new(
                        user_store.clone(),
                        state.ws_connection_manager.clone(),
                    ));

                let ingestion_manager = Arc::new(
                    crate::ingestion::IngestionManager::new(
                        Arc::new(store),
                        catalog_store.clone(),
                        search_vault.clone(),
                        ingestion_config,
                        manager_traits,
                    )
                    .with_notifier(notifier)
                    .with_notification_service(notification_service),
                );

                // Initialize the manager (creates temp directory)
                if let Err(e) = ingestion_manager.init().await {
                    error!("Failed to initialize ingestion manager: {:?}", e);
                } else {
                    state.ingestion_manager = Some(ingestion_manager);
                    info!("Ingestion manager initialized successfully");
                }
            }
            Err(e) => {
                error!("Failed to open ingestion database: {:?}", e);
            }
        }
    } else {
        debug!("Ingestion not enabled");
    }

    let auth_routes = route_builder::auth_routes(&state);
    let route_limits = route_builder::RouteLimits::new();

    let mut content_routes =
        route_builder::content_read_routes(&state, config.content_cache_age_sec, &route_limits);

    let liked_content_routes = route_builder::liked_content_routes(&state, &route_limits);
    let playlist_routes = route_builder::playlist_routes(&state, &route_limits);

    let user_support_routes = route_builder::user_support_routes(&state, &route_limits);
    let user_routes = liked_content_routes
        .merge(playlist_routes)
        .merge(user_support_routes);

    let sync_routes = route_builder::sync_routes(&state, &route_limits);

    content_routes =
        content_routes.merge(route_builder::catalog_write_routes(&state, &route_limits));

    let admin_routes = route_builder::admin_routes(&state, &route_limits);

    Ok(route_builder::assemble_app(
        &state,
        &config,
        auth_routes,
        content_routes,
        user_routes,
        admin_routes,
        sync_routes,
    ))
}

/// Interval between stale batch checks (10 minutes in seconds)
/// The actual staleness threshold is configured in ChangeLogStore (default 1 hour).
const STALE_BATCH_CHECK_INTERVAL_SECS: u64 = 600;

#[allow(clippy::too_many_arguments)]
pub async fn run_server(
    catalog_store: Arc<dyn CatalogStore>,
    guarded_search_vault: super::state::GuardedSearchVault,
    user_store: Arc<dyn FullUserStore>,
    user_manager: GuardedUserManager,
    requests_logging_level: RequestsLoggingLevel,
    port: u16,
    metrics_port: u16,
    content_cache_age_sec: usize,
    frontend_dir_path: Option<String>,
    secure_session_cookies: bool,
    session_cookie_max_age_secs: u64,
    scheduler_handle: Option<SchedulerHandle>,
    server_store: Arc<dyn crate::server_store::ServerStore>,
    oidc_config: Option<crate::config::OidcConfig>,
    streaming_search: crate::config::StreamingSearchSettings,
    download_manager: crate::config::DownloadManagerSettings,
    db_dir: std::path::PathBuf,
    media_path: std::path::PathBuf,
    agent: crate::config::AgentSettings,
    ingestion: crate::config::IngestionSettings,
    audio_embeddings: Option<crate::config::AudioEmbeddingsSettings>,
    shows: crate::config::ShowsSettings,
    db_registry: Arc<crate::backup::DbRegistry>,
    enrichment_store: OptionalEnrichmentStore,
) -> Result<()> {
    let disable_password_auth = oidc_config
        .as_ref()
        .map(|c| c.disable_password_auth)
        .unwrap_or(false);

    let config = ServerConfig {
        port,
        requests_logging_level,
        content_cache_age_sec,
        frontend_dir_path,
        disable_password_auth,
        secure_session_cookies,
        session_cookie_max_age_secs,
        streaming_search,
        download_manager,
        db_dir,
        media_path,
        agent,
        ingestion,
        shows,
        audio_embeddings,
    };

    let app = make_app(
        config,
        catalog_store.clone(),
        guarded_search_vault.clone(),
        user_store,
        user_manager,
        scheduler_handle,
        server_store,
        oidc_config,
        db_registry,
        enrichment_store,
    )
    .await?;

    // Create a minimal metrics-only server (always HTTP, internal use)
    let metrics_app = Router::new().route("/metrics", get(super::metrics::metrics_handler));
    let metrics_listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", metrics_port))
        .await
        .unwrap();

    // Spawn the stale batch auto-close background task
    let catalog_store_for_bg = catalog_store.clone();
    let search_vault_for_bg = guarded_search_vault.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            STALE_BATCH_CHECK_INTERVAL_SECS,
        ));
        loop {
            interval.tick().await;
            check_and_close_stale_batches(&catalog_store_for_bg, &search_vault_for_bg);
        }
    });

    info!("Starting HTTP server on port {}", port);
    let main_listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    tokio::select! {
        result = axum::serve(
            main_listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        ) => {
            result?;
        }
        result = axum::serve(metrics_listener, metrics_app) => {
            result?;
        }
    }

    Ok(())
}

/// Close stale changelog batches automatically and rebuild search index if any were closed.
/// NOTE: Disabled for Spotify schema - catalog is read-only, no changelog batches.
fn check_and_close_stale_batches(
    _catalog_store: &Arc<dyn CatalogStore>,
    _search_vault: &super::state::GuardedSearchVault,
) {
    // Changelog functionality disabled - Spotify schema is read-only
    // No stale batches to close
}

include!("server_tests.rs");
