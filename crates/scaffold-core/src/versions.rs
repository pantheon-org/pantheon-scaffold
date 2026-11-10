//! Appends `# <version>` to pinned `uses: owner/repo[/path]@<40 hex>` lines in rendered workflow text. Ported from
//! `runner/src/versions.ts`.

use crate::contract::Pin;

/// One line of the text, and what it is when it is a pinned `uses:` line.
pub struct Line {
    pub index: usize,
    pub text: String,
    pub uses: Option<Uses>,
}

pub struct Uses {
    pub lead: String,
    pub quote: String,
    pub action: String,
    pub sha: String,
    pub tail: String,
}

fn uses_pattern() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        // The quote before the action and the one after the commit must be the same; the regex crate has no
        // back-reference, so both are captured and compared afterwards.
        regex::Regex::new(
            r##"^(?P<lead>\s*(?:-\s+)?uses:\s+)(?P<q1>['"]?)(?P<action>[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+(?:/[^@\s'"]+)?)@(?P<sha>[0-9a-f]{40})(?P<q2>['"]?)(?P<tail>\s*(?:\#.*)?)$"##,
        )
        .expect("valid")
    });
    &RE
}

fn unpinned_pattern() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r##"^\s*(?:-\s+)?uses:\s+['"]?[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+(?:/[^@\s'"]+)?@[^\s'"\#]+"##).expect("valid")
    });
    &RE
}

fn block_start() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^(?P<indent>\s*)(?:-\s+)?[^#\n]*:\s*[|>][-+0-9]*\s*(?:#.*)?$")
            .expect("valid")
    });
    &RE
}

fn dash_prefix() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^(\s*)-\s+").expect("valid"));
    &RE
}

fn parse_uses(raw: &str) -> Option<Uses> {
    let m = uses_pattern().captures(raw)?;
    if m["q1"] != m["q2"] {
        return None;
    }
    Some(Uses {
        lead: m["lead"].to_owned(),
        quote: m["q1"].to_owned(),
        action: m["action"].to_owned(),
        sha: m["sha"].to_owned(),
        tail: m["tail"].to_owned(),
    })
}

fn indent_of(line: &str) -> usize {
    line.chars().count() - line.trim_start().chars().count()
}

/// Walks the lines, skipping the content of block scalars (`run: |`), which can hold text that looks like a uses line.
pub fn scan(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    let mut block_key_column: Option<usize> = None;
    for (index, raw) in text.split('\n').enumerate() {
        if let Some(column) = block_key_column {
            if raw.trim().is_empty() || indent_of(raw) > column {
                out.push(Line {
                    index,
                    text: raw.to_owned(),
                    uses: None,
                });
                continue;
            }
            block_key_column = None;
        }
        if let Some(block) = block_start().captures(raw) {
            block_key_column = Some(match dash_prefix().find(raw) {
                Some(dash) => dash.as_str().chars().count(),
                None => block["indent"].chars().count(),
            });
        }
        out.push(Line {
            index,
            text: raw.to_owned(),
            uses: parse_uses(raw),
        });
    }
    out
}

/// The text with a version comment on every pinned `uses:` line. An existing matching comment is kept, a different one
/// is an error, and so is a pin with no known version. The error message starts with the line number.
pub fn append_versions(text: &str, pins: &[Pin]) -> Result<String, String> {
    let mut result = Vec::new();
    for line in scan(text) {
        let Some(u) = line.uses else {
            result.push(line.text);
            continue;
        };
        let n = line.index + 1;
        let pin = pins
            .iter()
            .find(|p| p.action == u.action && p.sha == u.sha)
            .ok_or_else(|| format!("line {n}: no version known for {}@{}", u.action, u.sha))?;
        let comment = u.tail.trim();
        if comment.is_empty() {
            result.push(format!(
                "{}{}{}@{}{} # {}",
                u.lead, u.quote, u.action, u.sha, u.quote, pin.version
            ));
        } else if comment == format!("# {}", pin.version) {
            result.push(line.text);
        } else {
            return Err(format!(
                "line {n}: comment \"{comment}\" differs from \"# {}\" for {}",
                pin.version, u.action
            ));
        }
    }
    Ok(result.join("\n"))
}

/// A line number and the trimmed text of the line.
#[derive(Debug, PartialEq, Eq)]
pub struct Found {
    pub line: usize,
    pub text: String,
}

/// Pinned `uses:` lines with no version comment.
pub fn find_missing_comments(text: &str) -> Vec<Found> {
    scan(text)
        .into_iter()
        .filter(|l| l.uses.as_ref().is_some_and(|u| u.tail.trim().is_empty()))
        .map(|l| Found {
            line: l.index + 1,
            text: l.text.trim().to_owned(),
        })
        .collect()
}

