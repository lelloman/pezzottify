use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use super::ServerConfig;

pub const SESSION_COOKIE_NAME: &str = "session_token";
pub const SECURE_SESSION_COOKIE_NAME: &str = "__Host-session_token";
pub const CSRF_COOKIE_NAME: &str = "csrf_token";
pub const SECURE_CSRF_COOKIE_NAME: &str = "__Host-csrf_token";
pub const CSRF_HEADER_NAME: &str = "x-csrf-token";

pub fn session_cookie_name(secure: bool) -> &'static str {
    if secure {
        SECURE_SESSION_COOKIE_NAME
    } else {
        SESSION_COOKIE_NAME
    }
}

pub fn csrf_cookie_name(secure: bool) -> &'static str {
    if secure {
        SECURE_CSRF_COOKIE_NAME
    } else {
        CSRF_COOKIE_NAME
    }
}

pub fn new_csrf_token() -> String {
    Uuid::new_v4().to_string()
}

pub fn session_cookie(token: String, config: &ServerConfig) -> Cookie<'static> {
    Cookie::build((session_cookie_name(config.secure_session_cookies), token))
        .path("/")
        .secure(config.secure_session_cookies)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(cookie_max_age(config))
        .build()
}

pub fn csrf_cookie(token: String, config: &ServerConfig) -> Cookie<'static> {
    Cookie::build((csrf_cookie_name(config.secure_session_cookies), token))
        .path("/")
        .secure(config.secure_session_cookies)
        // This cookie must be readable by the browser so it can be echoed in a header.
        .http_only(false)
        .same_site(SameSite::Lax)
        .max_age(cookie_max_age(config))
        .build()
}

pub fn expired_session_cookie(config: &ServerConfig) -> Cookie<'static> {
    expired_cookie(
        session_cookie_name(config.secure_session_cookies),
        true,
        config,
    )
}

pub fn expired_csrf_cookie(config: &ServerConfig) -> Cookie<'static> {
    expired_cookie(
        csrf_cookie_name(config.secure_session_cookies),
        false,
        config,
    )
}

fn expired_cookie(name: &'static str, http_only: bool, config: &ServerConfig) -> Cookie<'static> {
    Cookie::build((name, ""))
        .path("/")
        .secure(config.secure_session_cookies)
        .http_only(http_only)
        .same_site(SameSite::Lax)
        .max_age(Duration::ZERO)
        .expires(OffsetDateTime::UNIX_EPOCH)
        .build()
}

fn cookie_max_age(config: &ServerConfig) -> Duration {
    Duration::seconds(i64::try_from(config.session_cookie_max_age_secs).unwrap_or(i64::MAX))
}

pub fn append_cookie(headers: &mut HeaderMap, cookie: Cookie<'_>) {
    headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie.to_string()).expect("generated cookie is a valid header"),
    );
}

pub fn append_session_cookies(
    response: &mut Response,
    token: String,
    csrf_token: Option<String>,
    config: &ServerConfig,
) {
    append_cookie(response.headers_mut(), session_cookie(token, config));
    append_cookie(
        response.headers_mut(),
        csrf_cookie(csrf_token.unwrap_or_else(new_csrf_token), config),
    );
}

pub fn append_expired_session_cookies(response: &mut Response, config: &ServerConfig) {
    append_cookie(response.headers_mut(), expired_session_cookie(config));
    append_cookie(response.headers_mut(), expired_csrf_cookie(config));
}

/// Double-submit CSRF protection for requests authenticated by an ambient cookie.
/// Header-token clients are not vulnerable to CSRF and intentionally bypass this check.
pub async fn require_csrf(
    State(config): State<ServerConfig>,
    request: Request,
    next: Next,
) -> Response {
    if is_safe_method(request.method()) || request.headers().contains_key("Authorization") {
        return next.run(request).await;
    }

    // Login is unauthenticated and must remain usable even if a stale cookie is present.
    if request.uri().path() == "/v1/auth/login" {
        return next.run(request).await;
    }

    let jar = CookieJar::from_headers(request.headers());
    if jar
        .get(session_cookie_name(config.secure_session_cookies))
        .is_none()
    {
        return next.run(request).await;
    }

    let cookie_token = jar
        .get(csrf_cookie_name(config.secure_session_cookies))
        .map(Cookie::value);
    let header_token = request
        .headers()
        .get(CSRF_HEADER_NAME)
        .and_then(|value| value.to_str().ok());

    match (cookie_token, header_token) {
        (Some(cookie), Some(header)) if constant_time_eq(cookie.as_bytes(), header.as_bytes()) => {
            next.run(request).await
        }
        _ => (StatusCode::FORBIDDEN, "missing or invalid CSRF token").into_response(),
    }
}

fn is_safe_method(method: &Method) -> bool {
    matches!(
        method,
        &Method::GET | &Method::HEAD | &Method::OPTIONS | &Method::TRACE
    )
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_cookie_uses_host_prefix_and_hardened_attributes() {
        let config = ServerConfig::default();
        let cookie = session_cookie("secret".to_owned(), &config).to_string();

        assert!(cookie.starts_with("__Host-session_token=secret"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("Path=/"));
        assert!(cookie.contains("Max-Age=604800"));
        assert!(!cookie.contains("Domain="));
    }

    #[test]
    fn expiration_matches_cookie_security_attributes() {
        let config = ServerConfig::default();
        let cookie = expired_session_cookie(&config).to_string();

        assert!(cookie.starts_with("__Host-session_token="));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("Path=/"));
        assert!(cookie.contains("Max-Age=0"));
    }

    #[test]
    fn csrf_comparison_rejects_different_values_and_lengths() {
        assert!(constant_time_eq(b"same", b"same"));
        assert!(!constant_time_eq(b"same", b"diff"));
        assert!(!constant_time_eq(b"same", b"shorter"));
    }
}
