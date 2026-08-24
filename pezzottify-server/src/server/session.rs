use super::{api_error::ApiError, state::ServerState};
use crate::db_executor::{DbPriority, DbRunError};
use crate::oidc::IdTokenClaims;
use crate::user::auth::AuthTokenValue;
use crate::user::device::{DeviceRegistration, DeviceType};
use crate::user::{Permission, UserManager};

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, HeaderMap, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};
use tracing::{debug, warn};

use super::session_cookie::session_cookie_name;

#[derive(Debug, Clone)]
pub struct Session {
    pub user_id: usize,
    pub token: String,
    pub permissions: Vec<Permission>,
    pub device_id: Option<usize>,
    pub device_type: Option<DeviceType>,
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
    Database(DbRunError),
}

impl IntoResponse for SessionExtractionError {
    fn into_response(self) -> axum::response::Response {
        match self {
            SessionExtractionError::AccessDenied => StatusCode::UNAUTHORIZED.into_response(),
            SessionExtractionError::InternalError => {
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            SessionExtractionError::Database(error) => ApiError::from(error).into_response(),
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
        .get(session_cookie_name(ctx.config.secure_session_cookies))
        .map(Cookie::value)
        .map(|s| s.to_string())
}

#[derive(Debug)]
enum AuthorizationHeaderError {
    Duplicate,
    InvalidEncoding,
    MalformedBearer,
    UnsupportedScheme,
    LegacyRawDisabled,
}

static LEGACY_AUTHORIZATION_WARNING_EMITTED: AtomicBool = AtomicBool::new(false);

fn valid_token68(value: &str) -> bool {
    let mut padding_started = false;
    !value.is_empty()
        && value.bytes().all(|byte| {
            if byte == b'=' {
                padding_started = true;
                return true;
            }
            if padding_started {
                return false;
            }
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'+' | b'/')
        })
}

fn parse_authorization_header(
    headers: &HeaderMap,
    allow_legacy_raw: bool,
) -> Result<Option<String>, AuthorizationHeaderError> {
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(AuthorizationHeaderError::Duplicate);
    }

    let value = value
        .to_str()
        .map_err(|_| AuthorizationHeaderError::InvalidEncoding)?;
    let value_bytes = value.as_bytes();
    let starts_with_bearer = value_bytes
        .get(.."Bearer".len())
        .map(|scheme| scheme.eq_ignore_ascii_case(b"Bearer"))
        .unwrap_or(false);
    if starts_with_bearer && value_bytes.get("Bearer".len()) == Some(&b' ') {
        let credential = value["Bearer".len() + 1..].trim_start_matches(' ');
        if valid_token68(credential) {
            return Ok(Some(credential.to_owned()));
        }
        return Err(AuthorizationHeaderError::MalformedBearer);
    }

    if value.eq_ignore_ascii_case("Bearer") {
        return Err(AuthorizationHeaderError::MalformedBearer);
    }
    if value.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(AuthorizationHeaderError::UnsupportedScheme);
    }
    if !allow_legacy_raw {
        return Err(AuthorizationHeaderError::LegacyRawDisabled);
    }
    if !valid_token68(value) {
        return Err(AuthorizationHeaderError::MalformedBearer);
    }

    if !LEGACY_AUTHORIZATION_WARNING_EMITTED.swap(true, Ordering::Relaxed) {
        warn!(
            "Accepted a deprecated raw Authorization credential; migrate the client to 'Bearer <token>' and disable allow_legacy_raw_authorization"
        );
    }
    Ok(Some(value.to_owned()))
}

/// Try to validate the token as an OIDC JWT and create a session
async fn try_oidc_session(token: &str, ctx: &ServerState) -> Result<Option<Session>, DbRunError> {
    // Check if OIDC is configured
    let Some(oidc_client) = ctx.oidc_client.as_ref() else {
        return Ok(None);
    };

    // Try to validate as OIDC ID token
    let claims = match oidc_client.validate_id_token(token).await {
        Ok(claims) => {
            debug!("Validated OIDC ID token for subject={}", claims.subject);
            claims
        }
        Err(e) => {
            // Not a valid OIDC token - this is expected for legacy sessions
            debug!("Token is not a valid OIDC ID token: {}", e);
            return Ok(None);
        }
    };

    let token = token.to_owned();
    ctx.database
        .user_manager
        .run(DbPriority::Critical, move |user_manager| {
            resolve_oidc_session(user_manager, claims, token)
        })
        .await
}

