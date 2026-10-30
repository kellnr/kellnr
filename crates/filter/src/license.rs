use std::collections::HashSet;

use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::license::UNKNOWN_LICENSE;
use kellnr_settings::LicenseUnknown;
use spdx::{Expression, LicenseReq, Licensee, ParseMode};
use tracing::debug;

use crate::{CrateFilter, CrateInfo, FilterError, Verdict};

/// Allows a crate if its SPDX license expression is satisfied by the allowed
/// licenses. With `OR`, one allowed license is enough. With `AND`, every
/// license must be allowed.
pub struct LicenseFilter {
    allowed: Vec<AllowedLicense>,
    unknown: LicenseUnknown,
    exempt_crates: HashSet<CanonicalName>,
}

struct AllowedLicense {
    licensee: Licensee,
    /// Without an exception (`WITH`), the license is allowed with any
    /// exception, as exceptions only grant additional permissions.
    any_addition: bool,
}

impl AllowedLicense {
    fn satisfies(&self, req: &LicenseReq) -> bool {
        if self.any_addition {
            self.licensee.satisfies(&LicenseReq {
                license: req.license.clone(),
                addition: None,
            })
        } else {
            self.licensee.satisfies(req)
        }
    }
}

impl LicenseFilter {
    /// `section` names the settings section in errors about invalid licenses.
    pub fn new(
        section: &'static str,
        allowed: &[String],
        unknown: LicenseUnknown,
        exempt_crates: &[String],
    ) -> Result<Self, FilterError> {
        let allowed = allowed
            .iter()
            .map(|license| {
                let licensee =
                    Licensee::parse_mode(license.trim(), ParseMode::LAX).map_err(|e| {
                        FilterError::InvalidLicense {
                            section,
                            license: license.clone(),
                            reason: e.reason.to_string(),
                        }
                    })?;
                let any_addition = licensee.clone().into_req().addition.is_none();
                Ok(AllowedLicense {
                    licensee,
                    any_addition,
                })
            })
            .collect::<Result<_, _>>()?;

        Ok(Self {
            allowed,
            unknown,
            // Crate names are compared like cargo does. A name no crate can
            // have never matches.
            exempt_crates: exempt_crates
                .iter()
                .filter_map(|c| CanonicalName::try_from(c.trim()).ok())
                .collect(),
        })
    }

    fn unknown(&self, reason: String) -> Verdict {
        match self.unknown {
            LicenseUnknown::Allow => Verdict::Allow,
            LicenseUnknown::Deny => Verdict::Deny {
                filter: self.name(),
                reason,
            },
        }
    }
}

