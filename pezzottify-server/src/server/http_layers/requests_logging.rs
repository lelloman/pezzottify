//! Request logging middleware
#![allow(dead_code)] // Used as middleware

use super::super::state::ServerState;
use crate::server::metrics::{
    categorize_endpoint, record_bandwidth, record_http_request, request_route_label,
};
use axum::extract::State;
use axum::{
    body::Body,
    http::{header::HeaderMap, Request, Response, Uri},
    middleware::Next,
    response::IntoResponse,
};
use chrono::Datelike;
use serde_json::Value;
use std::time::Instant;
use tracing::{debug, error, info};

#[derive(PartialEq, PartialOrd, Clone, Debug, Default, clap::ValueEnum)]
pub enum RequestsLoggingLevel {
    None,
    #[default]
    Path,
    Headers,
    Body,
}

impl std::fmt::Display for RequestsLoggingLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

const MAX_LOGGABLE_BODY_LENGTH: usize = 1024;

// Keep these lists deliberately small. Headers not explicitly known to be safe are not logged.
// In particular, Authorization, Cookie, Set-Cookie, and proxy authentication headers must never
// be added here.
const SAFE_REQUEST_HEADERS: &[&str] = &[
    "accept",
    "accept-encoding",
    "accept-language",
    "content-length",
    "content-type",
    "range",
    "user-agent",
];

const SAFE_RESPONSE_HEADERS: &[&str] = &[
    "accept-ranges",
    "cache-control",
    "content-length",
    "content-range",
    "content-type",
    "etag",
    "last-modified",
];

const REDACTED_VALUE: &str = "[REDACTED]";

