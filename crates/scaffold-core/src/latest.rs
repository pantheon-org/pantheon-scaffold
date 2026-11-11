//! The newest release the consumer upgrade workflow should propose. Ported from `runner/src/latest.ts`.

use std::time::Duration;

use std::cmp::Ordering;

use crate::preflight::KnownBad;
use crate::semver::{compare_semver, parse_semver};

pub const RELEASES_API: &str =
    "https://api.github.com/repos/pantheon-org/pantheon-scaffold/releases?per_page=100";

#[derive(Debug, PartialEq, Eq)]
pub struct LatestError(pub String);

impl std::fmt::Display for LatestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for LatestError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag_name: String,
    pub draft: bool,
    pub prerelease: bool,
}

/// The newest release as a version without the leading `v`: never a draft, a pre-release only when asked for, never a
/// version on the known-bad list, and never a tag that is not a version.
pub fn latest_version(
    releases: &[Release],
    include_prereleases: bool,
    known_bad: Option<&KnownBad>,
) -> Option<String> {
    let is_bad = |version: &str| {
        known_bad.is_some_and(|bad| bad.versions.iter().any(|b| b.version == version))
    };
    releases
        .iter()
        .filter(|release| !release.draft && (include_prereleases || !release.prerelease))
        .map(|release| {
            release
                .tag_name
                .strip_prefix('v')
                .unwrap_or(&release.tag_name)
        })
        .filter(|version| parse_semver(version).is_some() && !is_bad(version))
        // The first of two equal versions stays, like the TypeScript `reduce`.
        .reduce(|best, next| {
            if compare_semver(next, best) == Ok(Ordering::Greater) {
                next
            } else {
                best
            }
        })
        .map(str::to_owned)
}

/// Fetches a URL with request headers and returns its text; the error is the message to show.
pub type HeaderFetcher<'a> = dyn Fn(&str, &[(&str, &str)]) -> Result<String, String> + 'a;

pub struct FetchOptions<'a> {
    pub fetcher: &'a HeaderFetcher<'a>,
    pub url: &'a str,
    pub token: Option<&'a str>,
    pub attempts: u32,
    pub delay: Duration,
}

