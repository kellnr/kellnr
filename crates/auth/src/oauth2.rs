//! OAuth2/OpenID Connect authentication handler
//!
//! This module provides OAuth2/OIDC authentication support using the authorization
//! code flow with PKCE.

use std::future::Future;
use std::sync::{Arc, Mutex};

use kellnr_settings::OAuth2 as OAuth2Settings;
use openidconnect::core::{
    CoreAuthDisplay, CoreAuthPrompt, CoreAuthenticationFlow, CoreClient, CoreErrorResponseType,
    CoreGenderClaim, CoreIdTokenClaims, CoreJsonWebKey, CoreJweContentEncryptionAlgorithm,
    CoreProviderMetadata, CoreRevocationErrorResponse, CoreTokenIntrospectionResponse,
    CoreTokenResponse,
};
use openidconnect::{
    AuthorizationCode, ClaimsVerificationError, ClientId, ClientSecret, CsrfToken,
    EmptyAdditionalClaims, EndpointMaybeSet, EndpointNotSet, EndpointSet, IssuerUrl, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, reqwest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, trace, warn};
use url::Url;

/// Type alias for the configured OIDC client after provider discovery
type ConfiguredCoreClient = openidconnect::Client<
    EmptyAdditionalClaims,
    CoreAuthDisplay,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJsonWebKey,
    CoreAuthPrompt,
    openidconnect::StandardErrorResponse<CoreErrorResponseType>,
    CoreTokenResponse,
    CoreTokenIntrospectionResponse,
    openidconnect::core::CoreRevocableToken,
    CoreRevocationErrorResponse,
    EndpointSet,      // HasAuthUrl - set by from_provider_metadata
    EndpointNotSet,   // HasDeviceAuthUrl
    EndpointNotSet,   // HasIntrospectionUrl
    EndpointNotSet,   // HasRevocationUrl
    EndpointMaybeSet, // HasTokenUrl - maybe set by provider
    EndpointMaybeSet, // HasUserInfoUrl - maybe set by provider
>;

/// Errors that can occur during OAuth2/OIDC operations
#[derive(Debug, Error)]
pub enum OAuth2Error {
    #[error("OAuth2 is not enabled")]
    NotEnabled,

    #[error("OAuth2 configuration is invalid: {0}")]
    ConfigurationError(String),

    #[error("Failed to discover OIDC provider: {0}")]
    DiscoveryError(String),

    #[error("Failed to parse URL: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("Failed to exchange authorization code: {0}")]
    TokenExchangeError(String),

    #[error("Failed to verify ID token: {0}")]
    TokenVerificationError(String),

    #[error("Missing required claim: {0}")]
    MissingClaim(String),

    #[error("HTTP request failed: {0}")]
    HttpError(String),
}

/// Information extracted from the OIDC ID token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// The subject claim (unique user identifier from the provider)
    pub subject: String,
    /// Email address (if available)
    pub email: Option<String>,
    /// Preferred username (if available)
    pub preferred_username: Option<String>,
    /// Groups the user belongs to (from the configured claim)
    pub groups: Vec<String>,
    /// Whether the user should be an admin (derived from group claims)
    pub is_admin: bool,
    /// Whether the user should be read-only (derived from group claims)
    pub is_read_only: bool,
}

/// Request data for initiating `OAuth2` authentication
#[derive(Debug)]
pub struct AuthRequest {
    /// The URL to redirect the user to for authentication
    pub auth_url: Url,
    /// CSRF protection state (to be stored in database)
    pub state: String,
    /// PKCE verifier (to be stored in database for code exchange)
    pub pkce_verifier: String,
    /// Nonce for ID token verification (to be stored in database)
    pub nonce: String,
}

/// Token response from the `OAuth2` provider
#[derive(Debug)]
pub struct TokenResult {
    /// The standard ID token claims
    pub claims: CoreIdTokenClaims,
    /// Raw JWT payload for extracting additional claims
    pub raw_payload: serde_json::Value,
}

/// OAuth2/OIDC authentication handler
pub struct OAuth2Handler {
    /// The configured OIDC client, wrapped so it can be rebuilt when the
    /// provider rotates its signing keys (see [`Self::rediscover`]).
    client: Mutex<Arc<ConfiguredCoreClient>>,
    client_id: ClientId,
    client_secret: Option<ClientSecret>,
    redirect_url: RedirectUrl,
    settings: Arc<OAuth2Settings>,
    issuer_url: IssuerUrl,
    http_client: reqwest::Client,
}