enum ContentLengthParseResult {
    Ok(usize),
    No(&'static str),
}

fn parse_content_length(headers: &HeaderMap) -> ContentLengthParseResult {
    let value = match headers.get("content-length") {
        Some(x) => x,
        None => return ContentLengthParseResult::No("Content-length not set."),
    };

    let str_value = match value.to_str() {
        Ok(x) => x,
        Err(_) => {
            return ContentLengthParseResult::No("Could not get Content-length string value.")
        }
    };

    match str_value.parse::<usize>() {
        Ok(x) => ContentLengthParseResult::Ok(x),
        Err(_) => ContentLengthParseResult::No("Could not parse Content-length numeric value."),
    }
}

fn format_safe_headers(headers: &HeaderMap, allowlist: &[&str]) -> String {
    headers
        .iter()
        .filter(|(name, _)| allowlist.contains(&name.as_str()))
        .map(|(name, value)| {
            let value = value.to_str().unwrap_or("<non-UTF-8>");
            format!("    {}: {}", name.as_str(), value)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_authentication_path(path: &str) -> bool {
    path == "/v1/auth"
        || path.starts_with("/v1/auth/")
        || (path.starts_with("/v1/admin/users/")
            && (path.ends_with("/password") || path.ends_with("/credentials")))
}

fn is_sensitive_json_key(key: &str) -> bool {
    let key: String = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect();
    key == "authorization"
        || key == "cookie"
        || key == "setcookie"
        || key == "credential"
        || key == "credentials"
        || key.contains("password")
        || key.contains("secret")
        || key.ends_with("token")
        || key.ends_with("apikey")
}

fn redact_json(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if is_sensitive_json_key(key) {
                    *value = Value::String(REDACTED_VALUE.to_string());
                } else {
                    redact_json(value);
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(redact_json),
        _ => {}
    }
}

fn format_loggable_body(path: &str, headers: &HeaderMap, bytes: &[u8]) -> String {
    if is_authentication_path(path) {
        return "[omitted: authentication endpoint]".to_string();
    }

    let is_json = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
        });

    if !is_json {
        return "[omitted: only JSON bodies are loggable]".to_string();
    }

    let Ok(mut value) = serde_json::from_slice::<Value>(bytes) else {
        return "[omitted: invalid JSON body]".to_string();
    };
    redact_json(&mut value);
    serde_json::to_string(&value).unwrap_or_else(|_| "[omitted: invalid JSON body]".to_string())
}

fn safe_request_target(uri: &Uri) -> &str {
    uri.path()
}

pub async fn log_requests(
    State(state): State<ServerState>,
    mut request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    let level = state.config.requests_logging_level.clone();

    // Extract user_id from request extensions (set by earlier middleware like extract_user_id_for_rate_limit)
    let user_id = request.extensions().get::<usize>().copied();

    let start = Instant::now();

    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    // MatchedPath contains the bounded route template (for example
    // `/v1/content/track/{id}`), never path parameters or a query string.
    let metric_route = request_route_label(request.extensions()).to_string();

    if level > RequestsLoggingLevel::None {
        // Query parameters may contain OAuth authorization codes and other credentials.
        info!(">>> {} {}", method, safe_request_target(request.uri()));
    }

    if level >= RequestsLoggingLevel::Headers {
        let headers = format_safe_headers(request.headers(), SAFE_REQUEST_HEADERS);
        if headers.is_empty() {
            info!("  Req Headers: [no allowlisted headers]");
        } else {
            info!("  Req Headers:\n{}", headers);
        }
    }

    if level >= RequestsLoggingLevel::Body {
        match parse_content_length(request.headers()) {
            ContentLengthParseResult::No(reason) => info!("  Req Body: {}", reason),
            ContentLengthParseResult::Ok(size) => {
                if size < MAX_LOGGABLE_BODY_LENGTH {
                    let (parts, body) = request.into_parts();
                    let bytes = match axum::body::to_bytes(body, size).await {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            error!("Failed to read request body: {:?}", err);
                            return Response::builder()
                                .status(500)
                                .body(axum::body::Body::from("Internal Server Error"))
                                .unwrap();
                        }
                    };
                    info!(
                        "  Req Body:\n{}",
                        format_loggable_body(&path, &parts.headers, &bytes)
                    );
                    request = Request::from_parts(parts, Body::from(bytes))
                } else {
                    info!(
                        "  Req Body: Too big to log ({:#})",
                        byte_unit::Byte::from(size)
                    );
                }
            }
        }
    }

    let mut response = next.run(request).await;

    if level >= RequestsLoggingLevel::Headers {
        let headers = format_safe_headers(response.headers(), SAFE_RESPONSE_HEADERS);
        if headers.is_empty() {
            info!("  Resp Headers: [no allowlisted headers]");
        } else {
            info!("  Resp Headers:\n{}", headers);
        }
    }

    if level >= RequestsLoggingLevel::Body {
        match parse_content_length(response.headers()) {
            ContentLengthParseResult::No(reason) => info!("  Resp Body: {}", reason),
            ContentLengthParseResult::Ok(size) => {
                if size < MAX_LOGGABLE_BODY_LENGTH {
                    let (parts, body) = response.into_parts();
                    let bytes = match axum::body::to_bytes(body, size).await {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            error!("Failed to read response body: {:?}", err);
                            return Response::builder()
                                .status(500)
                                .body(axum::body::Body::from("Internal Server Error"))
                                .unwrap();
                        }
                    };
                    info!(
                        "  Resp Body:\n{}",
                        format_loggable_body(&path, &parts.headers, &bytes)
                    );
                    response = Response::from_parts(parts, Body::from(bytes))
                } else {
                    info!(
                        "  Resp Body: Too big to log ({:#})",
                        byte_unit::Byte::from(size)
                    );
                }
            }
        }
    }

    let status = response.status().as_u16();
    let duration: std::time::Duration = start.elapsed();

    if level > RequestsLoggingLevel::None {
        info!("<<< {} ({}ms)", status, duration.as_millis());
    }

    // Record HTTP request metrics for Prometheus
    record_http_request(&method, &metric_route, status, duration);

    // Record bandwidth metrics
    let response_bytes = match parse_content_length(response.headers()) {
        ContentLengthParseResult::Ok(size) => size as u64,
        ContentLengthParseResult::No(_) => 0,
    };

    // Get endpoint category for aggregation
    let endpoint_category = categorize_endpoint(&path);

    // Record to Prometheus
    record_bandwidth(endpoint_category, response_bytes);

