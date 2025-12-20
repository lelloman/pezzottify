use super::state::ServerState;
use crate::user::auth::AuthTokenValue;
use crate::user::device::{DeviceRegistration, DeviceType};
use crate::user::Permission;

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use tracing::{debug, warn};

#[derive(Debug)]
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

/// Try to validate the token as an OIDC JWT and create a session
async fn try_oidc_session(token: &str, ctx: &ServerState) -> Option<Session> {
    // Check if OIDC is configured
    let oidc_client = ctx.oidc_client.as_ref()?;

    // Try to validate as OIDC ID token
    let claims = match oidc_client.validate_id_token(token) {
        Ok(claims) => {
            debug!("Validated OIDC ID token for subject={}", claims.subject);
            claims
        }
        Err(e) => {
            // Not a valid OIDC token - this is expected for legacy sessions
            debug!("Token is not a valid OIDC ID token: {}", e);
            return None;
        }
    };

    // Look up or provision local user by OIDC subject
    let user_manager = ctx.user_manager.lock().unwrap();
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
                    return None;
                }
            }
        }
        Err(e) => {
            warn!("Failed to look up user by OIDC subject: {}", e);
            return None;
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
            return None;
        }
    };

    // Extract device info from the ID token claims and look up/register the device
    let (device_id, device_type) = if let Some(device_uuid) = &claims.device_id {
        // Get device_type from JWT claims, defaulting to "web" if not provided
        let jwt_device_type = claims.device_type.as_deref().unwrap_or("web");

        // First try to find existing device by UUID
        match user_manager.get_device_by_uuid(device_uuid) {
            Ok(Some(device)) => {
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

    Some(Session {
        user_id,
        token: token.to_string(),
        permissions,
        device_id,
        device_type,
    })
}

/// Try to validate the token as a legacy database auth token
async fn try_legacy_session(token: &str, ctx: &ServerState) -> Option<Session> {
    let user_manager = ctx.user_manager.lock().unwrap();
    let auth_token_value = AuthTokenValue(token.to_string());
    let auth_token = match user_manager.get_auth_token(&auth_token_value) {
        Ok(Some(token)) => {
            debug!("Found legacy auth token for user_id={}", token.user_id);

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
            debug!(
                "Resolved permissions for user_id={}: {:?}",
                auth_token.user_id, perms
            );
            perms
        }
        Err(e) => {
            debug!(
                "Failed to resolve permissions for user_id={}: {}",
                auth_token.user_id, e
            );
            return None;
        }
    };

    // Look up device info if device_id is present
    let (device_id, device_type) = if let Some(device_id) = auth_token.device_id {
        match user_manager.get_device(device_id) {
            Ok(Some(device)) => {
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
            Err(e) => {
                debug!(
                    "Failed to get device info for device_id={}: {}",
                    device_id, e
                );
                (Some(device_id), None)
            }
        }
    } else {
        (None, None)
    };

    Some(Session {
        user_id: auth_token.user_id,
        token: auth_token.value.0,
        permissions,
        device_id,
        device_type,
    })
}

async fn extract_session_from_request_parts(
    parts: &mut Parts,
    ctx: &ServerState,
) -> Option<Session> {
    debug!("extracting session from request parts...");
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

    debug!("Got session token (length={})", token.len());

    // Try OIDC JWT validation first (if OIDC is configured)
    if let Some(session) = try_oidc_session(&token, ctx).await {
        debug!("Session validated via OIDC for user_id={}", session.user_id);
        return Some(session);
    }

    // Fall back to legacy database token lookup
    if let Some(session) = try_legacy_session(&token, ctx).await {
        debug!(
            "Session validated via legacy auth for user_id={}",
            session.user_id
        );
        return Some(session);
    }

    debug!("Token validation failed for both OIDC and legacy auth");
    None
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
        headers.insert(
            "authorization",
            HeaderValue::from_static("lowercase-header"),
        );

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
