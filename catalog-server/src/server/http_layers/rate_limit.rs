//! Rate limiting middleware using tower-governor
//!
//! Implements both per-minute and per-hour rate limits with different configurations
//! for different route groups. Uses IP-based limiting for login endpoints and
//! user-based limiting for authenticated endpoints.
#![allow(dead_code)]

use crate::server::metrics::record_rate_limit_hit;
use axum::{
    body::Body,
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;
use tower_governor::{key_extractor::KeyExtractor, GovernorError};
use tracing::warn;

// ============================================================================
// Rate Limit Constants (per minute)
// ============================================================================

/// Login attempts per minute per IP (strict - prevents brute force)
pub const LOGIN_PER_MINUTE: u32 = 10;

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

// ============================================================================
// Rate Limit Constants (per hour)
// ============================================================================

/// Login attempts per hour per IP
#[allow(dead_code)]
pub const LOGIN_PER_HOUR: u32 = 100;

/// Global requests per hour per user
#[allow(dead_code)]
pub const GLOBAL_PER_HOUR: u32 = 10000;

/// Search requests per hour per user
#[allow(dead_code)]
pub const SEARCH_PER_HOUR: u32 = 5000;

/// Content read requests per hour per user
#[allow(dead_code)]
pub const CONTENT_READ_PER_HOUR: u32 = 50000;

/// Stream requests per hour per user
#[allow(dead_code)]
pub const STREAM_PER_HOUR: u32 = 5000;

/// Write operations per hour per user
#[allow(dead_code)]
pub const WRITE_PER_HOUR: u32 = 2000;

// ============================================================================
// Key Extractors
// ============================================================================

/// Extracts IP address from ConnectInfo for IP-based rate limiting
#[derive(Clone)]
pub struct IpKeyExtractor;

impl KeyExtractor for IpKeyExtractor {
    type Key = SocketAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        req.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| *addr)
            .ok_or(GovernorError::UnableToExtractKey)
    }
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
            record_rate_limit_hit(path, identifier_type);

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
    }
    next.run(request).await
}

// Note: Configuration builders are inlined in server.rs due to complex type signatures
// The constants above define the rate limits used throughout the application

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Method;
    use std::net::{IpAddr, Ipv4Addr};

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
        assert_eq!(GLOBAL_PER_MINUTE, 1000);
        assert_eq!(SEARCH_PER_MINUTE, 100);
        assert_eq!(CONTENT_READ_PER_MINUTE, 500);
        assert_eq!(STREAM_PER_MINUTE, 100);
        assert_eq!(WRITE_PER_MINUTE, 60);

        // Verify ordering makes sense (global should be highest)
        assert!(GLOBAL_PER_MINUTE > CONTENT_READ_PER_MINUTE);
        assert!(CONTENT_READ_PER_MINUTE > SEARCH_PER_MINUTE);
        assert!(SEARCH_PER_MINUTE >= STREAM_PER_MINUTE);
        assert!(SEARCH_PER_MINUTE > WRITE_PER_MINUTE);
    }

    #[test]
    fn test_rate_limit_constants_per_hour() {
        // Verify per-hour rate limits are reasonable
        assert_eq!(LOGIN_PER_HOUR, 100);
        assert_eq!(GLOBAL_PER_HOUR, 50000);
        assert_eq!(SEARCH_PER_HOUR, 5000);
        assert_eq!(CONTENT_READ_PER_HOUR, 25000);
        assert_eq!(STREAM_PER_HOUR, 5000);
        assert_eq!(WRITE_PER_HOUR, 2000);

        // Verify ordering
        assert!(GLOBAL_PER_HOUR > CONTENT_READ_PER_HOUR);
        assert!(CONTENT_READ_PER_HOUR > SEARCH_PER_HOUR);
    }

    #[test]
    fn test_rate_limit_consistency_minute_vs_hour() {
        // Some endpoints have intentionally restrictive hourly limits
        // to prevent sustained abuse, even with short burst allowances

        // Login has very low hourly limit to prevent brute force
        assert_eq!(LOGIN_PER_MINUTE, 10);
        assert_eq!(LOGIN_PER_HOUR, 100);
        // Note: 10 per minute would be 600/hour if sustained, but capped at 100

        // Search has restrictive hourly limit
        assert_eq!(SEARCH_PER_MINUTE, 100);
        assert_eq!(SEARCH_PER_HOUR, 5000);
        // Note: 100 per minute would be 6000/hour if sustained, but capped at 5000

        // Write operations have moderate hourly limit
        assert_eq!(WRITE_PER_MINUTE, 60);
        assert_eq!(WRITE_PER_HOUR, 2000);

        // Stream operations
        assert_eq!(STREAM_PER_MINUTE, 100);
        assert_eq!(STREAM_PER_HOUR, 5000);
    }

    #[test]
    fn test_ip_key_extractor_extracts_socket_addr() {
        let extractor = IpKeyExtractor;
        let mut request = create_test_request();

        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        request.extensions_mut().insert(ConnectInfo(socket_addr));

        let result = extractor.extract(&request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), socket_addr);
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
        assert_eq!(result1.ip(), IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(result2.ip(), IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
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
