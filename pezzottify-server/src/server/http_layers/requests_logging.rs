//! Request logging middleware
#![allow(dead_code)] // Used as middleware

use super::super::state::ServerState;
use crate::server::metrics::{
    categorize_endpoint, record_bandwidth, record_http_request, request_route_label,
};
use chrono::Datelike;
use serde_json::Value;
use simple_server::web::extract::State;
use simple_server::web::{
    body::Body,
    http::{header::HeaderMap, Request, Response},
    middleware::Next,
};
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

pub(crate) fn is_report_path(path: &str) -> bool {
    [
        "/v1/reports",
        "/v1/admin/reports",
        "/v1/user/bug-report",
        "/v1/admin/bug-report",
        "/v1/admin/bug-reports",
    ]
    .iter()
    .any(|prefix| {
        path == *prefix
            || path
                .strip_prefix(prefix)
                .is_some_and(|s| s.starts_with('/'))
    })
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
    if is_report_path(path) {
        return "[omitted: report endpoint]".to_string();
    }
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

#[test]
fn report_bodies_are_never_loggable() {
    for path in [
        "/v1/reports",
        "/v1/reports/id/attachments/id",
        "/v1/admin/reports/settings",
        "/v1/user/bug-report",
        "/v1/admin/bug-reports",
        "/v1/admin/bug-report/id",
    ] {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        assert_eq!(
            format_loggable_body(path, &headers, br#"{"description":"private chat"}"#),
            "[omitted: report endpoint]"
        );
        assert_eq!(
            format_loggable_body(path, &HeaderMap::new(), b"private chat"),
            "[omitted: report endpoint]"
        );
    }
}

pub async fn log_requests(
    State(state): State<ServerState>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    with_request_trace(&state.config.requests_logging_level, request, |request| {
        log_request_details(state.clone(), request, next)
    })
    .await
}

async fn with_request_trace<F, Fut>(
    level: &RequestsLoggingLevel,
    request: Request<Body>,
    next: F,
) -> Response<Body>
where
    F: FnOnce(Request<Body>) -> Fut,
    Fut: std::future::Future<Output = Response<Body>>,
{
    if *level == RequestsLoggingLevel::None {
        next(request).await
    } else {
        simple_server::web::compat::trace_with_observer(request, RequestObserver, next).await
    }
}

// Preserve immediate INFO response visibility, including long-lived streams.
struct RequestObserver;
impl simple_server::http_tracing::Observer for RequestObserver {
    fn on_response(
        &mut self,
        span: &tracing::Span,
        response: &simple_server::axum::response::Response,
        latency: std::time::Duration,
    ) {
        info!(target: "simple_server::http_tracing", parent: span,
            status = response.status().as_u16(),
            header_latency_ms = latency.as_secs_f64() * 1000.0,
            "http.response_headers");
    }
    fn on_finish(
        &mut self,
        span: &tracing::Span,
        outcome: simple_server::http_tracing::Outcome,
        phase: simple_server::http_tracing::Phase,
        duration: std::time::Duration,
    ) {
        simple_server::http_tracing::Observer::on_finish(
            &mut simple_server::http_tracing::TracingObserver,
            span,
            outcome,
            phase,
            duration,
        );
    }
}

async fn log_request_details(
    state: ServerState,
    mut request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let level = state.config.requests_logging_level.clone();

    // Extract user_id from request extensions (set by earlier middleware like extract_user_id_for_rate_limit)
    let user_id = request.extensions().get::<usize>().copied();

    let start = Instant::now();

    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    // MatchedPath contains the bounded route template (for example
    // `/v1/content/track/{id}`), never path parameters or a query string.
    let metric_route = request_route_label(request.extensions()).to_string();

    if level >= RequestsLoggingLevel::Headers {
        let headers = format_safe_headers(request.headers(), SAFE_REQUEST_HEADERS);
        if headers.is_empty() {
            info!("  Req Headers: [no allowlisted headers]");
        } else {
            info!("  Req Headers:\n{}", headers);
        }
    }

    if level >= RequestsLoggingLevel::Body && !is_report_path(&path) {
        match parse_content_length(request.headers()) {
            ContentLengthParseResult::No(reason) => info!("  Req Body: {}", reason),
            ContentLengthParseResult::Ok(size) => {
                if size < MAX_LOGGABLE_BODY_LENGTH {
                    let (parts, body) = request.into_parts();
                    let bytes = match simple_server::web::body::to_bytes(body, size).await {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            error!("Failed to read request body: {:?}", err);
                            return Response::builder()
                                .status(500)
                                .body(simple_server::web::body::Body::from(
                                    "Internal Server Error",
                                ))
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

    if level >= RequestsLoggingLevel::Body && !is_report_path(&path) {
        match parse_content_length(response.headers()) {
            ContentLengthParseResult::No(reason) => info!("  Resp Body: {}", reason),
            ContentLengthParseResult::Ok(size) => {
                if size < MAX_LOGGABLE_BODY_LENGTH {
                    let (parts, body) = response.into_parts();
                    let bytes = match simple_server::web::body::to_bytes(body, size).await {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            error!("Failed to read response body: {:?}", err);
                            return Response::builder()
                                .status(500)
                                .body(simple_server::web::body::Body::from(
                                    "Internal Server Error",
                                ))
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
        format_loggable_body, format_safe_headers, is_authentication_path, with_request_trace,
        RequestsLoggingLevel, SAFE_REQUEST_HEADERS, SAFE_RESPONSE_HEADERS,
    };
    use simple_server::web::http::{HeaderMap, HeaderValue};

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

    #[tokio::test(flavor = "current_thread")]
    async fn configured_modes_trace_safe_routes_and_preserve_response() {
        use simple_server::web::{
            body::{to_bytes, Body},
            http::{Request, Response},
            middleware::{self, Next},
            routing::get,
            Router,
        };
        use std::sync::{Arc, Mutex};
        use tower::ServiceExt;
        use tracing::instrument::WithSubscriber;

        #[derive(Clone)]
        struct Buffer(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Buffer {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Buffer {
            type Writer = Self;
            fn make_writer(&'a self) -> Self {
                self.clone()
            }
        }
        for level in [
            RequestsLoggingLevel::None,
            RequestsLoggingLevel::Path,
            RequestsLoggingLevel::Headers,
            RequestsLoggingLevel::Body,
        ] {
            let bytes = Arc::new(Mutex::new(Vec::new()));
            let subscriber = tracing_subscriber::fmt()
                .with_ansi(false)
                .without_time()
                .with_max_level(tracing::Level::INFO)
                .with_writer(Buffer(bytes.clone()))
                .finish();
            let enabled = level != RequestsLoggingLevel::None;
            async move {
                let app = Router::new()
                    .route(
                        "/probe/{id}",
                        get(|| async {
                            Response::builder()
                                .status(201)
                                .header("x-preserved", "yes")
                                .body(Body::from("unchanged"))
                                .unwrap()
                        }),
                    )
                    .layer(middleware::from_fn(
                        move |request: Request<Body>, next: Next| {
                            let level = level.clone();
                            async move {
                                with_request_trace(&level, request, |request| next.run(request))
                                    .await
                            }
                        },
                    ));
                let response = app
                    .oneshot(
                        Request::builder()
                            .uri("/probe/private-id?token=secret")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), 201);
                assert_eq!(response.headers()["x-preserved"], "yes");
                assert_eq!(
                    to_bytes(response.into_body(), 100).await.unwrap(),
                    "unchanged"
                );
            }
            .with_subscriber(subscriber)
            .await;
            let output = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
            if enabled {
                assert!(output.contains("http.response_headers"), "{output}");
                assert!(output.contains("http.finished"), "{output}");
                assert!(output.contains("/probe/{id}"), "{output}");
                assert!(output.contains("status=201"), "{output}");
            } else {
                assert!(output.is_empty(), "{output}");
            }
            assert!(!output.contains("private-id"));
            assert!(!output.contains("secret"));
            assert!(!output.contains(">>>"));
            assert!(!output.contains("<<<"));
        }
    }
}