    // Record to database if user is authenticated
    if let Some(uid) = user_id {
        if response_bytes > 0 {
            // Get current date in YYYYMMDD format
            let today = chrono::Utc::now();
            let date = today.year() as u32 * 10000 + today.month() * 100 + today.day();

            // Record to database (fire and forget - don't block the response)
            let user_manager = state.user_manager;
            if let Err(e) =
                user_manager.record_bandwidth_usage(uid, date, endpoint_category, response_bytes, 1)
            {
                debug!("Failed to record bandwidth usage to database: {}", e);
            }
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::{
        format_loggable_body, format_safe_headers, is_authentication_path, safe_request_target,
        RequestsLoggingLevel, SAFE_REQUEST_HEADERS, SAFE_RESPONSE_HEADERS,
    };
    use axum::http::{HeaderMap, HeaderValue, Uri};

    #[test]
    fn level_ordering() {
        let none = RequestsLoggingLevel::None;

        assert!(none < RequestsLoggingLevel::Headers);
        assert!(RequestsLoggingLevel::Body > RequestsLoggingLevel::None);
    }

    #[test]
    fn request_headers_are_allowlisted_and_credentials_are_not_rendered() {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer sentinel-access-token"),
        );
        headers.insert(
            "cookie",
            HeaderValue::from_static("session_token=sentinel-cookie"),
        );
        headers.insert(
            "x-unexpected-secret",
            HeaderValue::from_static("sentinel-custom-secret"),
        );

        let rendered = format_safe_headers(&headers, SAFE_REQUEST_HEADERS);

        assert!(rendered.contains("content-type: application/json"));
        assert!(!rendered.contains("sentinel-access-token"));
        assert!(!rendered.contains("sentinel-cookie"));
        assert!(!rendered.contains("sentinel-custom-secret"));
        assert!(!rendered.contains("authorization"));
        assert!(!rendered.contains("cookie"));
    }

    #[test]
    fn response_headers_are_allowlisted_and_session_cookie_is_not_rendered() {
        let mut headers = HeaderMap::new();
        headers.insert("content-length", HeaderValue::from_static("42"));
        headers.insert(
            "set-cookie",
            HeaderValue::from_static("session_token=sentinel-session-token; HttpOnly"),
        );

        let rendered = format_safe_headers(&headers, SAFE_RESPONSE_HEADERS);

        assert!(rendered.contains("content-length: 42"));
        assert!(!rendered.contains("sentinel-session-token"));
        assert!(!rendered.contains("set-cookie"));
    }

    #[test]
    fn authentication_bodies_are_always_omitted() {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        let body = br#"{"user_handle":"alice","password":"sentinel-password"}"#;

        let rendered = format_loggable_body("/v1/auth/login", &headers, body);

        assert_eq!(rendered, "[omitted: authentication endpoint]");
        assert!(!rendered.contains("sentinel-password"));
        assert!(is_authentication_path("/v1/auth/oidc/callback"));
        assert!(is_authentication_path("/v1/admin/users/alice/password"));
    }

    #[test]
    fn sensitive_fields_are_recursively_redacted_from_other_json_bodies() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "content-type",
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        let body = br#"{
            "name":"safe-name",
            "password":"sentinel-password",
            "nested":{"access_token":"sentinel-access-token"},
            "items":[{"clientSecret":"sentinel-client-secret"}],
            "api-key":"sentinel-api-key"
        }"#;

        let rendered = format_loggable_body("/v1/content/batch", &headers, body);

        assert!(rendered.contains("safe-name"));
        assert!(rendered.contains("[REDACTED]"));
        assert!(!rendered.contains("sentinel-password"));
        assert!(!rendered.contains("sentinel-access-token"));
        assert!(!rendered.contains("sentinel-client-secret"));
        assert!(!rendered.contains("sentinel-api-key"));
    }

    #[test]
    fn non_json_bodies_are_not_logged() {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("text/plain"));

        let rendered = format_loggable_body("/v1/bug-report", &headers, b"sentinel-plain-secret");

        assert_eq!(rendered, "[omitted: only JSON bodies are loggable]");
        assert!(!rendered.contains("sentinel-plain-secret"));
    }

    #[test]
    fn query_parameters_are_not_in_the_logged_request_target() {
        let uri: Uri = "/v1/auth/oidc/callback?code=sentinel-code&state=sentinel-state"
            .parse()
            .unwrap();

        let rendered = safe_request_target(&uri);

        assert_eq!(rendered, "/v1/auth/oidc/callback");
        assert!(!rendered.contains("sentinel-code"));
        assert!(!rendered.contains("sentinel-state"));
    }
}
