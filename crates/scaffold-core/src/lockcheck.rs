//! Checks what `mise lock` recorded for the runner. Ported from `runner/src/lockcheck.ts`.

pub const SUPPORTED_PLATFORMS: [(&str, &str); 3] = [
    ("linux-x64", "pantheon-scaffold-linux-x64"),
    ("linux-arm64", "pantheon-scaffold-linux-arm64"),
    ("macos-arm64", "pantheon-scaffold-darwin-arm64"),
];

/// With no platform list mise locks every platform it knows, and for one the release has no binary for it picks whatever
/// asset it can, which for this release is the package zip. A lock that names a platform we do not build for, or the wrong
/// asset for one we do, is a problem. Returns the problems, empty when the lock is right or has no entry for the runner.
pub fn check_runner_lock(lock_text: &str) -> Vec<String> {
    use crate::pins::RUNNER_TOOL;
    // The entry is a header line and the lines under it up to the next line that starts a section. The TypeScript finds it
    // with a look-ahead pattern, which the `regex` crate does not have, so the lines are read one by one.
    let header = regex::Regex::new(&format!(
        r#"^\[tools\."{}"\."platforms\.([^"]+)"\][ \t]*$"#,
        regex::escape(RUNNER_TOOL)
    ))
    .expect("valid");
    let url_line = regex::Regex::new(r#"(?m)^url\s*=\s*"([^"]*)""#).expect("valid");
    let lines: Vec<&str> = lock_text.split('\n').collect();
    let mut problems = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        // A header with no line break after it is not an entry.
        let Some(platform) = header
            .captures(line)
            .filter(|_| i + 1 < lines.len())
            .map(|m| m[1].to_owned())
        else {
            continue;
        };
        let body: Vec<&str> = lines[i + 1..]
            .iter()
            .copied()
            .take_while(|l| !l.starts_with('['))
            .collect();
        let url = url_line.captures(&body.join("\n")).map(|m| m[1].to_owned());
        let asset = url.map(|u| percent_decode(u.rsplit('/').next().unwrap_or("")));
        let expected = SUPPORTED_PLATFORMS
            .iter()
            .find(|(name, _)| *name == platform)
            .map(|(_, asset)| *asset);
        match (asset, expected) {
            (None, _) => problems.push(format!("{platform}: no url recorded for the runner")),
            (Some(asset), None) => problems.push(format!(
                "{platform}: not a supported platform, mise chose {asset}"
            )),
            (Some(asset), Some(expected)) if asset != expected => problems.push(format!(
                "{platform}: mise chose {asset}, expected {expected}"
            )),
            _ => {}
        }
    }
    problems
}

