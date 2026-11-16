//! Ported from `runner/test/consumer-check.test.ts`: properties of the sample consumer check workflow that make it safe to run.

use regex::Regex;
use scaffold_package_tests::parse_yaml;
use scaffold_package_tests::pkg::read;
use serde_json::{Value, json};

const PATH: &str = "runner/test/fixtures/consumer-check.yml";

fn text() -> String {
    read(PATH)
}

fn workflow() -> Value {
    parse_yaml(&text()).expect("the workflow parses")
}

#[test]
fn the_consumer_check_runs_on_pull_request_only_never_pull_request_target() {
    let wf = workflow();
    let keys: Vec<&str> = wf["on"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["pull_request"]);
    assert!(!text().contains("pull_request_target"));
}

#[test]
fn the_consumer_check_uses_no_secrets() {
    assert!(!Regex::new(r"secrets\.").unwrap().is_match(&text()));
}

#[test]
fn the_token_is_the_built_in_one_with_read_access_only() {
    let wf = workflow();
    assert_eq!(wf["permissions"], json!({}));
    assert_eq!(
        wf["jobs"]["check"]["permissions"],
        json!({ "contents": "read" })
    );
    assert_eq!(
        wf["jobs"]["check"]["env"]["GITHUB_TOKEN"],
        "${{ github.token }}"
    );
}

#[test]
fn every_action_is_pinned_by_a_40_character_sha() {
    let uses_line = Regex::new(r"^\s*-?\s*uses:").unwrap();
    let pinned = Regex::new(r"@[0-9a-f]{40} # v").unwrap();
    let text = text();
    let uses: Vec<&str> = text.lines().filter(|l| uses_line.is_match(l)).collect();
    assert!(!uses.is_empty());
    for line in uses {
        assert!(pinned.is_match(line), "{line}");
    }
}

#[test]
fn checkout_does_not_persist_credentials() {
    assert!(text().contains("persist-credentials: false"));
}

#[test]
fn the_tools_come_from_the_lock_file() {
    assert!(text().contains("mise install --locked"));
}

#[test]
fn the_package_cache_is_kept_between_runs_so_an_outage_does_not_block_the_check() {
    let text = text();
    assert!(text.contains("path: .pkl-gen/cache"));
    assert!(
        Regex::new(r"uses: actions/cache@[0-9a-f]{40} # v\d+\.\d+\.\d+")
            .unwrap()
            .is_match(&text)
    );
}
