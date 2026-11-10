//! Semantic versions, ported from `runner/src/semver.ts`. Build metadata is not accepted.

use std::cmp::Ordering;

#[derive(Debug, PartialEq, Eq)]
pub struct Semver {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Vec<String>,
}

/// Reads `1.2.3`, `v1.2.3` and `1.2.3-rc.1`. Anything else is `None`.
pub fn parse_semver(text: &str) -> Option<Semver> {
    static PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$").expect("valid")
    });
    let m = PATTERN.captures(text)?;
    let number = |i: usize| m[i].parse::<u64>().ok();
    Some(Semver {
        major: number(1)?,
        minor: number(2)?,
        patch: number(3)?,
        pre: m.get(4).map_or_else(Vec::new, |p| {
            p.as_str().split('.').map(str::to_owned).collect()
        }),
    })
}

fn is_numeric(part: &str) -> bool {
    !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
}

/// Semver 2.0 precedence. Text that is not a version is an error, never a guess.
pub fn compare_semver(a: &str, b: &str) -> Result<Ordering, String> {
    let not_a_version = |text: &str| {
        format!(
            "{} is not a version",
            serde_json::to_string(text).expect("a string serialises")
        )
    };
    let x = parse_semver(a).ok_or_else(|| not_a_version(a))?;
    let y = parse_semver(b).ok_or_else(|| not_a_version(b))?;
    let core = (x.major, x.minor, x.patch).cmp(&(y.major, y.minor, y.patch));
    if core != Ordering::Equal {
        return Ok(core);
    }
    // A version with a pre-release is lower than the same version without one.
    if x.pre.is_empty() || y.pre.is_empty() {
        return Ok(y.pre.is_empty().cmp(&x.pre.is_empty()).reverse());
    }
    for (p, q) in x.pre.iter().zip(&y.pre) {
        if p == q {
            continue;
        }
        return Ok(match (is_numeric(p), is_numeric(q)) {
            // Numbers compare as numbers, and sort below text. A number too long for u64 compares by length then text.
            (true, true) => (p.len(), p).cmp(&(q.len(), q)),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => p.cmp(q),
        });
    }
    Ok(x.pre.len().cmp(&y.pre.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pre(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn cmp(a: &str, b: &str) -> Ordering {
        compare_semver(a, b).unwrap()
    }

    #[test]
    fn parses_a_plain_version_a_leading_v_and_a_pre_release() {
        assert_eq!(
            parse_semver("1.2.3"),
            Some(Semver {
                major: 1,
                minor: 2,
                patch: 3,
                pre: vec![]
            })
        );
        assert_eq!(
            parse_semver("v0.10.0"),
            Some(Semver {
                major: 0,
                minor: 10,
                patch: 0,
                pre: vec![]
            })
        );
        assert_eq!(
            parse_semver("0.1.0-rc.1"),
            Some(Semver {
                major: 0,
                minor: 1,
                patch: 0,
                pre: pre(&["rc", "1"])
            })
        );
    }

    #[test]
    fn rejects_text_that_is_not_a_version() {
        for bad in [
            "",
            "1.2",
            "1.2.3.4",
            "a.b.c",
            "01.2.3",
            "1.2.3-",
            "latest",
            "1.2.3+build",
            " 1.2.3",
            "1.2.3\n",
            "99999999999999999999.0.0",
        ] {
            assert_eq!(parse_semver(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn compares_numerically_not_as_text() {
        assert_eq!(cmp("0.10.0", "0.9.0"), Ordering::Greater);
        assert_eq!(cmp("1.0.0", "1.0.0"), Ordering::Equal);
        assert_eq!(cmp("0.0.1", "0.1.0"), Ordering::Less);
        assert_eq!(cmp("1.0.0", "0.9.9"), Ordering::Greater);
        assert_eq!(cmp("1.2.4", "1.2.3"), Ordering::Greater);
        assert_eq!(cmp("1.3.0", "1.2.9"), Ordering::Greater);
    }

    #[test]
    fn a_pre_release_is_lower_than_its_release_and_pre_releases_follow_the_semver_order() {
        assert_eq!(cmp("0.1.0-rc.1", "0.1.0"), Ordering::Less);
        assert_eq!(cmp("0.1.0", "0.1.0-rc.1"), Ordering::Greater);
        assert_eq!(cmp("0.1.0-rc.2", "0.1.0-rc.10"), Ordering::Less);
        assert_eq!(cmp("0.1.0-rc.10", "0.1.0-rc.2"), Ordering::Greater);
        assert_eq!(cmp("0.1.0-alpha", "0.1.0-alpha.1"), Ordering::Less);
        assert_eq!(cmp("0.1.0-alpha.1", "0.1.0-alpha"), Ordering::Greater);
        assert_eq!(cmp("0.1.0-alpha.1", "0.1.0-beta"), Ordering::Less);
        assert_eq!(cmp("0.1.0-beta", "0.1.0-alpha.1"), Ordering::Greater);
        assert_eq!(cmp("0.1.0-1", "0.1.0-a"), Ordering::Less);
        assert_eq!(cmp("0.1.0-a", "0.1.0-1"), Ordering::Greater);
        assert_eq!(cmp("0.1.0-rc.1", "0.1.0-rc.1"), Ordering::Equal);
    }

    #[test]
    fn comparing_an_unparsable_version_is_an_error_naming_it() {
        assert_eq!(
            compare_semver("nope", "1.0.0"),
            Err("\"nope\" is not a version".to_owned())
        );
        assert_eq!(
            compare_semver("1.0.0", "x\"y"),
            Err("\"x\\\"y\" is not a version".to_owned())
        );
    }
}
