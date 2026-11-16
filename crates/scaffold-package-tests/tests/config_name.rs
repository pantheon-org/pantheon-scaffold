//! Ported from `runner/test/config-name.test.ts`: the configuration file is `scaffold.pkl`, and the old name is said only where it is
//! allowed to be.

use scaffold_package_tests::pkg::root;
use scaffold_package_tests::repo::tracked_files;
use std::collections::BTreeMap;

/// The only tracked files that may still say the old name, and why.
fn allowed() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        (
            "crates/scaffold-core/src/config_name.rs",
            "the one error and its unit tests",
        ),
        (
            "features/renamed-config.feature",
            "the scenarios for that error",
        ),
        (
            "crates/scaffold-package-tests/tests/config_name.rs",
            "this test",
        ),
        (
            "docs/adopting.md",
            "the upgrade note and the note about 0.2.0, which describe the old name",
        ),
        (
            "docs/runner-upgrade.md",
            "what was observed on the real upgrade of 0.1.7 to 0.1.8",
        ),
        (
            "docs/workflow-layer.md",
            "the 0.1.4 header note, a dated fact",
        ),
        (
            "docs/ts-runner-test-mapping.md",
            "the title of a deleted TypeScript test",
        ),
        (
            "docs/ts-tooling-test-mapping.md",
            "the title of the TypeScript test of this one, ported here",
        ),
        ("docs/refocus-2026-11-10.md", "a dated record"),
        ("docs/presets-design.md", "a dated design record"),
        (
            "crates/scaffold-core/tests/data/yaml-corpus.json",
            "a frozen snapshot of old workflows, text and parsed",
        ),
        (
            "crates/scaffold-release/baseline/README.md",
            "the account of the one baseline refresh for the rename",
        ),
    ])
}

fn says_old_name(file: &str) -> bool {
    let text = std::fs::read(root().join(file)).unwrap_or_default();
    String::from_utf8_lossy(&text).contains("pantheon.pkl")
}

#[test]
fn no_tracked_file_is_named_pantheon_pkl() {
    let named: Vec<String> = tracked_files()
        .into_iter()
        .filter(|f| f == "pantheon.pkl" || f.ends_with("/pantheon.pkl"))
        .collect();
    assert_eq!(named, Vec::<String>::new());
}

#[test]
fn the_old_config_name_appears_only_in_the_files_that_are_allowed_to_say_it() {
    let allowed = allowed();
    let found: Vec<String> = tracked_files()
        .into_iter()
        .filter(|f| says_old_name(f) && !allowed.contains_key(f.as_str()))
        .collect();
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn every_allowed_file_still_says_it_so_the_list_does_not_outlive_its_reasons() {
    let stale: Vec<&str> = allowed()
        .keys()
        .copied()
        .filter(|f| !says_old_name(f))
        .collect();
    assert_eq!(stale, Vec::<&str>::new());
}
