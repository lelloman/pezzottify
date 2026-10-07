//! OIDC authentication module for Pezzottify.
//!
//! This module handles OpenID Connect authentication flow:
//! - Authorization URL generation with PKCE
//! - Token exchange (authorization code for tokens)
//! - JWT validation using JWKS
//! - Session state management

use anyhow::{anyhow, Context, Result};
use openidconnect::core::{CoreAuthenticationFlow, CoreIdTokenClaims, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex as AsyncMutex, RwLock as AsyncRwLock};
use tracing::{debug, info, warn};

use crate::config::OidcConfig;

/// HTTP client for OIDC requests
fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("Failed to create HTTP client")
}

/// State stored during the authorization flow (between /login and /callback)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    /// CSRF token for state validation
    pub csrf_token: String,
    /// Nonce for ID token validation
    pub nonce: String,
    /// PKCE code verifier (stored server-side for security)
    pub pkce_verifier: String,
    /// Timestamp when this state was created (for expiration)
    pub created_at: i64,
    /// Device ID for multi-device tracking (passed to the OIDC provider)
    pub device_id: Option<String>,
    /// Device type (web, android, ios, desktop)
    pub device_type: Option<String>,
    /// Human-readable device name
    pub device_name: Option<String>,
}

/// Device info passed to OIDC authorization
#[derive(Debug, Clone, Default)]
pub struct DeviceInfo {
    pub device_id: Option<String>,
    pub device_type: Option<String>,
    pub device_name: Option<String>,
}

/// Result of a successful token exchange
#[derive(Debug, Clone)]
pub struct AuthResult {
    /// The OIDC subject claim (user identifier from the IdP)
    pub subject: String,
    /// User's email (if available)
    pub email: Option<String>,
    /// User's preferred username (if available)
    pub preferred_username: Option<String>,
}

/// OIDC client wrapper
pub struct OidcClient {
    issuer_url: IssuerUrl,
    provider_metadata: RwLock<CoreProviderMetadata>,
    metadata_refreshed_at: Mutex<Instant>,
    metadata_refresh_attempted_at: Mutex<Option<Instant>>,
    metadata_refresh_guard: AsyncMutex<()>,
    client_id: ClientId,
    client_secret: Option<ClientSecret>,
    redirect_url: RedirectUrl,
    scopes: Vec<String>,
}

const JWKS_CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const JWKS_REFRESH_RETRY_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
enum MetadataRefreshReason {
    CacheExpired,
    TokenValidationFailed,
}

fn metadata_refresh_due(age: Duration, reason: MetadataRefreshReason) -> bool {
    match reason {
        MetadataRefreshReason::CacheExpired => age >= JWKS_CACHE_TTL,
        MetadataRefreshReason::TokenValidationFailed => age >= JWKS_REFRESH_RETRY_INTERVAL,
    }
}

impl OidcClient {
    /// Create a new OIDC client by discovering provider metadata
    pub async fn new(config: OidcConfig) -> Result<Self> {
        info!(
            "Initializing OIDC client for provider: {}",
            config.provider_url
        );

        let issuer_url =
            IssuerUrl::new(config.provider_url.clone()).context("Invalid OIDC provider URL")?;

        let http = http_client()?;

        // Discover the provider's metadata (endpoints, keys, etc.)
        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url.clone(), &http)
            .await
            .context("Failed to discover OIDC provider metadata")?;

        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = Some(ClientSecret::new(config.client_secret.clone()));
        let redirect_url =
            RedirectUrl::new(config.redirect_uri.clone()).context("Invalid OIDC redirect URI")?;

        info!("OIDC client initialized successfully");

