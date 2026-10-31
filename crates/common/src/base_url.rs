use std::fmt;

use thiserror::Error;
use url::Url;

/// An http or https URL whose path ends in a slash, the base of the URLs of
/// an upstream service like the crates.io index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseUrl(Url);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BaseUrlError {
    #[error("must be an http or https URL, got {0}")]
    NotHttp(Url),
}

impl BaseUrl {
    /// Accepts `url` with or without a trailing slash.
    pub fn new(mut url: Url) -> Result<Self, BaseUrlError> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(BaseUrlError::NotHttp(url));
        }
        if !url.path().ends_with('/') {
            let path = format!("{}/", url.path());
            url.set_path(&path);
        }
        Ok(Self(url))
    }

    /// Appends the `/` separated segments of `path` to the base URL. Unlike
    /// [`Url::join`], this keeps every segment of the base URL: a leading
    /// slash, `.` and `..` segments in `path` are ignored, and characters
    /// like `?` and `#` are percent-encoded.
    pub fn join(&self, path: &str) -> Url {
        let mut url = self.0.clone();
        url.set_query(None);
        url.set_fragment(None);
        url.path_segments_mut()
            .expect("http and https URLs have a path")
            .pop_if_empty()
            .extend(path.trim_start_matches('/').split('/'));
        url
    }

    pub fn as_url(&self) -> &Url {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<Url> for BaseUrl {
    type Error = BaseUrlError;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        Self::new(url)
    }
}

impl fmt::Display for BaseUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(url: &str) -> BaseUrl {
        BaseUrl::new(Url::parse(url).unwrap()).unwrap()
    }

    #[test]
    fn new_appends_a_trailing_slash() {
        assert_eq!(
            base("https://mirror.example.com/crates").as_str(),
            "https://mirror.example.com/crates/"
        );
        assert_eq!(
            base("https://mirror.example.com/crates/").as_str(),
            "https://mirror.example.com/crates/"
        );
        assert_eq!(
            base("http://mirror.example.com").as_str(),
            "http://mirror.example.com/"
        );
    }

    #[test]
    fn new_rejects_urls_that_are_not_http() {
        for url in [
            "sparse+https://index.crates.io/",
            "ftp://example.com/",
            "file:///tmp/",
        ] {
            assert!(
                matches!(
                    BaseUrl::new(Url::parse(url).unwrap()),
                    Err(BaseUrlError::NotHttp(_))
                ),
                "{url}"
            );
        }
    }

    /// `Url::join` drops the last segment of a base URL without trailing
    /// slash, and all segments for a path with a leading slash.
    #[test]
    fn join_keeps_all_path_segments() {
        let base = base("https://mirror.example.com/api/v1/crates");
        assert_eq!(
            base.join("serde").as_str(),
            "https://mirror.example.com/api/v1/crates/serde"
        );
        assert_eq!(
            base.join("/serde").as_str(),
            "https://mirror.example.com/api/v1/crates/serde"
        );
        assert_eq!(
            base.join("serde/1.0.0/download").as_str(),
            "https://mirror.example.com/api/v1/crates/serde/1.0.0/download"
        );
        assert_eq!(
            base.join("../../serde").as_str(),
            "https://mirror.example.com/api/v1/crates/serde"
        );
    }

    #[test]
    fn join_matches_url_join_for_crate_paths() {
        let base = base("https://index.crates.io/");
        for path in ["se/rd/serde", "3/s/syn", "1/a", "serde/serde-1.0.0.crate"] {
            assert_eq!(base.join(path), base.as_url().join(path).unwrap(), "{path}");
        }
    }

    #[test]
    fn join_encodes_query_and_fragment_characters() {
        let base = base("https://index.crates.io/?x=1");
        assert_eq!(
            base.join("a?b#c").as_str(),
            "https://index.crates.io/a%3Fb%23c"
        );
    }
}
