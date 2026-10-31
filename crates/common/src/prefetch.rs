use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Deserialize;

/// Format in which Kellnr stores dates in the database.
const DB_DATE_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct Prefetch {
    pub data: Vec<u8>,
    pub etag: String,
    pub last_modified: String,
}

#[derive(Debug)]
pub struct Headers {
    pub if_none_match: Option<String>,
    pub if_modified_since: Option<String>,
}

/// Whether a client that sent the conditional request headers already has the
/// current index entry with the given `etag` and `last_modified` date, so that
/// a 304 can be returned. Like RFC 7232, `If-None-Match` takes precedence over
/// `If-Modified-Since`.
pub fn is_not_modified(
    if_none_match: Option<&str>,
    if_modified_since: Option<&str>,
    etag: &str,
    last_modified: &str,
) -> bool {
    match (if_none_match, if_modified_since) {
        (Some(if_none_match), _) => etag_matches(if_none_match, etag),
        (None, Some(if_modified_since)) => not_modified_since(last_modified, if_modified_since),
        (None, None) => false,
    }
}

/// Whether one of the entity tags in an `If-None-Match` header matches `etag`.
/// The comparison is weak, so quotes and the `W/` prefix are ignored.
fn etag_matches(if_none_match: &str, etag: &str) -> bool {
    let etag = opaque_tag(etag);
    if etag.is_empty() {
        return false;
    }
    if_none_match
        .split(',')
        .map(str::trim)
        .any(|tag| tag == "*" || opaque_tag(tag) == etag)
}

fn opaque_tag(etag: &str) -> &str {
    let etag = etag.trim();
    let etag = etag.strip_prefix("W/").unwrap_or(etag);
    etag.strip_prefix('"')
        .and_then(|e| e.strip_suffix('"'))
        .unwrap_or(etag)
}

fn not_modified_since(last_modified: &str, if_modified_since: &str) -> bool {
    match (parse_date(last_modified), parse_date(if_modified_since)) {
        (Some(last_modified), Some(since)) => last_modified <= since,
        _ => last_modified == if_modified_since,
    }
}

/// Parse an HTTP date or a date in the database format.
fn parse_date(date: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc2822(date)
        .map(|date| date.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(date, DB_DATE_FORMAT)
                .ok()
                .map(|date| date.and_utc())
        })
}

/// Quote an entity tag as required in the `ETag` header.
pub fn quote_etag(etag: &str) -> String {
    format!("\"{}\"", opaque_tag(etag))
}

/// Convert a date in the database format to an HTTP date for the
/// `Last-Modified` header. Other values are returned unchanged.
pub fn db_date_to_http_date(date: &str) -> String {
    NaiveDateTime::parse_from_str(date, DB_DATE_FORMAT).map_or_else(
        |_| date.to_string(),
        |date| {
            date.and_utc()
                .format("%a, %d %b %Y %H:%M:%S GMT")
                .to_string()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ETAG: &str = "abc";
    const DB_DATE: &str = "2026-10-30 12:00:00";
    const HTTP_DATE: &str = "Fri, 30 Oct 2026 12:00:00 GMT";

    #[test]
    fn matching_etag_alone_is_not_modified() {
        assert!(is_not_modified(Some("\"abc\""), None, ETAG, DB_DATE));
        assert!(is_not_modified(Some("abc"), None, ETAG, DB_DATE));
        assert!(is_not_modified(Some("W/\"abc\""), None, ETAG, DB_DATE));
        assert!(is_not_modified(Some("\"x\", \"abc\""), None, ETAG, DB_DATE));
        assert!(is_not_modified(Some("\"abc\""), None, "\"abc\"", DB_DATE));
    }

    #[test]
    fn other_etag_is_modified() {
        assert!(!is_not_modified(Some("\"abd\""), None, ETAG, DB_DATE));
        assert!(!is_not_modified(Some("\"\""), None, "", DB_DATE));
    }

    #[test]
    fn if_none_match_takes_precedence_over_if_modified_since() {
        assert!(!is_not_modified(
            Some("\"old\""),
            Some(HTTP_DATE),
            ETAG,
            DB_DATE
        ));
        assert!(is_not_modified(
            Some("\"abc\""),
            Some("Thu, 01 Jan 2026 00:00:00 GMT"),
            ETAG,
            DB_DATE
        ));
    }

    #[test]
    fn if_modified_since_alone_compares_dates() {
        assert!(is_not_modified(None, Some(HTTP_DATE), ETAG, DB_DATE));
        assert!(is_not_modified(
            None,
            Some("Sat, 31 Oct 2026 12:00:00 GMT"),
            ETAG,
            DB_DATE
        ));
        assert!(!is_not_modified(
            None,
            Some("Fri, 30 Oct 2026 11:59:59 GMT"),
            ETAG,
            DB_DATE
        ));
        assert!(is_not_modified(None, Some(HTTP_DATE), ETAG, HTTP_DATE));
    }

    #[test]
    fn if_modified_since_with_the_old_database_format_still_matches() {
        assert!(is_not_modified(None, Some(DB_DATE), ETAG, DB_DATE));
    }

    #[test]
    fn no_conditional_headers_is_modified() {
        assert!(!is_not_modified(None, None, ETAG, DB_DATE));
    }

    #[test]
    fn database_date_is_sent_as_http_date() {
        assert_eq!(HTTP_DATE, db_date_to_http_date(DB_DATE));
        assert_eq!(HTTP_DATE, db_date_to_http_date(HTTP_DATE));
    }

    #[test]
    fn etag_is_quoted_once() {
        assert_eq!("\"abc\"", quote_etag("abc"));
        assert_eq!("\"abc\"", quote_etag("\"abc\""));
    }
}
