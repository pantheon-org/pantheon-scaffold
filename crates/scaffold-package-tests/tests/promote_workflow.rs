//! Ported from `runner/test/promote-workflow.test.ts`: properties of `.github/workflows/promote.yml` that make promoting safe. They are read from
//! the file, so a change that breaks one fails here.

use regex::Regex;
use scaffold_package_tests::workflow::{Workflow, needs, run_of, steps_of, uses_of, writes};
use serde_json::{Value, json};
use std::sync::LazyLock;

static WF: LazyLock<Workflow> = LazyLock::new(|| Workflow::load(".github/workflows/promote.yml"));

/// How a run asks the tool to edit a release.
const RUST_EDIT: &str = "tool/scaffold-release promote promote \"$TAG\"";

#[test]
fn it_runs_by_hand_only_never_on_a_push_a_pull_request_or_pull_request_target() {
    let keys: Vec<&str> = WF.doc["on"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["workflow_dispatch"]);
    assert!(!WF.text.contains("pull_request_target"));
}

#[test]
fn it_takes_a_tag_and_is_a_dry_run_unless_told_otherwise() {
    let inputs = &WF.doc["on"]["workflow_dispatch"]["inputs"];
    assert_eq!(inputs["tag"]["type"], "string");
    assert_eq!(inputs["tag"]["required"], true);
    assert_eq!(inputs["dry_run"]["type"], "boolean");
    assert_eq!(inputs["dry_run"]["default"], true);
}

#[test]
fn nothing_has_permissions_by_default_and_every_job_asks_for_its_own() {
    assert_eq!(WF.doc["permissions"], json!({}));
    for (name, job) in WF.jobs() {
        assert!(!job["permissions"].is_null(), "job {name}");
    }
}

#[test]
fn only_the_promote_job_can_write_and_it_writes_only_contents() {
    for (name, job) in WF.jobs() {
        let want: Vec<(String, String)> = if name == "promote" {
            vec![("contents".into(), "write".into())]
        } else {
            vec![]
        };
        assert_eq!(writes(job), want, "job {name}");
    }
}

#[test]
fn the_promote_job_sits_behind_the_protected_environment_and_the_dry_run_job_never_touches_it() {
    assert_eq!(WF.job("promote")["environment"], "promote");
    assert!(WF.job("guards")["environment"].is_null());
}

#[test]
fn the_promote_job_runs_only_after_the_tool_and_guards_jobs_and_only_when_the_run_is_not_a_dry_run()
{
    let mut after = needs(WF.job("promote"));
    after.sort();
    assert_eq!(after, ["guards", "tool"]);
    assert_eq!(WF.job("promote")["if"], "inputs.dry_run == false");
}

#[test]
fn the_guards_job_checks_with_the_release_tool_and_only_the_promote_job_can_ask_it_to_edit() {
    let guards: Vec<&str> = WF.steps("guards").iter().map(run_of).collect();
    assert!(
        guards
            .join("\n")
            .contains("tool/scaffold-release promote check \"$TAG\"")
    );
    let edits: Vec<(&str, &str)> = WF
        .all_steps()
        .into_iter()
        .filter(|(_, s)| run_of(s).contains("promote promote"))
        .map(|(job, s)| (job, run_of(s)))
        .collect();
    assert_eq!(edits, [("promote", RUST_EDIT)]);
}

