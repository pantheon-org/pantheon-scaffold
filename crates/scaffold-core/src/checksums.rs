//! Release checksums in `sha256sum` format. Ported from `runner/src/checksums.ts`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksum {
    pub sha256: String,
    pub name: String,
}

/// Release assets are listed in sha256sum format: 64 hex characters, a space, an optional `*` binary marker, then the file
/// name. A line that is not in that format is an error, never skipped.
pub fn parse_checksums(text: &str) -> Result<Vec<Checksum>, String> {
    // A name cannot hold a line break: JavaScript's `.` stops at \r,   and   as well as \n.
    static LINE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^([0-9a-f]{64}) [ *](\S[^\n\r\x{2028}\x{2029}]*)$").expect("valid")
    });
    let mut out = Vec::new();
    for line in text.split('\n') {
        if line.trim().is_empty() {
            continue;
        }
        let Some(m) = LINE.captures(line) else {
            return Err(format!(
                "{} is not a checksum line (expected sha256sum format)",
                json(line)
            ));
        };
        out.push(Checksum {
            sha256: m[1].to_owned(),
            name: m[2].to_owned(),
        });
    }
    Ok(out)
}

fn json(text: &str) -> String {
    serde_json::to_string(text).expect("a string serialises")
}

/// The checksum of one named file. A missing or repeated name is an error: a checksum must never be guessed or picked.
pub fn find_checksum(text: &str, name: &str) -> Result<String, String> {
    let all = parse_checksums(text)?;
    let mut hits = all.iter().filter(|c| c.name == name);
    match (hits.next(), hits.next()) {
        (None, _) => Err(format!(
            "no checksum for {name} in the release's checksums file"
        )),
        (Some(_), Some(_)) => Err(format!(
            "more than one checksum for {name} in the release's checksums file"
        )),
        (Some(hit), None) => Ok(hit.sha256.clone()),
    }
}

/// Where the checksums file of a release lives, next to the release it describes.
pub fn checksum_asset_url(base: &str, version: &str) -> String {
    format!("{}/v{version}/checksums.txt", base.trim_end_matches('/'))
}

