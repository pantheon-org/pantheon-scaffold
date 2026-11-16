//! Ported from `runner/test/release-bump.test.ts`: release-bump.yml and release-bump.sh. After a verified release they open and merge the
//! version bump. The workflow properties are read from the file; the script runs against a stand-in `gh` that records its calls.

use regex::Regex;
use scaffold_package_tests::b64;
use scaffold_package_tests::parse_yaml;
use scaffold_package_tests::pkg::{read, root};
use scaffold_package_tests::repo::{temp_dir, write_script};
use serde_json::{Value, json};
use std::process::Command;
use std::sync::LazyLock;

static WORKFLOW_TEXT: LazyLock<String> =
    LazyLock::new(|| read(".github/workflows/release-bump.yml"));
static WORKFLOW: LazyLock<Value> =
    LazyLock::new(|| parse_yaml(&WORKFLOW_TEXT).expect("the workflow parses"));

fn job() -> &'static Value {
    &WORKFLOW["jobs"]["bump"]
}

fn steps() -> &'static Vec<Value> {
    job()["steps"].as_array().unwrap()
}

#[test]
fn it_runs_only_after_the_release_workflow_completed_and_only_for_a_successful_tag_push() {
    let keys: Vec<&str> = WORKFLOW["on"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["workflow_run"]);
    assert_eq!(
        WORKFLOW["on"]["workflow_run"],
        json!({ "workflows": ["release"], "types": ["completed"] })
    );
    let condition = job()["if"].as_str().unwrap();
    assert!(condition.contains("workflow_run.conclusion == 'success'"));
    assert!(condition.contains("workflow_run.event == 'push'"));
    assert!(!WORKFLOW_TEXT.contains("pull_request"));
}

#[test]
fn nothing_has_permissions_by_default_the_job_reads_contents_and_the_app_token_can_write_only_contents_and_pull_requests()
 {
    assert_eq!(WORKFLOW["permissions"], json!({}));
    assert_eq!(job()["permissions"], json!({ "contents": "read" }));
    let mint = steps()
        .iter()
        .find(|s| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.starts_with("actions/create-github-app-token@"))
        })
        .expect("the app token step");
    let mut permissions: Vec<&str> = mint["with"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .filter(|k| k.starts_with("permission-"))
        .collect();
    permissions.sort_unstable();
    assert_eq!(
        permissions,
        ["permission-contents", "permission-pull-requests"]
    );
}

#[test]
fn it_checks_out_main_without_credentials_pins_every_action_by_sha_and_passes_event_text_through_env_only()
 {
    let checkout = steps()
        .iter()
        .find(|s| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.starts_with("actions/checkout@"))
        })
        .expect("the checkout step");
    assert_eq!(checkout["with"]["ref"], "main");
    assert_eq!(checkout["with"]["persist-credentials"], false);
    let pinned = Regex::new(r"@[0-9a-f]{40}( |$)").unwrap();
    for step in steps().iter().filter(|s| s.get("uses").is_some()) {
        let uses = step["uses"].as_str().unwrap();
        assert!(pinned.is_match(uses), "{uses}");
    }
    for step in steps().iter().filter(|s| s.get("run").is_some()) {
        assert!(!step["run"].as_str().unwrap().contains("${{"));
    }
    let run = steps().iter().find(|s| s.get("run").is_some()).unwrap();
    assert_eq!(
        run["env"]["TAG"],
        "${{ github.event.workflow_run.head_branch }}"
    );
    assert_eq!(
        run["env"]["TAG_SHA"],
        "${{ github.event.workflow_run.head_sha }}"
    );
}

#[derive(Default)]
struct Case {
    tag: &'static str,
    head_sha: Option<&'static str>,
    file_version: &'static str,
    checks: Option<&'static str>,
    open_pr: bool,
    path: Option<&'static str>,
    file_text: Option<&'static str>,
    fail_pr_create: bool,
}

struct Outcome {
    code: i32,
    calls: String,
    stderr: String,
}

