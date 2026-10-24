use url::Url;

/// Parses a webhook callback URL and ensures it is an absolute `http` or
/// `https` URL with a host.
pub(crate) fn validate_callback_url(callback_url: &str) -> Result<Url, String> {
    let url = Url::parse(callback_url).map_err(|e| format!("Invalid callback URL: {e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Callback URL must use http or https".to_string());
    }
    if url.host().is_none() {
        return Err("Callback URL must contain a host".to_string());
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_urls() {
        assert!(validate_callback_url("http://my-service:8000").is_ok());
        assert!(validate_callback_url("https://example.com/hook?x=1").is_ok());
        assert!(validate_callback_url("http://10.0.0.1/hook").is_ok());
    }

    #[test]
    fn rejects_invalid_urls() {
        assert!(validate_callback_url("").is_err());
        assert!(validate_callback_url("not a url").is_err());
        assert!(validate_callback_url("/relative/path").is_err());
    }

    #[test]
    fn rejects_non_http_schemes() {
        assert!(validate_callback_url("file:///etc/passwd").is_err());
        assert!(validate_callback_url("ftp://example.com").is_err());
        assert!(validate_callback_url("unix:/var/run/docker.sock").is_err());
    }
}
