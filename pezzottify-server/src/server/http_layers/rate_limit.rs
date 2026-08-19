//! Rate limiting middleware using tower-governor
//!
//! Login endpoints have burst and sustained limits. Password login is limited by
//! both peer IP and account handle; other route groups use user-based burst limits.
#![allow(dead_code)]

use crate::server::metrics::{record_rate_limit_hit, request_route_label};
use axum::{
    body::{to_bytes, Body},
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::net::{IpAddr, SocketAddr};
use tower_governor::{key_extractor::KeyExtractor, GovernorError};
use tracing::warn;

// ============================================================================
// Rate Limit Constants (per minute)
// ============================================================================

/// Login attempts per minute per IP (strict - prevents brute force)
pub const LOGIN_PER_MINUTE: u32 = 10;

/// Maximum size buffered to obtain the account key before password login.
const MAX_LOGIN_BODY_LENGTH: usize = 16 * 1024;

/// Global requests per minute per user (prevents runaway bugs)
pub const GLOBAL_PER_MINUTE: u32 = 5000;

/// Search requests per minute per user (expensive operation)
pub const SEARCH_PER_MINUTE: u32 = 100;

/// Content read requests per minute per user (catalog browsing)
pub const CONTENT_READ_PER_MINUTE: u32 = 2000;

/// Stream requests per minute per user (prevents rapid skipping)
pub const STREAM_PER_MINUTE: u32 = 200;

/// Write operations per minute per user (playlists, likes)
pub const WRITE_PER_MINUTE: u32 = 60;

/// Analytics events per minute per device. The existing write limiter separately
/// enforces the per-user budget for the same routes.
pub const ANALYTICS_PER_DEVICE_PER_MINUTE: u32 = 30;

// ============================================================================
// Rate Limit Constants (per hour)
// ============================================================================

/// Sustained login-attempt refill rate per hour.
pub const LOGIN_PER_HOUR: u32 = 100;

/// Replenishment interval for the sustained login bucket. The bucket allows the
/// same initial burst as the per-minute limiter, then replenishes at 100/hour.
pub const LOGIN_SUSTAINED_REPLENISH_MILLIS: u64 = 3_600_000_u64 / LOGIN_PER_HOUR as u64;

// ============================================================================
// Key Extractors
// ============================================================================

/// Extracts IP address from ConnectInfo for IP-based rate limiting
#[derive(Clone)]
pub struct IpKeyExtractor;

impl KeyExtractor for IpKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        req.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| addr.ip())
            .ok_or(GovernorError::UnableToExtractKey)
    }
}

/// Extracts a privacy-preserving account key inserted by
/// [`extract_login_account_for_rate_limit`].
#[derive(Clone)]
pub struct LoginAccountKeyExtractor;

#[derive(Clone, Copy)]
struct LoginAccountKey([u8; 32]);

impl KeyExtractor for LoginAccountKeyExtractor {
    type Key = [u8; 32];

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        req.extensions()
            .get::<LoginAccountKey>()
            .map(|key| key.0)
            .ok_or(GovernorError::UnableToExtractKey)
    }
}

#[derive(Deserialize)]
struct LoginAccountBody {
    user_handle: String,
}

fn login_account_key(bytes: &[u8]) -> LoginAccountKey {
    // Malformed login requests share a bucket. Valid handles deliberately retain
    // their exact form because account lookup is case-sensitive.
    let handle = serde_json::from_slice::<LoginAccountBody>(bytes)
        .map(|body| body.user_handle)
        .unwrap_or_default();
    LoginAccountKey(Sha256::digest(handle.as_bytes()).into())
}

/// Buffers the small password-login body once, inserts its hashed account key for
/// the account limiter, and restores the body for Axum's JSON extractor.
pub async fn extract_login_account_for_rate_limit(
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let body = std::mem::replace(request.body_mut(), Body::empty());
    let bytes = match to_bytes(body, MAX_LOGIN_BODY_LENGTH).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };

    request.extensions_mut().insert(login_account_key(&bytes));
    *request.body_mut() = Body::from(bytes);
    next.run(request).await
}

/// Extracts user ID from session for user-based rate limiting
/// Falls back to IP if no session exists
#[derive(Clone)]
pub struct UserOrIpKeyExtractor;

impl KeyExtractor for UserOrIpKeyExtractor {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        // Try to get user ID from session stored in extensions
        // The session middleware should have already extracted and validated the session
        if let Some(user_id) = req.extensions().get::<usize>() {
            return Ok(format!("user:{}", user_id));
        }

        // Fall back to IP address
        if let Some(ConnectInfo(addr)) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
            return Ok(format!("ip:{}", addr.ip()));
        }

        Err(GovernorError::UnableToExtractKey)
    }
}

