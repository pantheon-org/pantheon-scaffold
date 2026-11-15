//! The built binary, run the way the release workflow runs it: from the checkout root, with the exit code and the two
//! streams as the only things the workflow sees. This covers `main`, which the unit tests of `run` do not.

use std::path::Path;
use std::process::Command;

fn project(version: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("pkl")).unwrap();
    std::fs::write(
        root.path().join("pkl/PklProject"),
        format!("package {{\n  version = \"{version}\"\n}}\n"),
    )
    .unwrap();
    root
}

fn run(root: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_scaffold-release"))
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn a_matching_tag_prints_the_version_and_exits_0() {
    let root = project("0.2.0");
    assert_eq!(
        run(root.path(), &["release-assets", "check-tag", "v0.2.0"]),
        (Some(0), "0.2.0\n".to_owned(), String::new())
    );
}

#[test]
fn a_tag_for_another_version_exits_1_with_the_reason_on_stderr() {
    let root = project("0.2.0");
    assert_eq!(
        run(root.path(), &["release-assets", "check-tag", "v9.9.9"]),
        (
            Some(1),
            String::new(),
            "tag v9.9.9 does not match the PklProject version 0.2.0\n".to_owned()
        )
    );
}

#[test]
fn the_version_command_reads_pkl_project_under_the_directory_it_is_run_from() {
    let root = project("1.4.0-rc.2");
    assert_eq!(
        run(root.path(), &["release-assets", "version"]),
        (Some(0), "1.4.0-rc.2\n".to_owned(), String::new())
    );
}

#[test]
fn no_command_exits_2_and_prints_the_usage_on_stderr() {
    let root = project("0.2.0");
    let (code, out, err) = run(root.path(), &[]);
    assert_eq!((code, out.as_str()), (Some(2), ""));
    assert!(
        err.starts_with("usage: scaffold-release release-assets"),
        "{err}"
    );
}

fn run_promote(root: &Path, git_ref: &str, args: &[&str]) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_scaffold-release"))
        .arg("promote")
        .args(args)
        .env("GITHUB_REF", git_ref)
        .env_remove("GH_REPO")
        .current_dir(root)
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn promote_from_a_branch_is_refused_before_anything_is_asked_of_gh() {
    let root = project("0.2.0");
    let (code, out, err) = run_promote(root.path(), "refs/heads/feat/x", &["check", "v0.2.0"]);
    assert_eq!(code, Some(1));
    assert_eq!(
        out,
        "FAIL  run from main: the workflow ran on \"refs/heads/feat/x\", a promotion runs only from refs/heads/main\n"
    );
    assert_eq!(
        err,
        "refused: run from main: the workflow ran on \"refs/heads/feat/x\", a promotion runs only from refs/heads/main\n"
    );
}

#[test]
fn promote_with_a_bogus_tag_is_refused_at_the_tag_shape() {
    let root = project("0.2.0");
    let (code, out, err) = run_promote(root.path(), "refs/heads/main", &["promote", "latest"]);
    assert_eq!(code, Some(1));
    assert!(
        out.starts_with("ok    run from main: refs/heads/main\nFAIL  tag shape: "),
        "{out}"
    );
    assert!(
        err.starts_with("refused: tag shape: \"latest\" is not a release tag"),
        "{err}"
    );
}

#[test]
fn promote_with_no_tag_exits_2_and_prints_its_usage() {
    let root = project("0.2.0");
    let (code, out, err) = run_promote(root.path(), "refs/heads/main", &["check"]);
    assert_eq!((code, out.as_str()), (Some(2), ""));
    assert_eq!(
        err,
        "usage: scaffold-release promote check <tag> | promote <tag>\n"
    );
}

fn run_update_pins(root: &Path, args: &[&str], api: &str) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_scaffold-release"))
        .arg("update-pins")
        .args(args)
        .env("PANTHEON_SCAFFOLD_GITHUB_API", api)
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .current_dir(root)
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn update_pins_with_an_unknown_option_exits_2_and_prints_its_usage() {
    let root = project("0.2.0");
    let (code, out, err) = run_update_pins(root.path(), &["--nope"], "http://127.0.0.1:1");
    assert_eq!((code, out.as_str()), (Some(2), ""));
    assert!(
        err.starts_with("unknown option --nope\nusage: scaffold-release update-pins"),
        "{err}"
    );
}

#[test]
fn update_pins_reads_pkl_pins_pkl_under_the_directory_it_is_run_from() {
    let root = project("0.2.0");
    let (code, out, err) = run_update_pins(root.path(), &["--no-write"], "http://127.0.0.1:1");
    assert_eq!((code, out.as_str()), (Some(1), ""));
    assert!(err.contains("pkl/Pins.pkl"), "{err}");
}

#[test]
fn update_pins_with_no_pins_checks_zero_and_writes_the_freshness_record_when_asked() {
    let root = project("0.2.0");
    std::fs::write(root.path().join("pkl/Pins.pkl"), "module x\n").unwrap();
    let freshness = root.path().join("freshness.json");
    let (code, out, err) = run_update_pins(
        root.path(),
        &["--freshness-out", freshness.to_str().unwrap()],
        "http://127.0.0.1:1",
    );
    assert_eq!((code, err.as_str()), (Some(0), ""));
    assert_eq!(out, "checked 0 pin(s)\nno pin changed\n");
    let record = std::fs::read_to_string(freshness).unwrap();
    assert!(
        record.starts_with("{\n  \"schemaVersion\": 1,\n  \"lastSuccessfulPinCheck\": \""),
        "{record}"
    );
}
