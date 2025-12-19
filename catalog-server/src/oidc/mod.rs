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
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tokio::sync::RwLock;
use tracing::{debug, info};

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
    /// The raw access token
    pub access_token: String,
    /// The raw ID token (serialized)
    pub id_token: String,
}

/// OIDC client wrapper
pub struct OidcClient {
    provider_metadata: CoreProviderMetadata,
    client_id: ClientId,
    client_secret: Option<ClientSecret>,
    redirect_url: RedirectUrl,
    scopes: Vec<String>,
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
        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &http)
            .await
            .context("Failed to discover OIDC provider metadata")?;

        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = Some(ClientSecret::new(config.client_secret.clone()));
        let redirect_url =
            RedirectUrl::new(config.redirect_uri.clone()).context("Invalid OIDC redirect URI")?;

        info!("OIDC client initialized successfully");

        Ok(Self {
            provider_metadata,
            client_id,
            client_secret,
            redirect_url,
            scopes: config.scopes,
        })
    }

    /// Generate an authorization URL for the OIDC flow
    ///
    /// Returns the URL to redirect the user to, along with the state that must
    /// be stored server-side and validated in the callback.
    pub fn authorize_url(&self) -> Result<(String, AuthState)> {
        use openidconnect::core::CoreClient;

        let client = CoreClient::from_provider_metadata(
            self.provider_metadata.clone(),
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

        let state = AuthState {
            csrf_token: csrf_token.secret().clone(),
            nonce: nonce.secret().clone(),
            pkce_verifier: pkce_verifier.secret().clone(),
            created_at: chrono::Utc::now().timestamp(),
        };

        debug!(
            "Generated authorization URL with state: {}",
            state.csrf_token
        );

        Ok((auth_url.to_string(), state))
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

        let client = CoreClient::from_provider_metadata(
            self.provider_metadata.clone(),
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
        let claims: &CoreIdTokenClaims = id_token
            .claims(&id_token_verifier, &nonce)
            .map_err(|e| anyhow!("Failed to verify ID token: {}", e))?;

        // Extract user information from claims
        let subject = claims.subject().to_string();
        let email = claims.email().map(|e| e.to_string());
        let preferred_username = claims
            .preferred_username()
            .map(|u| u.as_str().to_string());

        debug!("Successfully authenticated user with subject: {}", subject);

        Ok(AuthResult {
            subject,
            email,
            preferred_username,
            access_token: token_response.access_token().secret().clone(),
            id_token: id_token.to_string(),
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
    /// Note: This skips nonce validation since we're validating a stored session token,
    /// not a fresh token from the auth callback.
    pub fn validate_id_token(&self, id_token_str: &str) -> Result<IdTokenClaims> {
        use openidconnect::core::{CoreClient, CoreIdToken};

        // Parse the ID token
        let id_token: CoreIdToken = CoreIdToken::from_str(id_token_str)
            .map_err(|e| anyhow!("Failed to parse ID token: {}", e))?;

        // Create client for verification
        let client = CoreClient::from_provider_metadata(
            self.provider_metadata.clone(),
            self.client_id.clone(),
            self.client_secret.clone(),
        );

        // Get the verifier (skips nonce validation for session tokens)
        let verifier = client.id_token_verifier();

        // Verify the token signature and claims (without nonce)
        // We use an empty nonce which causes nonce validation to be skipped
        let claims = id_token
            .claims(&verifier, |_: Option<&Nonce>| Ok(()))
            .map_err(|e| anyhow!("Failed to verify ID token: {}", e))?;

        let subject = claims.subject().to_string();
        let email = claims.email().map(|e| e.to_string());
        let preferred_username = claims
            .preferred_username()
            .map(|u| u.as_str().to_string());
        let expiration = claims.expiration().timestamp();

        // Check if token is expired
        let now = chrono::Utc::now().timestamp();
        if now > expiration {
            return Err(anyhow!("ID token has expired"));
        }

        debug!("Validated ID token for subject: {}", subject);

        Ok(IdTokenClaims {
            subject,
            email,
            preferred_username,
            expiration,
        })
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
}

/// Thread-safe storage for auth states (in-memory for simplicity)
/// In production, consider using Redis or a similar store for distributed deployments
pub struct AuthStateStore {
    states: RwLock<std::collections::HashMap<String, AuthState>>,
}

impl AuthStateStore {
    pub fn new() -> Self {
        Self {
            states: RwLock::new(std::collections::HashMap::new()),
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
    fn test_auth_state_store() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = AuthStateStore::new();

            let state = AuthState {
                csrf_token: "test-csrf".to_string(),
                nonce: "test-nonce".to_string(),
                pkce_verifier: "test-verifier".to_string(),
                created_at: chrono::Utc::now().timestamp(),
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
            };

            store.store(state).await;
            store.cleanup_expired().await;

            // Should not retrieve expired state
            let retrieved = store.take("expired-csrf").await;
            assert!(retrieved.is_none());
        });
    }
}
