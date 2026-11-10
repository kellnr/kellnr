use std::time::Duration;

use chrono::{DateTime, Utc};
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::version::Version;
use moka::sync::Cache;

const MAX_ENTRIES: u64 = 100_000;
/// Limits how long a Kellnr instance uses a stale publish time if the version
/// is removed from the cache and cached again with another one.
const TIME_TO_LIVE: Duration = Duration::from_hours(1);

/// When a crates.io version was published, once it is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownPubtime {
    Published(DateTime<Utc>),
    /// The index entry of the version has no `pubtime`.
    Unrecorded,
}

impl KnownPubtime {
    pub fn into_pubtime(self) -> Option<DateTime<Utc>> {
        match self {
            Self::Published(pubtime) => Some(pubtime),
            Self::Unrecorded => None,
        }
    }
}

impl From<Option<DateTime<Utc>>> for KnownPubtime {
    fn from(pubtime: Option<DateTime<Utc>>) -> Self {
        pubtime.map_or(Self::Unrecorded, Self::Published)
    }
}

/// Publish times of crate versions that are known, so that a download needs
/// no database query. The publish time of a version does not change. Unlike a
/// missing license, a missing publish time is remembered too, as a later sync
/// does not add it to a cached version.
#[derive(Clone)]
pub struct KnownPubtimes(Cache<(CanonicalName, Version), KnownPubtime>);

impl Default for KnownPubtimes {
    fn default() -> Self {
        Self(
            Cache::builder()
                .max_capacity(MAX_ENTRIES)
                .time_to_live(TIME_TO_LIVE)
                .build(),
        )
    }
}

impl KnownPubtimes {
    pub fn get(&self, name: &CanonicalName, version: &Version) -> Option<KnownPubtime> {
        self.0.get(&(name.clone(), version.clone()))
    }

    pub fn insert(&self, name: &CanonicalName, version: &Version, pubtime: KnownPubtime) {
        self.0.insert((name.clone(), version.clone()), pubtime);
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn remembers_publish_times_and_unrecorded_ones() {
        let known = KnownPubtimes::default();
        let name = CanonicalName::try_from("foo").unwrap();
        let version = Version::try_from("1.0.0").unwrap();
        let other = Version::try_from("2.0.0").unwrap();
        let pubtime = Utc.with_ymd_and_hms(2026, 11, 5, 1, 18, 41).unwrap();

        known.insert(&name, &version, KnownPubtime::Published(pubtime));
        known.insert(&name, &other, KnownPubtime::Unrecorded);

        assert_eq!(
            Some(KnownPubtime::Published(pubtime)),
            known.get(&name, &version)
        );
        assert_eq!(Some(KnownPubtime::Unrecorded), known.get(&name, &other));
        assert_eq!(None, known.get(&name, &Version::try_from("3.0.0").unwrap()));
    }

    #[test]
    fn converts_from_and_into_an_optional_publish_time() {
        let pubtime = Utc.with_ymd_and_hms(2026, 11, 5, 1, 18, 41).unwrap();

        assert_eq!(
            KnownPubtime::Published(pubtime),
            KnownPubtime::from(Some(pubtime))
        );
        assert_eq!(KnownPubtime::Unrecorded, KnownPubtime::from(None));
        assert_eq!(
            Some(pubtime),
            KnownPubtime::Published(pubtime).into_pubtime()
        );
        assert_eq!(None, KnownPubtime::Unrecorded.into_pubtime());
    }
}
