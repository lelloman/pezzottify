use super::state::ServerState;
use crate::user::auth::AuthTokenValue;
use crate::user::Permission;

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use tracing::debug;

#[derive(Debug)]
pub struct Session {
    pub user_id: usize,
    pub token: String,
    pub permissions: Vec<Permission>,
}

impl Session {
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }
}

pub const COOKIE_SESSION_TOKEN_KEY: &str = "session_token";
pub const HEADER_SESSION_TOKEN_KEY: &str = "Authorization";

pub enum SessionExtractionError {
    AccessDenied,
    InternalError,
}

impl IntoResponse for SessionExtractionError {
    fn into_response(self) -> axum::response::Response {
        match self {
            SessionExtractionError::AccessDenied => StatusCode::FORBIDDEN.into_response(),
            SessionExtractionError::InternalError => {
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

async fn extract_session_token_from_cookies(
    parts: &mut Parts,
    ctx: &ServerState,
) -> Option<String> {
    CookieJar::from_request_parts(parts, &ctx)
        .await
        .expect("Could not read cookies into CookieJar.")
        .get(COOKIE_SESSION_TOKEN_KEY)
        .map(Cookie::value)
        .map(|s| s.to_string())
}

fn extract_session_token_from_headers(parts: &mut Parts) -> Option<String> {
    parts
        .headers
        .get(HEADER_SESSION_TOKEN_KEY)
        .map(|v| v.as_bytes().to_owned())
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

async fn extract_session_from_request_parts(
    parts: &mut Parts,
    ctx: &ServerState,
) -> Option<Session> {
    debug!("exctracting session from request parts...");
    let token = match extract_session_token_from_cookies(parts, ctx)
        .await
        .or_else(|| extract_session_token_from_headers(parts))
    {
        None => {
            debug!("No token in cookies nor headers.");
            return None;
        }
        Some(x) => x,
    };

    debug!("Got session token {}", token);
    let user_manager = ctx.user_manager.lock().unwrap();
    let auth_token_value = AuthTokenValue(token.clone());
    let auth_token = match user_manager.get_auth_token(&auth_token_value) {
        Ok(Some(token)) => {
            debug!("Found auth token for user_id={}", token.user_id);

            // Update last_used timestamp
            if let Err(e) = user_manager.update_auth_token_last_used(&auth_token_value) {
                debug!("Failed to update auth token last_used timestamp: {}", e);
                // Continue anyway, as this is not critical for authentication
            }

            token
        }
        Ok(None) => {
            debug!("Auth token not found in database");
            return None;
        }
        Err(e) => {
            debug!("Failed to get auth token from database: {}", e);
            return None;
        }
    };

    let permissions = match user_manager.get_user_permissions(auth_token.user_id) {
        Ok(perms) => {
            debug!("Resolved permissions for user_id={}: {:?}", auth_token.user_id, perms);
            perms
        }
        Err(e) => {
            debug!("Failed to resolve permissions for user_id={}: {}", auth_token.user_id, e);
            return None;
        }
    };

    Some(Session {
        user_id: auth_token.user_id,
        token: auth_token.value.0,
        permissions,
    })
}

impl FromRequestParts<ServerState> for Session {
    type Rejection = SessionExtractionError;

    async fn from_request_parts(
        parts: &mut Parts,
        ctx: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        extract_session_from_request_parts(parts, ctx)
            .await
            .ok_or(SessionExtractionError::AccessDenied)
    }
}

impl FromRequestParts<ServerState> for Option<Session> {
    type Rejection = SessionExtractionError;

    async fn from_request_parts(
        parts: &mut Parts,
        ctx: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        Ok(extract_session_from_request_parts(parts, ctx).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue, Method};

    #[test]
    fn session_has_permission_returns_true_when_permission_exists() {
        let session = Session {
            user_id: 1,
            token: "test-token".to_string(),
            permissions: vec![
                Permission::AccessCatalog,
                Permission::LikeContent,
                Permission::OwnPlaylists,
            ],
        };

        assert!(session.has_permission(Permission::AccessCatalog));
        assert!(session.has_permission(Permission::LikeContent));
        assert!(session.has_permission(Permission::OwnPlaylists));
    }

    #[test]
    fn session_has_permission_returns_false_when_permission_missing() {
        let session = Session {
            user_id: 1,
            token: "test-token".to_string(),
            permissions: vec![Permission::AccessCatalog, Permission::LikeContent],
        };

        assert!(!session.has_permission(Permission::EditCatalog));
        assert!(!session.has_permission(Permission::ManagePermissions));
        assert!(!session.has_permission(Permission::RebootServer));
    }

    #[test]
    fn session_has_permission_returns_false_for_empty_permissions() {
        let session = Session {
            user_id: 1,
            token: "test-token".to_string(),
            permissions: vec![],
        };

        assert!(!session.has_permission(Permission::AccessCatalog));
        assert!(!session.has_permission(Permission::LikeContent));
        assert!(!session.has_permission(Permission::EditCatalog));
    }

    fn create_parts_with_headers(headers: HeaderMap) -> Parts {
        let request = axum::http::Request::builder()
            .method(Method::GET)
            .uri("/")
            .body(())
            .unwrap();

        let (mut parts, _) = request.into_parts();
        parts.headers = headers;
        parts
    }

    #[test]
    fn extract_session_token_from_headers_with_valid_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_SESSION_TOKEN_KEY,
            HeaderValue::from_static("test-auth-token-123"),
        );

        let mut parts = create_parts_with_headers(headers);
        let token = extract_session_token_from_headers(&mut parts);
        assert_eq!(token, Some("test-auth-token-123".to_string()));
    }

    #[test]
    fn extract_session_token_from_headers_without_token() {
        let headers = HeaderMap::new();
        let mut parts = create_parts_with_headers(headers);
        let token = extract_session_token_from_headers(&mut parts);
        assert_eq!(token, None);
    }

    #[test]
    fn extract_session_token_from_headers_with_empty_value() {
        let mut headers = HeaderMap::new();
        headers.insert(HEADER_SESSION_TOKEN_KEY, HeaderValue::from_static(""));

        let mut parts = create_parts_with_headers(headers);
        let token = extract_session_token_from_headers(&mut parts);
        assert_eq!(token, Some("".to_string()));
    }

    #[test]
    fn extract_session_token_from_headers_with_special_characters() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_SESSION_TOKEN_KEY,
            HeaderValue::from_static("token-with-dashes_and_underscores.123"),
        );

        let mut parts = create_parts_with_headers(headers);
        let token = extract_session_token_from_headers(&mut parts);
        assert_eq!(
            token,
            Some("token-with-dashes_and_underscores.123".to_string())
        );
    }

    #[test]
    fn extract_session_token_from_headers_case_sensitive() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("lowercase-header"));

        let mut parts = create_parts_with_headers(headers);
        let token = extract_session_token_from_headers(&mut parts);
        // HTTP headers are case-insensitive, so this should work
        assert_eq!(token, Some("lowercase-header".to_string()));
    }

    #[test]
    fn session_extraction_error_access_denied_status_code() {
        let error = SessionExtractionError::AccessDenied;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn session_extraction_error_internal_error_status_code() {
        let error = SessionExtractionError::InternalError;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn session_debug_format() {
        let session = Session {
            user_id: 42,
            token: "secret-token".to_string(),
            permissions: vec![Permission::AccessCatalog],
        };

        let debug_str = format!("{:?}", session);
        assert!(debug_str.contains("user_id"));
        assert!(debug_str.contains("42"));
        assert!(debug_str.contains("token"));
        assert!(debug_str.contains("secret-token"));
        assert!(debug_str.contains("permissions"));
    }

    #[test]
    fn cookie_and_header_constants() {
        assert_eq!(COOKIE_SESSION_TOKEN_KEY, "session_token");
        assert_eq!(HEADER_SESSION_TOKEN_KEY, "Authorization");
    }
}
