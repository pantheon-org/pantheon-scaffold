//! Ported from `runner/test/workflows-c.test.ts`: properties of the consumer upgrade workflow (wave C). There is no upstream copy to
//! compare with: it is new, so its safety is asserted here. The steps that use the GitHub App are built but unverified until the App has
//! `workflows: write` (decision 83, plan P.4).

use regex::Regex;
use scaffold_package_tests::parse_yaml;
use scaffold_package_tests::pkg::{file, paths, render};
use serde_json::{Value, json};
use std::sync::LazyLock;

static CONTRACT: LazyLock<Value> = LazyLock::new(|| render("workflows-c"));
const PATH: &str = ".github/workflows/upgrade-scaffold.yml";

fn text() -> &'static str {
    file(&CONTRACT, PATH)["content"].as_str().unwrap()
}

static WORKFLOW: LazyLock<Value> =
    LazyLock::new(|| parse_yaml(text()).expect("the workflow parses"));

fn steps() -> Vec<&'static Value> {
    WORKFLOW["jobs"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|job| job["steps"].as_array().unwrap())
        .collect()
}

fn runs() -> Vec<&'static Value> {
    steps()
        .into_iter()
        .filter(|s| s.get("run").is_some())
        .collect()
}

fn run_of(step: &Value) -> &str {
    step["run"].as_str().unwrap()
}

fn step_running(needle: &str) -> &'static Value {
    runs()
        .into_iter()
        .find(|s| run_of(s).contains(needle))
        .unwrap_or_else(|| panic!("no run step has {needle}"))
}

#[test]
fn it_renders_one_owned_file_that_detach_deletes() {
    assert_eq!(paths(&CONTRACT), [PATH]);
    assert_eq!(file(&CONTRACT, PATH)["class"], "owned");
    assert_eq!(file(&CONTRACT, PATH)["removeOnDetach"], true);
}

#[test]
fn it_runs_on_a_schedule_or_by_hand_never_on_a_pull_request_or_a_push() {
    let mut keys: Vec<&str> = WORKFLOW["on"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["schedule", "workflow_dispatch"]);
    assert!(!text().contains("pull_request"));
}

#[test]
fn nothing_has_permissions_by_default_and_the_job_reads_contents_only() {
    assert_eq!(WORKFLOW["permissions"], json!({}));
    for job in WORKFLOW["jobs"].as_object().unwrap().values() {
        assert_eq!(job["permissions"], json!({ "contents": "read" }));
    }
}

#[test]
fn the_app_token_is_the_only_credential_and_it_asks_for_workflows_write_as_well_as_contents_and_pull_requests()
 {
    let secret = Regex::new(r"secrets\.(\w+)").unwrap();
    let names: Vec<&str> = secret
        .captures_iter(text())
        .map(|c| c.get(1).unwrap().as_str())
        .collect();
    assert_eq!(names, ["GH_APP_PRIVATE_KEY"]);
    let mint = steps()
        .into_iter()
        .find(|s| s["id"] == "app-token")
        .expect("the app-token step");
    for (key, value) in [
        ("permission-contents", "write"),
        ("permission-pull-requests", "write"),
        ("permission-workflows", "write"),
    ] {
        assert_eq!(mint["with"][key], value, "{key}");
    }
}

#[test]
fn every_action_is_pinned_by_a_40_character_sha_and_checkout_keeps_no_credentials() {
    let pinned = Regex::new(r"@[0-9a-f]{40}$").unwrap();
    for step in steps().into_iter().filter(|s| s.get("uses").is_some()) {
        let uses = step["uses"].as_str().unwrap();
        assert!(pinned.is_match(uses), "{uses}");
        if uses.starts_with("actions/checkout@") {
            assert_eq!(step["with"]["persist-credentials"], false);
        }
    }
}

#[test]
fn no_run_script_interpolates_an_expression_everything_goes_through_env() {
    for step in runs() {
        assert!(!run_of(step).contains("${{"), "{}", step["name"]);
    }
}

#[test]
fn every_run_script_is_15_lines_or_fewer() {
    for step in runs() {
        let lines = run_of(step).trim_end().split('\n').count();
        assert!(lines <= 15, "{}: {lines} lines", step["name"]);
    }
}

#[test]
fn it_checks_freshness_then_finds_the_latest_release_then_upgrades_in_that_order() {
    let at = |needle: &str| runs().iter().position(|s| run_of(s).contains(needle));
    let freshness = at("pantheon-scaffold freshness").expect("a freshness step");
    let latest = at("pantheon-scaffold latest").expect("a latest step");
    let upgrade = at("pantheon-scaffold upgrade").expect("an upgrade step");
    assert!(freshness < latest);
    assert!(latest < upgrade);
}

#[test]
fn a_repository_already_on_the_latest_release_opens_no_pull_request() {
    assert!(
        step_running("pantheon-scaffold upgrade")
            .get("if")
            .is_some()
    );
    assert!(step_running("gh pr create").get("if").is_some());
}

#[test]
fn the_pull_request_is_opened_with_the_app_token_never_the_default_token() {
    let pr = step_running("gh pr create");
    assert_eq!(
        pr["env"]["GH_TOKEN"],
        "${{ steps.app-token.outputs.token }}"
    );
    assert!(!text().contains("secrets.GITHUB_TOKEN"));
    assert!(!text().contains("github.token"));
}

#[test]
fn the_commit_and_the_pull_request_title_are_conventional_commits() {
    let pr = run_of(step_running("gh pr create"));
    assert!(
        Regex::new(r#"commit .*"chore: upgrade pantheon-scaffold to "#)
            .unwrap()
            .is_match(pr)
    );
    assert!(
        Regex::new(r#"--title "chore: upgrade pantheon-scaffold to "#)
            .unwrap()
            .is_match(pr)
    );
}
