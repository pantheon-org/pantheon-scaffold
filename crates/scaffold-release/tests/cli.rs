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