impl CrateFilter for LicenseFilter {
    fn name(&self) -> &'static str {
        "license"
    }

    fn check(&self, krate: &CrateInfo) -> Verdict {
        if self.exempt_crates.contains(&krate.name.to_canonical()) {
            return Verdict::Allow;
        }

        let license = match krate.license.map(str::trim) {
            None | Some("") => return self.unknown("the license is unknown".to_string()),
            Some(l) if l == UNKNOWN_LICENSE => {
                return self.unknown("the crate declares no license".to_string());
            }
            Some(l) => l,
        };

        // An expression that cannot be parsed is treated like an unknown
        // license, so `license_unknown` decides about it.
        let expression = match Expression::parse_mode(license, ParseMode::LAX) {
            Ok(expression) => expression,
            Err(e) => {
                debug!(
                    "License \"{license}\" of {} {} is not a valid SPDX expression, applying license_unknown: {e}",
                    krate.name, krate.version
                );
                return self.unknown(format!(
                    "license \"{license}\" is not a valid SPDX expression"
                ));
            }
        };

        if expression.evaluate(|req| self.allowed.iter().any(|a| a.satisfies(req))) {
            Verdict::Allow
        } else {
            Verdict::Deny {
                filter: self.name(),
                reason: format!("license \"{license}\" is not allowed"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use kellnr_common::original_name::OriginalName;
    use kellnr_common::version::Version;

    use super::*;

    fn filter(allowed: &[&str], unknown: LicenseUnknown, exempt: &[&str]) -> LicenseFilter {
        LicenseFilter::new(
            "cratesio_filter",
            &allowed.iter().map(ToString::to_string).collect::<Vec<_>>(),
            unknown,
            &exempt.iter().map(ToString::to_string).collect::<Vec<_>>(),
        )
        .unwrap()
    }

    fn check_crate(filter: &LicenseFilter, name: &str, license: Option<&str>) -> Verdict {
        filter.check(&CrateInfo {
            name: &OriginalName::try_from(name).unwrap(),
            version: &Version::from_unchecked_str("1.0.0"),
            license,
        })
    }

    fn check(filter: &LicenseFilter, license: Option<&str>) -> Verdict {
        check_crate(filter, "foo", license)
    }

    fn is_allowed(filter: &LicenseFilter, license: Option<&str>) -> bool {
        check(filter, license) == Verdict::Allow
    }

    #[test]
    fn single_license() {
        let f = filter(&["MIT"], LicenseUnknown::Deny, &[]);

        assert!(is_allowed(&f, Some("MIT")));
        assert_eq!(
            Verdict::Deny {
                filter: "license",
                reason: "license \"GPL-3.0-only\" is not allowed".to_string()
            },
            check(&f, Some("GPL-3.0-only"))
        );
    }

    #[test]
    fn or_needs_one_allowed_license() {
        let f = filter(&["Apache-2.0"], LicenseUnknown::Deny, &[]);

        assert!(is_allowed(&f, Some("MIT OR Apache-2.0")));
        assert!(!is_allowed(&f, Some("MIT OR GPL-3.0-only")));
    }

    #[test]
    fn and_needs_all_licenses_allowed() {
        let f = filter(&["MIT", "Unicode-3.0"], LicenseUnknown::Deny, &[]);

        assert!(is_allowed(&f, Some("MIT AND Unicode-3.0")));
        assert!(!is_allowed(&f, Some("MIT AND BSD-3-Clause")));
        assert!(is_allowed(&f, Some("(MIT OR Apache-2.0) AND Unicode-3.0")));
    }

    #[test]
    fn with_exception_is_allowed_by_base_license() {
        let f = filter(&["Apache-2.0"], LicenseUnknown::Deny, &[]);

        assert!(is_allowed(&f, Some("Apache-2.0 WITH LLVM-exception")));
    }

    #[test]
    fn with_exception_listed_exactly_does_not_allow_base_license() {
        let f = filter(
            &["Apache-2.0 WITH LLVM-exception"],
            LicenseUnknown::Deny,
            &[],
        );

        assert!(is_allowed(&f, Some("Apache-2.0 WITH LLVM-exception")));
        assert!(!is_allowed(&f, Some("Apache-2.0")));
    }

    #[test]
    fn lax_parsing_accepts_old_syntax() {
        let f = filter(&["MIT"], LicenseUnknown::Deny, &[]);

        assert!(is_allowed(&f, Some("MIT/Apache-2.0")));
        assert!(is_allowed(&f, Some("Apache-2.0/MIT")));
    }

    #[test]
    fn no_license_is_unknown() {
        let deny = filter(&["MIT"], LicenseUnknown::Deny, &[]);
        let allow = filter(&["MIT"], LicenseUnknown::Allow, &[]);

        for license in [None, Some(UNKNOWN_LICENSE), Some("")] {
            assert!(!is_allowed(&deny, license), "{license:?}");
            assert!(is_allowed(&allow, license), "{license:?}");
        }
        assert_eq!(
            Verdict::Deny {
                filter: "license",
                reason: "the crate declares no license".to_string()
            },
            check(&deny, Some(UNKNOWN_LICENSE))
        );
    }

    #[test]
    fn unparsable_expression_is_unknown() {
        let deny = filter(&["MIT"], LicenseUnknown::Deny, &[]);
        let allow = filter(&["MIT"], LicenseUnknown::Allow, &[]);

        assert_eq!(
            Verdict::Deny {
                filter: "license",
                reason: "license \"My own license\" is not a valid SPDX expression".to_string()
            },
            check(&deny, Some("My own license"))
        );
        assert!(is_allowed(&allow, Some("My own license")));
    }

    #[test]
    fn exempt_crate_is_always_allowed() {
        let f = filter(&["MIT"], LicenseUnknown::Deny, &["Ring", "my_crate"]);

        assert_eq!(Verdict::Allow, check_crate(&f, "ring", None));
        assert_eq!(Verdict::Allow, check_crate(&f, "ring", Some("ISC")));
        assert_eq!(Verdict::Allow, check_crate(&f, "my-crate", Some("ISC")));
        assert!(!is_allowed(&f, Some("ISC")));
    }

    #[test]
    fn invalid_allowed_licenses_are_rejected() {
        for license in ["MTI", "MIT OR Apache-2.0", ""] {
            let result = LicenseFilter::new(
                "registry_filter",
                &[license.to_string()],
                LicenseUnknown::Deny,
                &[],
            );

            assert!(result.is_err(), "{license:?} must be rejected");
        }
    }
}
