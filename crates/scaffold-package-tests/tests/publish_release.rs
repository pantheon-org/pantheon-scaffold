//! Ported from `runner/test/publish-release.test.ts`: runs `.github/scripts/publish-release.sh` with a stand-in `gh` that records its calls, so
//! each path of the script is seen to do what the release process says: adopt one draft, create when nothing exists, and refuse a published
//! release or two drafts.

use scaffold_package_tests::pkg::root;
use scaffold_package_tests::repo::{temp_dir, write_script};
use std::process::Command;

struct Outcome {
    code: i32,
    calls: Vec<String>,
    stderr: String,
}

fn run(draft_ids: &[&str], published_exists: bool) -> Outcome {
    let dir = temp_dir();
    let log = dir.path().join("calls.log");
    let gh = dir.path().join("gh");
    write_script(
        &gh,
        &format!(
            "#!/usr/bin/env bash\necho \"$*\" >> \"{log}\"\ncase \"$1 $2\" in\n  \"api repos/o/r/releases\") printf '%b' \"{ids}\" ;;\n  \"api repos/o/r/releases/\"*) [ \"$3\" = \"--jq\" ] && echo \"changelog body\" ;;\n  \"release view\") exit {view} ;;\nesac\nexit 0\n",
            log = log.display(),
            ids = draft_ids.join("\\n"),
            view = if published_exists { 0 } else { 1 },
        ),
    );
    let output = Command::new("bash")
        .arg(root().join(".github/scripts/publish-release.sh"))
        .env_clear()
        .env(
            "PATH",
            format!(
                "{}:{}",
                dir.path().display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .env("GH_REPO", "o/r")
        .env("TAG", "v1.2.3")
        .env("GITHUB_STEP_SUMMARY", dir.path().join("summary"))
        .current_dir(dir.path())
        .output()
        .expect("bash runs");
    let calls = std::fs::read_to_string(&log)
        .map(|t| t.trim().split('\n').map(str::to_owned).collect())
        .unwrap_or_default();
    Outcome {
        code: output.status.code().unwrap_or(-1),
        calls,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn any(calls: &[String], f: impl Fn(&str) -> bool) -> bool {
    calls.iter().any(|c| f(c))
}

#[test]
fn one_draft_for_the_tag_assets_are_uploaded_to_it_and_it_is_published_as_a_pre_release_by_id() {
    let r = run(&["777"], false);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(any(&r.calls, |c| c.starts_with("release upload v1.2.3")));
    let patch = r
        .calls
        .iter()
        .find(|c| c.contains("--method PATCH"))
        .expect("a PATCH call");
    assert!(patch.contains("repos/o/r/releases/777"));
    assert!(patch.contains("draft=false"));
    assert!(patch.contains("prerelease=true"));
    assert!(patch.contains("v1.2.3 (pre-release)"));
    assert!(r.calls.contains(&"changelog body".to_owned()));
    assert!(!any(&r.calls, |c| c.starts_with("release create")));
}

#[test]
fn no_release_for_the_tag_it_is_created_as_a_pre_release_as_for_a_tag_pushed_by_hand() {
    let r = run(&[], false);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let create = r
        .calls
        .iter()
        .find(|c| c.starts_with("release create v1.2.3"))
        .expect("a create call");
    assert!(create.contains("--verify-tag"));
    assert!(create.contains("--prerelease"));
    assert!(!any(&r.calls, |c| c.contains("--method PATCH")));
}

#[test]
fn a_published_release_for_the_tag_stops_it_and_nothing_is_created_or_edited() {
    let r = run(&[], true);
    assert_eq!(r.code, 1);
    assert!(r.stderr.contains("never re-published"), "{}", r.stderr);
    assert!(!any(&r.calls, |c| c.starts_with("release create")
        || c.starts_with("release upload")
        || c.contains("PATCH")));
}

#[test]
fn two_drafts_for_the_tag_stop_it_before_anything_is_uploaded() {
    let r = run(&["1", "2"], false);
    assert_eq!(r.code, 1);
    assert!(r.stderr.contains("More than one draft"), "{}", r.stderr);
    assert!(!any(&r.calls, |c| c.starts_with("release upload")
        || c.contains("PATCH")));
}