/// The first 100 releases of the package repository. A fetch that fails is retried and then fails closed.
pub fn fetch_releases(opts: &FetchOptions) -> Result<Vec<Release>, LatestError> {
    let mut headers = vec![("Accept", "application/vnd.github+json")];
    let bearer = opts
        .token
        .filter(|t| !t.is_empty())
        .map(|t| format!("Bearer {t}"));
    if let Some(value) = &bearer {
        headers.push(("Authorization", value));
    }
    let attempts = opts.attempts.max(1);
    let mut fetched = None;
    for attempt in 1..=attempts {
        match (opts.fetcher)(opts.url, &headers) {
            Ok(text) => {
                fetched = Some(text);
                break;
            }
            Err(message) if attempt == attempts => {
                return Err(LatestError(format!(
                    "could not fetch {} after {attempts} attempts, failing closed: {message}",
                    opts.url
                )));
            }
            Err(_) => std::thread::sleep(opts.delay),
        }
    }
    let text = fetched.expect("the last attempt either returns an error or has the text");
    let not_a_list = || LatestError(format!("{} did not return a list of releases", opts.url));
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|_| LatestError(format!("{} did not return JSON", opts.url)))?;
    let items = json.as_array().ok_or_else(not_a_list)?;
    items
        .iter()
        .map(|item| {
            let tag_name = item
                .get("tag_name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(not_a_list)?;
            let flag = |name: &str| item.get(name) == Some(&serde_json::Value::Bool(true));
            Ok(Release {
                tag_name: tag_name.to_owned(),
                draft: flag("draft"),
                prerelease: flag("prerelease"),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preflight::KnownBadVersion;
    use std::cell::{Cell, RefCell};

    fn r(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            draft: false,
            prerelease: false,
        }
    }

    fn pre(tag: &str) -> Release {
        Release {
            prerelease: true,
            ..r(tag)
        }
    }

    fn draft(tag: &str) -> Release {
        Release {
            draft: true,
            ..r(tag)
        }
    }

    fn latest(releases: &[Release]) -> Option<String> {
        latest_version(releases, false, None)
    }

    #[test]
    fn the_releases_come_from_the_package_repository() {
        assert_eq!(
            RELEASES_API,
            "https://api.github.com/repos/pantheon-org/pantheon-scaffold/releases?per_page=100"
        );
    }

    #[test]
    fn the_newest_stable_release_wins_compared_as_versions_and_not_as_text() {
        assert_eq!(
            latest(&[r("v0.9.0"), r("v0.10.0"), r("v0.2.0")]).as_deref(),
            Some("0.10.0")
        );
        assert_eq!(
            latest(&[r("v0.10.0"), r("v0.9.0")]).as_deref(),
            Some("0.10.0")
        );
    }

    #[test]
    fn a_tag_without_a_leading_v_counts_and_only_one_v_is_removed() {
        assert_eq!(latest(&[r("0.4.0"), r("v0.3.0")]).as_deref(), Some("0.4.0"));
        // Only one `v` is removed, but the version parser also accepts a leading `v`, so `vv0.5.0` is still a version and
        // comes back as `v0.5.0`. The TypeScript runner does the same.
        assert_eq!(
            latest(&[r("vv0.5.0"), r("v0.3.0")]).as_deref(),
            Some("v0.5.0")
        );
        assert_eq!(
            latest(&[r("vvv0.5.0"), r("v0.3.0")]).as_deref(),
            Some("0.3.0")
        );
    }

    #[test]
    fn drafts_are_never_offered() {
        assert_eq!(
            latest(&[r("v0.1.0"), draft("v0.2.0")]).as_deref(),
            Some("0.1.0")
        );
        assert_eq!(
            latest_version(&[r("v0.1.0"), draft("v0.2.0")], true, None).as_deref(),
            Some("0.1.0")
        );
    }

    #[test]
    fn pre_releases_are_skipped_unless_asked_for() {
        let list = [r("v0.1.0"), pre("v0.2.0")];
        assert_eq!(latest_version(&list, false, None).as_deref(), Some("0.1.0"));
        assert_eq!(latest_version(&list, true, None).as_deref(), Some("0.2.0"));
    }

    #[test]
    fn with_pre_releases_a_pre_release_ranks_below_its_own_release() {
        assert_eq!(
            latest_version(&[pre("v0.2.0-rc.1"), r("v0.2.0")], true, None).as_deref(),
            Some("0.2.0")
        );
        assert_eq!(
            latest_version(&[pre("v0.2.0-rc.2"), pre("v0.2.0-rc.10")], true, None).as_deref(),
            Some("0.2.0-rc.10")
        );
    }

    #[test]
    fn a_tag_that_is_not_a_version_is_ignored_never_guessed_at() {
        assert_eq!(
            latest(&[r("latest"), r("v1"), r("nightly-2026"), r("v0.3.0")]).as_deref(),
            Some("0.3.0")
        );
    }

    #[test]
    fn a_version_on_the_known_bad_list_is_skipped() {
        let bad = KnownBad {
            versions: vec![KnownBadVersion {
                version: "0.2.0".into(),
                reason: "broken".into(),
            }],
        };
        assert_eq!(
            latest_version(&[r("v0.1.0"), r("v0.2.0")], false, Some(&bad)).as_deref(),
            Some("0.1.0")
        );
        assert_eq!(
            latest_version(&[r("v0.1.0"), r("v0.2.0")], false, None).as_deref(),
            Some("0.2.0")
        );
    }

    #[test]
    fn no_release_that_qualifies_gives_none() {
        assert_eq!(latest(&[]), None);
        assert_eq!(latest(&[pre("v0.1.0")]), None);
        assert_eq!(latest(&[draft("v0.1.0")]), None);
        assert_eq!(latest(&[r("nightly")]), None);
    }

    fn options<'a>(
        fetcher: &'a HeaderFetcher<'a>,
        token: Option<&'a str>,
        attempts: u32,
    ) -> FetchOptions<'a> {
        FetchOptions {
            fetcher,
            url: "http://x/releases",
            token,
            attempts,
            delay: Duration::from_millis(1),
        }
    }

    #[test]
    fn fetch_releases_reads_the_list_and_sends_the_token_when_there_is_one() {
        let seen: RefCell<Vec<(String, String)>> = RefCell::new(Vec::new());
        let fetcher = |_: &str, headers: &[(&str, &str)]| {
            *seen.borrow_mut() = headers
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect();
            Ok(r#"[{"tag_name":"v0.1.0","draft":false,"prerelease":true},{"tag_name":"v0.2.0"},{"tag_name":"v0.3.0","draft":"yes","prerelease":1}]"#.to_owned())
        };
        let releases = fetch_releases(&options(&fetcher, Some("t0ken"), 3)).unwrap();
        assert_eq!(releases, [pre("v0.1.0"), r("v0.2.0"), r("v0.3.0")]);
        assert_eq!(
            *seen.borrow(),
            [
                (
                    "Accept".to_owned(),
                    "application/vnd.github+json".to_owned()
                ),
                ("Authorization".to_owned(), "Bearer t0ken".to_owned())
            ]
        );
        fetch_releases(&options(&fetcher, None, 3)).unwrap();
        assert_eq!(
            *seen.borrow(),
            [(
                "Accept".to_owned(),
                "application/vnd.github+json".to_owned()
            )]
        );
        fetch_releases(&options(&fetcher, Some(""), 3)).unwrap();
        assert_eq!(seen.borrow().len(), 1, "an empty token sends no header");
    }

    #[test]
    fn a_response_that_is_not_a_list_of_releases_is_refused() {
        for body in [
            "{}",
            r#"{"message":"rate limited"}"#,
            r#"[{"tag_name":3}]"#,
            r#"[{"draft":false}]"#,
            r#"[null]"#,
            r#"["v1"]"#,
        ] {
            let fetcher = move |_: &str, _: &[(&str, &str)]| Ok(body.to_owned());
            let message = fetch_releases(&options(&fetcher, None, 1))
                .unwrap_err()
                .to_string();
            assert_eq!(
                message, "http://x/releases did not return a list of releases",
                "{body}"
            );
        }
        let fetcher = |_: &str, _: &[(&str, &str)]| Ok("nope".to_owned());
        assert_eq!(
            fetch_releases(&options(&fetcher, None, 1))
                .unwrap_err()
                .to_string(),
            "http://x/releases did not return JSON"
        );
    }

    #[test]
    fn an_empty_list_is_a_list() {
        let fetcher = |_: &str, _: &[(&str, &str)]| Ok("[]".to_owned());
        assert_eq!(fetch_releases(&options(&fetcher, None, 1)), Ok(vec![]));
    }

    #[test]
    fn a_fetch_that_fails_is_retried_and_fails_closed_after_every_attempt() {
        let calls = Cell::new(0);
        let flaky = |_: &str, _: &[(&str, &str)]| {
            calls.set(calls.get() + 1);
            if calls.get() < 3 {
                Err("HTTP 504".to_owned())
            } else {
                Ok("[]".to_owned())
            }
        };
        assert_eq!(fetch_releases(&options(&flaky, None, 3)), Ok(vec![]));
        assert_eq!(calls.get(), 3);
        let calls = Cell::new(0);
        let broken = |_: &str, _: &[(&str, &str)]| {
            calls.set(calls.get() + 1);
            Err("HTTP 504".to_owned())
        };
        let message = fetch_releases(&options(&broken, None, 2))
            .unwrap_err()
            .to_string();
        assert_eq!(
            message,
            "could not fetch http://x/releases after 2 attempts, failing closed: HTTP 504"
        );
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn a_body_that_is_fetched_but_invalid_is_not_retried() {
        let calls = Cell::new(0);
        let fetcher = |_: &str, _: &[(&str, &str)]| {
            calls.set(calls.get() + 1);
            Ok("garbage".to_owned())
        };
        assert!(fetch_releases(&options(&fetcher, None, 3)).is_err());
        assert_eq!(calls.get(), 1);
    }
}
