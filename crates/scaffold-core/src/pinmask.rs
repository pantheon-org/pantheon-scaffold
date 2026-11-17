//! Masks the commit and the version comment of every pinned action in rendered workflow text, so that a comparison of renders does not change
//! when `update-pins` moves a pin.
//!
//! A pinned reference is a `uses:` line whose action ends in `@` and exactly 40 lowercase hex characters, with an optional quote and an optional
//! `# <version>` comment. The mask is `@<pin>`. Nothing else is touched: a 40 character literal in a `run:` line, a reference with 39 or 41
//! characters, a local action (`./...`) and a `docker://` reference are left as they are, so a change to them is still seen.
//!
//! This is not [`crate::versions`], which appends the version comment to a pinned line, and it is not `unpin` in the package tests, which masks
//! any `@<40 hex>` in text it compares with a pinned upstream copy.

use regex::Regex;
use std::sync::LazyLock;

/// What replaces the commit, and the version comment that follows it.
pub const MARKER: &str = "@<pin>";

static PINNED_USES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r##"(?m)^(?P<lead>[ \t]*(?:-[ \t]+)?uses:[ \t]+['"]?[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+(?:/[^@\s'"]+)?)@[0-9a-f]{40}(?P<quote>['"]?)(?:[ \t]+\#[^\r\n]*)?(?P<end>\r?)$"##,
    )
    .expect("valid")
});

/// The text with the commit and the version comment of every pinned `uses:` line replaced by [`MARKER`].
pub fn mask_pinned_uses(text: &str) -> String {
    PINNED_USES
        .replace_all(text, format!("${{lead}}{MARKER}${{quote}}${{end}}"))
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha() -> String {
        "a".repeat(40)
    }

    #[test]
    fn a_pinned_uses_line_loses_its_commit_and_its_version_comment() {
        let line = format!("      - uses: actions/checkout@{} # v7.0.1\n", sha());
        assert_eq!(
            mask_pinned_uses(&line),
            "      - uses: actions/checkout@<pin>\n"
        );
    }

    #[test]
    fn a_pinned_line_with_no_comment_or_a_comment_that_is_not_a_version_is_masked() {
        let bare = format!("  uses: actions/cache@{}\n", sha());
        assert_eq!(mask_pinned_uses(&bare), "  uses: actions/cache@<pin>\n");
        let odd = format!("- uses: a/b@{} # release 2026-11\n", sha());
        assert_eq!(mask_pinned_uses(&odd), "- uses: a/b@<pin>\n");
    }

    #[test]
    fn a_tag_that_does_not_start_with_v_is_masked_with_its_comment() {
        let line = format!("uses: owner/repo@{} # 2026.10.25\n", sha());
        assert_eq!(mask_pinned_uses(&line), "uses: owner/repo@<pin>\n");
    }

    #[test]
    fn a_quoted_reference_keeps_its_quote() {
        let double = format!("uses: \"a/b@{}\" # v1\n", sha());
        let single = format!("uses: 'a/b@{}'\n", sha());
        assert_eq!(mask_pinned_uses(&double), "uses: \"a/b@<pin>\"\n");
        assert_eq!(mask_pinned_uses(&single), "uses: 'a/b@<pin>'\n");
    }

    #[test]
    fn an_action_in_a_path_is_masked_and_keeps_its_path() {
        let line = format!("uses: owner/repo/.github/workflows/x.yml@{} # v3\n", sha());
        assert_eq!(
            mask_pinned_uses(&line),
            "uses: owner/repo/.github/workflows/x.yml@<pin>\n"
        );
    }

    #[test]
    fn a_reference_with_39_or_41_characters_or_upper_case_is_not_a_pin_and_is_left_alone() {
        for sha in ["a".repeat(39), "a".repeat(41), "A".repeat(40)] {
            let line = format!("uses: a/b@{sha} # v1\n");
            assert_eq!(mask_pinned_uses(&line), line, "{sha}");
        }
    }

    #[test]
    fn a_forty_character_literal_anywhere_but_a_uses_line_is_left_alone() {
        let run = format!("      run: git checkout {}\n", sha());
        let env = format!("        SHA: a/b@{}\n", sha());
        let comment = format!("# uses: a/b@{}\n", sha());
        for text in [&run, &env, &comment] {
            assert_eq!(&mask_pinned_uses(text), text);
        }
    }

    #[test]
    fn a_local_a_docker_and_a_branch_reference_are_left_alone() {
        for line in [
            "uses: ./.github/actions/x\n",
            "uses: docker://alpine:3.20\n",
            "uses: actions/checkout@main\n",
            "uses: actions/checkout@v4\n",
        ] {
            assert_eq!(mask_pinned_uses(line), line);
        }
    }

    #[test]
    fn two_files_that_differ_in_a_pin_mask_to_the_same_text_and_two_that_differ_elsewhere_do_not() {
        let one = format!("uses: a/b@{} # v1\nrun: echo one\n", "a".repeat(40));
        let two = format!("uses: a/b@{} # v2\nrun: echo one\n", "b".repeat(40));
        let other = format!("uses: a/b@{} # v1\nrun: echo two\n", "a".repeat(40));
        assert_eq!(mask_pinned_uses(&one), mask_pinned_uses(&two));
        assert_ne!(mask_pinned_uses(&one), mask_pinned_uses(&other));
    }

    #[test]
    fn a_different_action_with_the_same_commit_is_still_told_apart() {
        let one = format!("uses: a/b@{}\n", sha());
        let two = format!("uses: a/c@{}\n", sha());
        assert_ne!(mask_pinned_uses(&one), mask_pinned_uses(&two));
    }

    #[test]
    fn windows_line_endings_are_kept() {
        let line = format!("uses: a/b@{} # v1\r\nrun: x\r\n", sha());
        assert_eq!(mask_pinned_uses(&line), "uses: a/b@<pin>\r\nrun: x\r\n");
    }

    #[test]
    fn the_marker_is_what_the_comment_says() {
        assert_eq!(MARKER, "@<pin>");
    }
}