fn resolve_oidc_session(
    user_manager: &UserManager,
    claims: IdTokenClaims,
    token: String,
) -> anyhow::Result<Option<Session>> {
    // Look up or provision local user by OIDC subject.
    let user_id = match user_manager.get_user_id_by_oidc_subject(&claims.subject) {
        Ok(Some(id)) => {
            debug!("Found existing user for OIDC subject={}", claims.subject);
            id
        }
        Ok(None) => {
            // Auto-provision new user from ID token claims
            debug!(
                "Provisioning new user for OIDC subject={} (email={:?}, username={:?})",
                claims.subject, claims.email, claims.preferred_username
            );
            match user_manager.provision_oidc_user(
                &claims.subject,
                claims.preferred_username.as_deref(),
                claims.email.as_deref(),
            ) {
                Ok(id) => {
                    debug!(
                        "Successfully provisioned new user_id={} for OIDC subject={}",
                        id, claims.subject
                    );
                    id
                }
                Err(e) => {
                    warn!("Failed to provision OIDC user: {}", e);
                    return Ok(None);
                }
            }
        }
        Err(e) => {
            warn!("Failed to look up user by OIDC subject: {}", e);
            return Ok(None);
        }
    };

    // Get user permissions
    let permissions = match user_manager.get_user_permissions(user_id) {
        Ok(perms) => {
            debug!(
                "Resolved OIDC session permissions for user_id={}: {:?}",
                user_id, perms
            );
            perms
        }
        Err(e) => {
            warn!(
                "Failed to resolve permissions for OIDC user_id={}: {}",
                user_id, e
            );
            return Ok(None);
        }
    };

    // Extract device info from the ID token claims and look up/register the device
    let (device_id, device_type) = if let Some(device_uuid) = &claims.device_id {
        // Get device_type from JWT claims, defaulting to "web" if not provided
        let jwt_device_type = claims.device_type.as_deref().unwrap_or("web");

        // First try to find existing device by UUID
        match user_manager.get_device_by_uuid(device_uuid) {
            Ok(Some(device)) => {
                // Throttled touch: only update last_seen if >1 hour stale
                let one_hour_ago = SystemTime::now() - Duration::from_secs(3600);
                if device.last_seen < one_hour_ago {
                    let _ = user_manager.touch_device(device.id);
                }
                debug!(
                    "Found existing device for OIDC session: device_id={}, uuid={}",
                    device.id, device_uuid
                );
                (Some(device.id), Some(device.device_type))
            }
            Ok(None) => {
                // Device doesn't exist, register it with info from JWT
                match DeviceRegistration::validate_and_sanitize(
                    device_uuid,
                    jwt_device_type,
                    Some(device_uuid),
                    claims.device_name.as_deref(),
                ) {
                    Ok(registration) => {
                        let registered_device_type = registration.device_type.clone();
                        match user_manager.register_or_update_device(&registration) {
                            Ok(device_id) => {
                                // Associate with user
                                if let Err(e) =
                                    user_manager.associate_device_with_user(device_id, user_id)
                                {
                                    debug!(
                                        "Failed to associate device {} with user {}: {}",
                                        device_id, user_id, e
                                    );
                                }
                                // Enforce per-user device limit
                                if let Err(e) = user_manager.enforce_user_device_limit(
                                    user_id,
                                    super::server::MAX_DEVICES_PER_USER,
                                ) {
                                    debug!(
                                        "Failed to enforce device limit for user {}: {}",
                                        user_id, e
                                    );
                                }
                                debug!(
                                    "Registered new device for OIDC session: device_id={}, uuid={}, type={:?}",
                                    device_id, device_uuid, registered_device_type
                                );
                                (Some(device_id), Some(registered_device_type))
                            }
                            Err(e) => {
                                debug!("Failed to register device UUID={}: {}", device_uuid, e);
                                (None, None)
                            }
                        }
                    }
                    Err(e) => {
                        debug!(
                            "Invalid device registration for UUID={}: {}",
                            device_uuid, e
                        );
                        (None, None)
                    }
                }
            }
            Err(e) => {
                debug!("Failed to look up device by UUID={}: {}", device_uuid, e);
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    Ok(Some(Session {
        user_id,
        token,
        permissions,
        device_id,
        device_type,
    }))
}

/// Try to validate the token as a legacy database auth token
async fn try_legacy_session(token: &str, ctx: &ServerState) -> Result<Option<Session>, DbRunError> {
    let auth_token_value = AuthTokenValue(token.to_string());
    ctx.database
        .user_manager
        .run(DbPriority::Critical, move |user_manager| {
            resolve_legacy_session(user_manager, auth_token_value)
        })
        .await
}

fn resolve_legacy_session(
    user_manager: &UserManager,
    auth_token_value: AuthTokenValue,
) -> anyhow::Result<Option<Session>> {
    let auth_token = match user_manager.get_auth_token(&auth_token_value) {
        Ok(Some(token)) => {
            debug!("Found legacy auth token for user_id={}", token.user_id);

            if let Err(error) = user_manager.update_auth_token_last_used(&auth_token_value) {
                debug!("Failed to update auth token last_used timestamp: {error}");
            }
            token
        }
        Ok(None) => {
            debug!("Auth token not found in database");
            return Ok(None);
        }
        Err(error) => {
            debug!("Failed to get auth token from database: {error}");
            return Ok(None);
        }
    };

    let permissions = match user_manager.get_user_permissions(auth_token.user_id) {
        Ok(permissions) => {
            debug!(
                "Resolved permissions for user_id={}: {:?}",
                auth_token.user_id, permissions
            );
            permissions
        }
        Err(error) => {
            debug!(
                "Failed to resolve permissions for user_id={}: {}",
                auth_token.user_id, error
            );
            return Ok(None);
        }
    };

    let (device_id, device_type) = if let Some(device_id) = auth_token.device_id {
        match user_manager.get_device(device_id) {
            Ok(Some(device)) => {
                let one_hour_ago = SystemTime::now() - Duration::from_secs(3600);
                if device.last_seen < one_hour_ago {
                    let _ = user_manager.touch_device(device.id);
                }
                debug!(
                    "Found device for session: device_id={}, type={:?}",
                    device_id, device.device_type
                );
                (Some(device_id), Some(device.device_type))
            }
            Ok(None) => {
                debug!(
                    "Device not found for device_id={}, continuing without device info",
                    device_id
                );
                (Some(device_id), None)
            }
            Err(error) => {
                debug!("Failed to get device info for device_id={device_id}: {error}");
                (Some(device_id), None)
            }
        }
    } else {
        (None, None)
    };

    Ok(Some(Session {
        user_id: auth_token.user_id,
        token: auth_token.value.0,
        permissions,
        device_id,
        device_type,
    }))
}

async fn extract_session_from_request_parts(
    parts: &mut Parts,
    ctx: &ServerState,
) -> Result<Option<Session>, DbRunError> {
    debug!("extracting session from request parts...");
    // Prefer Authorization header over cookies - the header is set fresh on each
    // request by the client, while cookies may contain stale tokens from before
    // a token refresh. Cookies are only used as fallback for WebSocket connections
    // which cannot send custom headers.
    let header_token =
        match parse_authorization_header(&parts.headers, ctx.config.allow_legacy_raw_authorization)
        {
            Ok(token) => token,
            Err(error) => {
                debug!(?error, "Rejecting malformed Authorization header");
                return Ok(None);
            }
        };
    let token = match header_token.or(extract_session_token_from_cookies(parts, ctx).await) {
        None => {
            debug!("No token in headers nor cookies.");
            return Ok(None);
        }
        Some(x) => x,
    };

    debug!("Got session token (length={})", token.len());

    // Try OIDC JWT validation first (if OIDC is configured)
    if let Some(session) = try_oidc_session(&token, ctx).await? {
        debug!("Session validated via OIDC for user_id={}", session.user_id);
        return Ok(Some(session));
    }

    // Fall back to legacy database token lookup
    if let Some(session) = try_legacy_session(&token, ctx).await? {
        debug!(
            "Session validated via legacy auth for user_id={}",
            session.user_id
        );
        return Ok(Some(session));
    }

    debug!("Token validation failed for both OIDC and legacy auth");
    Ok(None)
}

impl FromRequestParts<ServerState> for Session {
    type Rejection = SessionExtractionError;

    async fn from_request_parts(
        parts: &mut Parts,
        ctx: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        extract_session_from_request_parts(parts, ctx)
            .await
            .map_err(SessionExtractionError::Database)?
            .ok_or(SessionExtractionError::AccessDenied)
    }
}

impl FromRequestParts<ServerState> for Option<Session> {
    type Rejection = SessionExtractionError;

    async fn from_request_parts(
        parts: &mut Parts,
        ctx: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        extract_session_from_request_parts(parts, ctx)
            .await
            .map_err(SessionExtractionError::Database)
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
            device_id: None,
            device_type: None,
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
            device_id: None,
            device_type: None,
        };

        assert!(!session.has_permission(Permission::EditCatalog));
        assert!(!session.has_permission(Permission::ManagePermissions));
        assert!(!session.has_permission(Permission::ServerAdmin));
    }

    #[test]
    fn session_has_permission_returns_false_for_empty_permissions() {
        let session = Session {
            user_id: 1,
            token: "test-token".to_string(),
            permissions: vec![],
            device_id: None,
            device_type: None,
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
    fn authorization_header_accepts_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_SESSION_TOKEN_KEY,
            HeaderValue::from_static("Bearer test-auth-token-123"),
        );

        let parts = create_parts_with_headers(headers);
        let token = parse_authorization_header(&parts.headers, false).unwrap();
        assert_eq!(token.as_deref(), Some("test-auth-token-123"));
    }

    #[test]
    fn authorization_header_scheme_is_case_insensitive() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_SESSION_TOKEN_KEY,
            HeaderValue::from_static("bEaReR token.with-symbols_123-456"),
        );

        let parts = create_parts_with_headers(headers);
        let token = parse_authorization_header(&parts.headers, false).unwrap();
        assert_eq!(token.as_deref(), Some("token.with-symbols_123-456"));
    }

    #[test]
    fn authorization_header_absence_is_not_an_error() {
        let headers = HeaderMap::new();
        let parts = create_parts_with_headers(headers);
        assert_eq!(
            parse_authorization_header(&parts.headers, false).unwrap(),
            None
        );
    }

    #[test]
    fn authorization_header_rejects_empty_bearer_credential() {
        let mut headers = HeaderMap::new();
        headers.insert(HEADER_SESSION_TOKEN_KEY, HeaderValue::from_static("Bearer"));

        let parts = create_parts_with_headers(headers);
        assert!(parse_authorization_header(&parts.headers, false).is_err());
    }

    #[test]
    fn authorization_header_rejects_unsupported_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_SESSION_TOKEN_KEY,
            HeaderValue::from_static("Basic dXNlcjpwYXNz"),
        );

        let parts = create_parts_with_headers(headers);
        assert!(parse_authorization_header(&parts.headers, true).is_err());
    }

    #[test]
    fn authorization_header_rejects_duplicate_values() {
        let mut headers = HeaderMap::new();
        headers.append(
            HEADER_SESSION_TOKEN_KEY,
            HeaderValue::from_static("Bearer first"),
        );
        headers.append(
            HEADER_SESSION_TOKEN_KEY,
            HeaderValue::from_static("Bearer second"),
        );

        let parts = create_parts_with_headers(headers);
        assert!(parse_authorization_header(&parts.headers, true).is_err());
    }

    #[test]
    fn authorization_header_rejects_whitespace_inside_credential() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_SESSION_TOKEN_KEY,
            HeaderValue::from_static("Bearer first second"),
        );

        let parts = create_parts_with_headers(headers);
        assert!(parse_authorization_header(&parts.headers, false).is_err());
    }

    #[test]
    fn authorization_header_rejects_non_ascii_without_panicking() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_SESSION_TOKEN_KEY,
            HeaderValue::from_bytes("éééé".as_bytes()).unwrap(),
        );

        let parts = create_parts_with_headers(headers);
        assert!(parse_authorization_header(&parts.headers, true).is_err());
    }

    #[test]
    fn legacy_raw_authorization_requires_compatibility_mode() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_SESSION_TOKEN_KEY,
            HeaderValue::from_static("legacy-token.with-symbols_123"),
        );
        let parts = create_parts_with_headers(headers);

        assert!(parse_authorization_header(&parts.headers, false).is_err());
        assert_eq!(
            parse_authorization_header(&parts.headers, true)
                .unwrap()
                .as_deref(),
            Some("legacy-token.with-symbols_123")
        );
    }

    #[test]
    fn session_extraction_error_access_denied_status_code() {
        let error = SessionExtractionError::AccessDenied;
        let response = error.into_response();
        // 401 Unauthorized - not authenticated
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn session_extraction_error_internal_error_status_code() {
        let error = SessionExtractionError::InternalError;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn session_executor_saturation_is_retryable() {
        let response = SessionExtractionError::Database(DbRunError::QueueTimeout).into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()[axum::http::header::RETRY_AFTER], "1");
    }

    #[test]
    fn session_debug_format() {
        let session = Session {
            user_id: 42,
            token: "secret-token".to_string(),
            permissions: vec![Permission::AccessCatalog],
            device_id: Some(123),
            device_type: Some(DeviceType::Web),
        };

        let debug_str = format!("{:?}", session);
        assert!(debug_str.contains("user_id"));
        assert!(debug_str.contains("42"));
        assert!(debug_str.contains("token"));
        assert!(debug_str.contains("secret-token"));
        assert!(debug_str.contains("permissions"));
        assert!(debug_str.contains("device_id"));
        assert!(debug_str.contains("device_type"));
    }

    #[test]
    fn cookie_and_header_constants() {
        assert_eq!(COOKIE_SESSION_TOKEN_KEY, "session_token");
        assert_eq!(HEADER_SESSION_TOKEN_KEY, "Authorization");
    }
}
