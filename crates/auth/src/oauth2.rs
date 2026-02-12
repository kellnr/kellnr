//! OAuth2/OpenID Connect authentication handler
//!
//! This module provides OAuth2/OIDC authentication support using the authorization
//! code flow with PKCE.

use std::future::Future;
use std::sync::Arc;

use kellnr_settings::OAuth2 as OAuth2Settings;
use openidconnect::core::{
    CoreAuthDisplay, CoreAuthPrompt, CoreAuthenticationFlow, CoreClient, CoreErrorResponseType,
    CoreGenderClaim, CoreIdTokenClaims, CoreJsonWebKey, CoreJweContentEncryptionAlgorithm,
    CoreProviderMetadata, CoreRevocationErrorResponse, CoreTokenIntrospectionResponse,
    CoreTokenResponse,
};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EmptyAdditionalClaims, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenResponse, reqwest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, warn};
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
    client: ConfiguredCoreClient,
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

        // Perform OIDC discovery
        let provider_metadata =
            CoreProviderMetadata::discover_async(issuer_url.clone(), &http_client)
                .await
                .map_err(|e| OAuth2Error::DiscoveryError(e.to_string()))?;

        let client_id = ClientId::new(
            settings
                .client_id
                .clone()
                .ok_or_else(|| OAuth2Error::ConfigurationError("Missing client_id".to_string()))?,
        );

        let client_secret = settings.client_secret.clone().map(ClientSecret::new);

        let redirect_url = RedirectUrl::new(redirect_url.to_string())?;

        // Build the client from provider metadata
        let client =
            CoreClient::from_provider_metadata(provider_metadata, client_id, client_secret)
                .set_redirect_uri(redirect_url);

        Ok(Self {
            client,
            settings: Arc::new(settings.clone()),
            issuer_url,
            http_client,
        })
    }

    /// Generate an authorization URL for the `OAuth2` flow
    ///
    /// Returns an `AuthRequest` containing the URL to redirect the user to,
    /// along with the state, PKCE verifier, and nonce that must be stored
    /// for later verification.
    pub fn generate_auth_url(&self) -> AuthRequest {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Build the authorization request with configured scopes
        let mut auth_request = self
            .client
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
        let verifier = PkceCodeVerifier::new(pkce_verifier.to_string());

        let token_request = self
            .client
            .exchange_code(code)
            .map_err(|e| OAuth2Error::TokenExchangeError(e.to_string()))?;

        let token_response: CoreTokenResponse = token_request
            .set_pkce_verifier(verifier)
            .request_async(&self.http_client)
            .await
            .map_err(|e| OAuth2Error::TokenExchangeError(e.to_string()))?;

        // Get and validate the ID token
        let id_token = token_response
            .id_token()
            .ok_or_else(|| OAuth2Error::MissingClaim("id_token".to_string()))?;

        let nonce = Nonce::new(nonce.to_string());
        let verifier = self.client.id_token_verifier();

        let claims = id_token
            .claims(&verifier, &nonce)
            .map_err(|e| OAuth2Error::TokenVerificationError(e.to_string()))?
            .clone();

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
            return sanitize_username(username);
        }

        if let Some(email) = &user_info.email
            && let Some(local_part) = email.split('@').next()
            && !local_part.is_empty()
        {
            return sanitize_username(local_part);
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
    let mut result: String = input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
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
