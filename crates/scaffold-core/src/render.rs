//! What lands on disk for a contract file, and a short view of how two texts differ. Ported from `runner/src/apply.ts`.

use crate::contract::{ContractError, ContractFile, Pin};

/// The text that lands on disk and is hashed: version comments are added to pinned `uses` lines of workflows and
/// composite actions first.
pub fn final_content(f: &ContractFile, pins: &[Pin]) -> Result<String, ContractError> {
    static WORKFLOW: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^\.github/(workflows/[^/]+\.ya?ml|actions/[^/]+/action\.ya?ml)$")
            .expect("valid")
    });
    let content = f.content.as_deref().unwrap_or("");
    if !WORKFLOW.is_match(&f.path) {
        return Ok(content.to_owned());
    }
    crate::versions::append_versions(content, pins)
        .map_err(|message| ContractError(format!("{}: {message}", f.path)))
}

/// The first differing lines, enough to see what a refusal is about.
pub fn short_diff(on_disk: &str, rendered: &str, max: usize) -> String {
    let a: Vec<&str> = on_disk.split('\n').collect();
    let b: Vec<&str> = rendered.split('\n').collect();
    let mut out = Vec::new();
    for i in 0..a.len().max(b.len()) {
        if out.len() >= max * 2 {
            break;
        }
        if a.get(i) != b.get(i) {
            if let Some(line) = a.get(i) {
                out.push(format!("    line {} on disk:  {line}", i + 1));
            }
            if let Some(line) = b.get(i) {
                out.push(format!("    line {} rendered: {line}", i + 1));
            }
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::Mode;
    use crate::testutil::{external, owned};

    const SHA: &str = "3d3c42e5aac5ba805825da76410c181273ba90b1";

    fn pins() -> Vec<Pin> {
        vec![Pin {
            action: "actions/checkout".into(),
            sha: SHA.into(),
            version: "v7.0.1".into(),
        }]
    }

    fn uses() -> String {
        format!("steps:\n  - uses: actions/checkout@{SHA}\n")
    }

    #[test]
    fn a_workflow_gets_version_comments() {
        for path in [
            ".github/workflows/ci.yml",
            ".github/workflows/ci.yaml",
            ".github/actions/setup/action.yml",
            ".github/actions/setup/action.yaml",
        ] {
            let out = final_content(&owned(path, &uses(), Mode::Regular), &pins()).unwrap();
            assert!(out.contains("# v7.0.1"), "{path}");
        }
    }

    #[test]
    fn other_files_are_left_as_they_are() {
        for path in [
            "README.md",
            ".github/workflows/sub/ci.yml",
            ".github/workflows/ci.txt",
            "x/.github/workflows/ci.yml",
            ".github/actions/a/b/action.yml",
        ] {
            assert_eq!(
                final_content(&owned(path, &uses(), Mode::Regular), &pins()).unwrap(),
                uses(),
                "{path}"
            );
        }
    }

    #[test]
    fn a_problem_with_a_pin_names_the_file() {
        let err = final_content(
            &owned(".github/workflows/ci.yml", &uses(), Mode::Regular),
            &[],
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                ".github/workflows/ci.yml: line 2: no version known for actions/checkout@{SHA}"
            )
        );
    }

    #[test]
    fn a_file_without_content_gives_empty_text() {
        assert_eq!(final_content(&external("mise.lock"), &[]).unwrap(), "");
    }

    #[test]
    fn short_diff_shows_the_first_differing_lines() {
        assert_eq!(
            short_diff("a\nb\nc\n", "a\nB\nc\n", 6),
            "    line 2 on disk:  b\n    line 2 rendered: B"
        );
    }

    #[test]
    fn short_diff_shows_a_line_only_one_side_has() {
        // "a\n" is the lines "a" and "", "a\nb\n" is "a", "b" and "": line 2 differs on both sides, line 3 only on one.
        assert_eq!(
            short_diff("a\n", "a\nb\n", 6),
            "    line 2 on disk:  \n    line 2 rendered: b\n    line 3 rendered: "
        );
        assert_eq!(short_diff("a\nb", "a", 6), "    line 2 on disk:  b");
    }

    #[test]
    fn short_diff_stops_after_the_limit() {
        let a = (1..=20)
            .map(|i| format!("a{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let b = (1..=20)
            .map(|i| format!("b{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(short_diff(&a, &b, 6).lines().count(), 12);
        assert_eq!(short_diff(&a, &b, 2).lines().count(), 4);
    }

    #[test]
    fn equal_texts_have_no_diff() {
        assert_eq!(short_diff("x\ny", "x\ny", 6), "");
    }
}
