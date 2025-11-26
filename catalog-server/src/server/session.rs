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
