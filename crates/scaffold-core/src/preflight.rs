//! The rules checked before anything is rendered or compared. Ported from `runner/src/preflight.ts`.

use std::cmp::Ordering;

use crate::contract::Contract;
use crate::manifest::Manifest;
use crate::semver::{compare_semver, parse_semver};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct KnownBadVersion {
    pub version: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct KnownBad {
    pub versions: Vec<KnownBadVersion>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Render,
    Check,
}

pub struct PreflightOptions<'a> {
    pub mode: Mode,
    pub runner_version: &'a str,
    pub force: bool,
    pub strict: bool,
    pub known_bad: Option<&'a KnownBad>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Preflight {
    pub package_version: Option<String>,
    pub errors: Vec<String>,
    pub refusals: Vec<String>,
    pub warnings: Vec<String>,
}

/// The version the package was loaded as, read from its `package://` URI. A local package has none. A package URI whose
/// version cannot be read is an error, never ignored.
pub fn package_version_of(uri: Option<&str>) -> Result<Option<String>, String> {
    static VERSION: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"@([^@#/:]+)(?:::sha256:[0-9a-f]{64})?#").expect("valid")
    });
    let Some(uri) = uri.filter(|u| u.starts_with("package://")) else {
        return Ok(None);
    };
    // A consumer pins the package with its checksum, so the URI can read ...@0.1.0::sha256:<64 hex>#/Config.pkl.
    match VERSION
        .captures(uri)
        .map(|m| m[1].to_owned())
        .filter(|v| parse_semver(v).is_some())
    {
        Some(version) => Ok(Some(version)),
        None => Err(format!("{uri}: the package URI has no readable version")),
    }
}

/// The rules checked before anything is rendered: the runner is new enough for the package, the package is not older than
/// the one that last rendered this repo, the package is not on the known-bad list, and no required Security builtin is
/// disabled. errors stop the run (exit 1), refusals are exit 3, warnings are printed.
pub fn preflight(contract: &Contract, manifest: &Manifest, opts: &PreflightOptions) -> Preflight {
    let mut out = Preflight::default();
    match package_version_of(contract.package_uri.as_deref()) {
        Ok(version) => out.package_version = version,
        Err(message) => {
            out.errors.push(message);
            return out;
        }
    }
    let version = out.package_version.clone();

    if let Some(need) = contract.requires.as_ref().map(|r| r.runner.as_str()) {
        if parse_semver(need).is_none() {
            out.errors.push(format!(
                "the package requires runner {}, which is not a version",
                json(need)
            ));
        } else if parse_semver(opts.runner_version).is_none() {
            out.errors.push(format!(
                "runner version {} is not a version",
                json(opts.runner_version)
            ));
        } else if compare_semver(opts.runner_version, need) == Ok(Ordering::Less) {
            out.errors.push(format!(
                "runner {} is older than the {need} this package requires, upgrade the runner",
                opts.runner_version
            ));
        }
    }

    if let (Some(version), Some(last)) = (&version, &manifest.package_version) {
        if parse_semver(last).is_none() {
            out.errors.push(format!(
                "the manifest records packageVersion {}, which is not a version",
                json(last)
            ));
        } else if compare_semver(version, last) == Ok(Ordering::Less) {
            let message = format!(
                "package {version} is older than {last}, which rendered this repository last"
            );
            if opts.mode == Mode::Render && !opts.force {
                out.refusals.push(format!(
                    "{message} (use --force to render an older package)"
                ));
            } else {
                out.warnings.push(message);
            }
        }
    }

    let listed = version.as_ref().and_then(|v| {
        opts.known_bad
            .and_then(|bad| bad.versions.iter().find(|b| b.version == *v))
    });
    if let (Some(version), Some(listed)) = (&version, listed) {
        let message = format!(
            "package {version} is on the known-bad list: {}",
            listed.reason
        );
        if opts.strict {
            out.refusals.push(format!("{message} (--strict)"));
        } else {
            out.warnings.push(message);
        }
    }

    for builtin in contract
        .security_builtins
        .iter()
        .flatten()
        .filter(|b| !b.enabled)
    {
        out.warnings.push(format!(
            "required Security builtin {} is disabled",
            builtin.name
        ));
    }
    out
}

