// Allow needless_for_each in generated code from utoipa derive macro
#![allow(clippy::needless_for_each)]

use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};

pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "cargo_token",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("Authorization"))),
            );
            components.add_security_scheme(
                "session_cookie",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("kellnr_session_id"))),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Kellnr API",
        version = "5.14.1",
        description = "Self-hosted Rust crate registry with support for rustdocs and crates.io caching"
    ),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "users", description = "User management"),
        (name = "groups", description = "Group management"),
        (name = "acl", description = "Crate access control"),
        (name = "crates", description = "Kellnr registry API"),
        (name = "cratesio", description = "Crates.io proxy"),
        (name = "docs", description = "Documentation"),
        (name = "toolchains", description = "Toolchain distribution"),
        (name = "webhooks", description = "Webhooks"),
        (name = "oauth2", description = "OAuth2/OIDC"),
        (name = "ui", description = "Web UI API"),
        (name = "health", description = "Health check")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;
