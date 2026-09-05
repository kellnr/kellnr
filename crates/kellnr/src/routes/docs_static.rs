use std::time::SystemTime;

use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum_extra::TypedHeader;
use axum_extra::headers::{ETag, HeaderMapExt, IfModifiedSince, IfNoneMatch, LastModified};
use kellnr_appstate::DocsStorageState;

pub async fn serve_doc_file(
    State(docs_storage): DocsStorageState,
    Path(path): Path<String>,
    if_none_match: Option<TypedHeader<IfNoneMatch>>,
    if_modified_since: Option<TypedHeader<IfModifiedSince>>,
) -> Response {
    let Ok(object) = docs_storage.get_with_meta(&path).await else {
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
        // Doc content can be overwritten by republishing, so always revalidate
        // rather than letting clients cache without asking.
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        response
    };

    if let Some(etag) = etag {
        response.headers_mut().typed_insert(etag);
    }
    response.headers_mut().typed_insert(last_modified);

    response
}