/// Like JavaScript's `decodeURIComponent` for well-formed text; a stray `%` is kept as it is.
fn percent_decode(text: &str) -> String {
    let hex = |b: u8| char::from(b).to_digit(16);
    let mut out = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        let escaped = match tail {
            [high, low, ..] if byte == b'%' => hex(*high).zip(hex(*low)).map(|(h, l)| h * 16 + l),
            _ => None,
        };
        if let Some(value) = escaped {
            out.push(u8::try_from(value).expect("two hex digits fit in a byte"));
            rest = &tail[2..];
        } else {
            out.push(byte);
            rest = tail;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both locks are real `mise lock` output for the published v0.1.1 release.
    const ALL: &str = include_str!("../tests/data/mise-lock-all-platforms.lock");
    const THREE: &str = include_str!("../tests/data/mise-lock-three-platforms.lock");

    const TOOL: &str = "github:pantheon-org/pantheon-scaffold";

    #[test]
    fn the_supported_platforms_are_the_three_the_runner_is_built_for() {
        assert_eq!(
            SUPPORTED_PLATFORMS,
            [
                ("linux-x64", "pantheon-scaffold-linux-x64"),
                ("linux-arm64", "pantheon-scaffold-linux-arm64"),
                ("macos-arm64", "pantheon-scaffold-darwin-arm64")
            ]
        );
    }

    #[test]
    fn a_lock_limited_to_the_three_supported_platforms_has_no_problems() {
        assert_eq!(check_runner_lock(THREE), Vec::<String>::new());
    }

    #[test]
    fn a_lock_for_every_platform_names_the_package_zip_for_platforms_with_no_runner_and_the_glibc_build_for_musl()
     {
        let problems = check_runner_lock(ALL);
        assert_eq!(problems.len(), 4, "{problems:?}");
        let platform = |p: &String| p.split(':').next().unwrap().to_owned();
        let mut zip: Vec<_> = problems
            .iter()
            .filter(|p| p.contains("pantheon-scaffold@0.1.1.zip"))
            .map(platform)
            .collect();
        zip.sort();
        assert_eq!(zip, ["macos-x64", "windows-x64"]);
        let mut musl: Vec<_> = problems
            .iter()
            .filter(|p| p.starts_with("linux-"))
            .map(platform)
            .collect();
        musl.sort();
        assert_eq!(musl, ["linux-arm64-musl", "linux-x64-musl"]);
        for p in &problems {
            assert!(p.contains("not a supported platform"), "{p}");
        }
    }

    #[test]
    fn a_problem_names_the_platform_and_what_mise_chose() {
        let problems = check_runner_lock(ALL);
        assert!(
            problems.contains(
                &"windows-x64: not a supported platform, mise chose pantheon-scaffold@0.1.1.zip"
                    .to_owned()
            ),
            "{problems:?}"
        );
    }

    #[test]
    fn a_supported_platform_whose_entry_names_the_wrong_asset_is_a_problem() {
        let bad = THREE.replace(
            "pantheon-scaffold-linux-x64",
            "pantheon-scaffold%400.1.1.zip",
        );
        let problems = check_runner_lock(&bad);
        assert_eq!(
            problems,
            [
                "linux-x64: mise chose pantheon-scaffold@0.1.1.zip, expected pantheon-scaffold-linux-x64"
            ]
        );
    }

    #[test]
    fn a_lock_with_no_entry_for_the_runner_has_nothing_to_check() {
        assert!(check_runner_lock("# lock for 0.1.0\n").is_empty());
        assert!(check_runner_lock("[tools.\"github:other/tool\".\"platforms.windows-x64\"]\nurl = \"https://example.test/tool.zip\"\n").is_empty());
        assert!(check_runner_lock("").is_empty());
    }

    #[test]
    fn entries_of_other_tools_are_never_mistaken_for_the_runners() {
        let mixed = format!(
            "{THREE}\n[tools.\"github:other/tool\".\"platforms.windows-x64\"]\nurl = \"https://example.test/tool.zip\"\n"
        );
        assert!(check_runner_lock(&mixed).is_empty());
    }

    #[test]
    fn an_entry_with_no_url_is_reported_not_skipped() {
        let no_url = format!(
            "[tools.\"{TOOL}\".\"platforms.linux-x64\"]\nchecksum = \"sha256:{}\"\n",
            "a".repeat(64)
        );
        assert_eq!(
            check_runner_lock(&no_url),
            ["linux-x64: no url recorded for the runner"]
        );
    }

    #[test]
    fn a_url_after_another_section_belongs_to_that_section_not_to_the_runners_entry() {
        let text = format!(
            "[tools.\"{TOOL}\".\"platforms.linux-x64\"]\nchecksum = \"x\"\n\n[tools.\"github:other/tool\".\"platforms.linux-x64\"]\nurl = \"https://example.test/pantheon-scaffold-linux-x64\"\n"
        );
        assert_eq!(
            check_runner_lock(&text),
            ["linux-x64: no url recorded for the runner"]
        );
    }

    #[test]
    fn the_last_entry_of_a_file_without_a_final_newline_is_read() {
        let text = format!(
            "[tools.\"{TOOL}\".\"platforms.linux-arm64\"]\nurl = \"https://h/d/pantheon-scaffold-linux-x64\""
        );
        assert_eq!(
            check_runner_lock(&text),
            [
                "linux-arm64: mise chose pantheon-scaffold-linux-x64, expected pantheon-scaffold-linux-arm64"
            ]
        );
    }

    #[test]
    fn a_header_with_trailing_blanks_and_a_url_with_spaces_around_the_equals_sign_are_read() {
        let text = format!(
            "[tools.\"{TOOL}\".\"platforms.macos-arm64\"]  \t\nurl   =   \"https://h/d/pantheon-scaffold-darwin-arm64\"\n"
        );
        assert!(check_runner_lock(&text).is_empty());
    }

    #[test]
    fn a_header_at_the_very_end_with_no_line_break_after_it_is_not_an_entry() {
        let header = format!("[tools.\"{TOOL}\".\"platforms.linux-x64\"]");
        assert!(check_runner_lock(&header).is_empty());
        assert_eq!(
            check_runner_lock(&format!("{header}\n")),
            ["linux-x64: no url recorded for the runner"]
        );
    }

    #[test]
    fn a_stray_percent_sign_and_half_escapes_are_kept_as_they_are() {
        let name = |asset: &str| {
            format!("[tools.\"{TOOL}\".\"platforms.windows-x64\"]\nurl = \"https://h/{asset}\"\n")
        };
        for (asset, shown) in [
            ("100%.zip", "100%.zip"),
            ("a%2.zip", "a%2.zip"),
            ("a%zz.zip", "a%zz.zip"),
            ("%", "%"),
            ("a%4", "a%4"),
            ("%41%42", "AB"),
        ] {
            assert_eq!(
                check_runner_lock(&name(asset)),
                [format!(
                    "windows-x64: not a supported platform, mise chose {shown}"
                )],
                "{asset}"
            );
        }
    }

    #[test]
    fn the_last_path_segment_is_decoded_as_a_file_name() {
        let text = format!(
            "[tools.\"{TOOL}\".\"platforms.linux-x64\"]\nurl = \"https://h/a%20b/pantheon-scaffold%2Dlinux-x64\"\n"
        );
        assert!(
            check_runner_lock(&text).is_empty(),
            "{:?}",
            check_runner_lock(&text)
        );
        let odd = format!(
            "[tools.\"{TOOL}\".\"platforms.linux-x64\"]\nurl = \"https://h/%E2%82%AC.zip\"\n"
        );
        assert_eq!(
            check_runner_lock(&odd),
            ["linux-x64: mise chose \u{20AC}.zip, expected pantheon-scaffold-linux-x64"]
        );
    }
}