impl OAuth2Handler {
    /// Create a new `OAuth2Handler` using OIDC discovery
    ///
    /// This performs automatic discovery of the OIDC provider's configuration
    /// using the well-known endpoint.
    pub async fn from_discovery(
        settings: &OAuth2Settings,
        redirect_url: &str,
    ) -> Result<Self, OAuth2Error> {
        if !settings.enabled {
            return Err(OAuth2Error::NotEnabled);
        }

        // Validate configuration
        settings
            .validate()
            .map_err(OAuth2Error::ConfigurationError)?;

        let issuer_url_str = settings
            .issuer_url
            .as_ref()
            .ok_or_else(|| OAuth2Error::ConfigurationError("Missing issuer_url".to_string()))?;

        let issuer_url = IssuerUrl::new(issuer_url_str.clone())
            .map_err(|e| OAuth2Error::ConfigurationError(format!("Invalid issuer URL: {e}")))?;

        info!("Discovering OIDC provider at: {}", issuer_url_str);

        // Create HTTP client
        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| OAuth2Error::HttpError(e.to_string()))?;

        let client_id = ClientId::new(
            settings
                .client_id
                .clone()
                .ok_or_else(|| OAuth2Error::ConfigurationError("Missing client_id".to_string()))?,
        );

        let client_secret = settings.client_secret.clone().map(ClientSecret::new);

        let redirect_url = RedirectUrl::new(redirect_url.to_string())?;

        // Perform OIDC discovery and build the client from provider metadata
        let client = Self::discover_client(
            &issuer_url,
            &client_id,
            client_secret.as_ref(),
            &redirect_url,
            &http_client,
        )
        .await?;

