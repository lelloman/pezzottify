//! HTTP caching middleware
#![allow(dead_code)] // Used as middleware

use axum::{body::Body, extract::State, http::Request, middleware::Next, response::IntoResponse};

pub async fn http_cache(
    State(max_age_sec): State<usize>,
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    let response = next.run(request).await.into_response();

    let (mut parts, body) = response.into_parts();
    parts.headers.insert(
        "Cache-Control",
        format!("max-age={}", max_age_sec).parse().unwrap(),
    );

    axum::http::Response::from_parts(parts, body)
}
