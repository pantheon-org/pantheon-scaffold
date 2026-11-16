//! Ported from `runner/test/smoke-fixtures.test.ts`: the fixtures the compiled-binary smoke suite (`scaffold-release smoke`) renders.

use scaffold_package_tests::pkg::{eval, fixture, paths};

fn rendered(name: &str) -> Vec<String> {
    paths(&eval(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}")))
}

#[test]
fn smoke_orphan_a_renders_two_files_and_smoke_orphan_b_drops_the_second() {
    assert_eq!(rendered("smoke-orphan-a"), ["a.txt", "b.txt"]);
    assert_eq!(rendered("smoke-orphan-b"), ["a.txt"]);
}

#[test]
fn smoke_case_collision_renders_two_paths_that_differ_only_by_case() {
    assert_eq!(rendered("smoke-case-collision"), ["README.md", "readme.md"]);
}

#[test]
fn smoke_case_a_and_smoke_case_b_render_the_same_file_under_names_that_differ_only_by_case() {
    assert_eq!(rendered("smoke-case-a"), ["readme.md"]);
    assert_eq!(rendered("smoke-case-b"), ["README.md"]);
}

#[test]
fn the_phase_3_fixtures_render_the_same_19_files_with_the_default_and_with_all_features() {
    let expected = [
        ".claude/settings.json",
        ".github/scripts/agnix-assemble-comment.sh",
        ".github/scripts/agnix-file-issues.sh",
        ".github/scripts/agnix-introduced-errors.sh",
        ".github/scripts/auto-rebase.sh",
        ".github/scripts/cargo-deny.sh",
        ".github/scripts/check-commit-types.sh",
        ".github/scripts/check-inline-scripts.sh",
        ".github/scripts/check-toolchain-version.sh",
        ".github/scripts/release-automerge.sh",
        ".github/scripts/sync-labels.sh",
        ".mise/conf.d/repo.toml",
        ".rumdl.toml",
        "deny.toml",
        "hk.pkl",
        "hk.shared.pkl",
        "mise.lock",
        "mise.toml",
        "rust-toolchain.toml",
    ];
    for name in ["phase3", "phase3-all"] {
        let mut got = rendered(name);
        got.sort();
        assert_eq!(got, expected, "{name}");
    }
}