        Ok(Self {
            issuer_url,
            provider_metadata: RwLock::new(provider_metadata),
            metadata_refreshed_at: Mutex::new(Instant::now()),
            metadata_refresh_attempted_at: Mutex::new(None),
            metadata_refresh_guard: AsyncMutex::new(()),
            client_id,
            client_secret,
            redirect_url,
            scopes: config.scopes,
        })
    }

    fn provider_metadata(&self) -> CoreProviderMetadata {
        self.provider_metadata
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn metadata_age(&self) -> Duration {
        self.metadata_refreshed_at
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .elapsed()
    }

    fn refresh_due(&self, reason: MetadataRefreshReason) -> bool {
        if matches!(reason, MetadataRefreshReason::CacheExpired)
            && !metadata_refresh_due(self.metadata_age(), reason)
        {
            return false;
        }

        self.metadata_refresh_attempted_at
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .map(|attempted_at| attempted_at.elapsed() >= JWKS_REFRESH_RETRY_INTERVAL)
            .unwrap_or(true)
    }

    async fn refresh_metadata(&self, reason: MetadataRefreshReason) -> Result<bool> {
        if !self.refresh_due(reason) {
            return Ok(false);
        }

        // Serialize refreshes and re-check after acquiring the guard so a burst of
        // requests following a key rotation results in one discovery request.
        let _guard = self.metadata_refresh_guard.lock().await;
        if !self.refresh_due(reason) {
            return Ok(false);
        }

        *self
            .metadata_refresh_attempted_at
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Instant::now());

        let http = http_client()?;
        let metadata = CoreProviderMetadata::discover_async(self.issuer_url.clone(), &http)
            .await
            .context("Failed to refresh OIDC provider metadata and signing keys")?;

        *self
            .provider_metadata
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = metadata;
        *self
            .metadata_refreshed_at
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Instant::now();
        info!("Refreshed OIDC provider metadata and signing keys");
        Ok(true)
    }

    /// Generate an authorization URL for the OIDC flow
    ///
    /// Returns the URL to redirect the user to, along with the state that must
    /// be stored server-side and validated in the callback.
    pub fn authorize_url(&self, device_info: Option<&DeviceInfo>) -> Result<(String, AuthState)> {
        use openidconnect::core::CoreClient;

        let client = CoreClient::from_provider_metadata(
            self.provider_metadata(),
            self.client_id.clone(),
            self.client_secret.clone(),
        )
        .set_redirect_uri(self.redirect_url.clone());

        // Generate PKCE challenge
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Build the authorization request
        let mut auth_request = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .set_pkce_challenge(pkce_challenge);

        // Add requested scopes
        for scope in &self.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, csrf_token, nonce) = auth_request.url();

        // Add device info to URL if provided
        let mut final_url = auth_url.to_string();
        if let Some(device) = device_info {
            if let Some(id) = &device.device_id {
                final_url.push_str(&format!("&device_id={}", urlencoding::encode(id)));
            }
            if let Some(dtype) = &device.device_type {
                final_url.push_str(&format!("&device_type={}", urlencoding::encode(dtype)));
            }
            if let Some(name) = &device.device_name {
                final_url.push_str(&format!("&device_name={}", urlencoding::encode(name)));
            }
        }

        let state = AuthState {
            csrf_token: csrf_token.secret().clone(),
            nonce: nonce.secret().clone(),
            pkce_verifier: pkce_verifier.secret().clone(),
            created_at: chrono::Utc::now().timestamp(),
            device_id: device_info.and_then(|d| d.device_id.clone()),
            device_type: device_info.and_then(|d| d.device_type.clone()),
            device_name: device_info.and_then(|d| d.device_name.clone()),
        };

        debug!(
            "Generated authorization URL with state: {}, device_id: {:?}",
            state.csrf_token, state.device_id
        );

        Ok((final_url, state))
    }

    /// Exchange an authorization code for tokens
    ///
    /// Validates the state, exchanges the code, and validates the ID token.
    pub async fn exchange_code(
        &self,
        code: &str,
        state: &str,
        stored_state: &AuthState,
    ) -> Result<AuthResult> {
        use openidconnect::core::CoreClient;

        // Validate CSRF state
        if state != stored_state.csrf_token {
            return Err(anyhow!("CSRF state mismatch"));
        }

        // Check state expiration (5 minutes)
        let now = chrono::Utc::now().timestamp();
        if now - stored_state.created_at > 300 {
            return Err(anyhow!("Authorization state expired"));
        }

        if let Err(error) = self
            .refresh_metadata(MetadataRefreshReason::CacheExpired)
            .await
        {
            warn!("Could not refresh expired OIDC metadata cache: {error}");
        }

        let client = CoreClient::from_provider_metadata(
            self.provider_metadata(),
            self.client_id.clone(),
            self.client_secret.clone(),
        )
        .set_redirect_uri(self.redirect_url.clone());

        let http = http_client()?;

        // Reconstruct the PKCE verifier
        let pkce_verifier = PkceCodeVerifier::new(stored_state.pkce_verifier.clone());

        // Exchange the code for tokens
        let token_response = client
            .exchange_code(AuthorizationCode::new(code.to_string()))?
            .set_pkce_verifier(pkce_verifier)
            .request_async(&http)
            .await
            .map_err(|e| anyhow!("Failed to exchange authorization code: {}", e))?;

        // Get the ID token
        let id_token = token_response
            .id_token()
            .ok_or_else(|| anyhow!("Server did not return an ID token"))?;

        // Verify the ID token
        let nonce = Nonce::new(stored_state.nonce.clone());
        let id_token_verifier = client.id_token_verifier();
        let signing_key_missing = id_token.signing_key(&id_token_verifier).is_err();
        let claims_result = id_token.claims(&id_token_verifier, &nonce);
        let claims: &CoreIdTokenClaims = match claims_result {
            Ok(claims) => claims,
            Err(first_error) => {
                if !signing_key_missing {
                    return Err(anyhow!("Failed to verify ID token: {first_error}"));
                }
                let refreshed = self
                    .refresh_metadata(MetadataRefreshReason::TokenValidationFailed)
                    .await
                    .unwrap_or_else(|refresh_error| {
                        warn!("Could not refresh OIDC signing keys after validation failed: {refresh_error}");
                        false
                    });
                if !refreshed {
                    return Err(anyhow!("Failed to verify ID token: {first_error}"));
                }
                let refreshed_client = CoreClient::from_provider_metadata(
                    self.provider_metadata(),
                    self.client_id.clone(),
                    self.client_secret.clone(),
                );
                let refreshed_verifier = refreshed_client.id_token_verifier();
                id_token
                    .claims(&refreshed_verifier, &nonce)
                    .map_err(|e| anyhow!("Failed to verify ID token after key refresh: {e}"))?
            }
        };

        // Extract user information from claims
        let subject = claims.subject().to_string();
        let email = claims.email().map(|e| e.to_string());
        let preferred_username = claims.preferred_username().map(|u| u.as_str().to_string());

        debug!("Successfully authenticated user with subject: {}", subject);

        Ok(AuthResult {
            subject,
            email,
            preferred_username,
        })
    }

    /// Get the logout URL for the OIDC provider (if supported)
    pub fn logout_url(
        &self,
        _id_token_hint: Option<&str>,
        _post_logout_redirect: Option<&str>,
    ) -> Option<String> {
        // Try to get the end_session_endpoint from provider metadata
        // This is optional in OIDC and may not be available
        // For now, return None - can be implemented if the provider supports it
        None
    }

    /// Validate an ID token and extract the subject claim
    ///
    /// This is used for session validation - validates the JWT signature
    /// using the provider's JWKS and extracts the user identifier.
    ///
    /// Note: This compatibility path skips nonce validation because it accepts an
    /// already-issued bearer token, not the fresh token handled by the callback.
    pub async fn validate_id_token(&self, id_token_str: &str) -> Result<IdTokenClaims> {
        use openidconnect::core::{CoreClient, CoreIdToken};

        // Parse the ID token
        let id_token: CoreIdToken = CoreIdToken::from_str(id_token_str)
            .map_err(|e| anyhow!("Failed to parse ID token: {}", e))?;

        if let Err(error) = self
            .refresh_metadata(MetadataRefreshReason::CacheExpired)
            .await
        {
            warn!("Could not refresh expired OIDC metadata cache: {error}");
        }

        // Create client for verification
        let client = CoreClient::from_provider_metadata(
            self.provider_metadata(),
            self.client_id.clone(),
            self.client_secret.clone(),
        );

        // Get the verifier (skips nonce validation for session tokens)
        let verifier = client.id_token_verifier();
        let signing_key_missing = id_token.signing_key(&verifier).is_err();

        // Verify the token signature and claims (without nonce)
        // We use an empty nonce which causes nonce validation to be skipped
        let claims_result = id_token.claims(&verifier, |_: Option<&Nonce>| Ok(()));
        let claims = match claims_result {
            Ok(claims) => claims,
            Err(first_error) => {
                if !signing_key_missing {
                    return Err(anyhow!("Failed to verify ID token: {first_error}"));
                }
                let refreshed = self
                    .refresh_metadata(MetadataRefreshReason::TokenValidationFailed)
                    .await
                    .unwrap_or_else(|refresh_error| {
                        warn!("Could not refresh OIDC signing keys after validation failed: {refresh_error}");
                        false
                    });
                if !refreshed {
                    return Err(anyhow!("Failed to verify ID token: {first_error}"));
                }
                let refreshed_client = CoreClient::from_provider_metadata(
                    self.provider_metadata(),
                    self.client_id.clone(),
                    self.client_secret.clone(),
                );
                let refreshed_verifier = refreshed_client.id_token_verifier();
                id_token
                    .claims(&refreshed_verifier, |_: Option<&Nonce>| Ok(()))
                    .map_err(|e| anyhow!("Failed to verify ID token after key refresh: {e}"))?
            }
        };

        let subject = claims.subject().to_string();
        let email = claims.email().map(|e| e.to_string());
        let preferred_username = claims.preferred_username().map(|u| u.as_str().to_string());
        let expiration = claims.expiration().timestamp();

        // Check if token is expired
        let now = chrono::Utc::now().timestamp();
        if now > expiration {
            return Err(anyhow!("ID token has expired"));
        }

        // Extract custom device claims from the JWT payload
        // JWT format: header.payload.signature
        let device_claims = extract_device_claims(id_token_str);

        debug!(
            "Validated ID token for subject: {}, device_id: {:?}, device_type: {:?}",
            subject, device_claims.device_id, device_claims.device_type
        );

        Ok(IdTokenClaims {
            subject,
            email,
            preferred_username,
            expiration,
            device_id: device_claims.device_id,
            device_type: device_claims.device_type,
            device_name: device_claims.device_name,
        })
    }
}

