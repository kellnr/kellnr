// Allow needless_for_each and the qualified schema paths in generated code
// from utoipa derive macro
#![allow(clippy::needless_for_each, unused_qualifications)]

use std::collections::HashSet;

use kellnr_common::name_or_description::NameOrDescription;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_db::provider::CrateSort;
use kellnr_http::error::ErrorDetails;
use kellnr_settings::Settings;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::openapi::{Content, Ref, RefOr, Schema};
use utoipa::{Modify, OpenApi, PartialSchema, ToSchema};

pub(crate) struct SecurityAddon;

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
pub(crate) fn add_error_bodies(mut api: utoipa::openapi::OpenApi) -> utoipa::openapi::OpenApi {
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

/// Marks the fields of [`Settings`] and of the objects in it as required.
/// The server always sends all of them, but `#[serde(default)]` on the
/// settings structs makes them optional in the generated schemas.
pub(crate) fn require_settings_fields(
    mut api: utoipa::openapi::OpenApi,
) -> utoipa::openapi::OpenApi {
    let components = api.components.get_or_insert_with(Default::default);
    let mut pending = vec![Settings::name().into_owned()];
    let mut seen = HashSet::new();
    while let Some(name) = pending.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        if let Some(RefOr::T(Schema::Object(object))) = components.schemas.get_mut(&name) {
            object.required = object.properties.keys().cloned().collect();
            for property in object.properties.values() {
                collect_schema_refs(property, &mut pending);
            }
        }
    }
    api
}

/// Adds the names of the component schemas `schema` refers to.
fn collect_schema_refs(schema: &RefOr<Schema>, names: &mut Vec<String>) {
    match schema {
        RefOr::Ref(reference) => names.extend(
            reference
                .ref_location
                .strip_prefix("#/components/schemas/")
                .map(str::to_string),
        ),
        RefOr::T(Schema::AllOf(all_of)) => {
            for item in &all_of.items {
                collect_schema_refs(item, names);
            }
        }
        RefOr::T(_) => {}
    }
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
    // Query parameters only reference their types by name, so the types have
    // to be registered here.
    components(schemas(CrateSort, NameOrDescription, OriginalName, Version)),
    modifiers(&SecurityAddon)
)]
pub(crate) struct ApiDoc;

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::Request;
    use kellnr_http::test_support::test_state;
    use serde_json::Value;
    use tower::ServiceExt;
    use utoipa::OpenApi;

    use super::ApiDoc;
    use crate::routes::create_router;

    const SNAPSHOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.json");

    /// The document as `/api/openapi.json` serves it, with the optional
    /// toolchain routes enabled. The version is replaced by a placeholder,
    /// so the snapshot does not change with every release.
    async fn served_document() -> Value {
        let mut state = test_state();
        let mut settings = (*state.settings).clone();
        settings.toolchain.enabled = true;
        state.settings = Arc::new(settings);

        let response = create_router(state, None)
            .oneshot(
                Request::get("/api/openapi.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let mut document: Value = serde_json::from_slice(&body).unwrap();
        document["info"]["version"] = Value::from("0.0.0");
        document
    }

    /// Fails if the document differs from the committed `openapi.json`.
    /// Run the test with `UPDATE_OPENAPI=1` to write the new document.
    #[tokio::test]
    async fn openapi_document_matches_snapshot() {
        let mut document = serde_json::to_string_pretty(&served_document().await).unwrap();
        document.push('\n');

        if std::env::var_os("UPDATE_OPENAPI").is_some() {
            std::fs::write(SNAPSHOT, &document).unwrap();
            return;
        }
        let snapshot = std::fs::read_to_string(Path::new(SNAPSHOT)).unwrap_or_default();
        assert!(
            snapshot == document,
            "The OpenAPI document differs from crates/kellnr/openapi.json. If the change \
             is intended, update the file with \
             `UPDATE_OPENAPI=1 cargo nextest run -p kellnr openapi_document_matches_snapshot` \
             and review the diff."
        );
    }

    fn collect_refs<'a>(value: &'a Value, refs: &mut Vec<&'a str>) {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    match value {
                        Value::String(target) if key == "$ref" => refs.push(target),
                        _ => collect_refs(value, refs),
                    }
                }
            }
            Value::Array(values) => values.iter().for_each(|value| collect_refs(value, refs)),
            _ => {}
        }
    }

    #[tokio::test]
    async fn openapi_document_defines_every_referenced_schema() {
        let document = served_document().await;
        let mut refs = Vec::new();
        collect_refs(&document, &mut refs);

        let schemas = document["components"]["schemas"].as_object().unwrap();
        let mut undefined: Vec<&str> = refs
            .into_iter()
            .filter(|target| {
                target
                    .strip_prefix("#/components/schemas/")
                    .is_none_or(|name| !schemas.contains_key(name))
            })
            .collect();
        undefined.sort_unstable();
        undefined.dedup();
        assert!(undefined.is_empty(), "undefined references: {undefined:?}");
    }

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