// Phase 6 of docs/ts-tooling-rust-port.md: the TypeScript promote tool and the way back to it are gone. There is one tool, so there is no input
// that chooses one, nothing that compares two, and no step that installs bun or mise for a TypeScript run.
#[test]
fn there_is_one_tool_no_input_chooses_it_no_step_compares_two_and_nothing_installs_bun_or_mise() {
    let inputs: Vec<&str> = WF.doc["on"]["workflow_dispatch"]["inputs"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(inputs, ["tag", "dry_run"]);
    assert!(
        !Regex::new(r"implementation|promote\.ts|\bbun\b|\bmise\b|disagree")
            .unwrap()
            .is_match(&WF.text)
    );
    assert!(
        WF.all_steps()
            .iter()
            .all(|(_, s)| !uses_of(s).starts_with("jdx/mise-action@"))
    );
    let check = WF
        .steps("guards")
        .iter()
        .find(|s| run_of(s).contains("promote check"))
        .expect("the check step");
    assert_eq!(
        run_of(check),
        "tool/scaffold-release promote check \"$TAG\""
    );
}

#[test]
fn the_release_tool_is_built_in_a_job_that_cannot_write_and_holds_no_token() {
    let tool = WF.job("tool");
    assert_eq!(tool["permissions"], json!({ "contents": "read" }));
    assert!(tool["env"].is_null());
    let secret = Regex::new(r"github\.token|secrets\.").unwrap();
    for step in steps_of(tool) {
        assert!(!secret.is_match(&step.to_string()), "{}", step["name"]);
    }
    assert!(
        steps_of(tool)
            .iter()
            .any(|s| run_of(s).contains("cargo build --release --locked -p scaffold-release"))
    );
    let upload = steps_of(tool)
        .iter()
        .find(|s| uses_of(s).starts_with("actions/upload-artifact@"))
        .unwrap();
    assert_eq!(upload["with"]["name"], "scaffold-release");
}

#[test]
fn guards_and_promote_download_the_release_tool_and_check_its_checksum_before_they_run_it() {
    for name in ["guards", "promote"] {
        let job = WF.job(name);
        assert!(needs(job).contains(&"tool".to_owned()), "{name}");
        let steps = steps_of(job);
        let download = steps
            .iter()
            .position(|s| {
                uses_of(s).starts_with("actions/download-artifact@")
                    && s["with"]["name"] == "scaffold-release"
            })
            .unwrap_or_else(|| panic!("{name} downloads the tool"));
        let verify = steps
            .iter()
            .position(|s| run_of(s).contains("sha256sum --check --strict"))
            .unwrap_or_else(|| panic!("{name} checks its checksum"));
        assert!(verify > download, "{name}");
        assert_eq!(
            steps[verify]["env"]["EXPECTED"], "${{ needs.tool.outputs.sha256 }}",
            "{name}"
        );
        let first_run = steps
            .iter()
            .position(|s| run_of(s).contains("tool/scaffold-release promote"))
            .unwrap_or_else(|| panic!("{name} runs it"));
        assert!(first_run > verify, "{name}");
    }
}

#[test]
fn no_job_that_can_write_runs_cargo() {
    let cargo = Regex::new(r"\bcargo\b").unwrap();
    for (name, job) in WF.jobs() {
        let can_write = job["permissions"]
            .as_object()
            .is_some_and(|p| p.values().any(|v| v == "write"));
        let runs_cargo = steps_of(job).iter().any(|s| cargo.is_match(run_of(s)));
        assert!(!(can_write && runs_cargo), "job {name}");
    }
}

#[test]
fn no_job_has_a_job_wide_token_in_its_environment_each_step_that_needs_one_names_it() {
    for (name, job) in WF.jobs() {
        assert!(job["env"]["GITHUB_TOKEN"].is_null(), "{name}");
    }
}

#[test]
fn every_action_is_pinned_by_a_40_character_sha() {
    let pinned = Regex::new(r"@[0-9a-f]{40}$").unwrap();
    let uses: Vec<&str> = WF
        .all_steps()
        .iter()
        .map(|(_, s)| uses_of(s))
        .filter(|u| !u.is_empty() && !u.starts_with("./"))
        .collect();
    // checkout in three jobs, the tool upload, and the tool download in two jobs.
    assert_eq!(uses.len(), 6);
    for u in uses {
        assert!(pinned.is_match(u), "{u}");
    }
}

#[test]
fn it_uses_no_secrets_and_persists_no_checkout_credentials() {
    assert!(!Regex::new(r"secrets\.").unwrap().is_match(&WF.text));
    for (job, step) in WF.all_steps() {
        if uses_of(step).starts_with("actions/checkout@") {
            assert_eq!(step["with"]["persist-credentials"], false, "{job}");
        }
    }
}

#[test]
fn no_run_script_interpolates_text_a_dispatcher_controls_the_tag_goes_through_env() {
    for (job, step) in WF.all_steps() {
        if !run_of(step).is_empty() {
            assert!(!run_of(step).contains("${{"), "{job}: {}", step["name"]);
        }
        if run_of(step).contains("scaffold-release promote") {
            assert_eq!(
                step["env"]["TAG"], "${{ inputs.tag }}",
                "{job}: {}",
                step["name"]
            );
        }
    }
}

#[test]
fn runs_never_overlap_and_are_never_cancelled_half_way() {
    assert_eq!(
        WF.doc["concurrency"]["cancel-in-progress"],
        Value::Bool(false)
    );
}
