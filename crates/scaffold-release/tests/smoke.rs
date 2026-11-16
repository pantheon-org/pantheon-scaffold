//! The smoke suite against the real runner. The runner is built once, by the cargo that runs these tests, and the suite is run through
//! the built `scaffold-release` binary, so what is tested is what the workflows run. It needs `pkl` and `git` on the PATH, as the
//! tests of `scaffold-core` do.

mod common;

use common::settle;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// The runner binary: built on first use, in the target folder these tests run from.
fn runner() -> &'static Path {
    static RUNNER: OnceLock<PathBuf> = OnceLock::new();
    RUNNER.get_or_init(|| {
        let status = Command::new(env!("CARGO"))
            .args(["build", "-p", "scaffold-cli", "--locked"])
            .current_dir(workspace())
            .status()
            .unwrap();
        assert!(status.success(), "the runner builds");
        // This test binary is <target>/debug/deps/smoke-<hash>, and the runner is <target>/debug/pantheon-scaffold.
        let exe = std::env::current_exe().unwrap();
        let binary = exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("pantheon-scaffold");
        assert!(binary.exists(), "{} exists", binary.display());
        binary
    })
}

struct Outcome {
    code: Option<i32>,
    out: String,
    err: String,
}

fn smoke(binary: &Path, extra: &[&str]) -> Outcome {
    let output = Command::new(env!("CARGO_BIN_EXE_scaffold-release"))
        .arg("smoke")
        .arg(binary)
        .args(extra)
        .arg("--repo")
        .arg(workspace())
        .output()
        .unwrap();
    Outcome {
        code: output.status.code(),
        out: String::from_utf8_lossy(&output.stdout).into_owned(),
        err: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn names(out: &str, verdict: &str) -> Vec<String> {
    out.lines()
        .filter_map(|l| l.strip_prefix(&format!("{verdict}  ")))
        .map(str::to_owned)
        .collect()
}

#[test]
fn every_check_passes_on_the_runner_and_the_report_names_the_binary_and_the_filesystem() {
    let result = smoke(runner(), &["--version", "0.0.0"]);
    assert_eq!(
        (result.code, result.err.as_str()),
        (Some(0), ""),
        "{}",
        result.out
    );
    let passed = names(&result.out, "PASS");
    assert_eq!(passed.len(), 16, "{}", result.out);
    assert_eq!(
        passed[0],
        "--version prints the version the binary was built for"
    );
    assert_eq!(
        passed[15],
        "a dirty tracked tree refuses the deletion (exit 3) and keeps the file"
    );
    assert!(names(&result.out, "FAIL").is_empty());
    let mut lines = result.out.lines();
    assert_eq!(
        lines.next(),
        Some(format!("binary {}", runner().display()).as_str())
    );
    let filesystem = lines.next().unwrap();
    assert!(
        filesystem.starts_with("filesystem: case-") && filesystem.contains(", platform "),
        "{filesystem}"
    );
    assert!(result.out.ends_with("\nall passed\n"), "{}", result.out);
}

#[test]
fn a_binary_built_for_another_version_fails_the_first_check_and_only_that_one() {
    let result = smoke(runner(), &["--version", "9.9.9"]);
    assert_eq!(result.code, Some(1));
    assert_eq!(
        names(&result.out, "FAIL"),
        ["--version prints the version the binary was built for"]
    );
    assert_eq!(names(&result.out, "PASS").len(), 15);
    assert!(result.out.contains("FAIL  --version prints the version the binary was built for\n      exit 0\n      stdout: 0.0.0\n      stderr: \n"), "{}", result.out);
    assert!(result.out.ends_with("\n1 failing\n"), "{}", result.out);
}

#[test]
fn a_binary_that_finds_no_drift_and_cannot_detach_fails_those_two_checks() {
    let dir = tempfile::tempdir().unwrap();
    let wrapper = dir.path().join("broken");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\ncase \"$1\" in\n  check) \"{r}\" \"$@\"; exit 0 ;;\n  detach) echo 'detach is broken' >&2; exit 1 ;;\n  *) exec \"{r}\" \"$@\" ;;\nesac\n",
            r = runner().display()
        ),
    )
    .unwrap();
    settle(&wrapper);
    let result = smoke(&wrapper, &["--version", "0.0.0"]);
    assert_eq!(result.code, Some(1));
    assert_eq!(
        names(&result.out, "FAIL"),
        [
            "check reports a hand edit as drift (exit 4)",
            "detach removes .pkl-gen and leaves the files"
        ]
    );
    assert_eq!(names(&result.out, "PASS").len(), 14);
    assert!(
        result.out.contains("      stderr: detach is broken\n"),
        "{}",
        result.out
    );
    assert!(result.out.ends_with("\n2 failing\n"), "{}", result.out);
}

#[test]
fn a_binary_that_cannot_be_run_is_an_error_and_no_binary_is_a_usage_error() {
    let result = smoke(Path::new("/no/such/runner"), &[]);
    assert_eq!(result.code, Some(1));
    assert!(result.err.contains("/no/such/runner"), "{}", result.err);
    let usage = Command::new(env!("CARGO_BIN_EXE_scaffold-release"))
        .arg("smoke")
        .output()
        .unwrap();
    assert_eq!(usage.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&usage.stderr),
        "usage: scaffold-release smoke <binary> [--version <x.y.z>] [--repo <checkout>]\n"
    );
}

#[test]
fn a_scenario_whose_setup_fails_is_one_failed_check_named_after_it_and_the_others_still_run() {
    let output = Command::new(env!("CARGO_BIN_EXE_scaffold-release"))
        .args(["smoke"])
        .arg(runner())
        .args(["--version", "0.0.0", "--repo", "/no/such/checkout"])
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{out}");
    let failed = names(&out, "FAIL");
    assert!(
        failed.contains(&"orphan deletion in a clean git repository".to_owned()),
        "{out}"
    );
    assert!(
        failed.contains(&"a dirty tracked tree refuses the deletion".to_owned()),
        "{out}"
    );
    assert!(
        out.contains("      setup render of smoke-orphan-a failed: exit "),
        "{out}"
    );
    assert!(
        names(&out, "PASS")
            .contains(&"--version prints the version the binary was built for".to_owned()),
        "the checks that need no fixture still pass: {out}"
    );
}