/// The inverse of `parse_checksums`, sorted by name so the file is the same however it was assembled.
pub fn format_checksums(list: &[Checksum]) -> Result<String, String> {
    for c in list {
        if c.sha256.len() != 64
            || !c
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(format!("{} is not a sha256", json(&c.sha256)));
        }
        if c.name.is_empty() || c.name.contains(['\n', '\r']) || c.name != c.name.trim() {
            return Err(format!("{} is not a valid file name", json(&c.name)));
        }
    }
    let mut sorted: Vec<&Checksum> = list.iter().collect();
    // JavaScript compares strings by UTF-16 code unit.
    sorted.sort_by(|a, b| a.name.encode_utf16().cmp(b.name.encode_utf16()));
    Ok(sorted
        .iter()
        .map(|c| format!("{}  {}\n", c.sha256, c.name))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(c: char) -> String {
        c.to_string().repeat(64)
    }

    fn sum(c: char, name: &str) -> Checksum {
        Checksum {
            sha256: h(c),
            name: name.to_owned(),
        }
    }

    #[test]
    fn parses_sha256sum_lines_with_or_without_the_binary_marker_and_ignores_blank_lines() {
        let text = format!(
            "{}  pantheon-scaffold@0.2.0\n\n{} *runner-linux-x64\n",
            h('a'),
            h('b')
        );
        assert_eq!(
            parse_checksums(&text).unwrap(),
            [
                sum('a', "pantheon-scaffold@0.2.0"),
                sum('b', "runner-linux-x64")
            ]
        );
    }

    #[test]
    fn a_name_may_contain_spaces_and_a_line_of_spaces_counts_as_blank() {
        let text = format!("{}  two words.zip\n   \n", h('a'));
        assert_eq!(parse_checksums(&text).unwrap(), [sum('a', "two words.zip")]);
    }

    #[test]
    fn a_line_that_is_not_a_sha256sum_line_is_an_error_never_skipped() {
        let message = |text: &str| parse_checksums(text).unwrap_err();
        assert_eq!(
            message("nonsense\n"),
            "\"nonsense\" is not a checksum line (expected sha256sum format)"
        );
        assert!(message(&format!("{}  x\n", "a".repeat(63))).contains("not a checksum line"));
        assert!(message(&format!("{}  x\n", "G".repeat(64))).contains("not a checksum line"));
        assert!(
            message(&format!("{}  x\n", "A".repeat(64))).contains("not a checksum line"),
            "upper case hex is not accepted"
        );
        assert!(
            message(&format!("{} x\n", h('a'))).contains("not a checksum line"),
            "one space is not the format"
        );
        assert!(
            message(&format!("{}   x\n", h('a'))).contains("not a checksum line"),
            "a name cannot start with a space"
        );
        assert!(
            message(&format!("{}  x\r\n", h('a'))).contains("not a checksum line"),
            "a carriage return is not part of a name"
        );
        assert!(
            message(&format!("{}  \n", h('a'))).contains("not a checksum line"),
            "a name is required"
        );
    }

    #[test]
    fn an_empty_file_has_no_checksums() {
        assert_eq!(parse_checksums("\n\n"), Ok(vec![]));
        assert_eq!(parse_checksums(""), Ok(vec![]));
    }

    #[test]
    fn finds_the_checksum_of_one_named_file() {
        let text = format!("{}  one\n{}  two\n", h('a'), h('b'));
        assert_eq!(find_checksum(&text, "two"), Ok(h('b')));
    }

    #[test]
    fn a_missing_name_and_a_repeated_name_are_both_errors() {
        assert_eq!(
            find_checksum(&format!("{}  one\n", h('a')), "two"),
            Err("no checksum for two in the release's checksums file".to_owned())
        );
        let twice = format!("{}  one\n{}  one\n", h('a'), h('b'));
        assert_eq!(
            find_checksum(&twice, "one"),
            Err("more than one checksum for one in the release's checksums file".to_owned())
        );
    }

    #[test]
    fn a_bad_line_anywhere_stops_the_search() {
        let text = format!("{}  one\nbroken\n", h('a'));
        assert!(
            find_checksum(&text, "one")
                .unwrap_err()
                .contains("not a checksum line")
        );
    }

    #[test]
    fn the_asset_lives_next_to_the_release_it_describes() {
        assert_eq!(
            checksum_asset_url("https://github.com/o/r/releases/download", "0.2.0"),
            "https://github.com/o/r/releases/download/v0.2.0/checksums.txt"
        );
        assert_eq!(
            checksum_asset_url("https://x/", "0.2.0-rc.1"),
            "https://x/v0.2.0-rc.1/checksums.txt"
        );
        assert_eq!(
            checksum_asset_url("https://x///", "1.0.0"),
            "https://x/v1.0.0/checksums.txt"
        );
    }

    #[test]
    fn format_checksums_writes_sha256sum_lines_that_parse_back_sorted_by_name() {
        let text = format_checksums(&[sum('b', "z-file"), sum('a', "a-file")]).unwrap();
        assert_eq!(text, format!("{}  a-file\n{}  z-file\n", h('a'), h('b')));
        assert_eq!(
            parse_checksums(&text).unwrap(),
            [sum('a', "a-file"), sum('b', "z-file")]
        );
    }

    #[test]
    fn format_checksums_sorts_the_way_javascript_does() {
        // Upper case sorts before lower case, and a character outside the Basic Multilingual Plane (two UTF-16 units,
        // starting with a surrogate) sorts before U+FF5E.
        let text = format_checksums(&[
            sum('a', "b"),
            sum('b', "A"),
            sum('c', "\u{FF5E}"),
            sum('d', "\u{1F600}"),
        ])
        .unwrap();
        let names: Vec<_> = parse_checksums(&text)
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, ["A", "b", "\u{1F600}", "\u{FF5E}"]);
    }

    #[test]
    fn format_checksums_refuses_a_checksum_or_a_name_that_could_not_be_read_back() {
        let bad_sum = Checksum {
            sha256: "abc".into(),
            name: "x".into(),
        };
        assert_eq!(
            format_checksums(&[bad_sum]),
            Err("\"abc\" is not a sha256".to_owned())
        );
        let upper = Checksum {
            sha256: "A".repeat(64),
            name: "x".into(),
        };
        assert!(format_checksums(&[upper]).is_err());
        for name in ["two\nlines", "", "a\rb", " lead", "trail "] {
            let bad = Checksum {
                sha256: h('a'),
                name: name.into(),
            };
            let message = format_checksums(&[bad]).unwrap_err();
            assert!(
                message.ends_with("is not a valid file name"),
                "{name:?}: {message}"
            );
        }
    }
}
