//! Tests of `.github/scripts/open-pin-pull-request.sh`: the last step of the `update` job, which commits the applied pin update to `chore/update-pins`
//! and opens the pull request. The remote is a local bare repository and `gh` is a stand-in that records its calls.

use scaffold_package_tests::pkg::root;
use scaffold_package_tests::repo::{git, put, temp_dir, write_script};
use std::path::PathBuf;
use std::process::Command;

const BOT: &str = "pantheon-ai-bot[bot]@users.noreply.github.com";

struct World {
    remote: tempfile::TempDir,
    work: tempfile::TempDir,
    bin: tempfile::TempDir,
    log: PathBuf,
}

fn gh_log(world: &World) -> String {
    std::fs::read_to_string(&world.log).unwrap_or_default()
}

/// A bare remote and a working copy of `main`, with a `gh` stand-in on a folder of its own. `open_prs` is what `gh pr list` prints.
fn world(open_prs: &str) -> World {
    let remote = temp_dir();
    git(remote.path(), &["init", "-q", "--bare", "-b", "main"]);
    let work = temp_dir();
    git(work.path(), &["init", "-q", "-b", "main"]);
    put(work.path(), "pkl/Pins.pkl", "pins a\n");
    put(work.path(), "README.md", "# demo\n");
    git(work.path(), &["add", "-A"]);
    git(work.path(), &["commit", "-q", "-m", "main"]);
    git(
        work.path(),
        &["remote", "add", "origin", &remote.path().to_string_lossy()],
    );
    git(work.path(), &["push", "-q", "origin", "main"]);
    let bin = temp_dir();
    let log = bin.path().join("gh.log");
    write_script(
        &bin.path().join("gh"),
        &format!(
            "#!/usr/bin/env bash\nprintf '%s\\n' \"$*\" >> '{}'\nif [ \"$1 $2\" = 'pr list' ]; then printf '%s' '{open_prs}'; fi\n",
            log.display()
        ),
    );
    World {
        remote,
        work,
        bin,
        log,
    }
}

/// A commit on the remote branch, made by a clone of the remote.
fn remote_commit(world: &World, email: &str, file: &str) {
    let clone = temp_dir();
    git(
        clone.path(),
        &["clone", "-q", &world.remote.path().to_string_lossy(), "."],
    );
    let exists = Command::new("git")
        .arg("-C")
        .arg(clone.path())
        .args([
            "rev-parse",
            "--verify",
            "-q",
            "refs/remotes/origin/chore/update-pins",
        ])
        .output()
        .unwrap()
        .status
        .success();
    let args: &[&str] = if exists {
        &["switch", "-q", "chore/update-pins"]
    } else {
        &["switch", "-q", "-c", "chore/update-pins"]
    };
    git(clone.path(), args);
    put(clone.path(), file, "x\n");
    git(clone.path(), &["add", "-A"]);
    git(
        clone.path(),
        &[
            "-c",
            &format!("user.email={email}"),
            "commit",
            "-q",
            "-m",
            "earlier",
        ],
    );
    git(clone.path(), &["push", "-q", "origin", "chore/update-pins"]);
}

