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

pub(super) fn auth_routes(state: &ServerState) -> Router {
    // Login attempts are protected by a short burst bucket and a slower sustained
    // bucket. Peer IP is used directly: forwarded headers are intentionally ignored
    // unless a future deployment explicitly configures trusted proxies.
    let login_ip_burst_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(60000_u64.saturating_div(u64::from(LOGIN_PER_MINUTE)))
            .burst_size(LOGIN_PER_MINUTE)
            .key_extractor(IpKeyExtractor)
            .finish()
            .expect("valid login IP burst limiter"),
    );
    let login_ip_sustained_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(LOGIN_SUSTAINED_REPLENISH_MILLIS)
            .burst_size(LOGIN_PER_MINUTE)
            .key_extractor(IpKeyExtractor)
            .finish()
            .expect("valid login IP sustained limiter"),
    );
    let login_account_burst_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(60000_u64.saturating_div(u64::from(LOGIN_PER_MINUTE)))
            .burst_size(LOGIN_PER_MINUTE)
            .key_extractor(LoginAccountKeyExtractor)
            .finish()
            .expect("valid login account burst limiter"),
    );
    let login_account_sustained_limit = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(LOGIN_SUSTAINED_REPLENISH_MILLIS)
            .burst_size(LOGIN_PER_MINUTE)
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
