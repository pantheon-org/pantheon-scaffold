//! The whole chain of a pin update, through the real tools and the real scripts, without GitHub: every pin of `pkl/Pins.pkl` is lowered in a copy of this
//! repository and rendered (that is `main` before the weekly run), then the `pins` job's steps run (`scaffold-release update-pins` against a local stand-in
//! for the GitHub API, `pantheon-scaffold render` and `check`, `collect-pin-update.sh`), and the `update` job's `apply-pin-update.sh` applies the tar to a
//! clean clone of that `main`. The clone must pass `check`, and the files it changed must be exactly the files of this repository: the update undoes the
//! lowering, byte for byte.
//!
//! This is the proof that a pin pull request needs no human commit. It needs `pkl` and `jq` on the PATH (CI runs the tests under `mise x`, and the runners have
//! `jq`); a missing tool is a failure, not a skip.

use regex::Regex;
use scaffold_package_tests::cli::run;
use scaffold_package_tests::pinupdate::apply;
use scaffold_package_tests::pkg::root;
use scaffold_package_tests::repo::{clone_of, commit_all, git, temp_dir, tracked_files};
use scaffold_package_tests::serve::serve_routes;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

struct Pin {
    action: String,
    sha: String,
    version: String,
}

fn pins_of(text: &str) -> Vec<Pin> {
    let line = Regex::new(
        r#"(?m)^\w+: Pin = new \{ action = "([^"]+)"; sha = "([0-9a-f]{40})"; version = "([^"]+)" \}$"#,
    )
    .unwrap();
    line.captures_iter(text)
        .map(|c| Pin {
            action: c[1].to_owned(),
            sha: c[2].to_owned(),
            version: c[3].to_owned(),
        })
        .collect()
}

fn repo_of(action: &str) -> String {
    action.split('/').take(2).collect::<Vec<_>>().join("/")
}