/// `uses:` lines that name an action by a tag or branch instead of a commit.
pub fn find_unpinned(text: &str) -> Vec<Found> {
    static COMMIT: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"@[0-9a-f]{40}").expect("valid"));
    scan(text)
        .into_iter()
        .filter(|l| {
            l.uses.is_none() && unpinned_pattern().is_match(&l.text) && !COMMIT.is_match(&l.text)
        })
        .map(|l| Found {
            line: l.index + 1,
            text: l.text.trim().to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "3d3c42e5aac5ba805825da76410c181273ba90b1";

    fn pins() -> Vec<Pin> {
        vec![
            Pin {
                action: "actions/checkout".into(),
                sha: SHA.into(),
                version: "v7.0.1".into(),
            },
            Pin {
                action: "owner/repo/.github/workflows/x.yml".into(),
                sha: SHA.into(),
                version: "v1.0.0".into(),
            },
        ]
    }

    fn wf(body: &str) -> String {
        format!("jobs:\n  a:\n    steps:\n{body}\n")
    }

    fn append(text: &str) -> String {
        append_versions(text, &pins()).unwrap()
    }

    #[test]
    fn appends_the_comment_to_a_pinned_line() {
        assert!(
            append(&wf(&format!("      - uses: actions/checkout@{SHA}")))
                .contains(&format!("- uses: actions/checkout@{SHA} # v7.0.1"))
        );
    }

    #[test]
    fn is_idempotent() {
        let once = append(&wf(&format!("      - uses: actions/checkout@{SHA}")));
        assert_eq!(append(&once), once);
    }

    #[test]
    fn keeps_an_existing_matching_comment_byte_for_byte() {
        let text = wf(&format!("      - uses: actions/checkout@{SHA} # v7.0.1"));
        assert_eq!(append(&text), text);
    }

    #[test]
    fn refuses_to_overwrite_a_differing_comment() {
        let err = append_versions(
            &wf(&format!("      - uses: actions/checkout@{SHA} # v6")),
            &pins(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            "line 4: comment \"# v6\" differs from \"# v7.0.1\" for actions/checkout"
        );
    }

    #[test]
    fn fails_on_a_pin_with_no_known_version() {
        let err =
            append_versions(&wf(&format!("      - uses: other/thing@{SHA}")), &pins()).unwrap_err();
        assert_eq!(
            err,
            format!("line 4: no version known for other/thing@{SHA}")
        );
    }

    #[test]
    fn a_known_action_at_another_commit_has_no_known_version() {
        let other = "a".repeat(40);
        let err = append_versions(
            &wf(&format!("      - uses: actions/checkout@{other}")),
            &pins(),
        )
        .unwrap_err();
        assert!(err.contains("no version known"), "{err}");
    }

    #[test]
    fn handles_quoted_values_and_keeps_the_quotes() {
        let out = append(&wf(&format!("      - uses: \"actions/checkout@{SHA}\"")));
        assert!(out.contains(&format!("\"actions/checkout@{SHA}\" # v7.0.1")));
        let out = append(&wf(&format!("      - uses: 'actions/checkout@{SHA}'")));
        assert!(out.contains(&format!("'actions/checkout@{SHA}' # v7.0.1")));
    }

    #[test]
    fn a_quote_that_is_not_closed_the_same_way_is_not_a_uses_line() {
        let text = wf(&format!("      - uses: \"actions/checkout@{SHA}'"));
        assert_eq!(append(&text), text);
    }

    #[test]
    fn handles_a_reusable_workflow_path() {
        assert!(
            append(&wf(&format!(
                "    uses: owner/repo/.github/workflows/x.yml@{SHA}"
            )))
            .contains("# v1.0.0")
        );
    }

    #[test]
    fn ignores_local_actions_and_docker_references() {
        let text = wf(&format!(
            "      - uses: ./.github/actions/local\n      - uses: docker://alpine@sha256:{}",
            "a".repeat(64)
        ));
        assert_eq!(append(&text), text);
    }

    #[test]
    fn ignores_a_uses_line_inside_a_run_block_scalar() {
        let text = wf(&format!(
            "      - run: |\n          echo hello\n          uses: actions/checkout@{SHA}\n      - uses: actions/checkout@{SHA}"
        ));
        let out = append(&text);
        assert!(out.contains(&format!("          uses: actions/checkout@{SHA}\n")));
        assert_eq!(out.matches("# v7.0.1").count(), 1);
    }

    #[test]
    fn a_block_scalar_ends_at_the_next_line_with_less_indent() {
        let text = wf(&format!(
            "      - run: |\n          echo\n      - uses: actions/checkout@{SHA}"
        ));
        assert!(append(&text).contains("# v7.0.1"));
        let text = "run: |\n  x\nuses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1\n";
        assert!(append(text).contains("# v7.0.1"));
    }

    #[test]
    fn ignores_uses_text_inside_a_quoted_string_and_a_comment() {
        let text = wf(&format!(
            "      - name: \"uses: actions/checkout@{SHA}\"\n      # uses: actions/checkout@{SHA}"
        ));
        assert_eq!(append(&text), text);
    }

    #[test]
    fn a_blank_line_inside_a_block_scalar_stays_inside_it() {
        let text = wf(&format!(
            "      - run: |\n          a\n\n          uses: actions/checkout@{SHA}"
        ));
        assert_eq!(append(&text), text);
    }

    #[test]
    fn the_checker_flags_a_pinned_line_with_no_comment_and_passes_one_with_a_comment() {
        assert_eq!(
            find_missing_comments(&wf(&format!("      - uses: actions/checkout@{SHA}"))),
            [Found {
                line: 4,
                text: format!("- uses: actions/checkout@{SHA}")
            }]
        );
        assert!(
            find_missing_comments(&wf(&format!(
                "      - uses: actions/checkout@{SHA} # v7.0.1"
            )))
            .is_empty()
        );
    }

    #[test]
    fn the_checker_reports_an_unpinned_tag() {
        assert_eq!(
            find_unpinned(&wf("      - uses: actions/checkout@v4")),
            [Found {
                line: 4,
                text: "- uses: actions/checkout@v4".into()
            }]
        );
        assert!(find_unpinned(&wf(&format!("      - uses: actions/checkout@{SHA}"))).is_empty());
    }

    #[test]
    fn scan_reports_every_line_with_its_index() {
        let lines = scan("a\nuses: x/y@3d3c42e5aac5ba805825da76410c181273ba90b1\n");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].index, 1);
        assert_eq!(lines[1].uses.as_ref().unwrap().action, "x/y");
        assert_eq!(lines[1].uses.as_ref().unwrap().lead, "uses: ");
    }
}
