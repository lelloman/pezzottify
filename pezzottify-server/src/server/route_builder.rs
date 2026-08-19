//! Domain-oriented HTTP route composition.
//!
//! Keeping route policy here makes the exposed API, rate limits, and authorization
//! boundaries reviewable without mixing them into application initialization.

use super::*;

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