fn run_script(case: Case) -> Outcome {
    let holder = temp_dir();
    let dir = holder.path();
    let path = case.path.unwrap_or("pkl/PklProject");
    std::fs::create_dir_all(dir.join(path).parent().unwrap()).unwrap();
    let file = case.file_text.map(str::to_owned).unwrap_or_else(|| {
        format!("@ModuleInfo {{ minPklVersion = \"0.32.0\" }}\npackage {{\n  version = \"{}\" // x-release-please-version\n}}\n", case.file_version)
    });
    std::fs::write(dir.join(path), &file).unwrap();
    // The script is rendered for pkl/PklProject; a test of another path renders it again with that path.
    let original = root().join(".github/scripts/release-bump.sh");
    let script = if let Some(custom) = case.path {
        let copy = dir.join("release-bump.sh");
        let text = std::fs::read_to_string(&original).unwrap().replacen(
            "version_file='pkl/PklProject'",
            &format!("version_file='{custom}'"),
            1,
        );
        std::fs::write(&copy, text).unwrap();
        copy
    } else {
        original
    };
    let log = dir.join("calls.log");
    let gh = dir.join("gh");
    write_script(
        &gh,
        &format!(
            "#!/usr/bin/env bash\necho \"$*\" >> \"{log}\"\ncase \"$1 $2\" in\n  \"api repos/o/r/git/ref/tags/\"*) echo '{{\"object\":{{\"sha\":\"abc123\",\"type\":\"commit\"}}}}' ;;\n  \"api repos/o/r/git/ref/heads/main\") echo \"mainsha\" ;;\n  \"api repos/o/r/contents/\"*\"?ref=\"*) echo '{{\"sha\":\"blob1\",\"content\":\"{b64}\"}}' ;;\n  \"pr list\") {pr_list} ;;\n  \"pr create\") {pr_create} ;;\n  \"pr checks\") echo '{checks}' ;;\nesac\nexit 0\n",
            log = log.display(),
            b64 = b64::encode(file.as_bytes()),
            pr_list = if case.open_pr { "echo 41" } else { "true" },
            pr_create = if case.fail_pr_create {
                "exit 1"
            } else {
                "echo \"https://github.com/o/r/pull/50\""
            },
            checks = case.checks.unwrap_or("[\"pass\",\"pass\"]"),
        ),
    );
    let output = Command::new("bash")
        .arg(&script)
        .env_clear()
        .env(
            "PATH",
            format!("{}:{}", dir.display(), std::env::var("PATH").unwrap()),
        )
        .env("GH_REPO", "o/r")
        .env("TAG", case.tag)
        .env("TAG_SHA", case.head_sha.unwrap_or("abc123"))
        .env("POLL_SECONDS", "0")
        .env("MAX_POLLS", "3")
        .env("GITHUB_STEP_SUMMARY", dir.join("summary"))
        .current_dir(dir)
        .output()
        .expect("bash runs");
    Outcome {
        code: output.status.code().unwrap_or(-1),
        calls: std::fs::read_to_string(&log).unwrap_or_default(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn put_call(calls: &str) -> &str {
    calls
        .split('\n')
        .find(|c| c.contains("--method PUT"))
        .expect("a PUT call")
}

fn sent_content(put: &str) -> String {
    let content = Regex::new(r"content=(\S+)")
        .unwrap()
        .captures(put)
        .expect("a content field")[1]
        .to_owned();
    String::from_utf8(b64::decode(&content)).unwrap()
}

#[test]
fn v0_3_0_released_and_main_still_at_0_3_0_the_next_patch_is_put_in_a_branch_a_pull_request_is_opened_and_it_is_merged_when_checks_pass()
 {
    let r = run_script(Case {
        tag: "v0.3.0",
        file_version: "0.3.0",
        ..Case::default()
    });
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.calls.contains("refs/heads/chore/start-0.3.1"));
    let put = put_call(&r.calls);
    assert!(put.contains("branch=chore/start-0.3.1"));
    assert!(sent_content(put).contains("version = \"0.3.1\" // x-release-please-version"));
    assert!(r.calls.contains("pr create"));
    assert!(r.calls.contains("--title chore(pkl): start 0.3.1"));
    assert!(
        r.calls
            .contains("pr merge https://github.com/o/r/pull/50 --squash --delete-branch")
    );
}

#[test]
fn the_patch_carries_over_two_digits_v1_2_9_gives_1_2_10() {
    let r = run_script(Case {
        tag: "v1.2.9",
        file_version: "1.2.9",
        ..Case::default()
    });
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.calls.contains("chore/start-1.2.10"));
}

#[test]
fn a_failing_check_leaves_the_pull_request_open_and_fails_the_run() {
    let r = run_script(Case {
        tag: "v0.3.0",
        file_version: "0.3.0",
        checks: Some("[\"pass\",\"fail\"]"),
        ..Case::default()
    });
    assert_eq!(r.code, 1);
    assert!(r.stderr.contains("stays open"), "{}", r.stderr);
    assert!(!r.calls.contains("pr merge"));
}

#[test]
fn no_check_yet_is_not_green_it_waits_then_gives_up_without_merging() {
    let r = run_script(Case {
        tag: "v0.3.0",
        file_version: "0.3.0",
        checks: Some("[]"),
        ..Case::default()
    });
    assert_eq!(r.code, 1);
    assert!(r.stderr.contains("did not finish"), "{}", r.stderr);
    assert!(!r.calls.contains("pr merge"));
}

#[test]
fn main_already_above_the_released_version_a_visible_notice_and_no_branch() {
    let r = run_script(Case {
        tag: "v0.3.0",
        file_version: "0.3.1",
        ..Case::default()
    });
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(!r.calls.contains("git/refs"));
    assert!(!r.calls.contains("pr create"));
}

#[test]
fn main_below_the_released_version_a_tag_that_is_not_the_runs_commit_and_a_pre_release_tag_are_each_stopped_or_skipped_without_a_branch()
 {
    let below = run_script(Case {
        tag: "v0.3.0",
        file_version: "0.2.9",
        ..Case::default()
    });
    assert_eq!(below.code, 1);
    let spoof = run_script(Case {
        tag: "v0.3.0",
        file_version: "0.3.0",
        head_sha: Some("other"),
        ..Case::default()
    });
    assert_eq!(spoof.code, 1);
    assert!(spoof.stderr.contains("No bump"), "{}", spoof.stderr);
    let rc = run_script(Case {
        tag: "v0.4.0-rc1",
        file_version: "0.3.0",
        ..Case::default()
    });
    assert_eq!(rc.code, 0, "{}", rc.stderr);
    for r in [&below, &spoof, &rc] {
        assert!(!r.calls.contains("git/refs"));
    }
}

#[test]
fn an_open_pull_request_for_the_same_branch_is_not_duplicated() {
    let r = run_script(Case {
        tag: "v0.3.0",
        file_version: "0.3.0",
        open_pr: true,
        ..Case::default()
    });
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(!r.calls.contains("pr create"));
}

#[test]
fn a_version_file_with_a_space_and_regex_characters_in_its_path_is_read_fetched_and_written_with_the_path_encoded()
 {
    let r = run_script(Case {
        tag: "v0.3.0",
        file_version: "0.3.0",
        path: Some("a b/v[1].(x)+.toml"),
        ..Case::default()
    });
    assert_eq!(r.stderr, "");
    assert_eq!(r.code, 0);
    assert!(
        r.calls
            .contains("contents/a%20b/v%5B1%5D.%28x%29%2B.toml?ref=chore/start-0.3.1")
    );
    assert!(sent_content(put_call(&r.calls)).contains("version = \"0.3.1\""));
}

#[test]
fn a_version_file_with_no_version_line_or_with_two_is_refused_before_anything_is_created() {
    for file_text in [
        "package {\n  name = \"x\"\n}\n",
        "a {\n  version = \"0.3.0\"\n}\nb {\n  version = \"0.3.0\"\n}\n",
    ] {
        let r = run_script(Case {
            tag: "v0.3.0",
            file_version: "0.3.0",
            file_text: Some(file_text),
            ..Case::default()
        });
        assert_eq!(r.code, 1, "{file_text}");
        assert!(r.stderr.contains("exactly one line"), "{}", r.stderr);
        assert!(!r.calls.contains("git/refs"), "{file_text}");
    }
}

#[test]
fn a_failure_after_the_branch_is_made_and_before_the_pull_request_is_open_deletes_the_branch_again()
{
    let r = run_script(Case {
        tag: "v0.3.0",
        file_version: "0.3.0",
        fail_pr_create: true,
        ..Case::default()
    });
    assert_ne!(r.code, 0);
    assert!(
        r.calls
            .contains("--method DELETE repos/o/r/git/refs/heads/chore/start-0.3.1")
    );
}

#[test]
fn a_failure_after_the_pull_request_is_open_leaves_the_branch_alone() {
    let r = run_script(Case {
        tag: "v0.3.0",
        file_version: "0.3.0",
        checks: Some("[\"fail\"]"),
        ..Case::default()
    });
    assert_eq!(r.code, 1);
    assert!(!r.calls.contains("--method DELETE"));
}