        Ok(Self {
            client: Mutex::new(Arc::new(client)),
            client_id,
            client_secret,
            redirect_url,
            settings: Arc::new(settings.clone()),
            issuer_url,
            http_client,
        })
    }

    /// Perform OIDC discovery and build a configured client from the fetched
    /// provider metadata (which includes the current JWKS signing keys).
    async fn discover_client(
        issuer_url: &IssuerUrl,
        client_id: &ClientId,
        client_secret: Option<&ClientSecret>,
        redirect_url: &RedirectUrl,
        http_client: &reqwest::Client,
    ) -> Result<ConfiguredCoreClient, OAuth2Error> {
        let provider_metadata =
            CoreProviderMetadata::discover_async(issuer_url.clone(), http_client)
                .await
                .map_err(|e| OAuth2Error::DiscoveryError(e.to_string()))?;

        Ok(CoreClient::from_provider_metadata(
            provider_metadata,
            client_id.clone(),
            client_secret.cloned(),
        )
        .set_redirect_uri(redirect_url.clone()))
    }

    fn current_client(&self) -> Arc<ConfiguredCoreClient> {
        self.client
            .lock()
            .expect("OAuth2 client mutex poisoned")
            .clone()
    }

    /// Re-run OIDC discovery to obtain a client with a fresh JWKS.
    ///
    /// Used to recover from provider signing-key rotation, after which the
    /// keys cached at startup can no longer verify newly issued tokens.
    async fn rediscover(&self) -> Result<Arc<ConfiguredCoreClient>, OAuth2Error> {
        Self::discover_client(
            &self.issuer_url,
            &self.client_id,
            self.client_secret.as_ref(),
            &self.redirect_url,
            &self.http_client,
        )
        .await
        .map(Arc::new)
    }

    /// Generate an authorization URL for the `OAuth2` flow
    ///
    /// Returns an `AuthRequest` containing the URL to redirect the user to,
    /// along with the state, PKCE verifier, and nonce that must be stored
    /// for later verification.
    pub fn generate_auth_url(&self) -> AuthRequest {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let client = self.current_client();

        // Build the authorization request with configured scopes
        let mut auth_request = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .set_pkce_challenge(pkce_challenge);

        // Add configured scopes
        for scope in &self.settings.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, csrf_state, nonce) = auth_request.url();

        AuthRequest {
            auth_url,
            state: csrf_state.secret().clone(),
            pkce_verifier: pkce_verifier.secret().clone(),
            nonce: nonce.secret().clone(),
        }
    }

    /// Exchange an authorization code for tokens and validate the ID token
    ///
    /// # Arguments
    /// * `code` - The authorization code received from the provider
    /// * `pkce_verifier` - The PKCE verifier stored during `generate_auth_url`
    /// * `nonce` - The nonce stored during `generate_auth_url`
    ///
    /// # Returns
    /// The validated ID token claims
    pub async fn exchange_and_validate(
        &self,
        code: &str,
        pkce_verifier: &str,
        nonce: &str,
    ) -> Result<TokenResult, OAuth2Error> {
        let code = AuthorizationCode::new(code.to_string());
        let pkce_verifier = PkceCodeVerifier::new(pkce_verifier.to_string());

        let client = self.current_client();

        let token_request = client
            .exchange_code(code)
            .map_err(|e| OAuth2Error::TokenExchangeError(e.to_string()))?;

        let token_response: CoreTokenResponse = token_request
            .set_pkce_verifier(pkce_verifier)
            .request_async(&self.http_client)
            .await
            .map_err(|e| OAuth2Error::TokenExchangeError(e.to_string()))?;

        // Get and validate the ID token
        let id_token = token_response
            .id_token()
            .ok_or_else(|| OAuth2Error::MissingClaim("id_token".to_string()))?;

        let nonce = Nonce::new(nonce.to_string());

        let claims = match id_token.claims(&client.id_token_verifier(), &nonce) {
            Ok(claims) => claims.clone(),
            // A signature failure typically means the provider rotated its
            // signing keys since discovery, so the JWKS cached at startup is
            // stale. Re-fetch it once and retry before giving up.
            Err(ClaimsVerificationError::SignatureVerification(_)) => {
                trace!(
                    "ID token signature verification failed; refreshing OIDC JWKS and retrying (provider may have rotated signing keys)"
                );
                let refreshed = self.rediscover().await?;
                let claims = id_token
                    .claims(&refreshed.id_token_verifier(), &nonce)
                    .map_err(|e| OAuth2Error::TokenVerificationError(e.to_string()))?
                    .clone();
                // Persist the refreshed client so subsequent logins reuse the
                // new keys instead of re-fetching on every request.
                *self.client.lock().expect("OAuth2 client mutex poisoned") = refreshed;
                claims
            }
            Err(e) => return Err(OAuth2Error::TokenVerificationError(e.to_string())),
        };

        // Also extract raw payload for additional claims
        let raw_payload = extract_jwt_payload(id_token.to_string().as_str())
            .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));

        Ok(TokenResult {
            claims,
            raw_payload,
        })
    }

    /// Extract user information from ID token claims
    ///
    /// This extracts the subject, email, `preferred_username`, and group membership
    /// based on the `OAuth2` settings configuration.
    pub fn extract_user_info(&self, result: &TokenResult) -> UserInfo {
        let subject = result.claims.subject().as_str().to_string();

        let email = result.claims.email().map(|e| e.as_str().to_string());

        let preferred_username = result
            .claims
            .preferred_username()
            .map(|u| u.as_str().to_string());

        // Extract groups from raw JWT payload
        let groups = self.extract_groups(&result.raw_payload);

        // Determine admin status from group claims
        let is_admin = self.check_group_membership(
            &groups,
            &result.raw_payload,
            self.settings.admin_group_claim.as_deref(),
            self.settings.admin_group_value.as_deref(),
        );

        // Determine read-only status from group claims
        let is_read_only = self.check_group_membership(
            &groups,
            &result.raw_payload,
            self.settings.read_only_group_claim.as_deref(),
            self.settings.read_only_group_value.as_deref(),
        );

        UserInfo {
            subject,
            email,
            preferred_username,
            groups,
            is_admin,
            is_read_only,
        }
    }

    /// Extract groups from the raw JWT payload
    fn extract_groups(&self, payload: &serde_json::Value) -> Vec<String> {
        // First try the configured admin group claim if it exists
        if let Some(claim_name) = &self.settings.admin_group_claim
            && let Some(groups) = get_string_array_from_json(payload, claim_name)
        {
            return groups;
        }

        // Then try the configured read-only group claim if different
        if let Some(claim_name) = &self.settings.read_only_group_claim
            && self.settings.admin_group_claim.as_ref() != Some(claim_name)
            && let Some(groups) = get_string_array_from_json(payload, claim_name)
        {
            return groups;
        }

        // Try common group claim names
        for claim_name in &["groups", "roles", "group"] {
            if let Some(groups) = get_string_array_from_json(payload, claim_name) {
                return groups;
            }
        }

        Vec::new()
    }

    /// Check if the user belongs to a specific group based on claims
    #[allow(clippy::unused_self)]
    fn check_group_membership(
        &self,
        groups: &[String],
        payload: &serde_json::Value,
        claim_name: Option<&str>,
        claim_value: Option<&str>,
    ) -> bool {
        let (Some(claim_name), Some(claim_value)) = (claim_name, claim_value) else {
            return false;
        };

        // First check in the extracted groups
        if groups.iter().any(|g| g == claim_value) {
            return true;
        }

        // Also check directly in the claim (in case it's a different claim than groups)
        if let Some(values) = get_string_array_from_json(payload, claim_name) {
            return values.iter().any(|v| v == claim_value);
        }

        // Check if it's a boolean claim
        if let Some(value) = payload.get(claim_name)
            && let Some(b) = value.as_bool()
        {
            // If the claim is a boolean and we're checking for "true"
            return b && claim_value.eq_ignore_ascii_case("true");
        }

        false
    }

    /// Generate a unique username for auto-provisioning
    ///
    /// Priority:
    /// 1. `preferred_username` claim
    /// 2. Local part of email (before @)
    /// 3. Subject claim
    pub fn generate_username(user_info: &UserInfo) -> String {
        if let Some(username) = &user_info.preferred_username
            && !username.is_empty()
        {
            return sanitize_username_with_dots(username);
        }

        if let Some(email) = &user_info.email
            && let Some(local_part) = email.split('@').next()
            && !local_part.is_empty()
        {
            return sanitize_username_with_dots(local_part);
        }

        sanitize_username(&user_info.subject)
    }

    /// Get the issuer URL string
    pub fn issuer_url(&self) -> &str {
        self.issuer_url.as_str()
    }

    /// Get a reference to the settings
    pub fn settings(&self) -> &OAuth2Settings {
        &self.settings
    }

    /// Whether admin status is governed by an `IdP` group claim.
    ///
    /// Only when both the claim name and value are configured does the `IdP`
    /// act as the source of truth for a user's admin state. Otherwise the
    /// extracted `is_admin` is always `false` and must not be used to
    /// overwrite an existing user's state.
    pub fn admin_claim_configured(&self) -> bool {
        self.settings.admin_group_claim.is_some() && self.settings.admin_group_value.is_some()
    }

    /// Whether read-only status is governed by an `IdP` group claim.
    ///
    /// See [`Self::admin_claim_configured`]; the same source-of-truth rule
    /// applies to the read-only flag.
    pub fn read_only_claim_configured(&self) -> bool {
        self.settings.read_only_group_claim.is_some()
            && self.settings.read_only_group_value.is_some()
    }
}

