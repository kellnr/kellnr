// Allow needless_for_each in generated code from utoipa derive macro
#![allow(clippy::needless_for_each)]

use kellnr_http::error::ErrorDetails;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::openapi::{Content, Ref, RefOr};
use utoipa::{Modify, OpenApi, PartialSchema, ToSchema};

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

/// Documents the JSON body of [`ErrorDetails`] for every error response.
/// Kellnr answers all errors in this format, so the single annotations only
/// name the status codes.
pub fn add_error_bodies(mut api: utoipa::openapi::OpenApi) -> utoipa::openapi::OpenApi {
    let components = api.components.get_or_insert_with(Default::default);
    let mut schemas = vec![(ErrorDetails::name().into_owned(), ErrorDetails::schema())];
    ErrorDetails::schemas(&mut schemas);
    components.schemas.extend(schemas);

    let error_body = Content::new(Some(Ref::from_schema_name(ErrorDetails::name())));
    for item in api.paths.paths.values_mut() {
        let operations = [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
        ];
        for operation in operations.into_iter().flatten() {
            for (status, response) in &mut operation.responses.responses {
                if let RefOr::T(response) = response
                    && (status.starts_with('4') || status.starts_with('5'))
                    && response.content.is_empty()
                {
                    response
                        .content
                        .insert("application/json".to_string(), RefOr::T(error_body.clone()));
                }
            }
        }
    }
    api
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Kellnr API",
        // Swagger UI renders the description below the title. The relative
        // link leads from /api/docs/ back to the web UI, also with a path prefix.
        description = "Self-hosted Rust crate registry with support for rustdocs and crates.io caching\n\n[Back to Kellnr](../../)"
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

#[cfg(test)]
mod tests {
    use utoipa::OpenApi;

    use super::ApiDoc;

    #[test]
    fn api_version_matches_crate_version() {
        assert_eq!(ApiDoc::openapi().info.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn api_description_links_back_to_the_web_ui() {
        let description = ApiDoc::openapi().info.description.unwrap_or_default();
        assert!(description.contains("[Back to Kellnr](../../)"));
    }
}