/// Device claims extracted from the JWT payload.
#[derive(Debug, Clone, Default)]
pub struct JwtDeviceClaims {
    pub device_id: Option<String>,
    pub device_type: Option<String>,
    pub device_name: Option<String>,
}

/// Extract device claims from a JWT payload.
/// Returns default (all None) if the JWT is malformed or claims are missing.
fn extract_device_claims(jwt: &str) -> JwtDeviceClaims {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    // JWT format: header.payload.signature
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return JwtDeviceClaims::default();
    }

    // Decode the payload (second part)
    let payload_bytes = match URL_SAFE_NO_PAD.decode(parts[1]) {
        Ok(bytes) => bytes,
        Err(_) => return JwtDeviceClaims::default(),
    };

    let payload: serde_json::Value = match serde_json::from_slice(&payload_bytes) {
        Ok(v) => v,
        Err(_) => return JwtDeviceClaims::default(),
    };

    JwtDeviceClaims {
        device_id: payload
            .get("device_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        device_type: payload
            .get("device_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        device_name: payload
            .get("device_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    }
}

/// Claims extracted from a validated ID token
#[derive(Debug, Clone)]
pub struct IdTokenClaims {
    /// The OIDC subject claim (user identifier from the IdP)
    pub subject: String,
    /// User's email (if available)
    pub email: Option<String>,
    /// User's preferred username (if available)
    pub preferred_username: Option<String>,
    /// Token expiration timestamp (Unix seconds)
    pub expiration: i64,
    /// Device ID (custom claim from LelloAuth for multi-device tracking)
    pub device_id: Option<String>,
    /// Device type (web, android, ios, desktop)
    pub device_type: Option<String>,
    /// Human-readable device name
    pub device_name: Option<String>,
}

/// Thread-safe storage for auth states (in-memory for simplicity)
/// In production, consider using Redis or a similar store for distributed deployments
pub struct AuthStateStore {
    states: AsyncRwLock<std::collections::HashMap<String, AuthState>>,
}

impl AuthStateStore {
    pub fn new() -> Self {
        Self {
            states: AsyncRwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Store an auth state, keyed by the CSRF token
    pub async fn store(&self, state: AuthState) {
        let key = state.csrf_token.clone();
        let mut states = self.states.write().await;
        states.insert(key, state);
    }

    /// Retrieve and remove an auth state by CSRF token
    pub async fn take(&self, csrf_token: &str) -> Option<AuthState> {
        let mut states = self.states.write().await;
        states.remove(csrf_token)
    }

    /// Clean up expired states (older than 5 minutes)
    pub async fn cleanup_expired(&self) {
        let now = chrono::Utc::now().timestamp();
        let mut states = self.states.write().await;
        states.retain(|_, state| now - state.created_at < 300);
    }
}

impl Default for AuthStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_cache_refreshes_at_ttl_boundary() {
        assert!(!metadata_refresh_due(
            JWKS_CACHE_TTL - Duration::from_secs(1),
            MetadataRefreshReason::CacheExpired
        ));
        assert!(metadata_refresh_due(
            JWKS_CACHE_TTL,
            MetadataRefreshReason::CacheExpired
        ));
    }

    #[test]
    fn validation_failure_refresh_is_rate_limited() {
        assert!(!metadata_refresh_due(
            JWKS_REFRESH_RETRY_INTERVAL - Duration::from_secs(1),
            MetadataRefreshReason::TokenValidationFailed
        ));
        assert!(metadata_refresh_due(
            JWKS_REFRESH_RETRY_INTERVAL,
            MetadataRefreshReason::TokenValidationFailed
        ));
    }

    #[test]
    fn test_auth_state_store() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = AuthStateStore::new();

            let state = AuthState {
                csrf_token: "test-csrf".to_string(),
                nonce: "test-nonce".to_string(),
                pkce_verifier: "test-verifier".to_string(),
                created_at: chrono::Utc::now().timestamp(),
                device_id: None,
                device_type: None,
                device_name: None,
            };

            store.store(state.clone()).await;

            // Should retrieve the state
            let retrieved = store.take("test-csrf").await;
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().nonce, "test-nonce");

            // Should not retrieve again (was removed)
            let retrieved_again = store.take("test-csrf").await;
            assert!(retrieved_again.is_none());
        });
    }

    #[test]
    fn test_auth_state_expiration() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = AuthStateStore::new();

            // Create an expired state
            let state = AuthState {
                csrf_token: "expired-csrf".to_string(),
                nonce: "test-nonce".to_string(),
                pkce_verifier: "test-verifier".to_string(),
                created_at: chrono::Utc::now().timestamp() - 400, // 6+ minutes ago
                device_id: None,
                device_type: None,
                device_name: None,
            };

            store.store(state).await;
            store.cleanup_expired().await;

            // Should not retrieve expired state
            let retrieved = store.take("expired-csrf").await;
            assert!(retrieved.is_none());
        });
    }

    #[test]
    fn test_extract_device_claims_full() {
        use base64::Engine;

        // A fake JWT with all device claims in payload
        let payload = r#"{"sub":"user123","device_id":"test-device-abc","device_type":"android","device_name":"My Phone"}"#;
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload);
        let fake_jwt = format!("header.{}.signature", payload_b64);

        let result = extract_device_claims(&fake_jwt);
        assert_eq!(result.device_id, Some("test-device-abc".to_string()));
        assert_eq!(result.device_type, Some("android".to_string()));
        assert_eq!(result.device_name, Some("My Phone".to_string()));
    }

    #[test]
    fn test_extract_device_claims_partial() {
        use base64::Engine;

        // Only device_id, no type or name
        let payload = r#"{"sub":"user123","device_id":"test-device-abc"}"#;
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload);
        let fake_jwt = format!("header.{}.signature", payload_b64);

        let result = extract_device_claims(&fake_jwt);
        assert_eq!(result.device_id, Some("test-device-abc".to_string()));
        assert_eq!(result.device_type, None);
        assert_eq!(result.device_name, None);
    }

    #[test]
    fn test_extract_device_claims_none() {
        use base64::Engine;

        // No device claims at all
        let payload = r#"{"sub":"user123"}"#;
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload);
        let fake_jwt = format!("header.{}.signature", payload_b64);

        let result = extract_device_claims(&fake_jwt);
        assert_eq!(result.device_id, None);
        assert_eq!(result.device_type, None);
        assert_eq!(result.device_name, None);
    }

    #[test]
    fn test_extract_device_claims_invalid_jwt() {
        // Invalid JWT format
        let result = extract_device_claims("not-a-jwt");
        assert_eq!(result.device_id, None);
        assert_eq!(result.device_type, None);
        assert_eq!(result.device_name, None);

        // Invalid base64
        let result = extract_device_claims("header.!!!invalid!!!.signature");
        assert_eq!(result.device_id, None);
    }
}
