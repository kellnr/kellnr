use std::time::SystemTime;

use axum::extract::{FromRequestParts, Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_extra::TypedHeader;
use axum_extra::headers::{ETag, HeaderMapExt, IfModifiedSince, IfNoneMatch, LastModified};
use kellnr_http::state::{AppStateData, DocsStorageState};
use kellnr_http::maybe_user::MaybeUser;
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::original_name::OriginalName;
use kellnr_registry::kellnr_api::has_restricted_crate_access;
use kellnr_storage::storage_key::StorageKey;
use tracing::error;

/// Middleware that applies the per-crate download restriction to hosted docs.
///
/// The docs contain the crate's source pages, so they get the same protection
/// as the crate archive. Denied requests get a 404 to not reveal that the
/// crate exists.
pub async fn restrict_docs_access(
    State(state): State<AppStateData>,
    Path(path): Path<String>,
    request: Request,
    next: Next,
) -> Response {
    let Some(crate_name) = crate_name_from_doc_path(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    match state.db.is_download_restricted(&crate_name).await {
        Ok(false) => return next.run(request).await,
        Ok(true) => {}
        Err(e) => {
            error!("Failed to check download restriction for {crate_name}: {e}");
            return StatusCode::NOT_FOUND.into_response();
        }
    }

    let (mut parts, body) = request.into_parts();
    let Ok(user) = MaybeUser::from_request_parts(&mut parts, &state).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    match has_restricted_crate_access(&crate_name, &user.name, user.is_admin, &state.db).await {
        Ok(true) => next.run(Request::from_parts(parts, body)).await,
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            error!("Failed to check docs access for {crate_name}: {e}");
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

/// Extracts the crate name from a docs path like `{crate}/{version}/doc/...`.
///
/// Rejects paths with empty segments (`//`, leading or trailing `/`). The
/// storage layer drops them, so the checked segment could differ from the
/// served key.
fn crate_name_from_doc_path(path: &str) -> Option<CanonicalName> {
    if path.split('/').any(str::is_empty) {
        return None;
    }
    let first = path.split('/').next()?;
    OriginalName::try_from(first)
        .ok()
        .map(|name| name.to_canonical())
}

pub async fn serve_doc_file(
    State(docs_storage): DocsStorageState,
    Path(path): Path<String>,
    if_none_match: Option<TypedHeader<IfNoneMatch>>,
    if_modified_since: Option<TypedHeader<IfModifiedSince>>,
) -> Response {
    let Ok(object) = docs_storage
        .get_with_meta(&StorageKey::from_stored(path.clone()))
        .await
    else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let etag = object.e_tag.as_deref().and_then(|s| s.parse::<ETag>().ok());
    let last_modified_time = SystemTime::from(object.last_modified);
    let last_modified = LastModified::from(last_modified_time);

    // If-None-Match takes precedence over If-Modified-Since when both are
    // present, per RFC 9110 §13.1.2.
    let not_modified =
        if let (Some(TypedHeader(if_none_match)), Some(etag)) = (&if_none_match, &etag) {
            !if_none_match.precondition_passes(etag)
        } else if let Some(TypedHeader(if_modified_since)) = &if_modified_since {
            !if_modified_since.is_modified(last_modified_time)
        } else {
            false
        };

    let mut response = if not_modified {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let mime = mime_guess::from_path(&path).first_or_octet_stream();
        let mut response = (StatusCode::OK, object.bytes).into_response();
        let headers = response.headers_mut();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(mime.as_ref())
                .unwrap_or(HeaderValue::from_static("application/octet-stream")),
        );
        headers.insert(header::CACHE_CONTROL, cache_control(&path));
        response
    };

    if let Some(etag) = etag {
        response.headers_mut().typed_insert(etag);
    }
    response.headers_mut().typed_insert(last_modified);

    response
}

/// Cache policy for a docs file. `private` keeps shared caches from serving
/// restricted docs to other users.
///
/// Rustdoc's shared assets in `static.files/` have content hashes in their
/// names, so browsers may keep them. This avoids re-requesting them on every
/// page, each of which costs an access check. Everything else can be
/// overwritten by republishing and is always revalidated.
fn cache_control(path: &str) -> HeaderValue {
    if path.split('/').any(|segment| segment == "static.files") {
        HeaderValue::from_static("private, max-age=31536000, immutable")
    } else {
        HeaderValue::from_static("private, no-cache")
    }
}
