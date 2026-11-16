//! Ported from `runner/test/phase5-dist.test.ts`. Phase 5 wave D: dist-check.yml (the drift check of the generated release.yml, `binary`) and
//! scaffold-check.yml (`check` in consumer CI, every repository). Neither exists in the four repositories yet, so dist-check is asserted by
//! structure and scaffold-check is compared with the consumer-check sample that docs/check-in-ci.md describes.

use regex::Regex;
use scaffold_package_tests::pkg::{content, expect_pins, file, read, render, unpin};
use scaffold_package_tests::{compare_directives, compare_workflows, parse_yaml};
use serde_json::{Value, json};
use std::sync::LazyLock;

static ADR: LazyLock<Value> = LazyLock::new(|| render("real-workflows-pantheon-adr"));
static KIT: LazyLock<Value> =
    LazyLock::new(|| render("real-workflows-pantheon-agentic-harness-dev-kit"));
const DIST: &str = ".github/workflows/dist-check.yml";
const CHECK: &str = ".github/workflows/scaffold-check.yml";

fn has_file(contract: &Value, path: &str) -> bool {
    contract["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"] == path)
}

fn steps_of(text: &str) -> Vec<Value> {
    parse_yaml(text).expect("the workflow parses")["jobs"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|job| job["steps"].as_array().unwrap().clone())
        .collect()
}

#[test]
fn dist_check_yml_is_rendered_for_a_repository_with_a_binary_and_not_for_one_without() {
    assert!(has_file(&ADR, DIST));
    assert!(!has_file(&KIT, DIST));
}

#[test]
fn scaffold_check_yml_is_rendered_for_every_repository() {
    assert!(has_file(&ADR, CHECK));
    assert!(has_file(&KIT, CHECK));
}

#[test]
fn the_dist_version_the_check_installs_is_the_one_dist_workspace_toml_names() {
    let toml = content(&ADR, "dist-workspace.toml");
    let version = &Regex::new(r#"cargo-dist-version = "([^"]+)""#)
        .unwrap()
        .captures(toml)
        .expect("a dist version")[1];
    assert!(content(&ADR, DIST).contains(&format!("DIST_VERSION: {version}")));
}

#[test]
fn dist_check_yml_runs_dist_generate_check_on_a_pull_request_a_push_to_main_or_by_hand_and_never_on_pull_request_target()
 {
    let text = content(&ADR, DIST);
    let parsed = parse_yaml(text).unwrap();
    let mut triggers: Vec<&str> = parsed["on"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    triggers.sort_unstable();
    assert_eq!(triggers, ["pull_request", "push", "workflow_dispatch"]);
    assert!(!text.contains("pull_request_target"));
    assert!(steps_of(text).iter().any(|s| {
        s["run"]
            .as_str()
            .is_some_and(|r| r.contains("dist generate --check"))
    }));
    let paths = parsed["on"]["pull_request"]["paths"].as_array().unwrap();
    assert!(paths.contains(&json!("dist-workspace.toml")));
    assert!(paths.contains(&json!(".github/workflows/release.yml")));
}

#[test]
fn dist_check_yml_uses_no_secrets_reads_contents_only_pins_every_action_and_keeps_no_checkout_credentials()
 {
    let text = content(&ADR, DIST);
    assert!(!Regex::new(r"secrets\.").unwrap().is_match(text));
    assert_eq!(
        parse_yaml(text).unwrap()["permissions"],
        json!({ "contents": "read" })
    );
    let pinned = Regex::new(r"@[0-9a-f]{40}$").unwrap();
    for step in steps_of(text) {
        if let Some(uses) = step["uses"].as_str() {
            assert!(pinned.is_match(uses), "{uses}");
            if uses.starts_with("actions/checkout@") {
                assert_eq!(step["with"]["persist-credentials"], false);
            }
        }
        if let Some(run) = step["run"].as_str() {
            assert!(!run.contains("${{"), "{}", step["name"]);
            assert!(run.trim_end().split('\n').count() <= 15, "{}", step["name"]);
        }
    }
}

#[test]
fn scaffold_check_yml_equals_the_consumer_check_sample_under_p1_with_the_pins_masked_and_the_directive_comments()
 {
    let sample = read("runner/test/fixtures/consumer-check.yml");
    let mine = content(&ADR, CHECK);
    let result = compare_workflows(&unpin(&sample), &unpin(mine)).unwrap();
    assert!(result.equal, "{:?}", result.differences);
    assert!(result.duplicate_keys.is_empty());
    assert_eq!(compare_directives(&sample, mine), Vec::<String>::new());
}

#[test]
fn scaffold_check_yml_is_the_same_for_every_repository() {
    assert_eq!(file(&KIT, CHECK)["content"], file(&ADR, CHECK)["content"]);
}

#[test]
fn the_pins_the_two_workflows_use_are_the_cache_checkout_mise_and_nothing_unlisted() {
    let wanted = ["actions/cache", "actions/checkout", "jdx/mise-action"];
    let used: Vec<Value> = ADR["pins"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| wanted.contains(&p["action"].as_str().unwrap()))
        .cloned()
        .collect();
    expect_pins(&Value::Array(used), &wanted);
}
