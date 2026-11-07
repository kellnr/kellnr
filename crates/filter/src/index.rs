use std::sync::Arc;

use kellnr_common::index_metadata::IndexMetadata;
use kellnr_common::original_name::OriginalName;
use tracing::{debug, warn};

/// Decides whether an entry of a sparse index file is served. A left out
/// version is hidden from cargo's resolver, but can still be downloaded, e.g.
/// with a lockfile that pins it. A filter that has to prevent that implements
/// [`CrateFilter`](crate::CrateFilter) too.
pub trait IndexFilter: Send + Sync {
    fn name(&self) -> &'static str;
    /// Whether the entry is served.
    fn keep(&self, entry: &IndexMetadata) -> bool;
}

/// Runs all index filters, an entry is served if every filter keeps it.
#[derive(Clone, Default)]
pub struct IndexFilterChain {
    filters: Vec<Arc<dyn IndexFilter>>,
}

impl IndexFilterChain {
    pub fn new(filters: Vec<Arc<dyn IndexFilter>>) -> Self {
        Self { filters }
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    /// The entries of the sparse index file `data` of the crate `name` that
    /// all filters keep, `None` if there are none. The kept lines are not
    /// changed. Entries that cannot be parsed are left out, as the filters
    /// cannot decide on them.
    pub fn filter(&self, name: &OriginalName, data: &[u8]) -> Option<Vec<u8>> {
        let mut kept = Vec::with_capacity(data.len());
        for line in data.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
            let entry = match serde_json::from_slice::<IndexMetadata>(line) {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("Leaving out an unreadable index entry of {name}: {e}");
                    continue;
                }
            };
            if let Some(filter) = self.filters.iter().find(|f| !f.keep(&entry)) {
                debug!(
                    "Index filter {} left out {name}@{}",
                    filter.name(),
                    entry.vers
                );
                continue;
            }
            kept.extend_from_slice(line);
            kept.push(b'\n');
        }
        (!kept.is_empty()).then_some(kept)
    }
}

/// The index filters for the sparse index of the crates.io proxy.
#[derive(Clone, Default)]
pub struct IndexFilters {
    pub cratesio: IndexFilterChain,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Leaves out the version `vers`.
    struct Without(&'static str);

    impl IndexFilter for Without {
        fn name(&self) -> &'static str {
            "without"
        }

        fn keep(&self, entry: &IndexMetadata) -> bool {
            entry.vers != self.0
        }
    }

    fn line(vers: &str) -> String {
        format!(
            r#"{{"name":"foo","vers":"{vers}","deps":[],"cksum":"c","features":{{}},"yanked":false}}"#
        )
    }

    fn index(lines: &[String]) -> Vec<u8> {
        lines
            .iter()
            .flat_map(|l| [l.as_bytes(), b"\n"])
            .flatten()
            .copied()
            .collect()
    }

    fn filter(chain: &IndexFilterChain, data: &[u8]) -> Option<String> {
        chain
            .filter(&OriginalName::try_from("foo").unwrap(), data)
            .map(|kept| String::from_utf8(kept).unwrap())
    }

    #[test]
    fn entries_are_served_if_every_filter_keeps_them() {
        let chain =
            IndexFilterChain::new(vec![Arc::new(Without("1.1.0")), Arc::new(Without("1.2.0"))]);
        let data = index(&[line("1.0.0"), line("1.1.0"), line("1.2.0"), line("1.3.0")]);

        assert_eq!(
            Some(String::from_utf8(index(&[line("1.0.0"), line("1.3.0")])).unwrap()),
            filter(&chain, &data)
        );
    }

    #[test]
    fn kept_lines_are_not_changed() {
        let chain = IndexFilterChain::new(vec![Arc::new(Without("2.0.0"))]);
        // Field order and an unknown field that a reserialization would change
        let kept = r#"{"vers":"1.0.0","name":"foo","deps":[],"cksum":"c","features":{},"yanked":false,"new_field":1}"#;
        let data = format!("{kept}\n{}", line("2.0.0"));

        assert_eq!(Some(format!("{kept}\n")), filter(&chain, data.as_bytes()));
    }

    #[test]
    fn unreadable_entries_are_left_out() {
        let chain = IndexFilterChain::new(vec![Arc::new(Without("2.0.0"))]);
        let data = format!("{}\nnot json\n{{\"vers\":\"1.1.0\"}}\n", line("1.0.0"));

        assert_eq!(
            Some(format!("{}\n", line("1.0.0"))),
            filter(&chain, data.as_bytes())
        );
    }

    #[test]
    fn file_without_kept_entries_is_none() {
        let chain = IndexFilterChain::new(vec![Arc::new(Without("1.0.0"))]);

        assert_eq!(None, filter(&chain, &index(&[line("1.0.0")])));
        assert_eq!(None, filter(&chain, b"\n"));
    }

    #[test]
    fn empty_chain_keeps_every_readable_entry() {
        let chain = IndexFilterChain::default();
        let data = index(&[line("1.0.0"), line("2.0.0")]);

        assert!(chain.is_empty());
        assert_eq!(
            Some(String::from_utf8(data.clone()).unwrap()),
            filter(&chain, &data)
        );
    }
}
