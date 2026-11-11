//! The known-bad list: package versions that must not be offered or, with `--strict`, rendered. Fetched and cached, with a
//! snapshot built into the runner as the last fallback. A fetch that fails or returns something invalid never stops a
//! run: it falls back and says so. Ported from `runner/src/knownbad.ts`.

use std::path::Path;

use crate::preflight::{KnownBad, KnownBadVersion};
use crate::semver::parse_semver;

pub const KNOWN_BAD_URL: &str =
    "https://raw.githubusercontent.com/pantheon-org/pantheon-scaffold/main/known-bad.json";
pub const CACHE_FILE: &str = ".pkl-gen/cache/known-bad.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Fetched,
    Cache,
    Bundled,
}

#[derive(Debug, PartialEq, Eq)]
pub struct KnownBadResult {
    pub list: KnownBad,
    pub source: Source,
    pub note: Option<String>,
}

/// The list when the text is a valid known-bad document, otherwise `None`.
pub fn parse_known_bad(text: &str) -> Option<KnownBad> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let mut versions = Vec::new();
    for entry in json.get("versions")?.as_array()? {
        let version = entry.get("version")?.as_str()?;
        let reason = entry.get("reason")?.as_str()?;
        parse_semver(version)?;
        versions.push(KnownBadVersion {
            version: version.to_owned(),
            reason: reason.to_owned(),
        });
    }
    Some(KnownBad { versions })
}

/// The `known-bad.json` of this repository as it was when the runner was built.
pub fn bundled_snapshot() -> KnownBad {
    parse_known_bad(include_str!("../../../known-bad.json"))
        .expect("known-bad.json in this repository is a valid list")
}

pub struct KnownBadOptions<'a> {
    pub url: &'a str,
    /// Fetches a URL and returns its text; the error is the message to show.
    pub fetcher: &'a dyn Fn(&str) -> Result<String, String>,
    pub bundled: &'a KnownBad,
}

pub fn load_known_bad(root: &Path, opts: &KnownBadOptions) -> KnownBadResult {
    let cache_path = root.join(CACHE_FILE);
    // Anything that goes wrong while fetching, checking or caching counts as a failed fetch, and the run goes on.
    let fetched = (opts.fetcher)(opts.url)
        .map_err(|e| format!("the fetch failed ({e})"))
        .and_then(|text| {
            let list =
                parse_known_bad(&text).ok_or_else(|| "the fetched list is not valid".to_owned())?;
            write_cache(&cache_path, &list).map_err(|e| format!("the fetch failed ({e})"))?;
            Ok(list)
        });
    let why = match fetched {
        Ok(list) => {
            return KnownBadResult {
                list,
                source: Source::Fetched,
                note: None,
            };
        }
        Err(why) => why,
    };
    let cached = std::fs::read_to_string(&cache_path)
        .ok()
        .and_then(|text| parse_known_bad(&text));
    if let Some(list) = cached {
        return KnownBadResult {
            list,
            source: Source::Cache,
            note: Some(format!("known-bad list: {why}, using the cached list")),
        };
    }
    KnownBadResult {
        list: opts.bundled.clone(),
        source: Source::Bundled,
        note: Some(format!("known-bad list: {why}, using the bundled snapshot")),
    }
}

