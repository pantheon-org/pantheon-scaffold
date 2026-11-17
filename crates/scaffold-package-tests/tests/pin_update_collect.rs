//! Tests of `.github/scripts/collect-pin-update.sh`, which the `pins` job of `update-pins.yml` runs after it has updated and rendered the pins, and of the seam
//! between it and `apply-pin-update.sh`: what the first writes is what the second accepts.

use scaffold_package_tests::pinupdate::{
    FRESHNESS, MANIFEST, PINS, WORKFLOW, apply, main_repo, manifest, pins, sha256_hex, snapshot,
    workflow,
};
use scaffold_package_tests::pkg::root;
use scaffold_package_tests::repo::{commit_all, git, put, temp_dir};
use std::path::{Path, PathBuf};
use std::process::Command;

/// `main`: the fixture committed to a git repository.
fn committed_main() -> tempfile::TempDir {
    let dir = main_repo();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    commit_all(dir.path(), "main");
    dir
}

/// The update `a`, `b` to `c`, `d` made in the working tree, as the pins job leaves it.
fn update(dir: &Path) {
    let wf = workflow('c', 'd', "");
    put(dir, WORKFLOW, &wf);
    put(dir, MANIFEST, &manifest(&sha256_hex(wf.as_bytes())));
    put(dir, PINS, &pins('c', 'd'));
}

struct Collected {
    code: i32,
    stdout: String,
    stderr: String,
    tar: PathBuf,
    _keep: tempfile::TempDir,
}

/// Runs the collect script in `repo`, with a record and, when given, a summary.
fn collect(repo: &Path, freshness: Option<&str>, summary: Option<&str>) -> Collected {
    let keep = temp_dir();
    let out = keep.path().join("out");
    let record = keep.path().join("freshness.json");
    if let Some(text) = freshness {
        std::fs::write(&record, text).unwrap();
    }
    let mut cmd = Command::new("bash");
    cmd.arg(root().join(".github/scripts/collect-pin-update.sh"))
        .arg(&out)
        .arg(&record)
        .current_dir(repo);
    if let Some(text) = summary {
        let path = keep.path().join("summary.md");
        std::fs::write(&path, text).unwrap();
        cmd.arg(path);
    }
    let output = cmd.output().expect("bash runs");
    Collected {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        tar: out.join("pin-update.tar"),
        _keep: keep,
    }
}

fn members(tar: &Path) -> Vec<String> {
    let output = Command::new("tar").arg("-tf").arg(tar).output().unwrap();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

fn extract(tar: &Path) -> tempfile::TempDir {
    let dir = temp_dir();
    let status = Command::new("tar")
        .arg("-xf")
        .arg(tar)
        .arg("-C")
        .arg(dir.path())
        .status()
        .unwrap();
    assert!(status.success());
    dir
}

fn refused(c: &Collected, needle: &str) {
    assert_eq!(c.code, 1, "{}{}", c.stdout, c.stderr);
    assert!(
        c.stderr.contains("refused:") && c.stderr.contains(needle),
        "{}",
        c.stderr
    );
    assert!(!c.tar.exists(), "a refusal writes no tar");
}

#[test]
fn the_modified_files_of_the_list_are_collected_with_the_record_and_the_summary() {
    let repo = committed_main();
    update(repo.path());
    let c = collect(
        repo.path(),
        Some(FRESHNESS),
        Some("Moves the pinned actions.\n"),
    );
    assert_eq!((c.code, c.stderr.as_str()), (0, ""));
    assert!(
        c.stdout.starts_with("collected 3 changed file(s)"),
        "{}",
        c.stdout
    );
    let dir = extract(&c.tar);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("changed.txt")).unwrap(),
        format!("{WORKFLOW}\n{MANIFEST}\n{PINS}\n")
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("freshness.json")).unwrap(),
        FRESHNESS
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("summary.md")).unwrap(),
        "Moves the pinned actions.\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("tree").join(PINS)).unwrap(),
        pins('c', 'd')
    );
}

#[test]
fn a_staged_change_is_collected_like_an_unstaged_one() {
    let repo = committed_main();
    update(repo.path());
    git(repo.path(), &["add", "-A"]);
    let c = collect(repo.path(), Some(FRESHNESS), None);
    assert_eq!((c.code, c.stderr.as_str()), (0, ""));
    assert!(members(&c.tar).contains(&"changed.txt".to_owned()));
}

