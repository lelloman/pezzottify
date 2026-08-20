//! Domain-oriented HTTP route composition.
//!
//! Keeping route policy here makes the exposed API, rate limits, and authorization
//! boundaries reviewable without mixing them into application initialization.

use super::*;
use governor::middleware::NoOpMiddleware;
use tower_governor::governor::GovernorConfig;

type UserRateLimit = Arc<GovernorConfig<UserOrIpKeyExtractor, NoOpMiddleware>>;
type AnalyticsRateLimit = Arc<GovernorConfig<AnalyticsDeviceKeyExtractor, NoOpMiddleware>>;

pub(super) struct RouteLimits {
    pub(super) stream: UserRateLimit,
    pub(super) content_read: UserRateLimit,
    pub(super) write: UserRateLimit,
    pub(super) user_content_read: UserRateLimit,
    pub(super) search: UserRateLimit,
    pub(super) analytics_device: AnalyticsRateLimit,
}

impl RouteLimits {
    pub(super) fn new() -> Self {
        Self {
            stream: user_limit(STREAM_PER_MINUTE),
            content_read: user_limit(CONTENT_READ_PER_MINUTE),
            write: user_limit(WRITE_PER_MINUTE),
            user_content_read: user_limit(CONTENT_READ_PER_MINUTE),
            search: user_limit(SEARCH_PER_MINUTE),
            analytics_device: Arc::new(
                GovernorConfigBuilder::default()
                    .per_millisecond(
                        60000_u64.saturating_div(u64::from(ANALYTICS_PER_DEVICE_PER_MINUTE)),
                    )
                    .burst_size(ANALYTICS_PER_DEVICE_PER_MINUTE)
                    .key_extractor(AnalyticsDeviceKeyExtractor)
                    .finish()
                    .expect("valid analytics rate limiter"),
            ),
        }
    }
}

fn user_limit(requests_per_minute: u32) -> UserRateLimit {
    Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(60000_u64.saturating_div(u64::from(requests_per_minute)))
            .burst_size(requests_per_minute)
            .key_extractor(UserOrIpKeyExtractor)
            .finish()
            .expect("valid user rate limiter"),
    )
}

pub(super) fn content_read_routes(
    state: &ServerState,
    content_cache_age_sec: usize,
    limits: &RouteLimits,
) -> Router {
    let stream_routes: Router = Router::new()
        .route("/stream/{id}", get(stream_track))
        .layer(GovernorLayer::new(limits.stream.clone()))
        .with_state(state.clone());

    let cacheable_catalog_routes: Router<ServerState> = Router::new()
        .route("/album/{id}", get(get_album))
        .route("/album/{id}/resolved", get(get_resolved_album))
        .route("/artist/{id}", get(get_artist))
        .route("/artist/{id}/discography", get(get_artist_discography))
        .route("/track/{id}", get(get_track))
        .route("/track/{id}/resolved", get(get_resolved_track))
        .route("/image/{id}", get(get_image))
        .route("/catalog/stats", get(get_catalog_stats_snapshot))
        .route("/genres", get(get_genres))
        .route("/genre/{name}/tracks", get(get_genre_tracks))
        .layer(middleware::from_fn_with_state(
            content_cache_age_sec,
            http_cache,
        ));

    let catalog_routes: Router = Router::new()
        .route("/whatsnew", get(get_whats_new))
        .route("/popular", get(get_popular_content))
        .route("/featured/albums", get(get_featured_albums))
        .route("/batch", post(post_batch_content))
        .route("/genre/{name}/radio", get(get_genre_radio))
        .merge(cacheable_catalog_routes)
        .merge(show_public_routes())
        .merge(embeddings::read_routes())
        .merge(recommendation_routes())
        .layer(GovernorLayer::new(limits.content_read.clone()))
        .with_state(state.clone());

    let search_routes =
        make_search_routes(state.clone()).layer(GovernorLayer::new(limits.search.clone()));

    let protected_content =
        stream_routes
            .merge(catalog_routes)
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_access_catalog,
            ));

    protected_content.merge(search_routes)
}

pub(super) fn liked_content_routes(state: &ServerState, limits: &RouteLimits) -> Router {
    let read_routes: Router = Router::new()
        .route("/liked/{content_type}", get(get_user_liked_content))
        .layer(GovernorLayer::new(limits.user_content_read.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_like_content,
        ))
        .with_state(state.clone());

    let write_routes: Router = Router::new()
        .route(
            "/liked/{content_type}/{content_id}",
            post(add_user_liked_content),
        )
        .route(
            "/liked/{content_type}/{content_id}",
            delete(delete_user_liked_content),
        )
        .layer(GovernorLayer::new(limits.write.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_like_content,
        ))
        .with_state(state.clone());

    read_routes.merge(write_routes)
}