fn run(world: &World) -> (i32, String, String) {
    let summary = world.bin.path().join("summary.md");
    std::fs::write(&summary, "Moves the pinned actions.\n").unwrap();
    let path = format!(
        "{}:{}",
        world.bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    let mut cmd = Command::new("bash");
    cmd.arg(root().join(".github/scripts/open-pin-pull-request.sh"))
        .current_dir(world.work.path())
        .env("PATH", path)
        .env("GH_TOKEN", "test-token")
        .env("REPO", "owner/name")
        .env("SUMMARY", &summary)
        .env("REMOTE", world.remote.path());
    for (key, _) in std::env::vars() {
        if key.starts_with("GIT_") {
            cmd.env_remove(key);
        }
    }
    let out = cmd.output().expect("bash runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn remote_branch(world: &World) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(world.remote.path())
        .args([
            "rev-parse",
            "--verify",
            "-q",
            "refs/heads/chore/update-pins",
        ])
        .output()
        .unwrap();
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn log_of_remote_branch(world: &World) -> Vec<String> {
    git(
        world.remote.path(),
        &["log", "--format=%ae %s", "main..chore/update-pins"],
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

#[test]
fn when_nothing_changed_nothing_is_pushed_and_no_pull_request_is_opened() {
    let world = world("");
    let (code, out, err) = run(&world);
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(out.contains("No pin changed."), "{out}");
    assert_eq!(remote_branch(&world), None);
    assert_eq!(gh_log(&world), "");
}

#[test]
fn a_change_is_committed_by_the_bot_pushed_and_opened_as_a_pull_request() {
    let world = world("");
    put(world.work.path(), "pkl/Pins.pkl", "pins b\n");
    let (code, _, err) = run(&world);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        log_of_remote_branch(&world),
        [format!("{BOT} chore(deps): update pinned actions")]
    );
    let calls = gh_log(&world);
    assert!(
        calls.contains("pr list --repo owner/name --head chore/update-pins"),
        "{calls}"
    );
    assert!(calls.contains("pr create --repo owner/name --head chore/update-pins --title chore(deps): update pinned actions --body-file"), "{calls}");
}

#[test]
fn only_tracked_files_that_changed_are_committed() {
    let world = world("");
    put(world.work.path(), "pkl/Pins.pkl", "pins b\n");
    put(
        world.work.path(),
        "stray.txt",
        "not meant to be committed\n",
    );
    let (code, _, err) = run(&world);
    assert_eq!(code, 0, "{err}");
    let files = git(
        world.remote.path(),
        &["diff", "--name-only", "main", "chore/update-pins"],
    );
    assert_eq!(files.trim(), "pkl/Pins.pkl");
}

#[test]
fn last_weeks_branch_of_the_bot_is_replaced_and_an_open_pull_request_is_kept() {
    let world = world("7");
    remote_commit(&world, BOT, "pkl/Pins.pkl");
    let old = remote_branch(&world).unwrap();
    put(world.work.path(), "pkl/Pins.pkl", "pins c\n");
    let (code, _, err) = run(&world);
    assert_eq!(code, 0, "{err}");
    assert_ne!(
        remote_branch(&world).unwrap(),
        old,
        "the branch was replaced"
    );
    assert_eq!(
        log_of_remote_branch(&world),
        [format!("{BOT} chore(deps): update pinned actions")],
        "one commit on main, not two on top of last week's"
    );
    assert!(!gh_log(&world).contains("pr create"), "{}", gh_log(&world));
}

#[test]
fn a_branch_with_a_commit_by_someone_else_is_left_alone_and_the_job_does_not_fail() {
    let world = world("7");
    remote_commit(&world, "person@example.invalid", "README.md");
    let before = remote_branch(&world).unwrap();
    put(world.work.path(), "pkl/Pins.pkl", "pins c\n");
    let (code, out, err) = run(&world);
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(
        out.contains("::notice::") && out.contains("not the bot's"),
        "{out}"
    );
    assert_eq!(remote_branch(&world).unwrap(), before);
    assert_eq!(gh_log(&world), "", "no pull request is touched");
}

#[test]
fn a_branch_with_the_bots_commit_and_a_commit_by_someone_else_is_left_alone() {
    let world = world("");
    remote_commit(&world, BOT, "pkl/Pins.pkl");
    remote_commit(&world, "person@example.invalid", "README.md");
    let before = remote_branch(&world).unwrap();
    put(world.work.path(), "pkl/Pins.pkl", "pins d\n");
    let (code, out, _) = run(&world);
    assert_eq!(code, 0);
    assert!(out.contains("not the bot's"), "{out}");
    assert_eq!(remote_branch(&world).unwrap(), before);
}

#[test]
fn the_token_is_sent_in_a_header_and_is_not_in_the_remote_url_or_the_output() {
    let world = world("");
    put(world.work.path(), "pkl/Pins.pkl", "pins b\n");
    let (code, out, err) = run(&world);
    assert_eq!(code, 0, "{err}");
    assert!(!out.contains("test-token") && !err.contains("test-token"));
    let script =
        std::fs::read_to_string(root().join(".github/scripts/open-pin-pull-request.sh")).unwrap();
    assert!(script.contains("http.extraheader") && !script.contains("x-access-token:${GH_TOKEN}@"));
}
