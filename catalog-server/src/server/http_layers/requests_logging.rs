//! Request logging middleware
#![allow(dead_code)] // Used as middleware

use super::super::state::ServerState;
use crate::server::metrics::{categorize_endpoint, record_bandwidth, record_http_request};
use axum::extract::State;
use axum::{
    body::Body,
    http::{header::HeaderMap, Request, Response},
    middleware::Next,
    response::IntoResponse,
};
use chrono::Datelike;
use std::time::Instant;
use tracing::{debug, error, info};

#[derive(PartialEq, PartialOrd, Clone, Debug, clap::ValueEnum)]
pub enum RequestsLoggingLevel {
    None,
    Path,
    Headers,
    Body,
}

impl Default for RequestsLoggingLevel {
    fn default() -> Self {
        Self::Path
    }
}

impl std::fmt::Display for RequestsLoggingLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

const MAX_LOGGABLE_BODY_LENGTH: usize = 1024;

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
    let uri = request.uri().to_string();
    let path = request.uri().path().to_string();

    if level > RequestsLoggingLevel::None {
        info!(">>> {} {}", method, uri);
    }

    if level >= RequestsLoggingLevel::Headers {
        info!("  Req Headers:");
        for header in request.headers().iter() {
            info!("    {:?}: {:?}", header.0, header.1);
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
                    info!("  Req Body:\n{}", String::from_utf8_lossy(&bytes));
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
        info!("  Resp Headers:");
        for header in response.headers().iter() {
            info!("    {:?}: {:?}", header.0, header.1);
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
                    info!("  Req Body:\n{}", String::from_utf8_lossy(&bytes));
                    response = Response::from_parts(parts, Body::from(bytes))
                } else {
                    info!(
                        "  Req Body: Too big to log ({:#})",
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
    record_http_request(&method, &uri, status, duration);

    // Record bandwidth metrics
    let response_bytes = match parse_content_length(response.headers()) {
        ContentLengthParseResult::Ok(size) => size as u64,
        ContentLengthParseResult::No(_) => 0,
    };

    // Get endpoint category for aggregation
    let endpoint_category = categorize_endpoint(&path);

    // Record to Prometheus
    record_bandwidth(user_id, endpoint_category, response_bytes);

    // Record to database if user is authenticated
    if let Some(uid) = user_id {
        if response_bytes > 0 {
            // Get current date in YYYYMMDD format
            let today = chrono::Utc::now();
            let date = today.year() as u32 * 10000 + today.month() * 100 + today.day();

            // Record to database (fire and forget - don't block the response)
            let user_manager = state.user_manager.lock().unwrap();
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
    use super::RequestsLoggingLevel;

    #[test]
    fn level_ordering() {
        let none = RequestsLoggingLevel::None;

        assert!(none < RequestsLoggingLevel::Headers);
        assert!(RequestsLoggingLevel::Body > RequestsLoggingLevel::None);
    }
}
