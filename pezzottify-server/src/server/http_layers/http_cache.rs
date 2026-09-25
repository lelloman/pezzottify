//! Route-level HTTP cache policy middleware.

use simple_server::web::{
    body::Body,
    extract::State,
    http::{
        header::{CACHE_CONTROL, CONTENT_RANGE, CONTENT_TYPE},
        HeaderValue, Method, Request, StatusCode,
    },
    middleware::Next,
    response::{IntoResponse, Response},
};

use simple_server::response_headers;

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
    response_headers::insert_if_absent(response.headers_mut(), CACHE_CONTROL, NO_STORE);
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
        response_headers::replace(response.headers_mut(), CACHE_CONTROL, value);
        response_headers::merge_vary(response.headers_mut(), "Cookie").expect("static field name");
        response_headers::merge_vary(response.headers_mut(), "Authorization")
            .expect("static field name");
    } else {
        response_headers::replace(response.headers_mut(), CACHE_CONTROL, NO_STORE);
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

#[cfg(test)]
mod tests {
    use super::*;
    use simple_server::web::{
        body::Body,
        http::{
            header::{AUTHORIZATION, VARY},
            Response as HttpResponse,
        },
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
        let vary = response
            .headers()
            .get_all(VARY)
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect::<Vec<_>>()
            .join(", ");
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

    #[tokio::test]
    async fn cache_contract_covers_head_partial_sse_and_existing_repeated_headers() {
        use simple_server::web::body::to_bytes;
        for (method, status, content_type, range, explicit, cacheable) in [
            (Method::GET, 200, "application/json", false, false, true),
            (Method::HEAD, 200, "application/json", false, false, true),
            (Method::POST, 200, "application/json", false, false, false),
            (Method::GET, 404, "application/json", false, false, false),
            (Method::GET, 206, "application/json", false, false, false),
            (Method::GET, 200, "application/json", true, false, false),
            (Method::GET, 200, "text/event-stream", false, false, false),
            (Method::GET, 200, "audio/ogg", false, false, false),
            (Method::GET, 200, "video/mp4", false, false, false),
            (Method::GET, 200, "application/json", false, true, false),
        ] {
            let app = Router::new()
                .route(
                    "/v1/check",
                    simple_server::web::routing::any(move || async move {
                        let mut response = HttpResponse::builder()
                            .status(status)
                            .header(CONTENT_TYPE, content_type)
                            .header(VARY, "Accept-Encoding, cookie")
                            .header(VARY, "COOKIE")
                            .header("set-cookie", "one=1")
                            .header("set-cookie", "two=2")
                            .body(Body::from("payload"))
                            .unwrap();
                        if range {
                            response
                                .headers_mut()
                                .insert(CONTENT_RANGE, HeaderValue::from_static("bytes 0-6/7"));
                        }
                        if explicit {
                            response
                                .headers_mut()
                                .append(CACHE_CONTROL, HeaderValue::from_static("private"));
                            response
                                .headers_mut()
                                .append(CACHE_CONTROL, HeaderValue::from_static("max-age=7"));
                        }
                        response.extensions_mut().insert(17u32);
                        response
                    }),
                )
                .layer(middleware::from_fn_with_state(60usize, http_cache))
                .layer(middleware::from_fn(http_api_no_store));
            let response = app
                .oneshot(
                    Request::builder()
                        .method(method.clone())
                        .uri("/v1/check")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status);
            assert_eq!(response.extensions().get::<u32>(), Some(&17));
            assert_eq!(response.headers().get_all("set-cookie").iter().count(), 2);
            let cache: Vec<_> = response
                .headers()
                .get_all(CACHE_CONTROL)
                .iter()
                .map(|v| v.to_str().unwrap())
                .collect();
            assert_eq!(
                cache,
                if explicit {
                    vec!["private", "max-age=7"]
                } else if cacheable {
                    vec!["private, max-age=60"]
                } else {
                    vec!["no-store"]
                }
            );
            let vary: Vec<_> = response
                .headers()
                .get_all(VARY)
                .iter()
                .flat_map(|v| v.to_str().unwrap().split(','))
                .map(|v| v.trim().to_ascii_lowercase())
                .collect();
            assert_eq!(vary.iter().filter(|v| *v == "cookie").count(), 2);
            assert_eq!(vary.iter().any(|v| v == "authorization"), cacheable);
            let bytes = to_bytes(response.into_body(), 100).await.unwrap();
            assert_eq!(
                bytes.as_ref(),
                if method == Method::HEAD {
                    b"".as_slice()
                } else {
                    b"payload".as_slice()
                }
            );
        }
    }

    #[tokio::test]
    async fn outer_policy_covers_early_auth_errors_and_respects_api_path_boundary() {
        let app = Router::new()
            .route("/v1", get(|| async { "unreachable" }))
            .route("/v1/error", get(|| async { "unreachable" }))
            .route("/v10", get(|| async { "unreachable" }))
            .layer(middleware::from_fn(|_: Request<Body>, _: Next| async {
                StatusCode::UNAUTHORIZED
            }))
            .layer(middleware::from_fn(http_api_no_store));
        for (path, expected) in [("/v1", true), ("/v1/error", true), ("/v10", false)] {
            let response = app
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(response.headers().contains_key(CACHE_CONTROL), expected);
        }
    }
}
