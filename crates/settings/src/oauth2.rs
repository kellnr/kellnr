use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};

fn default_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "profile".to_string(),
        "email".to_string(),
    ]
}

fn default_button_text() -> String {
    "Login with SSO".to_string()
}

/// `OAuth2`/`OpenID` Connect authentication configuration
#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, ClapSerde)]
#[serde(default)]
pub struct OAuth2 {
    /// Enable `OAuth2`/OIDC authentication
    #[default(false)]
    #[arg(id = "oauth2-enabled", long = "oauth2-enabled")]
    pub enabled: bool,

    /// OIDC issuer URL (discovery URL, e.g., `<https://authentik.example.com/application/o/kellnr/>`)
    #[default(None)]
    #[arg(id = "oauth2-issuer-url", long = "oauth2-issuer-url")]
    pub issuer_url: Option<String>,

    /// `OAuth2` client ID
    #[default(None)]
    #[arg(id = "oauth2-client-id", long = "oauth2-client-id")]
    pub client_id: Option<String>,

    /// `OAuth2` client secret (prefer setting via `KELLNR_OAUTH2__CLIENT_SECRET` env var)
    #[default(None)]
    #[serde(skip_serializing)]
    #[arg(id = "oauth2-client-secret", long = "oauth2-client-secret")]
    pub client_secret: Option<String>,

    /// `OAuth2` scopes to request (default: `["openid", "profile", "email"]`)
    #[default(default_scopes())]
    #[arg(id = "oauth2-scopes", long = "oauth2-scopes")]
    pub scopes: Vec<String>,

    /// Automatically create local user accounts for new `OAuth2` users
    #[default(true)]
    #[arg(
        id = "oauth2-auto-provision-users",
        long = "oauth2-auto-provision-users"
    )]
    pub auto_provision_users: bool,

    /// Claim name to check for admin group membership (e.g., "groups")
    #[default(None)]
    #[arg(id = "oauth2-admin-group-claim", long = "oauth2-admin-group-claim")]
    pub admin_group_claim: Option<String>,

    /// Value in the admin group claim that grants admin privileges (e.g., "kellnr-admins")
    #[default(None)]
    #[arg(id = "oauth2-admin-group-value", long = "oauth2-admin-group-value")]
    pub admin_group_value: Option<String>,

    /// Claim name to check for read-only group membership (e.g., "groups")
    #[default(None)]
    #[arg(
        id = "oauth2-read-only-group-claim",
        long = "oauth2-read-only-group-claim"
    )]
    pub read_only_group_claim: Option<String>,

    /// Value in the read-only group claim that grants read-only access (e.g., "kellnr-readonly")
    #[default(None)]
    #[arg(
        id = "oauth2-read-only-group-value",
        long = "oauth2-read-only-group-value"
    )]
    pub read_only_group_value: Option<String>,

    /// Text displayed on the `OAuth2` login button
    #[default(default_button_text())]
    #[arg(id = "oauth2-button-text", long = "oauth2-button-text")]
    pub button_text: String,
}

impl OAuth2 {
    /// Validate the `OAuth2` configuration
    /// Returns an error message if the configuration is invalid
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        if self.issuer_url.is_none() {
            return Err("OAuth2 is enabled but issuer_url is not set".to_string());
        }

        if self.client_id.is_none() {
            return Err("OAuth2 is enabled but client_id is not set".to_string());
        }

        // client_secret may be optional for public clients using PKCE,
        // but we require it for confidential clients (default use case)
        if self.client_secret.is_none() {
            return Err("OAuth2 is enabled but client_secret is not set. \
                Set it via KELLNR_OAUTH2__CLIENT_SECRET environment variable"
                .to_string());
        }

        if self.scopes.is_empty() {
            return Err("OAuth2 scopes cannot be empty".to_string());
        }

        if !self.scopes.contains(&"openid".to_string()) {
            return Err("OAuth2 scopes must contain 'openid' for OIDC".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_oauth2() {
        let oauth2 = OAuth2::default();
        assert!(!oauth2.enabled);
        assert!(oauth2.issuer_url.is_none());
        assert!(oauth2.client_id.is_none());
        assert!(oauth2.client_secret.is_none());
        assert_eq!(oauth2.scopes, vec!["openid", "profile", "email"]);
        assert!(oauth2.auto_provision_users);
        assert_eq!(oauth2.button_text, "Login with SSO");
    }

    #[test]
    fn test_validate_disabled() {
        let oauth2 = OAuth2::default();
        assert!(oauth2.validate().is_ok());
    }

    #[test]
    fn test_validate_enabled_missing_issuer() {
        let oauth2 = OAuth2 {
            enabled: true,
            ..Default::default()
        };
        let result = oauth2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("issuer_url"));
    }

    #[test]
    fn test_validate_enabled_missing_client_id() {
        let oauth2 = OAuth2 {
            enabled: true,
            issuer_url: Some("https://example.com".to_string()),
            ..Default::default()
        };
        let result = oauth2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("client_id"));
    }

    #[test]
    fn test_validate_enabled_missing_client_secret() {
        let oauth2 = OAuth2 {
            enabled: true,
            issuer_url: Some("https://example.com".to_string()),
            client_id: Some("client-id".to_string()),
            ..Default::default()
        };
        let result = oauth2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("client_secret"));
    }

    #[test]
    fn test_validate_enabled_valid() {
        let oauth2 = OAuth2 {
            enabled: true,
            issuer_url: Some("https://example.com".to_string()),
            client_id: Some("client-id".to_string()),
            client_secret: Some("client-secret".to_string()),
            ..Default::default()
        };
        assert!(oauth2.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_scopes() {
        let oauth2 = OAuth2 {
            enabled: true,
            issuer_url: Some("https://example.com".to_string()),
            client_id: Some("client-id".to_string()),
            client_secret: Some("client-secret".to_string()),
            scopes: vec![],
            ..Default::default()
        };
        let result = oauth2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_validate_missing_openid_scope() {
        let oauth2 = OAuth2 {
            enabled: true,
            issuer_url: Some("https://example.com".to_string()),
            client_id: Some("client-id".to_string()),
            client_secret: Some("client-secret".to_string()),
            scopes: vec!["profile".to_string(), "email".to_string()],
            ..Default::default()
        };
        let result = oauth2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("openid"));
    }
}
