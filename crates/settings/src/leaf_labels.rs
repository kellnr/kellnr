//! Human-readable display labels for individual settings leaves.
//!
//! The `/settings` UI used to hand-maintain a parallel label table in
//! `StartupConfig.vue`. That table drifted whenever a field was added or
//! renamed. The Vue file now humanizes the dotted path by default
//! (`data_dir` → `Data Directory`); this lookup only carries the
//! *exceptions* — labels
//! that the humanizer can't get right because they encode acronyms (DB, URL),
//! units ("(seconds)", "(MB)"), or a deliberate UI shortening.
//!
//! Add an entry here only if the auto-humanized form is wrong; missing keys
//! are not an error.

/// Returns a display label override for `dotted_path`, or `None` if the
/// auto-humanized form is fine.
#[must_use]
pub fn leaf_label(dotted_path: &str) -> Option<&'static str> {
    Some(match dotted_path {
        // Units that the humanizer can't infer from the field name.
        "registry.session_age_seconds" => "Session Age (seconds)",
        "registry.token_cache_ttl_seconds" => "Token Cache TTL (seconds)",
        "registry.token_db_retry_delay_ms" => "Token DB Retry Delay (ms)",
        "registry.download_timeout_seconds" => "Download Timeout (seconds)",
        "registry.download_counter_flush_seconds" => "Download Counter Flush (seconds)",
        "proxy.connect_timeout_seconds" | "s3.connect_timeout_seconds" => {
            "Connect Timeout (seconds)"
        }
        "proxy.request_timeout_seconds" | "s3.request_timeout_seconds" => {
            "Request Timeout (seconds)"
        }
        "toolchain.max_size" => "Max Size (MB)",

        // Acronyms — humanizer would emit "Db", "Url", "Api", "Ip", "Http".
        "registry.max_db_connections" => "Max DB Connections",
        "registry.token_db_retry_count" => "Token DB Retry Count",
        "proxy.url" => "URL",
        "proxy.index" => "Index URL",
        "proxy.api" => "API URL",
        "postgresql.db" => "Database",
        "postgresql.address" => "Address",
        "local.ip" => "IP",
        "s3.allow_http" => "Allow HTTP",

        // Spelled-out forms preferred over the abbreviation in the field name.
        "registry.data_dir" => "Data Directory",

        // Shortenings the UI deliberately chose.
        "proxy.num_threads" => "Number of Threads",
        "s3.cratesio_bucket" => "Crates.io Bucket",

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_overrides_for_known_keys() {
        assert_eq!(
            leaf_label("registry.session_age_seconds"),
            Some("Session Age (seconds)")
        );
        assert_eq!(leaf_label("proxy.url"), Some("URL"));
        assert_eq!(leaf_label("local.ip"), Some("IP"));
    }

    #[test]
    fn returns_none_for_humanizable_keys() {
        // Keys whose humanized form is fine ("Enabled", "Cache Size") have no
        // override — the UI synthesizes the label client-side.
        assert_eq!(leaf_label("docs.enabled"), None);
        assert_eq!(leaf_label("registry.cache_size"), None);
        assert_eq!(leaf_label("not.a.real.key"), None);
    }
}