/// Extract and decode the payload from a JWT
fn extract_jwt_payload(jwt: &str) -> Result<serde_json::Value, OAuth2Error> {
    use base64::Engine;

    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err(OAuth2Error::TokenVerificationError(
            "Invalid JWT format".to_string(),
        ));
    }

    let payload_b64 = parts[1];
    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|e| OAuth2Error::TokenVerificationError(format!("Base64 decode error: {e}")))?;

    serde_json::from_slice(&payload_bytes)
        .map_err(|e| OAuth2Error::TokenVerificationError(format!("JSON parse error: {e}")))
}

/// Get a string array from a JSON value
fn get_string_array_from_json(payload: &serde_json::Value, name: &str) -> Option<Vec<String>> {
    let value = payload.get(name)?;

    // Try to parse as array of strings
    if let Some(arr) = value.as_array() {
        let strings: Vec<String> = arr
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(String::from)
            .collect();
        if !strings.is_empty() {
            return Some(strings);
        }
    }

    // Try to parse as single string (some providers return single group as string)
    if let Some(s) = value.as_str() {
        return Some(vec![s.to_string()]);
    }

    None
}

/// Sanitize a username to be valid for Kellnr
///
/// - Converts to lowercase
/// - Replaces invalid characters with underscores
/// - Ensures it starts with a letter
fn sanitize_username(input: &str) -> String {
    sanitize_username_impl(input, false)
}

/// Same as `sanitize_username`, but preserves dots.
fn sanitize_username_with_dots(input: &str) -> String {
    sanitize_username_impl(input, true)
}