/// A commit that no real release has, the same for one repository.
fn fake_sha(repo: &str) -> String {
    Sha256::digest(repo.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()[..40]
        .to_owned()
}

/// The tracked files of this repository, copied.
fn copy_of_this_repository() -> tempfile::TempDir {
    let dir = temp_dir();
    for rel in tracked_files() {
        let from = root().join(&rel);
        if !from.is_file() {
            continue;
        }
        let to = dir.path().join(&rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(&from, &to).unwrap();
    }
    dir
}

fn render_and_check(dir: &Path) {
    let rendered = run(
        &["render", "scaffold.pkl", "--allow-local-package", "pkl"],
        dir,
    );
    assert_eq!(rendered.code, 0, "render: {}{}", rendered.out, rendered.err);
    let checked = run(
        &["check", "scaffold.pkl", "--allow-local-package", "pkl"],
        dir,
    );
    assert_eq!(checked.code, 0, "check: {}{}", checked.out, checked.err);
}

/// The answers of GitHub for these pins: each one's latest release is the pin of this repository, and the lowered pin is a release of a fake commit.
fn github_for(real: &[Pin]) -> BTreeMap<String, (u16, String)> {
    let mut routes = BTreeMap::new();
    for pin in real {
        let repo = repo_of(&pin.action);
        let fake = fake_sha(&repo);
        let ok = |body: String| (200, body);
        routes.insert(
            format!("/repos/{repo}/releases/latest"),
            ok(format!("{{\"tag_name\":\"{}\"}}", pin.version)),
        );
        routes.insert(
            format!("/repos/{repo}/releases/tags/{}", pin.version),
            ok("{\"draft\":false}".to_owned()),
        );
        routes.insert(
            format!("/repos/{repo}/git/ref/tags/{}", pin.version),
            ok(format!(
                "{{\"object\":{{\"type\":\"commit\",\"sha\":\"{}\"}}}}",
                pin.sha
            )),
        );
        routes.insert(
            format!("/repos/{repo}/git/ref/tags/v0.0.0"),
            ok(format!(
                "{{\"object\":{{\"type\":\"commit\",\"sha\":\"{fake}\"}}}}"
            )),
        );
        routes.insert(
            format!("/repos/{repo}/releases/tags/v0.0.0"),
            ok("{\"draft\":false}".to_owned()),
        );
        for sha in [&pin.sha, &fake] {
            routes.insert(format!("/repos/{repo}/commits/{sha}"), ok("{}".to_owned()));
        }
    }
    routes
}

#[test]
fn a_pin_update_goes_through_the_scripts_and_gives_back_exactly_the_files_of_main() {
    let tools = ["pkl", "jq", "tar"];
    for tool in tools {
        let found = Command::new("sh")
            .args(["-c", &format!("command -v {tool}")])
            .output()
            .unwrap();
        assert!(
            found.status.success(),
            "{tool} is needed on the PATH: this test does not skip"
        );
    }

    // `main` before the weekly run: every pin lowered, and the repository rendered to match.
    let real = pins_of(&std::fs::read_to_string(root().join("pkl/Pins.pkl")).unwrap());
    assert!(real.len() >= 5, "the pins of pkl/Pins.pkl are read");
    let lowered = copy_of_this_repository();
    git(lowered.path(), &["init", "-q", "-b", "main"]);
    let mut text = std::fs::read_to_string(lowered.path().join("pkl/Pins.pkl")).unwrap();
    for pin in &real {
        let was = format!(
            "action = \"{}\"; sha = \"{}\"; version = \"{}\"",
            pin.action, pin.sha, pin.version
        );
        let now = format!(
            "action = \"{}\"; sha = \"{}\"; version = \"v0.0.0\"",
            pin.action,
            fake_sha(&repo_of(&pin.action))
        );
        assert!(text.contains(&was), "{was}");
        text = text.replace(&was, &now);
    }
    std::fs::write(lowered.path().join("pkl/Pins.pkl"), text).unwrap();
    render_and_check(lowered.path());
    commit_all(lowered.path(), "main, every pin lowered");
    let (_update_keep, update_job) = clone_of(lowered.path());

    // The `pins` job: update the pins against the stand-in for GitHub, render, check, collect.
    let server = serve_routes(github_for(&real));
    let work = temp_dir();
    let (record, summary) = (
        work.path().join("freshness.json"),
        work.path().join("summary.md"),
    );
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let updated = Command::new(cargo)
        .args([
            "run",
            "-q",
            "-p",
            "scaffold-release",
            "--locked",
            "--",
            "update-pins",
            "--pins",
        ])
        .arg(lowered.path().join("pkl/Pins.pkl"))
        .arg("--freshness-out")
        .arg(&record)
        .arg("--summary")
        .arg(&summary)
        .env(
            "PANTHEON_SCAFFOLD_GITHUB_API",
            format!("http://127.0.0.1:{}", server.port),
        )
        .env("GITHUB_TOKEN", "test-token")
        .current_dir(root())
        .output()
        .expect("cargo runs");
    assert!(
        updated.status.success(),
        "update-pins: {}{}",
        String::from_utf8_lossy(&updated.stdout),
        String::from_utf8_lossy(&updated.stderr)
    );
    assert!(record.exists() && summary.exists());
    render_and_check(lowered.path());
    let collected = Command::new("bash")
        .arg(root().join(".github/scripts/collect-pin-update.sh"))
        .arg(work.path().join("out"))
        .arg(&record)
        .arg(&summary)
        .current_dir(lowered.path())
        .output()
        .unwrap();
    assert!(
        collected.status.success(),
        "collect: {}{}",
        String::from_utf8_lossy(&collected.stdout),
        String::from_utf8_lossy(&collected.stderr)
    );

    // The `update` job: the checks, then the apply, on the clean clone of `main`.
    let bytes = std::fs::read(work.path().join("out/pin-update.tar")).unwrap();
    let checked_only = apply(&update_job, &bytes, None, true);
    assert_eq!(
        (checked_only.code, checked_only.stderr.as_str()),
        (0, ""),
        "{}",
        checked_only.stdout
    );
    let applied = apply(&update_job, &bytes, None, false);
    assert_eq!(
        (applied.code, applied.stderr.as_str()),
        (0, ""),
        "{}",
        applied.stdout
    );

    // The clone passes the same check as the pull request will, and its files are the files of this repository.
    let checked = run(
        &["check", "scaffold.pkl", "--allow-local-package", "pkl"],
        &update_job,
    );
    assert_eq!(
        checked.code, 0,
        "check after the apply: {}{}",
        checked.out, checked.err
    );
    let changed = git(&update_job, &["status", "--porcelain"]);
    assert!(
        changed.lines().count() >= 3,
        "pins, manifest and rendered workflows changed: {changed}"
    );
    for line in changed.lines() {
        let path = line[3..].trim();
        assert_eq!(
            std::fs::read(update_job.join(path)).unwrap(),
            std::fs::read(root().join(path)).unwrap(),
            "{path} is not the file of this repository after the update"
        );
    }
    assert_eq!(
        std::fs::read_to_string(applied.out.join("freshness.json")).unwrap(),
        std::fs::read_to_string(&record).unwrap()
    );
}
