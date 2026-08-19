//! Route-level HTTP cache policy middleware.

use axum::{
    body::Body,
    extract::State,
    http::{
        header::{CACHE_CONTROL, CONTENT_RANGE, CONTENT_TYPE, VARY},
        HeaderMap, HeaderValue, Method, Request, StatusCode,
    },
    middleware::Next,
    response::{IntoResponse, Response},
};

const NO_STORE: HeaderValue = HeaderValue::from_static("no-store");

/// Default policy for API routes. An inner route-specific policy can override
/// this by setting `Cache-Control` explicitly.
pub async fn http_no_store(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await.into_response();
    set_default_no_store(&mut response);
    response
}

/// Outermost API safety net. This also covers responses returned early by
/// authentication, CSRF, and rate-limit middleware without disabling normal
/// caching for frontend assets outside `/v1`.
pub async fn http_api_no_store(request: Request<Body>, next: Next) -> Response {
    let is_api = request.uri().path() == "/v1" || request.uri().path().starts_with("/v1/");
    let mut response = next.run(request).await.into_response();
    if is_api {
        set_default_no_store(&mut response);
    }
    response
}

fn set_default_no_store(response: &mut Response) {
    response
        .headers_mut()
        .entry(CACHE_CONTROL)
        .or_insert(NO_STORE);
}

/// Opt-in policy for stable, authenticated catalog reads.
///
/// Only successful GET/HEAD responses are cacheable, only in a private cache.
/// Errors, partial responses, and streaming media are always `no-store`.
pub async fn http_cache(
    State(max_age_sec): State<usize>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let mut response = next.run(request).await.into_response();

    if response.headers().contains_key(CACHE_CONTROL) {
        return response;
    }

    if is_private_cacheable(&method, &response) {
        let value = HeaderValue::from_str(&format!("private, max-age={max_age_sec}"))
            .expect("cache max age always produces a valid header");
        response.headers_mut().insert(CACHE_CONTROL, value);
        add_vary(response.headers_mut(), "Cookie");
        add_vary(response.headers_mut(), "Authorization");
    } else {
        response.headers_mut().insert(CACHE_CONTROL, NO_STORE);
    }

    response
}

fn is_private_cacheable(method: &Method, response: &Response) -> bool {
    if !matches!(*method, Method::GET | Method::HEAD)
        || !response.status().is_success()
        || response.status() == StatusCode::PARTIAL_CONTENT
        || response.headers().contains_key(CONTENT_RANGE)
    {
        return false;
    }

    !response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|content_type| {
            let content_type = content_type.to_ascii_lowercase();
            content_type.starts_with("audio/")
                || content_type.starts_with("video/")
                || content_type.starts_with("text/event-stream")
        })
}

fn add_vary(headers: &mut HeaderMap, required: &str) {
    let mut values = headers
        .get_all(VARY)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if values.iter().any(|value| value == "*")
        || values
            .iter()
            .any(|value| value.eq_ignore_ascii_case(required))
    {
        return;
    }

    values.push(required.to_owned());
    if let Ok(value) = HeaderValue::from_str(&values.join(", ")) {
        headers.insert(VARY, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{header::AUTHORIZATION, Response as HttpResponse},
        middleware,
        routing::{get, post},
        Router,
    };
    use tower::ServiceExt;

    fn cache_test_app() -> Router {
        Router::new()
            .route(
                "/ok",
                get(|| async {
                    HttpResponse::builder()
                        .header(VARY, "Accept-Encoding")
                        .body(Body::empty())
                        .unwrap()
                }),
            )
            .route(
                "/missing",
                get(|| async { StatusCode::NOT_FOUND.into_response() }),
            )
            .route(
                "/stream",
                get(|| async {
                    HttpResponse::builder()
                        .status(StatusCode::PARTIAL_CONTENT)
                        .header(CONTENT_TYPE, "audio/ogg")
                        .header(CONTENT_RANGE, "bytes 0-9/100")
                        .body(Body::empty())
                        .unwrap()
                }),
            )
            .route(
                "/mutation",
                post(|| async { StatusCode::NO_CONTENT.into_response() }),
            )
            .layer(middleware::from_fn_with_state(60usize, http_cache))
    }

    #[tokio::test]
    async fn successful_catalog_response_is_private_and_varies_by_credentials() {
        let response = cache_test_app()
            .oneshot(
                Request::builder()
                    .uri("/ok")
                    .header("Cookie", "session=secret")
                    .header(AUTHORIZATION, "Bearer secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.headers()[CACHE_CONTROL], "private, max-age=60");
        let vary = response.headers()[VARY].to_str().unwrap();
        assert!(vary.contains("Accept-Encoding"));
        assert!(vary.contains("Cookie"));
        assert!(vary.contains("Authorization"));
    }

    #[tokio::test]
    async fn errors_streams_and_mutations_are_not_stored() {
        for (method, uri) in [
            (Method::GET, "/missing"),
            (Method::GET, "/stream"),
            (Method::POST, "/mutation"),
        ] {
            let response = cache_test_app()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(uri)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.headers()[CACHE_CONTROL], "no-store", "{uri}");
        }
    }

    #[tokio::test]
    async fn api_default_does_not_override_an_explicit_route_policy() {
        let app = Router::new()
            .route(
                "/private",
                get(|| async {
                    HttpResponse::builder()
                        .header(CACHE_CONTROL, "private, max-age=10")
                        .body(Body::empty())
                        .unwrap()
                }),
            )
            .route("/sensitive", get(|| async { "secret" }))
            .layer(middleware::from_fn(http_no_store));

        let private = app
            .clone()
            .oneshot(Request::get("/private").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(private.headers()[CACHE_CONTROL], "private, max-age=10");

        let sensitive = app
            .oneshot(Request::get("/sensitive").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(sensitive.headers()[CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn api_safety_net_does_not_change_frontend_responses() {
        let app = Router::new()
            .route("/v1/error", get(|| async { StatusCode::UNAUTHORIZED }))
            .route("/asset.js", get(|| async { "asset" }))
            .layer(middleware::from_fn(http_api_no_store));

        let api = app
            .clone()
            .oneshot(Request::get("/v1/error").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(api.headers()[CACHE_CONTROL], "no-store");

        let asset = app
            .oneshot(Request::get("/asset.js").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(!asset.headers().contains_key(CACHE_CONTROL));
    }
}
