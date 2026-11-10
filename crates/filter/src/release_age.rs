use std::collections::HashSet;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::index_metadata::IndexMetadata;
use kellnr_common::original_name::OriginalName;

use crate::{CrateFilter, CrateInfo, FilterError, IndexFilter, Verdict};

const NAME: &str = "release age";
/// The setting of the exempt crates, named in errors about invalid names.
const EXEMPT_CRATES_SETTING: &str = "cratesio_filter.min_release_age_exempt_crates";

/// Holds back crates.io versions until they are a minimum age, counted from
/// the `pubtime` of their index entry, so that a malicious release can be
/// caught upstream before builds pull it. Younger versions are left out of the
/// index and their downloads are denied.
///
/// Entries without a `pubtime` are old enough. crates.io writes `pubtime` into
/// every entry and backfilled the old ones, so only entries that Kellnr cached
/// before the field existed lack it.
pub struct ReleaseAgeFilter {
    min_age: TimeDelta,
    exempt_crates: HashSet<CanonicalName>,
}

impl ReleaseAgeFilter {
    /// The versions of `exempt_crates` are always served. Their names are
    /// compared like cargo does. Blank names, e.g. from a trailing comma in
    /// the environment variable, are skipped, other invalid names are an
    /// error.
    pub fn new(min_age: Duration, exempt_crates: &[String]) -> Result<Self, FilterError> {
        let exempt_crates = exempt_crates
            .iter()
            .filter(|name| !name.trim().is_empty())
            .map(|name| {
                CanonicalName::try_from(name.trim()).map_err(|e| FilterError::InvalidCrateName {
                    setting: EXEMPT_CRATES_SETTING,
                    name: name.clone(),
                    reason: e.to_string(),
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            // A minimum age beyond the representable time span is never reached.
            min_age: TimeDelta::from_std(min_age).unwrap_or(TimeDelta::MAX),
            exempt_crates,
        })
    }

    /// [`IndexFilter::keep`] at `now`.
    fn keep_at(&self, entry: &IndexMetadata, now: DateTime<Utc>) -> bool {
        let Some(pubtime) = entry.pubtime else {
            return true;
        };
        if self.held_back_for(pubtime, now).is_none() {
            return true;
        }
        // A name no crate can have is never exempt.
        CanonicalName::try_from(entry.name.as_str())
            .is_ok_and(|name| self.exempt_crates.contains(&name))
    }

    /// [`CrateFilter::check`] at `now`.
    fn check_at(&self, krate: &CrateInfo, now: DateTime<Utc>) -> Verdict {
        let Some(pubtime) = krate.pubtime else {
            return Verdict::Allow;
        };
        let Some(remaining) = self.held_back_for(pubtime, now) else {
            return Verdict::Allow;
        };
        if self.exempt_crates.contains(&krate.name.to_canonical()) {
            return Verdict::Allow;
        }
        Verdict::Deny {
            filter: NAME,
            reason: format!(
                "it was published on crates.io at {} and is held back by the minimum release age of {} for another {}",
                pubtime.to_rfc3339_opts(SecondsFormat::Secs, true),
                format_duration(self.min_age),
                format_duration(remaining),
            ),
        }
    }

    /// How much longer a version published at `pubtime` is held back at
    /// `now`, `None` if it is old enough.
    fn held_back_for(&self, pubtime: DateTime<Utc>, now: DateTime<Utc>) -> Option<TimeDelta> {
        // A version that would only be old enough after the last representable
        // date is never old enough.
        let remaining = pubtime
            .checked_add_signed(self.min_age)
            .map_or(TimeDelta::MAX, |old_enough| old_enough - now);
        (remaining > TimeDelta::zero()).then_some(remaining)
    }
}

impl IndexFilter for ReleaseAgeFilter {
    fn name(&self) -> &'static str {
        NAME
    }

    fn keep(&self, entry: &IndexMetadata) -> bool {
        self.keep_at(entry, Utc::now())
    }
}

impl CrateFilter for ReleaseAgeFilter {
    fn name(&self) -> &'static str {
        NAME
    }

    fn check(&self, krate: &CrateInfo) -> Verdict {
        self.check_at(krate, Utc::now())
    }