fn sanitize_username_impl(input: &str, allow_dot: bool) -> String {
    let mut result: String = input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || (allow_dot && c == '.') {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();

    // Ensure it starts with a letter
    if result
        .chars()
        .next()
        .is_none_or(|c| !c.is_ascii_alphabetic())
    {
        result = format!("u_{result}");
    }

    // Truncate if too long (max 64 chars is reasonable)
    if result.len() > 64 {
        result.truncate(64);
    }

    result
}

/// Generate a unique username with collision handling
///
/// If the base username is taken, appends _2, _3, etc.
pub async fn generate_unique_username<F, Fut>(user_info: &UserInfo, is_available: F) -> String
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = bool>,
{
    let base = OAuth2Handler::generate_username(user_info);

    // Try the base username first
    if is_available(base.clone()).await {
        return base;
    }

    // Try with numeric suffixes
    for i in 2..=100 {
        let candidate = format!("{base}_{i}");
        if is_available(candidate.clone()).await {
            return candidate;
        }
    }

    // Fallback: use subject with timestamp (very unlikely to reach here)
    warn!("Could not find unique username after 100 attempts, using fallback");
    format!(
        "{}_{:x}",
        sanitize_username(&user_info.subject),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_username() {
        assert_eq!(sanitize_username("JohnDoe"), "johndoe");
        assert_eq!(sanitize_username("john.doe"), "john_doe");
        assert_eq!(sanitize_username("john@example.com"), "john_example_com");
        assert_eq!(sanitize_username("123user"), "u_123user");
        assert_eq!(sanitize_username("_user"), "u__user");
        assert_eq!(sanitize_username("user-name"), "user-name");
    }

    #[test]
    fn test_sanitize_username_with_dots() {
        assert_eq!(sanitize_username_with_dots("john.doe"), "john.doe");
        assert_eq!(
            sanitize_username_with_dots("john@example.com"),
            "john_example.com"
        );
    }

    #[test]
    fn test_generate_username_preferred() {
        let user_info = UserInfo {
            subject: "sub123".to_string(),
            email: Some("john@example.com".to_string()),
            preferred_username: Some("johndoe".to_string()),
            groups: vec![],
            is_admin: false,
            is_read_only: false,
        };
        assert_eq!(OAuth2Handler::generate_username(&user_info), "johndoe");
    }

    #[test]
    fn test_generate_username_preferred_preserves_dot() {
        let user_info = UserInfo {
            subject: "sub123".to_string(),
            email: Some("john@example.com".to_string()),
            preferred_username: Some("john.doe".to_string()),
            groups: vec![],
            is_admin: false,
            is_read_only: false,
        };
        assert_eq!(OAuth2Handler::generate_username(&user_info), "john.doe");
    }

    #[test]
    fn test_generate_username_email() {
        let user_info = UserInfo {
            subject: "sub123".to_string(),
            email: Some("john@example.com".to_string()),
            preferred_username: None,
            groups: vec![],
            is_admin: false,
            is_read_only: false,
        };
        assert_eq!(OAuth2Handler::generate_username(&user_info), "john");
    }

    #[test]
    fn test_generate_username_email_preserves_dot_in_local_part() {
        let user_info = UserInfo {
            subject: "sub123".to_string(),
            email: Some("john.doe@example.com".to_string()),
            preferred_username: None,
            groups: vec![],
            is_admin: false,
            is_read_only: false,
        };
        assert_eq!(OAuth2Handler::generate_username(&user_info), "john.doe");
    }

    #[test]
    fn test_generate_username_subject() {
        let user_info = UserInfo {
            subject: "sub123".to_string(),
            email: None,
            preferred_username: None,
            groups: vec![],
            is_admin: false,
            is_read_only: false,
        };
        // "sub123" starts with 's' which is a letter, so no prefix needed
        assert_eq!(OAuth2Handler::generate_username(&user_info), "sub123");
    }

    #[tokio::test]
    async fn test_generate_unique_username() {
        let user_info = UserInfo {
            subject: "sub123".to_string(),
            email: Some("john@example.com".to_string()),
            preferred_username: Some("johndoe".to_string()),
            groups: vec![],
            is_admin: false,
            is_read_only: false,
        };

        // First username is available
        let username = generate_unique_username(&user_info, |_| async { true }).await;
        assert_eq!(username, "johndoe");

        // First username is taken, second is available
        let username =
            generate_unique_username(&user_info, |name| async move { name != "johndoe" }).await;
        assert_eq!(username, "johndoe_2");

        // First two are taken
        let username = generate_unique_username(&user_info, |name| async move {
            name != "johndoe" && name != "johndoe_2"
        })
        .await;
        assert_eq!(username, "johndoe_3");
    }

    #[test]
    fn test_extract_jwt_payload() {
        // A test JWT with payload: {"sub":"1234567890","name":"John Doe","iat":1516239022}
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let payload = extract_jwt_payload(jwt).unwrap();
        assert_eq!(
            payload.get("sub").and_then(|v| v.as_str()),
            Some("1234567890")
        );
        assert_eq!(
            payload.get("name").and_then(|v| v.as_str()),
            Some("John Doe")
        );
    }

    #[test]
    fn test_get_string_array_from_json() {
        let payload = serde_json::json!({
            "groups": ["admin", "users"],
            "single_group": "single",
            "number": 42
        });

        assert_eq!(
            get_string_array_from_json(&payload, "groups"),
            Some(vec!["admin".to_string(), "users".to_string()])
        );
        assert_eq!(
            get_string_array_from_json(&payload, "single_group"),
            Some(vec!["single".to_string()])
        );
        assert_eq!(get_string_array_from_json(&payload, "number"), None);
        assert_eq!(get_string_array_from_json(&payload, "missing"), None);
    }
}

/// End-to-end tests for recovering from OIDC provider signing-key rotation.
///
/// These spin up a minimal in-process OIDC provider (discovery, JWKS and token
/// endpoints) whose active signing key can be swapped at runtime, mirroring a
/// provider like Dex rotating its keys.
#[cfg(test)]
mod rotation_tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use axum::extract::State;
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use chrono::{Duration, Utc};
    use openidconnect::core::{
        CoreIdToken, CoreJsonWebKeySet, CoreJwsSigningAlgorithm, CoreRsaPrivateSigningKey,
    };
    use openidconnect::{
        Audience, EmptyAdditionalClaims, EndUserEmail, JsonWebKeyId, PrivateSigningKey,
        StandardClaims, SubjectIdentifier,
    };
    use serde_json::{Value, json};
    use tokio::net::TcpListener;

    use super::*;

    // Two distinct 2048-bit RSA keys (PKCS#1 PEM) used to simulate key rotation.
    const KEY_A_PEM: &str = "-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEAtZ/uvzCiQirPAQf0w4piStS+zUlL6A3/ZTpTOJCpnRHVtnfh
EPbqtQ+ohvkB8txKg2nlgDZ5+zS62jtvb2BYzwkH5qU4jlpDEOnwKVtTTjOrCWZx
a8DVihgvcVkrYREOMRPzlXbF2aACwQAaveM8sbfu0cZ49SmQ34RQNqA8Gs+v/Nmr
jYwwZ9rQ4XIBYE689TiBjtZJVvQa7I8Waj8euYxWP5rB1tpZVtgz2AARTn5iSgNX
KM40IaMWLcvVUajPgYdcHE75+GLCzQAeEY4tboYo8ehFuMuehizj5sHz8ghz6reU
D83gE5wlLndJj/c/9apvvyBYmvWyar5HIPcvEQIDAQABAoIBAAEXqMi+82H6h4JM
duFvLowyMMlgFQr7vldpSPuSYxBEhN2YoBtJOr34T5VXrK0rPAELDzNduEmepRoK
KHpiqSd1EJsAPVGLeWkxecPD29eoKFRjcdLT/l30ZdQVJZOa5lP+0ByGpUmhTw1y
J98/24TXcbRjGtUsQ+4F1rnYIRVVY+RsrtWs2IaLHR8RnI6M3rFjY7y5dKX9x3Wn
ERC1Nx7blrUMvzK1052qUTC6sCYku2nJXUKBylj095GVZpK9oje62J5c9+SSvuOQ
MFEX0JSXfpCvoReP+9Ywa+vf+jpywja7hAIbH+bAqEnWNdrsuicTbUaRM39D1MB1
XbKoemkCgYEA4wSokLEgQggsQ+HEjr5cTXE/dDjokp3CuuXAJg0mAhLWZro3PZnL
dOElavpRwP4ISt3Lj47CJWcQAPGzl+BuGKPyQkRPhFkpBsUCdxgBi1eA4Suq7xa+
ARhMEN9Ayfszh2xLD/dRgOh/cVi/x7N3IfihJ9Jd9L3p4TTCjngUC5UCgYEAzM+9
DtPCdsNfiWtqtK/YolHUMZo3mb8Vu0e3fecIgUH3neOm/XhtxJY2wHehemyFJkYE
f93Vp50GXEmmmPVnjXuYmrIk3jEjk3j4hn7/gi37EzNzgbSf1RN09swU31FJ5N+R
/XZDH5zA+ZRIKU+CW8ap4cDTa1hBJNjVLqW2Fo0CgYEA1AFetjlkCaZ+SCqIGFI3
+u5+trgKohmIaGf1CNQQobEb3rWarwF4Wr+D5SK9xIC4F8qHtpo4Pxu/e1I9SOGD
j6lTrYUDyXJGeRb01Wlqz8k5B49zQ3K2oGkjaEJFzBq2pYqBkviBeeQmWCDsgL/d
yrDZN0ojClNtHi7aXphPB/0CgYEAr5nVYOcSrjzopqvgezbhqJo8MqMk1L9O5Jmi
q2HwmtJyeX78aApfItQf8XkgjBSLPLt/lBog22r4TxweqLqPpHC58LiYf6Dl/cUU
YEx2yaiewmG0wRqah1f9SrTDmIzbrE47n3NMLch6dAI8tJ6lCAcXFKX9HuY2RF9c
uHf/3OkCgYA2KePOq4vGOGD+zKDFrI7PSJcEa62oczZidgOylc/Lr9qx2Oiz4HM9
Di85C5B3y/3ZnYf8+1nTPdvfqTPt4aMjJKBVVWvkEA5yPYmY5FY+Z3sAA4AzGZ8Z
Xx2ZJaH0xsmAtrhSK8StLDiXayT8ygyZnqXfbZS0nG2HX3tKksYi0Q==
-----END RSA PRIVATE KEY-----";

    const KEY_B_PEM: &str = "-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEArOXZ5D7VxWmRkijS8CXQznmFL5bf3XDJ/Vll1imhJjU/sOJP
aob5U40xIV+b0LbNhxP8C3lu+TKF3xldvv7lYTjXYfVZ3fwjfBMe6Vm5PYr31FiR
KQLNrTrBE2cTCDoCn51NMERw/fdt0zJmfDcA77o1T6us2jxnDlrddiWYyBs26Y8p
dYFacxObDsJwqmE60ijP92SR37NjMdwUNDg1Bzh6xw3puAP4KeFwWNNVPHwQpJEd
pKX3T+CL2Ybfboyh9XCo2FmawfWjoIgUVoecgdpDMgNqtPjQ2HUvsH5JsWCezduE
MW7sPloZr9doGnYVycuBCKOldwriCFJqjw2VlwIDAQABAoIBAD9lhohFLABXbcuw
kWwCCbbz4wyon0xko4P0qD0nhZHre3+h8+nFNR3YSzgIBSu6I9GQV95jN/hC+Mht
1iyG7VfBTmR6YOnfHqnLnw2EW0KANtBTa2KkxwLqZMp3BIkDMFwTgy6cIexVshz7
QY3xYzQDzLF6awaYmFcwpTzBm2xfxmsjDODnMeeq4YraC/qrLn6yxFC3jH4H5fCu
mlzYrGHAEN7mdIOQEktJXwyEzi0rVKHDfJEhStGS7gMILw7BZfFSQxnzXyHJEOt7
3ZJY40N23H2iKaqN3s1Af56PbOcd9xmv3KM0lcmrmp2/cpoZTGSQgHNLZXB8bQ76
L+u/WyECgYEA8hfrlpn1O66Jh7JKNl8iLFoiBGBozsyZrx0KP0yyFJ4apk7U6GzE
7dnZJQ6sXklyE1gHSjArtICDnF8I26Jovl7EfijbQr+E4zuQhcZG3vuBzF0/8hy/
D//QLKXvokqevEvYnt7V81Io17K80R11U5Elxhl1YwxKOMd468MUr3cCgYEAttRh
6baK/NgktXkDhOf5Mmcz1MtUO+gop8lpOQifcPslE//6M1DE3G+6oRSZuTApSP5o
NjwhHbu0ko/zbWWvJ760bdcv98R6z60AHFq3TZrG5R4f68o4aD+Llu1mLt26QppP
vnby8CcCbcvCBXM4xzrORp7LOFs7fx8IsJL/EuECgYAepEZuVmhCuJGxujBId3wU
zwe1NBqv6hedoXhVkLiNgYFwAVRTYsj+Le+dECFjAbrNlfu+OSCfBREjbqfpXMIT
Ll2CdltiNGl2dWiSdgksWfsQydC5LUhFlyRbMlmFWhqDTLpLSXsdBA4lVvstIKRk
AmAclcZU1g5i52R3usZYewKBgGPCn2qXYF920RD0ZycuLJuEFJQYHm5Rz10+WVv1
iIptf13aXvuBJunhTUR3qSmTTfO2Xca6KJfAxFb0lS8sPNbDCFCnClV3sEuBan4O
QdbmjYCXX3OZdA3uHozMHOWVtvyAluKmpQjFQF4IwGWY6XJMdCG0o86seVpz5Jn0
m5oBAoGBAL/uJIrh8ZiL1eQrhJTitkW7BgV6iTN518Pf/I9DzOMnq/kG3W7zhQ2m
lJZsY5m/jNCs0KrTBBEoVQzkPVlzoQ/dDYZyA2cj4LydXyuM76yth8F+DMr3ckN6
Y4dOyrc/PytM2BLxs06WhIWeneUpz64RtUlvZUrSDgnO5AQX0ba2
-----END RSA PRIVATE KEY-----";

    const CLIENT_ID: &str = "test-client";
    const SUBJECT: &str = "user-subject-123";
    const NONCE: &str = "test-nonce";

    /// State shared with the mock provider's request handlers.
    struct MockProvider {
        issuer: String,
        key_a: CoreRsaPrivateSigningKey,
        key_b: CoreRsaPrivateSigningKey,
        /// When `true`, the provider signs tokens with (and advertises) `key_b`.
        rotated: AtomicBool,
        /// Number of times the JWKS endpoint was fetched, i.e. how often the
        /// handler ran discovery.
        jwks_fetches: AtomicUsize,
    }

    impl MockProvider {
        fn active_key(&self) -> &CoreRsaPrivateSigningKey {
            if self.rotated.load(Ordering::SeqCst) {
                &self.key_b
            } else {
                &self.key_a
            }
        }

        /// Mint an ID token signed with the currently active key.
        fn sign_id_token(&self) -> String {
            let claims = CoreIdTokenClaims::new(
                IssuerUrl::new(self.issuer.clone()).unwrap(),
                vec![Audience::new(CLIENT_ID.to_string())],
                Utc::now() + Duration::minutes(5),
                Utc::now(),
                StandardClaims::new(SubjectIdentifier::new(SUBJECT.to_string()))
                    .set_email(Some(EndUserEmail::new("user@example.com".to_string()))),
                EmptyAdditionalClaims {},
            )
            .set_nonce(Some(Nonce::new(NONCE.to_string())));

            let id_token = CoreIdToken::new(
                claims,
                self.active_key(),
                CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
                None,
                None,
            )
            .expect("signing ID token");

            serde_json::to_value(&id_token)
                .expect("serialize ID token")
                .as_str()
                .expect("ID token is a JWT string")
                .to_string()
        }
    }

    async fn discovery(State(state): State<Arc<MockProvider>>) -> Json<Value> {
        Json(json!({
            "issuer": state.issuer,
            "authorization_endpoint": format!("{}/authorize", state.issuer),
            "token_endpoint": format!("{}/token", state.issuer),
            "jwks_uri": format!("{}/jwks", state.issuer),
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
        }))
    }

    async fn jwks(State(state): State<Arc<MockProvider>>) -> Json<Value> {
        state.jwks_fetches.fetch_add(1, Ordering::SeqCst);
        let jwks = CoreJsonWebKeySet::new(vec![state.active_key().as_verification_key()]);
        Json(serde_json::to_value(&jwks).expect("serialize JWKS"))
    }

    async fn token(State(state): State<Arc<MockProvider>>) -> Json<Value> {
        Json(json!({
            "access_token": "access-token",
            "token_type": "Bearer",
            "expires_in": 3600,
            "id_token": state.sign_id_token(),
        }))
    }

    fn test_settings(issuer: &str) -> OAuth2Settings {
        OAuth2Settings {
            enabled: true,
            issuer_url: Some(issuer.to_string()),
            client_id: Some(CLIENT_ID.to_string()),
            client_secret: Some("test-secret".to_string()),
            ..Default::default()
        }
    }

    /// A signing-key rotation makes the JWKS cached at discovery stale; the
    /// handler must re-fetch it and still verify the freshly signed token.
    #[tokio::test]
    async fn exchange_recovers_from_signing_key_rotation() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}", listener.local_addr().unwrap());

        let provider = Arc::new(MockProvider {
            issuer: issuer.clone(),
            key_a: CoreRsaPrivateSigningKey::from_pem(
                KEY_A_PEM,
                Some(JsonWebKeyId::new("key-a".to_string())),
            )
            .unwrap(),
            key_b: CoreRsaPrivateSigningKey::from_pem(
                KEY_B_PEM,
                Some(JsonWebKeyId::new("key-b".to_string())),
            )
            .unwrap(),
            rotated: AtomicBool::new(false),
            jwks_fetches: AtomicUsize::new(0),
        });

        let app = Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/jwks", get(jwks))
            .route("/token", post(token))
            .with_state(provider.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });

        // Discovery caches key A (first JWKS fetch).
        let handler = OAuth2Handler::from_discovery(&test_settings(&issuer), "http://localhost/cb")
            .await
            .expect("discovery should succeed");
        assert_eq!(provider.jwks_fetches.load(Ordering::SeqCst), 1);

        // Baseline: a token signed with key A verifies against the cached JWKS
        // without triggering another discovery.
        let result = handler
            .exchange_and_validate("auth-code", "pkce-verifier", NONCE)
            .await
            .expect("baseline exchange should verify");
        assert_eq!(result.claims.subject().as_str(), SUBJECT);
        assert_eq!(provider.jwks_fetches.load(Ordering::SeqCst), 1);

        // The provider rotates keys: tokens are now signed with key B and the
        // JWKS endpoint serves only key B. The key A cached at startup is stale.
        provider.rotated.store(true, Ordering::SeqCst);

        // Before the fix this failed with "Signature verification failed"; now
        // the handler re-discovers the JWKS (second fetch) and verifies.
        let result = handler
            .exchange_and_validate("auth-code", "pkce-verifier", NONCE)
            .await
            .expect("exchange should succeed after JWKS refresh");
        assert_eq!(result.claims.subject().as_str(), SUBJECT);
        assert_eq!(provider.jwks_fetches.load(Ordering::SeqCst), 2);

        // The refreshed client is cached, so a subsequent login reuses key B
        // instead of re-fetching the JWKS again.
        let result = handler
            .exchange_and_validate("auth-code", "pkce-verifier", NONCE)
            .await
            .expect("exchange should reuse the refreshed JWKS");
        assert_eq!(result.claims.subject().as_str(), SUBJECT);
        assert_eq!(provider.jwks_fetches.load(Ordering::SeqCst), 2);

        server.abort();
    }
}