#[test]
fn no_change_is_collected_as_the_record_alone() {
    let repo = committed_main();
    let c = collect(repo.path(), Some(FRESHNESS), None);
    assert_eq!((c.code, c.stderr.as_str()), (0, ""));
    let dir = extract(&c.tar);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("changed.txt")).unwrap(),
        ""
    );
    assert!(dir.path().join("freshness.json").exists());
    assert!(!dir.path().join("summary.md").exists());
    assert!(
        std::fs::read_dir(dir.path().join("tree"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn a_deleted_added_untracked_renamed_or_retyped_file_is_refused() {
    let repo = committed_main();
    std::fs::remove_file(repo.path().join(WORKFLOW)).unwrap();
    refused(
        &collect(repo.path(), Some(FRESHNESS), None),
        "only modified files may change",
    );
    git(repo.path(), &["checkout", "--", "."]);

    put(repo.path(), ".github/workflows/new.yml", "name: new\n");
    refused(&collect(repo.path(), Some(FRESHNESS), None), "??");
    git(repo.path(), &["add", "-A"]);
    refused(
        &collect(repo.path(), Some(FRESHNESS), None),
        "only modified files may change",
    );
    git(repo.path(), &["reset", "-q", "--hard"]);
    std::fs::remove_file(repo.path().join(".github/workflows/new.yml")).ok();

    git(
        repo.path(),
        &["mv", WORKFLOW, ".github/workflows/renamed.yml"],
    );
    refused(
        &collect(repo.path(), Some(FRESHNESS), None),
        "only modified files may change",
    );
    git(repo.path(), &["reset", "-q", "--hard"]);

    std::fs::remove_file(repo.path().join(PINS)).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", repo.path().join(PINS)).unwrap();
    refused(
        &collect(repo.path(), Some(FRESHNESS), None),
        "only modified files may change",
    );
}

#[test]
fn a_modified_file_outside_the_list_is_refused() {
    for path in ["README.md", ".github/scripts/apply-pin-update.sh"] {
        let repo = committed_main();
        put(repo.path(), path, "changed\n");
        refused(
            &collect(repo.path(), Some(FRESHNESS), None),
            "outside the list",
        );
    }
}

// The collect script has no manifest to ask: a workflow of the right name is collected, and the apply script refuses one that main's manifest does not
// list as owned.
#[test]
fn a_workflow_that_is_not_rendered_passes_the_collect_script_and_is_refused_by_the_apply_script() {
    let repo = committed_main();
    put(repo.path(), ".github/workflows/ci.yml", "changed\n");
    let c = collect(repo.path(), Some(FRESHNESS), None);
    assert_eq!((c.code, c.stderr.as_str()), (0, ""));
    let run = apply(
        main_repo().path(),
        &std::fs::read(&c.tar).unwrap(),
        None,
        false,
    );
    assert_eq!(run.code, 1);
    assert!(
        run.stderr
            .contains("not a workflow that main's manifest lists as owned"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_missing_record_is_refused() {
    let repo = committed_main();
    refused(&collect(repo.path(), None, None), "no freshness record");
}

#[test]
fn what_the_collect_script_writes_the_apply_script_accepts_and_applies() {
    let pins_job = committed_main();
    update(pins_job.path());
    let c = collect(
        pins_job.path(),
        Some(FRESHNESS),
        Some("Moves the pinned actions.\n"),
    );
    assert_eq!((c.code, c.stderr.as_str()), (0, ""));
    let bytes = std::fs::read(&c.tar).unwrap();
    let update_job = main_repo();
    let run = apply(update_job.path(), &bytes, None, false);
    assert_eq!((run.code, run.stderr.as_str()), (0, ""), "{}", run.stdout);
    let (applied, rendered) = (snapshot(update_job.path()), snapshot(pins_job.path()));
    for path in [WORKFLOW, MANIFEST, PINS] {
        assert_eq!(applied[path], rendered[path], "{path}");
    }
}

#[test]
fn a_quiet_collect_is_accepted_by_the_apply_script() {
    let pins_job = committed_main();
    let c = collect(pins_job.path(), Some(FRESHNESS), None);
    let bytes = std::fs::read(&c.tar).unwrap();
    let update_job = main_repo();
    let before = snapshot(update_job.path());
    let run = apply(update_job.path(), &bytes, None, false);
    assert_eq!((run.code, run.stderr.as_str()), (0, ""), "{}", run.stdout);
    assert_eq!(snapshot(update_job.path()), before);
}
