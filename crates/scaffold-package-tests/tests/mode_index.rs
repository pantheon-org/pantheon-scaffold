//! Ported from `runner/test/mode-index.test.ts`: the executable bit as git records it. The runner never reads the index. In CI `check` runs
//! on a checkout, where the mode on disk is the mode in the index, so a committed mode that is wrong shows up as mode drift. These tests
//! prove that with real git.

use scaffold_package_tests::cli::{Out, run};
use scaffold_package_tests::pkg::{fixture, root};
use scaffold_package_tests::repo::{clone_of, commit_all, git, git_init, modes};
use std::path::Path;

const NINE: [&str; 9] = [
    "agnix-assemble-comment",
    "agnix-file-issues",
    "agnix-introduced-errors",
    "auto-rebase",
    "cargo-deny",
    "check-commit-types",
    "check-inline-scripts",
    "release-automerge",
    "sync-labels",
];

fn scaffold(cwd: &Path, command: &str) -> Out {
    let config = fixture("tooling");
    let pkl = root().join("pkl");
    run(
        &[
            command,
            &config.to_string_lossy(),
            "--root",
            &cwd.to_string_lossy(),
            "--allow-local-package",
            &pkl.to_string_lossy(),
        ],
        cwd,
    )
}

fn committed_repo() -> tempfile::TempDir {
    let repo = git_init();
    let rendered = scaffold(repo.path(), "render");
    assert_eq!(rendered.code, 0, "{}", rendered.err);
    commit_all(repo.path(), "render");
    repo
}

#[test]
fn git_records_the_nine_shared_scripts_as_100755_and_the_configs_as_100644_after_a_render_and_a_commit()
 {
    let repo = committed_repo();
    let m = modes(repo.path());
    for name in NINE {
        let path = format!(".github/scripts/{name}.sh");
        assert_eq!(m[&path], "100755", "{path}");
    }
    for path in [
        ".rumdl.toml",
        ".claude/settings.json",
        "deny.toml",
        "rust-toolchain.toml",
    ] {
        assert_eq!(m[path], "100644", "{path}");
    }
}

#[test]
fn check_on_a_fresh_clone_of_the_committed_render_finds_no_drift() {
    let repo = committed_repo();
    let (_keep, clone) = clone_of(repo.path());
    assert_eq!(scaffold(&clone, "check").code, 0);
}

#[test]
fn a_script_committed_without_the_executable_bit_is_mode_drift_on_a_fresh_clone_exit_4() {
    let repo = committed_repo();
    git(
        repo.path(),
        &[
            "update-index",
            "--chmod=-x",
            ".github/scripts/auto-rebase.sh",
        ],
    );
    git(repo.path(), &["commit", "-q", "-m", "lose the bit"]);
    assert_eq!(
        modes(repo.path())[".github/scripts/auto-rebase.sh"],
        "100644"
    );
    let (_keep, clone) = clone_of(repo.path());
    let checked = scaffold(&clone, "check");
    assert_eq!(checked.code, 4, "{}", checked.err);
    assert!(
        checked.err.contains(".github/scripts/auto-rebase.sh: mode"),
        "{}",
        checked.err
    );
}