fn json(text: &str) -> String {
    serde_json::to_string(text).expect("a string serialises")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Mode as FileMode, Requires, SecurityBuiltin};
    use crate::testutil::{contract_of, owned};

    fn uri(v: &str) -> String {
        format!(
            "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{v}/pantheon-scaffold@{v}#/Config.pkl"
        )
    }

    fn contract(version: Option<&str>) -> Contract {
        let mut c = contract_of(vec![owned("a.txt", "x\n", FileMode::Regular)]);
        c.package_uri = version.map(uri);
        c
    }

    fn base() -> PreflightOptions<'static> {
        PreflightOptions {
            mode: Mode::Render,
            runner_version: "0.5.0",
            force: false,
            strict: false,
            known_bad: None,
        }
    }

    fn manifest_at(v: &str) -> Manifest {
        Manifest {
            package_version: Some(v.into()),
            ..Manifest::empty()
        }
    }

    fn requiring(version: Option<&str>, runner: &str) -> Contract {
        Contract {
            requires: Some(Requires {
                runner: runner.into(),
            }),
            ..contract(version)
        }
    }

    fn run(c: &Contract, m: &Manifest, opts: &PreflightOptions) -> Preflight {
        preflight(c, m, opts)
    }

    #[test]
    fn the_version_is_read_from_the_package_uri_pre_releases_included() {
        assert_eq!(
            package_version_of(Some(&uri("0.2.0"))),
            Ok(Some("0.2.0".into()))
        );
        assert_eq!(
            package_version_of(Some(&uri("0.2.0-rc.1"))),
            Ok(Some("0.2.0-rc.1".into()))
        );
    }

    #[test]
    fn a_local_package_or_no_uri_has_no_version() {
        assert_eq!(package_version_of(None), Ok(None));
        assert_eq!(
            package_version_of(Some("file:///work/pkl/Config.pkl")),
            Ok(None)
        );
        assert_eq!(package_version_of(Some("")), Ok(None));
    }

    #[test]
    fn a_package_uri_with_no_readable_version_is_an_error_never_ignored() {
        let bad = "package://github.com/x/y/releases/download/v1/y@latest#/Config.pkl";
        assert_eq!(
            package_version_of(Some(bad)),
            Err(format!("{bad}: the package URI has no readable version"))
        );
        assert!(package_version_of(Some("package://github.com/x/y#/Config.pkl")).is_err());
    }

    #[test]
    fn a_render_with_no_package_version_and_nothing_to_report_is_clean() {
        assert_eq!(
            run(&contract(None), &Manifest::empty(), &base()),
            Preflight::default()
        );
    }

    #[test]
    fn the_version_is_returned_for_the_manifest() {
        assert_eq!(
            run(&contract(Some("0.2.0")), &Manifest::empty(), &base())
                .package_version
                .as_deref(),
            Some("0.2.0")
        );
    }

    #[test]
    fn a_runner_older_than_the_package_requires_is_an_error() {
        let r = run(
            &requiring(Some("0.2.0"), "0.6.0"),
            &Manifest::empty(),
            &base(),
        );
        assert_eq!(
            r.errors,
            ["runner 0.5.0 is older than the 0.6.0 this package requires, upgrade the runner"]
        );
    }

    #[test]
    fn a_runner_that_meets_the_requirement_or_exceeds_it_passes() {
        assert!(
            run(
                &requiring(Some("0.2.0"), "0.5.0"),
                &Manifest::empty(),
                &base()
            )
            .errors
            .is_empty()
        );
        assert!(
            run(
                &requiring(Some("0.2.0"), "0.4.0"),
                &Manifest::empty(),
                &base()
            )
            .errors
            .is_empty()
        );
    }

    #[test]
    fn a_requirement_that_is_not_a_version_is_an_error() {
        let r = run(
            &requiring(Some("0.2.0"), "soon"),
            &Manifest::empty(),
            &base(),
        );
        assert_eq!(
            r.errors,
            ["the package requires runner \"soon\", which is not a version"]
        );
    }

    #[test]
    fn a_requirement_is_checked_for_a_local_package_too() {
        assert_eq!(
            run(&requiring(None, "9.0.0"), &Manifest::empty(), &base())
                .errors
                .len(),
            1
        );
    }

    #[test]
    fn a_runner_version_that_is_not_a_version_is_an_error() {
        let opts = PreflightOptions {
            runner_version: "dev",
            ..base()
        };
        let r = run(&requiring(None, "0.1.0"), &Manifest::empty(), &opts);
        assert_eq!(r.errors, ["runner version \"dev\" is not a version"]);
    }

    #[test]
    fn an_older_package_than_the_last_render_is_refused_and_force_allows_it_with_a_warning() {
        let refused = run(&contract(Some("0.2.0")), &manifest_at("0.3.0"), &base());
        assert_eq!(
            refused.refusals,
            [
                "package 0.2.0 is older than 0.3.0, which rendered this repository last (use --force to render an older package)"
            ]
        );
        let forced = run(
            &contract(Some("0.2.0")),
            &manifest_at("0.3.0"),
            &PreflightOptions {
                force: true,
                ..base()
            },
        );
        assert!(forced.refusals.is_empty());
        assert_eq!(
            forced.warnings,
            ["package 0.2.0 is older than 0.3.0, which rendered this repository last"]
        );
    }

    #[test]
    fn a_pre_release_of_the_recorded_version_counts_as_older() {
        assert_eq!(
            run(
                &contract(Some("0.3.0-rc.1")),
                &manifest_at("0.3.0"),
                &base()
            )
            .refusals
            .len(),
            1
        );
    }

    #[test]
    fn the_same_version_and_a_newer_version_are_fine() {
        assert!(
            run(&contract(Some("0.3.0")), &manifest_at("0.3.0"), &base())
                .refusals
                .is_empty()
        );
        assert!(
            run(&contract(Some("0.4.0")), &manifest_at("0.3.0"), &base())
                .refusals
                .is_empty()
        );
    }

    #[test]
    fn a_check_only_warns_about_an_older_package_it_writes_nothing() {
        let r = run(
            &contract(Some("0.2.0")),
            &manifest_at("0.3.0"),
            &PreflightOptions {
                mode: Mode::Check,
                ..base()
            },
        );
        assert!(r.refusals.is_empty());
        assert_eq!(r.warnings.len(), 1);
    }

    #[test]
    fn a_manifest_version_that_is_not_a_version_is_an_error() {
        let r = run(&contract(Some("0.2.0")), &manifest_at("weird"), &base());
        assert_eq!(
            r.errors,
            ["the manifest records packageVersion \"weird\", which is not a version"]
        );
    }

    #[test]
    fn a_manifest_version_is_only_compared_for_a_package_with_a_version() {
        assert_eq!(
            run(&contract(None), &manifest_at("weird"), &base()),
            Preflight::default()
        );
    }

    fn bad() -> KnownBad {
        KnownBad {
            versions: vec![KnownBadVersion {
                version: "0.2.0".into(),
                reason: "renders a broken deny.toml".into(),
            }],
        }
    }

    #[test]
    fn a_known_bad_package_version_warns_with_the_reason() {
        let list = bad();
        let r = run(
            &contract(Some("0.2.0")),
            &Manifest::empty(),
            &PreflightOptions {
                known_bad: Some(&list),
                ..base()
            },
        );
        assert_eq!(
            r.warnings,
            ["package 0.2.0 is on the known-bad list: renders a broken deny.toml"]
        );
        assert!(r.refusals.is_empty());
    }

    #[test]
    fn strict_turns_a_known_bad_package_version_into_a_refusal() {
        let list = bad();
        let r = run(
            &contract(Some("0.2.0")),
            &Manifest::empty(),
            &PreflightOptions {
                known_bad: Some(&list),
                strict: true,
                ..base()
            },
        );
        assert_eq!(
            r.refusals,
            ["package 0.2.0 is on the known-bad list: renders a broken deny.toml (--strict)"]
        );
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn a_version_that_is_not_on_the_list_passes_even_with_strict() {
        let list = bad();
        let r = run(
            &contract(Some("0.3.0")),
            &Manifest::empty(),
            &PreflightOptions {
                known_bad: Some(&list),
                strict: true,
                ..base()
            },
        );
        assert_eq!(
            r,
            Preflight {
                package_version: Some("0.3.0".into()),
                ..Preflight::default()
            }
        );
    }

    #[test]
    fn a_local_package_is_never_on_the_known_bad_list() {
        let list = bad();
        let r = run(
            &contract(None),
            &Manifest::empty(),
            &PreflightOptions {
                known_bad: Some(&list),
                strict: true,
                ..base()
            },
        );
        assert_eq!(r, Preflight::default());
    }

    #[test]
    fn a_disabled_security_builtin_is_a_warning_for_each_one_and_an_enabled_one_is_silent() {
        let mut c = contract(Some("0.2.0"));
        let builtin = |name: &str, enabled| SecurityBuiltin {
            name: name.into(),
            enabled,
        };
        c.security_builtins = Some(vec![
            builtin("cargo-deny", false),
            builtin("secrets-scan", true),
            builtin("codeql", false),
        ]);
        let r = run(&c, &Manifest::empty(), &base());
        assert_eq!(
            r.warnings,
            [
                "required Security builtin cargo-deny is disabled",
                "required Security builtin codeql is disabled"
            ]
        );
        assert!(r.refusals.is_empty());
        assert!(
            run(
                &c,
                &Manifest::empty(),
                &PreflightOptions {
                    strict: true,
                    ..base()
                }
            )
            .refusals
            .is_empty()
        );
    }

    #[test]
    fn a_pre_release_runner_of_the_required_version_is_older_than_the_requirement() {
        let c = requiring(Some("0.2.0"), "0.1.0");
        let rc = run(
            &c,
            &Manifest::empty(),
            &PreflightOptions {
                runner_version: "0.1.0-rc.1",
                ..base()
            },
        );
        assert_eq!(
            rc.errors,
            ["runner 0.1.0-rc.1 is older than the 0.1.0 this package requires, upgrade the runner"]
        );
        let next = run(
            &c,
            &Manifest::empty(),
            &PreflightOptions {
                runner_version: "0.1.1-rc.1",
                ..base()
            },
        );
        assert!(next.errors.is_empty());
    }

    fn pinned(v: &str, sum: &str) -> String {
        format!(
            "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{v}/pantheon-scaffold@{v}::sha256:{sum}#/Config.pkl"
        )
    }

    #[test]
    fn the_version_is_read_from_a_package_uri_that_carries_a_checksum() {
        let sum = "a".repeat(64);
        assert_eq!(
            package_version_of(Some(&pinned("0.1.0", &sum))),
            Ok(Some("0.1.0".into()))
        );
        assert_eq!(
            package_version_of(Some(&pinned("0.2.0-rc.1", &sum))),
            Ok(Some("0.2.0-rc.1".into()))
        );
    }

    #[test]
    fn a_checksum_that_is_not_64_hex_characters_is_not_accepted_as_part_of_the_uri() {
        assert!(package_version_of(Some(&pinned("0.1.0", "xyz"))).is_err());
        assert!(package_version_of(Some(&pinned("0.1.0", &"a".repeat(63)))).is_err());
    }

    #[test]
    fn a_render_with_a_checksum_pinned_package_uri_gets_its_version() {
        let c = Contract {
            package_uri: Some(pinned("0.1.0", &"a".repeat(64))),
            ..contract(None)
        };
        let r = run(&c, &Manifest::empty(), &base());
        assert_eq!(
            (r.errors, r.package_version),
            (vec![], Some("0.1.0".into()))
        );
    }
}
