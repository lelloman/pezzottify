use super::state::ServerState;
use crate::user::auth::AuthTokenValue;

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
    ctx.user_manager
        .lock()
        .unwrap()
        .get_auth_token(&AuthTokenValue(token))
        .map(|t| Session {
            user_id: t.user_id,
            token: t.value.0,
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