pub(super) fn playlist_routes(state: &ServerState, limits: &RouteLimits) -> Router {
    let read_routes: Router = Router::new()
        .route("/playlist/{id}", get(get_playlist))
        .route("/playlists", get(get_user_playlists))
        .layer(GovernorLayer::new(limits.user_content_read.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_own_playlists,
        ))
        .with_state(state.clone());

    let write_routes: Router = Router::new()
        .route("/playlist", post(post_playlist))
        .route("/playlist/{id}", put(put_playlist))
        .route("/playlist/{id}", delete(delete_playlist))
        .route("/playlist/{id}/add", put(add_playlist_tracks))
        .route("/playlist/{id}/remove", put(remove_tracks_from_playlist))
        .layer(GovernorLayer::new(limits.write.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_own_playlists,
        ))
        .with_state(state.clone());

    read_routes.merge(write_routes)
}

pub(super) fn user_support_routes(state: &ServerState, limits: &RouteLimits) -> Router {
    let listening_write: Router = Router::new()
        .route("/listening", post(post_listening_event))
        .route("/impression", post(post_impression))
        .layer(GovernorLayer::new(limits.analytics_device.clone()))
        .layer(GovernorLayer::new(limits.write.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .with_state(state.clone());
    let listening_read: Router = Router::new()
        .route("/listening/summary", get(get_user_listening_summary))
        .route("/listening/history", get(get_user_listening_history))
        .route("/listening/events", get(get_user_listening_events))
        .layer(GovernorLayer::new(limits.user_content_read.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .with_state(state.clone());

    let settings: Router = Router::new()
        .route("/settings", get(get_user_settings))
        .route("/settings", put(update_user_settings))
        .layer(GovernorLayer::new(limits.write.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .with_state(state.clone());

    let device_read: Router = Router::new()
        .route("/devices", get(get_user_devices))
        .layer(GovernorLayer::new(limits.user_content_read.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .with_state(state.clone());
    let device_write: Router = Router::new()
        .route(
            "/devices/{device_id}/share_policy",
            put(put_device_share_policy),
        )
        .layer(GovernorLayer::new(limits.write.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .with_state(state.clone());

    let notifications: Router = Router::new()
        .route("/notifications/{id}/read", post(mark_notification_read))
        .layer(GovernorLayer::new(limits.write.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .with_state(state.clone());

    let bug_reports: Router = Router::new()
        .route("/bug-report", post(submit_bug_report))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(GovernorLayer::new(limits.write.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_report_bug,
        ))
        .with_state(state.clone());

    listening_read
        .merge(listening_write)
        .merge(settings)
        .merge(device_read.merge(device_write))
        .merge(notifications)
        .merge(bug_reports)
}

pub(super) fn sync_routes(state: &ServerState, limits: &RouteLimits) -> Router {
    Router::new()
        .route("/state", get(get_sync_state))
        .route("/events", get(get_sync_events))
        .route("/catalog", get(get_catalog_sync))
        .layer(GovernorLayer::new(limits.user_content_read.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_catalog,
        ))
        .with_state(state.clone())
}

pub(super) fn catalog_write_routes(state: &ServerState, limits: &RouteLimits) -> Router {
    Router::new()
        .route("/artist", post(create_artist))
        .route("/artist/{id}", put(update_artist))
        .route("/artist/{id}", delete(delete_artist))
        .route("/album", post(create_album))
        .route("/album/{id}", put(update_album))
        .route("/album/{id}", delete(delete_album))
        .route("/track", post(create_track))
        .route("/track/{id}", put(update_track))
        .route("/track/{id}", delete(delete_track))
        .route("/image", post(create_image))
        .route("/image/{id}", put(update_image))
        .route("/image/{id}", delete(delete_image))
        .merge(embeddings::write_routes())
        .merge(show_admin_routes())
        .layer(GovernorLayer::new(limits.write.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_edit_catalog,
        ))
        .with_state(state.clone())
}

pub(super) fn admin_routes(state: &ServerState, limits: &RouteLimits) -> Router {
    let operations: Router = Router::new()
        .route("/reboot", post(reboot_server))
        .route("/backup/prepare", post(admin_prepare_backup))
        .route("/storage", get(admin_get_storage_report))
        .route("/jobs", get(admin_list_jobs))
        .route("/jobs/audit", get(admin_get_job_audit_log))
        .route("/jobs/{job_id}", get(admin_get_job))
        .route("/jobs/{job_id}/trigger", post(admin_trigger_job))
        .route("/jobs/{job_id}/cancel", post(admin_cancel_job))
        .route("/jobs/{job_id}/history", get(admin_get_job_history))
        .route("/jobs/{job_id}/audit", get(admin_get_job_audit_log_by_job))
        .route(
            "/embeddings/coverage",
            get(admin_get_audio_embedding_coverage),
        )
        .route("/bug-reports", get(admin_list_bug_reports))
        .route("/bug-report/{id}", get(admin_get_bug_report))
        .route("/bug-report/{id}", delete(admin_delete_bug_report))
        .layer(GovernorLayer::new(limits.write.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_server_admin,
        ))
        .with_state(state.clone());

    let users: Router = Router::new()
        .route("/users", get(admin_get_users))
        .route("/users", post(admin_create_user))
        .route("/users/{user_handle}", delete(admin_delete_user))
        .route("/users/{user_handle}/roles", get(admin_get_user_roles))
        .route("/users/{user_handle}/roles", post(admin_add_user_role))
        .route(
            "/users/{user_handle}/roles/{role}",
            delete(admin_remove_user_role),
        )
        .route(
            "/users/{user_handle}/permissions",
            get(admin_get_user_permissions),
        )
        .route(
            "/users/{user_handle}/permissions",
            post(admin_add_user_extra_permission),
        )
        .route(
            "/permissions/{permission_id}",
            delete(admin_remove_extra_permission),
        )
        .route(
            "/users/{user_handle}/credentials",
            get(admin_get_user_credentials_status),
        )
        .route(
            "/users/{user_handle}/password",
            put(admin_set_user_password),
        )
        .route(
            "/users/{user_handle}/password",
            delete(admin_delete_user_password),
        )
        .route("/bandwidth/summary", get(admin_get_bandwidth_summary))
        .route("/bandwidth/usage", get(admin_get_bandwidth_usage))
        .route(
            "/bandwidth/users/{user_handle}/summary",
            get(admin_get_user_bandwidth_summary),
        )
        .route(
            "/bandwidth/users/{user_handle}/usage",
            get(admin_get_user_bandwidth_usage),
        )
        .layer(GovernorLayer::new(limits.write.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_manage_permissions,
        ))
        .with_state(state.clone());

    let analytics: Router = Router::new()
        .route("/listening/daily", get(admin_get_daily_listening_stats))
        .route("/listening/top-tracks", get(admin_get_top_tracks))
        .route(
            "/listening/track/{track_id}",
            get(admin_get_track_listening_stats),
        )
        .route(
            "/listening/users/{user_handle}/summary",
            get(admin_get_user_listening_summary),
        )
        .route("/online-users", get(admin_get_online_users))
        .route("/playback/sessions", get(admin_get_playback_sessions))
        .layer(GovernorLayer::new(limits.write.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_view_analytics,
        ))
        .with_state(state.clone());

    let changelog: Router = Router::new()
        .route("/changelog/batch", post(admin_create_changelog_batch))
        .route("/changelog/batches", get(admin_list_changelog_batches))
        .route(
            "/changelog/batch/{batch_id}",
            get(admin_get_changelog_batch),
        )
        .route(
            "/changelog/batch/{batch_id}/close",
            post(admin_close_changelog_batch),
        )
        .route(
            "/changelog/batch/{batch_id}",
            delete(admin_delete_changelog_batch),
        )
        .route(
            "/changelog/batch/{batch_id}/changes",
            get(admin_get_changelog_batch_changes),
        )
        .route(
            "/changelog/entity/{entity_type}/{entity_id}",
            get(admin_get_changelog_entity_history),
        )
        .layer(GovernorLayer::new(limits.write.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_edit_catalog,
        ))
        .with_state(state.clone());

    let search = make_search_admin_routes(state.clone()).route_layer(
        middleware::from_fn_with_state(state.clone(), require_server_admin),
    );

    operations
        .merge(users)
        .merge(analytics)
        .merge(changelog)
        .merge(search)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn assemble_app(
    state: &ServerState,
    config: &ServerConfig,
    auth_routes: Router,
    content_routes: Router,
    user_routes: Router,
    admin_routes: Router,
    sync_routes: Router,
) -> Router {
    let download_routes = super::super::download_routes()
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_request_content,
        ))
        .with_state(state.clone());

    let ingestion_routes = super::super::ingestion_routes()
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_edit_catalog,
        ))
        .with_state(state.clone());

    let home_router = match config.frontend_dir_path.as_ref() {
        Some(frontend_path) => {
            let index_path = std::path::Path::new(frontend_path).join("index.html");
            let static_files_service = ServeDir::new(frontend_path)
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new(index_path));
            Router::new().fallback_service(static_files_service)
        }
        None => Router::new()
            .route("/", get(home))
            .with_state(state.clone()),
    };

    let ws_routes = Router::new()
        .route("/ws", get(super::super::websocket::ws_handler))
        .with_state(state.clone());
    let mcp_routes = Router::new()
        .route("/mcp", get(crate::mcp::handler::mcp_handler))
        .with_state(state.clone());

    let api_routes = Router::new()
        .nest("/v1/auth", auth_routes)
        .nest("/v1/content", content_routes)
        .nest("/v1/user", user_routes)
        .nest("/v1/admin", admin_routes)
        .nest("/v1/sync", sync_routes)
        .nest("/v1/download", download_routes)
        .nest("/v1/ingestion", ingestion_routes)
        .nest("/v1", ws_routes)
        .nest("/v1", mcp_routes);

    let mut app = home_router.merge(api_routes);

    #[cfg(feature = "slowdown")]
    {
        app = app.layer(middleware::from_fn(slowdown_request));
    }

    let global_rate_limit = user_limit(GLOBAL_PER_MINUTE);
    app = app.layer(GovernorLayer::new(global_rate_limit));
    app = app.layer(middleware::from_fn_with_state(
        state.clone(),
        extract_user_id_for_rate_limit,
    ));
    app = app.layer(middleware::from_fn_with_state(state.clone(), require_csrf));
    app = app.layer(middleware::from_fn_with_state(state.clone(), log_requests));
    app.layer(middleware::from_fn(http_api_no_store))
}

pub(super) fn auth_routes(state: &ServerState) -> Router {
    // Login attempts are protected by a short burst bucket and a slower sustained
    // bucket. Peer IP is used directly: forwarded headers are intentionally ignored
    // unless a future deployment explicitly configures trusted proxies.
    let per_minute = state.config.login_rate_limit_per_minute;
    let sustained_replenish_millis =
        3_600_000_u64.saturating_div(u64::from(state.config.login_rate_limit_per_hour));
    let login_ip_burst_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(60000_u64.saturating_div(u64::from(per_minute)))
            .burst_size(per_minute)
            .key_extractor(IpKeyExtractor)
            .finish()
            .expect("valid login IP burst limiter"),
    );
    let login_ip_sustained_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(sustained_replenish_millis)
            .burst_size(per_minute)
            .key_extractor(IpKeyExtractor)
            .finish()
            .expect("valid login IP sustained limiter"),
    );
    let login_account_burst_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(60000_u64.saturating_div(u64::from(per_minute)))
            .burst_size(per_minute)
            .key_extractor(LoginAccountKeyExtractor)
            .finish()
            .expect("valid login account burst limiter"),
    );
    let login_account_sustained_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(sustained_replenish_millis)
            .burst_size(per_minute)
            .key_extractor(LoginAccountKeyExtractor)
            .finish()
            .expect("valid login account sustained limiter"),
    );

    // Governor's keyed state store needs periodic maintenance when clients can
    // continuously introduce new IP/account keys.
    let ip_burst_limiter = login_ip_burst_limit.limiter().clone();
    let ip_sustained_limiter = login_ip_sustained_limit.limiter().clone();
    let account_burst_limiter = login_account_burst_limit.limiter().clone();
    let account_sustained_limiter = login_account_sustained_limit.limiter().clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(600));
        interval.tick().await;
        loop {
            interval.tick().await;
            ip_burst_limiter.retain_recent();
            ip_sustained_limiter.retain_recent();
            account_burst_limiter.retain_recent();
            account_sustained_limiter.retain_recent();
        }
    });

    let password_login_routes: Router = Router::new()
        .route("/login", post(login))
        .layer(GovernorLayer::new(login_account_burst_limit))
        .layer(GovernorLayer::new(login_account_sustained_limit))
        // Axum layers execute bottom-to-top: insert the account key before its limiters.
        .layer(middleware::from_fn(extract_login_account_for_rate_limit))
        .layer(GovernorLayer::new(login_ip_burst_limit.clone()))
        .layer(GovernorLayer::new(login_ip_sustained_limit.clone()))
        .with_state(state.clone());

    let oidc_login_routes: Router = Router::new()
        .route("/oidc/login", get(oidc_login))
        .route("/oidc/callback", get(oidc_callback))
        .layer(GovernorLayer::new(login_ip_burst_limit))
        .layer(GovernorLayer::new(login_ip_sustained_limit))
        .with_state(state.clone());

    let authenticated_routes: Router = Router::new()
        .route("/logout", post(logout))
        .route("/session", get(get_session))
        .route("/challenge", get(get_challenge))
        .route("/challenge", post(post_challenge))
        .with_state(state.clone());

    password_login_routes
        .merge(oidc_login_routes)
        .merge(authenticated_routes)
}