#[derive(Clone)]
pub struct AnalyticsDeviceKeyExtractor;

#[derive(Clone, Copy)]
struct AnalyticsRateLimitIdentity {
    user_id: usize,
    device_id: Option<usize>,
}

impl KeyExtractor for AnalyticsDeviceKeyExtractor {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        req.extensions()
            .get::<AnalyticsRateLimitIdentity>()
            .map(|identity| match identity.device_id {
                Some(device_id) => {
                    format!("user:{}:device:{}", identity.user_id, device_id)
                }
                None => format!("user:{}:device:none", identity.user_id),
            })
            .ok_or(GovernorError::UnableToExtractKey)
    }
}

// ============================================================================
// Error Handler
// ============================================================================

/// Custom error handler that logs rate limit violations and returns appropriate response
#[allow(dead_code)]
pub fn rate_limit_error_handler(err: GovernorError, req: Request<Body>) -> Response {
    match err {
        GovernorError::TooManyRequests { .. } => {
            // Extract context for logging
            let path = req.uri().path();
            let method = req.method().as_str();
            let metric_route = request_route_label(req.extensions());

            // Try to extract user_id or IP for logging and metrics
            let (identifier, identifier_type) =
                if let Some(user_id) = req.extensions().get::<usize>() {
                    (format!("user_id={}", user_id), "user")
                } else if let Some(ConnectInfo(addr)) =
                    req.extensions().get::<ConnectInfo<SocketAddr>>()
                {
                    (format!("ip={}", addr.ip()), "ip")
                } else {
                    ("unknown".to_string(), "unknown")
                };

            // Log rate limit violation
            warn!("Rate limit exceeded: {} {} {}", method, path, identifier);

            // Record metric for Prometheus
            record_rate_limit_hit(metric_route, identifier_type);

            // Return 429 with simple message
            StatusCode::TOO_MANY_REQUESTS.into_response()
        }
        _ => {
            warn!("Rate limiting error: {:?}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ============================================================================
// Middleware for User ID Extraction
// ============================================================================

/// Middleware to extract user_id from Session and add it to request extensions
/// This allows the rate limiter to use user_id as the key
pub async fn extract_user_id_for_rate_limit(
    session: Option<crate::server::session::Session>,
    mut request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    if let Some(session) = session {
        request.extensions_mut().insert(session.user_id);
        request.extensions_mut().insert(AnalyticsRateLimitIdentity {
            user_id: session.user_id,
            device_id: session.device_id,
        });
    }
    next.run(request).await
}

// Note: Configuration builders are inlined in server.rs due to complex type signatures
// The constants above define the rate limits used throughout the application

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::Method, middleware, routing::post, Router};
    use std::{
        net::{IpAddr, Ipv4Addr},
        sync::Arc,
    };
    use tower::ServiceExt;
    use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

    fn create_test_request() -> Request<Body> {
        Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap()
    }

    #[test]
    fn test_rate_limit_constants_per_minute() {
        // Verify per-minute rate limits are reasonable
        assert_eq!(LOGIN_PER_MINUTE, 10);
        assert_eq!(GLOBAL_PER_MINUTE, 5000);
        assert_eq!(SEARCH_PER_MINUTE, 100);
        assert_eq!(CONTENT_READ_PER_MINUTE, 2000);
        assert_eq!(STREAM_PER_MINUTE, 200);
        assert_eq!(WRITE_PER_MINUTE, 60);

        // Verify ordering makes sense (global should be highest)
        const { assert!(GLOBAL_PER_MINUTE > CONTENT_READ_PER_MINUTE) };
        const { assert!(CONTENT_READ_PER_MINUTE > SEARCH_PER_MINUTE) };
        const { assert!(STREAM_PER_MINUTE > SEARCH_PER_MINUTE) };
        const { assert!(SEARCH_PER_MINUTE > WRITE_PER_MINUTE) };
    }

    #[test]
    fn test_rate_limit_constants_per_hour() {
        // Verify sustained login throttling is configured at 100 attempts/hour.
        assert_eq!(LOGIN_PER_HOUR, 100);
        assert_eq!(LOGIN_SUSTAINED_REPLENISH_MILLIS, 36_000);
    }

    #[test]
    fn test_rate_limit_consistency_minute_vs_hour() {
        // Login allows a short burst but replenishes much more slowly in its
        // sustained bucket.
        assert_eq!(LOGIN_PER_MINUTE, 10);
        assert_eq!(LOGIN_PER_HOUR, 100);
        const { assert!(LOGIN_PER_HOUR < LOGIN_PER_MINUTE * 60) };
    }

    #[test]
    fn test_ip_key_extractor_extracts_ip_without_port() {
        let extractor = IpKeyExtractor;
        let mut request = create_test_request();

        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        request.extensions_mut().insert(ConnectInfo(socket_addr));

        let result = extractor.extract(&request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), socket_addr.ip());
    }

    #[test]
    fn test_ip_key_extractor_returns_error_when_no_connect_info() {
        let extractor = IpKeyExtractor;
        let request = create_test_request();

        let result = extractor.extract(&request);
        assert!(result.is_err());
        assert!(matches!(result, Err(GovernorError::UnableToExtractKey)));
    }

    #[test]
    fn test_ip_key_extractor_handles_different_ips() {
        let extractor = IpKeyExtractor;

        // Test IPv4
        let mut request1 = create_test_request();
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        request1.extensions_mut().insert(ConnectInfo(addr1));

        // Test different IPv4
        let mut request2 = create_test_request();
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 8080);
        request2.extensions_mut().insert(ConnectInfo(addr2));

        let result1 = extractor.extract(&request1).unwrap();
        let result2 = extractor.extract(&request2).unwrap();

        assert_ne!(result1, result2);
        assert_eq!(result1, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(result2, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
    }

    #[test]
    fn test_ip_key_extractor_same_ip_different_ports() {
        let extractor = IpKeyExtractor;
        let mut request1 = create_test_request();
        let mut request2 = create_test_request();
        request1
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                8080,
            )));
        request2
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                9090,
            )));

        assert_eq!(
            extractor.extract(&request1).unwrap(),
            extractor.extract(&request2).unwrap()
        );
    }

    #[test]
    fn test_ip_key_extractor_ignores_untrusted_forwarded_headers() {
        let extractor = IpKeyExtractor;
        let mut request = create_test_request();
        let peer_ip = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10));
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::new(peer_ip, 8080)));
        request
            .headers_mut()
            .insert("x-forwarded-for", "203.0.113.99".parse().unwrap());

        assert_eq!(extractor.extract(&request).unwrap(), peer_ip);
    }

    #[test]
    fn test_login_account_key_is_stable_and_does_not_contain_the_handle() {
        let first = login_account_key(br#"{"user_handle":"alice","password":"one"}"#);
        let second = login_account_key(br#"{"user_handle":"alice","password":"two"}"#);
        let other = login_account_key(br#"{"user_handle":"bob","password":"one"}"#);

        assert_eq!(first.0, second.0);
        assert_ne!(first.0, other.0);
        assert!(!format!("{:?}", first.0).contains("alice"));
    }

    #[tokio::test]
    async fn test_ip_limiter_cannot_be_bypassed_by_reconnecting_from_another_port() {
        let config = Arc::new(
            GovernorConfigBuilder::default()
                .per_second(60)
                .burst_size(1)
                .key_extractor(IpKeyExtractor)
                .finish()
                .unwrap(),
        );
        let app = Router::new()
            .route("/login", post(|| async { StatusCode::OK }))
            .layer(GovernorLayer::new(config));

        let mut first = Request::post("/login").body(Body::empty()).unwrap();
        first.extensions_mut().insert(ConnectInfo(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            10_001,
        )));
        let mut reconnect = Request::post("/login").body(Body::empty()).unwrap();
        reconnect
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                10_002,
            )));

        assert_eq!(app.clone().oneshot(first).await.unwrap().status(), 200);
        assert_eq!(app.oneshot(reconnect).await.unwrap().status(), 429);
    }

    #[tokio::test]
    async fn test_account_limiter_applies_across_different_peer_ips() {
        let config = Arc::new(
            GovernorConfigBuilder::default()
                .per_second(60)
                .burst_size(1)
                .key_extractor(LoginAccountKeyExtractor)
                .finish()
                .unwrap(),
        );
        let app = Router::new()
            .route("/login", post(|| async { StatusCode::OK }))
            .layer(GovernorLayer::new(config))
            .layer(middleware::from_fn(extract_login_account_for_rate_limit));

        let login_body = || Body::from(r#"{"user_handle":"alice","password":"sentinel-password"}"#);
        let mut first = Request::post("/login").body(login_body()).unwrap();
        first.extensions_mut().insert(ConnectInfo(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),
            10_001,
        )));
        let mut other_ip = Request::post("/login").body(login_body()).unwrap();
        other_ip
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)),
                10_002,
            )));

        assert_eq!(app.clone().oneshot(first).await.unwrap().status(), 200);
        assert_eq!(app.oneshot(other_ip).await.unwrap().status(), 429);
    }

    #[tokio::test]
    async fn test_account_extraction_restores_body_for_login_handler() {
        async fn handler(axum::Json(body): axum::Json<LoginAccountBody>) -> String {
            body.user_handle
        }

        let app = Router::new()
            .route("/login", post(handler))
            .layer(middleware::from_fn(extract_login_account_for_rate_limit));
        let request = Request::post("/login")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"user_handle":"alice"}"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let body = to_bytes(response.into_body(), 1024).await.unwrap();

        assert_eq!(body, "alice");
    }

    #[tokio::test]
    async fn test_account_extraction_rejects_oversized_login_body() {
        let app = Router::new()
            .route("/login", post(|| async { StatusCode::OK }))
            .layer(middleware::from_fn(extract_login_account_for_rate_limit));
        let request = Request::post("/login")
            .body(Body::from(vec![b'x'; MAX_LOGIN_BODY_LENGTH + 1]))
            .unwrap();

        assert_eq!(app.oneshot(request).await.unwrap().status(), 413);
    }

    #[test]
    fn test_user_or_ip_key_extractor_prefers_user_id() {
        let extractor = UserOrIpKeyExtractor;
        let mut request = create_test_request();

        // Add both user_id and IP
        let user_id = 42usize;
        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        request.extensions_mut().insert(user_id);
        request.extensions_mut().insert(ConnectInfo(socket_addr));

        let result = extractor.extract(&request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "user:42");
    }

    #[test]
    fn analytics_key_is_scoped_to_user_and_device() {
        let extractor = AnalyticsDeviceKeyExtractor;
        let mut request = create_test_request();
        request.extensions_mut().insert(AnalyticsRateLimitIdentity {
            user_id: 42,
            device_id: Some(7),
        });

        assert_eq!(extractor.extract(&request).unwrap(), "user:42:device:7");
    }

    #[test]
    fn test_user_or_ip_key_extractor_falls_back_to_ip() {
        let extractor = UserOrIpKeyExtractor;
        let mut request = create_test_request();

        // Add only IP, no user_id
        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        request.extensions_mut().insert(ConnectInfo(socket_addr));

        let result = extractor.extract(&request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "ip:192.168.1.100");
    }

    #[test]
    fn test_user_or_ip_key_extractor_returns_error_when_no_info() {
        let extractor = UserOrIpKeyExtractor;
        let request = create_test_request();

        let result = extractor.extract(&request);
        assert!(result.is_err());
        assert!(matches!(result, Err(GovernorError::UnableToExtractKey)));
    }

    #[test]
    fn test_user_or_ip_key_extractor_different_users() {
        let extractor = UserOrIpKeyExtractor;

        let mut request1 = create_test_request();
        request1.extensions_mut().insert(1usize);

        let mut request2 = create_test_request();
        request2.extensions_mut().insert(2usize);

        let result1 = extractor.extract(&request1).unwrap();
        let result2 = extractor.extract(&request2).unwrap();

        assert_ne!(result1, result2);
        assert_eq!(result1, "user:1");
        assert_eq!(result2, "user:2");
    }

    #[test]
    fn test_user_or_ip_key_extractor_same_ip_different_ports() {
        let extractor = UserOrIpKeyExtractor;

        let mut request1 = create_test_request();
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        request1.extensions_mut().insert(ConnectInfo(addr1));

        let mut request2 = create_test_request();
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9090);
        request2.extensions_mut().insert(ConnectInfo(addr2));

        let result1 = extractor.extract(&request1).unwrap();
        let result2 = extractor.extract(&request2).unwrap();

        // Should be the same because we only use IP, not port
        assert_eq!(result1, result2);
        assert_eq!(result1, "ip:127.0.0.1");
    }

    #[test]
    fn test_rate_limit_error_handler_too_many_requests() {
        let err = GovernorError::TooManyRequests {
            wait_time: 30,
            headers: Default::default(),
        };
        let request = create_test_request();

        let response = rate_limit_error_handler(err, request);

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_rate_limit_error_handler_other_errors() {
        let err = GovernorError::UnableToExtractKey;
        let request = create_test_request();

        let response = rate_limit_error_handler(err, request);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_rate_limit_error_handler_with_user_id() {
        let err = GovernorError::TooManyRequests {
            wait_time: 30,
            headers: Default::default(),
        };
        let mut request = create_test_request();
        request.extensions_mut().insert(123usize);

        let response = rate_limit_error_handler(err, request);

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_rate_limit_error_handler_with_ip() {
        let err = GovernorError::TooManyRequests {
            wait_time: 30,
            headers: Default::default(),
        };
        let mut request = create_test_request();
        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 42)), 8080);
        request.extensions_mut().insert(ConnectInfo(socket_addr));

        let response = rate_limit_error_handler(err, request);

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_ip_key_extractor_is_clone() {
        let extractor = IpKeyExtractor;
        let _cloned = extractor.clone();
        // Test passes if it compiles
    }

    #[test]
    fn test_user_or_ip_key_extractor_is_clone() {
        let extractor = UserOrIpKeyExtractor;
        let _cloned = extractor.clone();
        // Test passes if it compiles
    }
}