    fn needs_pubtime(&self, name: &OriginalName) -> bool {
        !self.exempt_crates.contains(&name.to_canonical())
    }
}

/// A duration like "2d 3h 4m 5s", without the units that are zero.
fn format_duration(duration: TimeDelta) -> String {
    let secs = duration.num_seconds().max(0);
    let parts: Vec<String> = [
        (secs / 86_400, "d"),
        (secs / 3600 % 24, "h"),
        (secs / 60 % 60, "m"),
        (secs % 60, "s"),
    ]
    .into_iter()
    .filter(|(value, _)| *value > 0)
    .map(|(value, unit)| format!("{value}{unit}"))
    .collect();
    if parts.is_empty() {
        "0s".to_string()
    } else {
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::TimeZone;
    use kellnr_common::version::Version;

    use super::*;
    use crate::IndexFilterChain;

    const THREE_DAYS: Duration = Duration::from_hours(72);

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 10, 12, 0, 0).unwrap()
    }

    fn ago(age: TimeDelta) -> DateTime<Utc> {
        now() - age
    }

    fn filter(exempt: &[&str]) -> ReleaseAgeFilter {
        let exempt: Vec<_> = exempt.iter().map(ToString::to_string).collect();
        ReleaseAgeFilter::new(THREE_DAYS, &exempt).unwrap()
    }

    fn keep(filter: &ReleaseAgeFilter, name: &str, pubtime: Option<DateTime<Utc>>) -> bool {
        let entry = IndexMetadata {
            pubtime,
            ..IndexMetadata::minimal(name, "1.0.0", "cksum")
        };
        filter.keep_at(&entry, now())
    }

    fn check(filter: &ReleaseAgeFilter, name: &str, pubtime: Option<DateTime<Utc>>) -> Verdict {
        filter.check_at(
            &CrateInfo {
                name: &OriginalName::try_from(name).unwrap(),
                version: &Version::try_from("1.0.0").unwrap(),
                license: None,
                pubtime,
            },
            now(),
        )
    }

    fn is_denied(verdict: &Verdict) -> bool {
        matches!(verdict, Verdict::Deny { filter: NAME, .. })
    }

    #[test]
    fn young_versions_are_held_back() {
        let f = filter(&[]);

        for pubtime in [
            ago(TimeDelta::hours(1)),
            ago(TimeDelta::days(3) - TimeDelta::seconds(1)),
            // Published in the future, e.g. with a skewed clock
            ago(TimeDelta::hours(-1)),
        ] {
            assert!(!keep(&f, "foo", Some(pubtime)), "{pubtime}");
            assert!(is_denied(&check(&f, "foo", Some(pubtime))), "{pubtime}");
        }
    }

    #[test]
    fn versions_are_served_once_they_are_old_enough() {
        let f = filter(&[]);

        for pubtime in [ago(TimeDelta::days(3)), ago(TimeDelta::days(400))] {
            assert!(keep(&f, "foo", Some(pubtime)), "{pubtime}");
            assert_eq!(Verdict::Allow, check(&f, "foo", Some(pubtime)), "{pubtime}");
        }
    }

    #[test]
    fn versions_without_publish_time_are_old_enough() {
        let f = filter(&[]);

        assert!(keep(&f, "foo", None));
        assert_eq!(Verdict::Allow, check(&f, "foo", None));
    }

    #[test]
    fn young_versions_of_exempt_crates_are_served() {
        let f = filter(&["OpenSSL", " Tokio-Util "]);
        let young = Some(ago(TimeDelta::hours(1)));

        // Names that only differ in case or in `-` versus `_` are the same crate
        for name in ["openssl", "OPENSSL", "tokio_util", "Tokio-Util"] {
            assert!(keep(&f, name, young), "{name}");
            assert_eq!(Verdict::Allow, check(&f, name, young), "{name}");
        }
        assert!(!keep(&f, "openssl-sys", young));
        assert!(is_denied(&check(&f, "openssl-sys", young)));
    }

    #[test]
    fn only_crates_that_are_not_exempt_need_the_publish_time() {
        let f = filter(&["openssl"]);

        assert!(f.needs_pubtime(&OriginalName::try_from("serde").unwrap()));
        assert!(!f.needs_pubtime(&OriginalName::try_from("OpenSSL").unwrap()));
    }

    #[test]
    fn deny_message_tells_the_remaining_time() {
        let f = filter(&[]);
        let krate = CrateInfo {
            name: &OriginalName::try_from("Upstream").unwrap(),
            version: &Version::try_from("1.0.0").unwrap(),
            license: None,
            pubtime: Some(Utc.with_ymd_and_hms(2026, 11, 2, 10, 0, 0).unwrap()),
        };
        let now = Utc.with_ymd_and_hms(2026, 11, 3, 11, 0, 30).unwrap();

        assert_eq!(
            Some(
                "Download of upstream@1.0.0 denied by Kellnr release age filter: it was published on crates.io at 2026-11-02T10:00:00Z and is held back by the minimum release age of 3d for another 1d 22h 59m 30s"
                    .to_string()
            ),
            f.check_at(&krate, now).deny_message(&krate)
        );
        assert_eq!("release age", IndexFilter::name(&f));
        assert_eq!("release age", CrateFilter::name(&f));
    }

    #[test]
    fn huge_minimum_age_is_never_reached() {
        let long_ago = Some(Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap());
        // Beyond the range of `TimeDelta`, and within it, but past the last
        // representable date
        for min_age in [u64::MAX, 300_000 * 365 * 86_400] {
            let f = ReleaseAgeFilter::new(Duration::from_secs(min_age), &[]).unwrap();

            assert!(!keep(&f, "foo", long_ago), "{min_age}");
            assert!(is_denied(&check(&f, "foo", long_ago)), "{min_age}");
        }
    }

    #[test]
    fn blank_exempt_crates_are_skipped() {
        let exempt = ["openssl".to_string(), String::new(), " ".to_string()];

        let f = ReleaseAgeFilter::new(THREE_DAYS, &exempt).unwrap();

        assert_eq!(
            HashSet::from([CanonicalName::try_from("openssl").unwrap()]),
            f.exempt_crates
        );
    }

    #[test]
    fn invalid_exempt_crates_are_rejected() {
        for name in ["open ssl", "-openssl", "1password", "foo/bar"] {
            let Err(e) = ReleaseAgeFilter::new(THREE_DAYS, &[name.to_string()]) else {
                panic!("{name:?} must be rejected");
            };

            assert_eq!(
                format!(
                    "Invalid crate name \"{name}\" in cratesio_filter.min_release_age_exempt_crates: Invalid character in name"
                ),
                e.to_string()
            );
        }
    }

    #[test]
    fn index_chain_leaves_out_young_entries_and_unreadable_publish_times() {
        let chain = IndexFilterChain::new(vec![Arc::new(filter(&[]))]);
        let line = |vers: &str, pubtime: &str| {
            format!(
                r#"{{"name":"foo","vers":"{vers}","deps":[],"cksum":"c","features":{{}},"yanked":false,"pubtime":{pubtime}}}"#
            )
        };
        let young = Utc::now().format("\"%Y-%m-%dT%H:%M:%SZ\"").to_string();
        let data = [
            line("1.0.0", r#""2016-12-23T09:00:00Z""#),
            line("1.1.0", "null"),
            line("1.2.0", &young),
            line("1.3.0", r#""yesterday""#),
        ]
        .join("\n");

        let kept = chain.filter(&OriginalName::try_from("foo").unwrap(), data.as_bytes());

        assert_eq!(
            Some(format!(
                "{}\n{}\n",
                line("1.0.0", r#""2016-12-23T09:00:00Z""#),
                line("1.1.0", "null")
            )),
            kept.map(|kept| String::from_utf8(kept).unwrap())
        );
    }

    #[test]
    fn durations_are_shown_in_units_up_to_days() {
        assert_eq!("3d", format_duration(TimeDelta::days(3)));
        assert_eq!("1h 1s", format_duration(TimeDelta::seconds(3601)));
        assert_eq!(
            "2d 6h 44m 13s",
            format_duration(TimeDelta::seconds(197_053))
        );
        assert_eq!("0s", format_duration(TimeDelta::zero()));
    }
}