fn write_cache(path: &Path, list: &KnownBad) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(list).expect("a list serialises") + "\n";
    std::fs::write(path, text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{get, has, put};

    fn version(v: &str, reason: &str) -> KnownBadVersion {
        KnownBadVersion {
            version: v.into(),
            reason: reason.into(),
        }
    }

    fn list() -> KnownBad {
        KnownBad {
            versions: vec![version("0.2.0", "broken deny.toml")],
        }
    }

    fn bundled() -> KnownBad {
        KnownBad {
            versions: vec![version("0.1.0", "bundled snapshot entry")],
        }
    }

    const LIST_JSON: &str = r#"{"versions":[{"version":"0.2.0","reason":"broken deny.toml"}]}"#;

    fn ok(_: &str) -> Result<String, String> {
        Ok(LIST_JSON.to_owned())
    }

    fn down(_: &str) -> Result<String, String> {
        Err("offline".to_owned())
    }

    fn load(root: &Path, fetcher: &dyn Fn(&str) -> Result<String, String>) -> KnownBadResult {
        let bundled = bundled();
        load_known_bad(
            root,
            &KnownBadOptions {
                url: "u",
                fetcher,
                bundled: &bundled,
            },
        )
    }

    #[test]
    fn a_valid_list_parses_anything_else_does_not() {
        assert_eq!(parse_known_bad(LIST_JSON), Some(list()));
        for bad in [
            "",
            "nope",
            "[]",
            "{}",
            r#"{"versions":[{"version":"x","reason":"r"}]}"#,
            r#"{"versions":[{"version":"1.0.0"}]}"#,
            r#"{"versions":[{"reason":"r"}]}"#,
            r#"{"versions":[null]}"#,
            r#"{"versions":[{"version":1,"reason":"r"}]}"#,
            r#"{"versions":[{"version":"1.0.0","reason":2}]}"#,
            r#"{"versions":"no"}"#,
        ] {
            assert_eq!(parse_known_bad(bad), None, "{bad}");
        }
        assert_eq!(
            parse_known_bad(r#"{"versions":[]}"#),
            Some(KnownBad { versions: vec![] })
        );
    }

    #[test]
    fn a_fetched_list_is_used_and_written_to_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let r = load(dir.path(), &ok);
        assert_eq!(
            (r.source, r.list.clone(), r.note),
            (Source::Fetched, list(), None)
        );
        let cached = get(dir.path(), CACHE_FILE);
        assert_eq!(
            cached,
            "{\n  \"versions\": [\n    {\n      \"version\": \"0.2.0\",\n      \"reason\": \"broken deny.toml\"\n    }\n  ]\n}\n"
        );
        assert_eq!(parse_known_bad(&cached), Some(list()));
    }

    #[test]
    fn when_the_fetch_fails_the_cached_list_is_used_with_a_note() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), CACHE_FILE, LIST_JSON);
        let r = load(dir.path(), &down);
        assert_eq!((r.source, r.list), (Source::Cache, list()));
        assert_eq!(
            r.note.as_deref(),
            Some("known-bad list: the fetch failed (offline), using the cached list")
        );
    }

    #[test]
    fn with_no_fetch_and_no_cache_the_bundled_snapshot_is_used_with_a_note() {
        let dir = tempfile::tempdir().unwrap();
        let r = load(dir.path(), &down);
        assert_eq!((r.source, r.list), (Source::Bundled, bundled()));
        assert_eq!(
            r.note.as_deref(),
            Some("known-bad list: the fetch failed (offline), using the bundled snapshot")
        );
    }

    #[test]
    fn a_fetched_list_that_does_not_validate_is_ignored_and_never_cached() {
        let dir = tempfile::tempdir().unwrap();
        let r = load(dir.path(), &|_: &str| {
            Ok(r#"{"versions":"oops"}"#.to_owned())
        });
        assert_eq!(r.source, Source::Bundled);
        assert_eq!(
            r.note.as_deref(),
            Some("known-bad list: the fetched list is not valid, using the bundled snapshot")
        );
        assert!(!has(dir.path(), CACHE_FILE));
    }

    #[test]
    fn a_corrupt_cache_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), CACHE_FILE, "{not json");
        assert_eq!(load(dir.path(), &down).source, Source::Bundled);
    }

    #[test]
    fn a_fetch_that_succeeds_after_an_earlier_cache_replaces_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        put(
            dir.path(),
            CACHE_FILE,
            r#"{"versions":[{"version":"0.1.0","reason":"old"}]}"#,
        );
        load(dir.path(), &ok);
        assert_eq!(parse_known_bad(&get(dir.path(), CACHE_FILE)), Some(list()));
    }

    #[test]
    fn a_cache_that_cannot_be_written_counts_as_a_failed_fetch() {
        let dir = tempfile::tempdir().unwrap();
        // `.pkl-gen` is a file, so the cache folder cannot be made.
        put(dir.path(), ".pkl-gen", "in the way");
        let r = load(dir.path(), &ok);
        assert_eq!(r.source, Source::Bundled);
        assert!(
            r.note
                .as_deref()
                .is_some_and(|n| n.starts_with("known-bad list: the fetch failed (")
                    && n.ends_with("), using the bundled snapshot")),
            "{:?}",
            r.note
        );
    }

    #[test]
    fn the_default_url_is_the_list_on_the_package_repos_main_branch() {
        assert_eq!(
            KNOWN_BAD_URL,
            "https://raw.githubusercontent.com/pantheon-org/pantheon-scaffold/main/known-bad.json"
        );
        assert_eq!(CACHE_FILE, ".pkl-gen/cache/known-bad.json");
    }

    #[test]
    fn the_built_in_snapshot_is_a_valid_list_and_names_the_version_that_cannot_read_a_checksum() {
        let snapshot = bundled_snapshot();
        let entry = snapshot
            .versions
            .iter()
            .find(|v| v.version == "0.1.0")
            .expect("0.1.0 is listed");
        assert!(entry.reason.contains("checksum"), "{}", entry.reason);
        let root = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../known-bad.json"),
        )
        .unwrap();
        let published = parse_known_bad(&root).expect("known-bad.json is valid");
        for v in &snapshot.versions {
            assert!(
                published.versions.contains(v),
                "{v:?} is not in known-bad.json"
            );
        }
    }
}
