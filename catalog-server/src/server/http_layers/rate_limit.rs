//! Rate limiting middleware using tower-governor
//!
//! Implements both per-minute and per-hour rate limits with different configurations
//! for different route groups. Uses IP-based limiting for login endpoints and
//! user-based limiting for authenticated endpoints.

use axum::{
    body::Body,
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{net::SocketAddr, sync::Arc};
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::KeyExtractor, GovernorError,
};
use tracing::warn;

// ============================================================================
// Rate Limit Constants (per minute)
// ============================================================================

/// Login attempts per minute per IP (strict - prevents brute force)
pub const LOGIN_PER_MINUTE: u32 = 10;

/// Global requests per minute per user (prevents runaway bugs)
pub const GLOBAL_PER_MINUTE: u32 = 1000;

/// Search requests per minute per user (expensive operation)
pub const SEARCH_PER_MINUTE: u32 = 100;

/// Content read requests per minute per user (catalog browsing)
pub const CONTENT_READ_PER_MINUTE: u32 = 500;

/// Stream requests per minute per user (prevents rapid skipping)
pub const STREAM_PER_MINUTE: u32 = 100;

/// Write operations per minute per user (playlists, likes)
pub const WRITE_PER_MINUTE: u32 = 60;

// ============================================================================
// Rate Limit Constants (per hour)
// ============================================================================

/// Login attempts per hour per IP
pub const LOGIN_PER_HOUR: u32 = 100;

/// Global requests per hour per user
pub const GLOBAL_PER_HOUR: u32 = 50000;

/// Search requests per hour per user
pub const SEARCH_PER_HOUR: u32 = 5000;

/// Content read requests per hour per user
pub const CONTENT_READ_PER_HOUR: u32 = 25000;

/// Stream requests per hour per user
pub const STREAM_PER_HOUR: u32 = 5000;

/// Write operations per hour per user
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
pub fn rate_limit_error_handler(err: GovernorError, req: Request<Body>) -> Response {
    match err {
        GovernorError::TooManyRequests { .. } => {
            // Extract context for logging
            let path = req.uri().path();
            let method = req.method().as_str();

            // Try to extract user_id or IP for logging
            let identifier = if let Some(user_id) = req.extensions().get::<usize>() {
                format!("user_id={}", user_id)
            } else if let Some(ConnectInfo(addr)) = req.extensions().get::<ConnectInfo<SocketAddr>>()
            {
                format!("ip={}", addr.ip())
            } else {
                "unknown".to_string()
            };

            // Log rate limit violation
            warn!(
                "Rate limit exceeded: {} {} {}",
                method, path, identifier
            );

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
