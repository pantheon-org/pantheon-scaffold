//! Ported from `runner/test/update-pins-workflow.test.ts` in Phase 6b: properties of `.github/workflows/update-pins.yml`, which keeps
//! `pkl/Pins.pkl` current and publishes the freshness record (decision 83). The tests read the file, so a change that breaks one fails here.
//!
//! Phase 6b removed the TypeScript tool and the way back to it, so the tests are written against the workflow as it now is: seventeen titles
//! became fourteen ported, three rewritten (the dry run, the choice of tool and the real run) and one new (nothing names a second tool).
//! `docs/ts-tooling-test-mapping.md` has a row for each title.

use regex::Regex;
use scaffold_package_tests::workflow::{Workflow, needs, run_of, steps_of, uses_of};
use serde_json::{Value, json};
use std::sync::LazyLock;

static WF: LazyLock<Workflow> =
    LazyLock::new(|| Workflow::load(".github/workflows/update-pins.yml"));

/// The condition of everything that writes: not a manual dry run. A schedule event has no inputs, so it is not a dry run.
const NOT_DRY: &str = "${{ !(github.event_name == 'workflow_dispatch' && inputs.dry_run) }}";
const DRY: &str = "${{ github.event_name == 'workflow_dispatch' && inputs.dry_run }}";
const APP_TOKEN: &str = "${{ steps.app-token.outputs.token }}";
const RUST_WRITE: &str = "tool/scaffold-release update-pins";

fn runs() -> Vec<&'static Value> {
    WF.all_steps()
        .into_iter()
        .map(|(_, s)| s)
        .filter(|s| !run_of(s).is_empty())
        .collect()
}

fn run_index(needle: &str) -> usize {
    runs()
        .iter()
        .position(|s| run_of(s).contains(needle))
        .unwrap_or_else(|| panic!("no run step contains {needle}"))
}

/// The step that checks every pin and rewrites the ones that moved.
fn rust_write() -> &'static Value {
    runs()
        .into_iter()
        .find(|s| run_of(s).contains(RUST_WRITE) && run_of(s).contains("--freshness-out"))
        .expect("the writing step")
}

/// The step that checks every pin and writes nothing.
fn dry_run_step() -> &'static Value {
    runs()
        .into_iter()
        .find(|s| run_of(s).contains("--no-write"))
        .expect("the dry run step")
}

fn all_steps() -> Vec<&'static Value> {
    WF.all_steps().into_iter().map(|(_, s)| s).collect()
}

#[test]
fn it_runs_on_a_schedule_or_by_hand_and_on_no_push_merging_a_change_to_the_tool_must_not_start_a_real_run()
 {
    let mut keys: Vec<&str> = WF.doc["on"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["schedule", "workflow_dispatch"]);
    assert!(WF.doc["on"]["push"].is_null());
    assert!(WF.text.contains("There is no push trigger on purpose"));
    assert!(!WF.text.contains("pull_request"));
}

// Rewritten in Phase 6b: the TypeScript test also asserted the `implementation` choice. There is one tool now, so the only input is `dry_run`.
#[test]
fn a_manual_run_is_a_dry_run_unless_told_otherwise_and_dry_run_is_its_only_input() {
    let inputs = WF.doc["on"]["workflow_dispatch"]["inputs"]
        .as_object()
        .unwrap();
    assert_eq!(inputs.keys().collect::<Vec<_>>(), ["dry_run"]);
    assert_eq!(inputs["dry_run"]["type"], "boolean");
    assert_eq!(inputs["dry_run"]["default"], true);
}

#[test]
fn nothing_has_permissions_by_default_and_every_job_reads_contents_only_writes_use_the_app_token() {
    assert_eq!(WF.doc["permissions"], json!({}));
    for (name, job) in WF.jobs() {
        assert_eq!(job["permissions"], json!({ "contents": "read" }), "{name}");
    }
    let mint = all_steps()
        .into_iter()
        .find(|s| s["id"] == "app-token")
        .unwrap();
    assert_eq!(mint["with"]["permission-contents"], "write");
    assert_eq!(mint["with"]["permission-pull-requests"], "write");
    assert!(mint["with"]["permission-workflows"].is_null());
}

#[test]
fn the_only_secret_is_the_app_key_and_the_default_token_is_never_used_to_write() {
    let found: Vec<String> = Regex::new(r"secrets\.(\w+)")
        .unwrap()
        .captures_iter(&WF.text)
        .map(|c| c[1].to_owned())
        .collect();
    assert_eq!(found, ["GH_APP_PRIVATE_KEY"]);
    assert!(!WF.text.contains("secrets.GITHUB_TOKEN"));
}

#[test]
fn every_action_is_pinned_by_a_40_character_sha_and_checkout_keeps_no_credentials() {
    let pinned = Regex::new(r"@[0-9a-f]{40}$").unwrap();
    for step in all_steps().into_iter().filter(|s| !uses_of(s).is_empty()) {
        assert!(pinned.is_match(uses_of(step)), "{}", uses_of(step));
    }
    for step in all_steps()
        .into_iter()
        .filter(|s| uses_of(s).starts_with("actions/checkout@"))
    {
        assert_eq!(step["with"]["persist-credentials"], false);
    }
}

#[test]
fn no_run_script_interpolates_an_expression_and_each_is_15_lines_or_fewer() {
    for step in runs() {
        let name = &step["name"];
        assert!(!run_of(step).contains("${{"), "{name}");
        assert!(run_of(step).trim_end().lines().count() <= 15, "{name}");
    }
}

// Adjusted in Phase 6b: the TypeScript test also placed the TypeScript write step before the pull request step.
#[test]
fn the_freshness_record_is_written_only_after_the_pins_were_checked_and_the_pull_request_step_ran()
{
    let update = run_index(RUST_WRITE);
    let pr = run_index("gh pr create");
    let publish = run_index("refs/heads/freshness");
    assert!(update < pr);
    assert!(pr < publish);
}

// Adjusted in Phase 6b: one writing step, not two.
#[test]
fn the_writing_step_asks_for_the_record_and_the_summary_and_a_failing_check_fails_the_job() {
    let update = rust_write();
    let run = run_of(update);
    assert!(run.contains("--freshness-out"));
    assert!(run.contains("--summary"));
    assert!(!run.contains("--no-write"));
    assert!(!Regex::new(r"\|\|\s*true").unwrap().is_match(run));
    assert!(update["continue-on-error"].is_null());
}

#[test]
fn the_record_goes_to_the_freshness_branch_never_main() {
    let publish = run_of(runs()[run_index("refs/heads/freshness")]);
    assert!(!publish.contains("refs/heads/main"));
    assert!(publish.contains("freshness.json"));
}

#[test]
fn the_pull_request_step_does_nothing_when_no_pin_changed() {
    assert!(
        Regex::new(r"git diff --quiet")
            .unwrap()
            .is_match(run_of(runs()[run_index("gh pr create")]))
    );
}

#[test]
fn the_pull_request_is_a_conventional_commit_and_is_opened_with_the_app_token() {
    let pr = runs()[run_index("gh pr create")];
    assert!(
        Regex::new(r#"--title "chore\(deps\): "#)
            .unwrap()
            .is_match(run_of(pr))
    );
    assert_eq!(pr["env"]["GH_TOKEN"], APP_TOKEN);
}

// Phase 3 of docs/ts-tooling-rust-port.md: the update tool is the Rust crate crates/scaffold-release. Building it runs the build scripts of every
// crate it depends on, so it is built in a job that cannot write and has no token, and the update job, which holds the App token, only
// downloads the finished binary and checks its checksum before it runs it.
#[test]
fn the_release_tool_is_built_in_a_job_that_holds_no_token() {
    let tool = WF.job("tool");
    assert_eq!(tool["permissions"], json!({ "contents": "read" }));
    assert!(tool["env"].is_null());
    let secret = Regex::new(r"github\.token|secrets\.|app-token").unwrap();
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
fn the_update_job_which_holds_the_app_token_never_runs_cargo_and_downloads_the_tool_and_checks_its_checksum_before_it_runs_it()
 {
    let job = WF.job("update");
    assert!(needs(job).contains(&"tool".to_owned()));
    let cargo = Regex::new(r"\bcargo\b").unwrap();
    assert!(!steps_of(job).iter().any(|s| cargo.is_match(run_of(s))));
    let steps = steps_of(job);
    let download = steps
        .iter()
        .position(|s| {
            uses_of(s).starts_with("actions/download-artifact@")
                && s["with"]["name"] == "scaffold-release"
        })
        .expect("the download step");
    let verify = steps
        .iter()
        .position(|s| run_of(s).contains("sha256sum --check --strict"))
        .expect("the checksum step");
    assert!(verify > download);
    assert_eq!(
        steps[verify]["env"]["EXPECTED"],
        "${{ needs.tool.outputs.sha256 }}"
    );
    // Stronger than the TypeScript test (found by the breakage comparison of 6b, where a check ended in `|| true` failed neither suite): the
    // step is exactly the check and the permission to run the tool, so a check that cannot fail the job fails here.
    assert_eq!(
        run_of(&steps[verify]),
        "echo \"$EXPECTED  tool/scaffold-release\" | sha256sum --check --strict -\nchmod +x tool/scaffold-release\n"
    );
    // Also stronger (a step that ran the tool, with any argument, before the check failed neither suite): no step before the check runs the tool.
    for step in &steps[..verify] {
        assert!(
            !run_of(step).contains("tool/scaffold-release"),
            "{}",
            step["name"]
        );
    }
    let first_run = steps
        .iter()
        .position(|s| run_of(s).contains(RUST_WRITE))
        .unwrap();
    assert!(first_run > verify);
}

#[test]
fn no_job_has_a_job_wide_token_in_its_environment_each_step_that_needs_one_names_it() {
    for (name, job) in WF.jobs() {
        assert!(job["env"]["GITHUB_TOKEN"].is_null(), "{name}");
    }
}

// Adjusted in Phase 6b: the TypeScript write step is not among the writers any more.
#[test]
fn a_dry_run_reaches_nothing_that_writes_no_app_token_no_write_step_no_pull_request_no_freshness_record()
 {
    let mint = all_steps()
        .into_iter()
        .find(|s| s["id"] == "app-token")
        .unwrap();
    assert_eq!(mint["if"], NOT_DRY);
    for step in [
        rust_write(),
        runs()[run_index("gh pr create")],
        runs()[run_index("refs/heads/freshness")],
    ] {
        assert_eq!(step["if"], NOT_DRY, "{}", step["name"]);
    }
    let dry = dry_run_step();
    assert_eq!(dry["if"], DRY);
    assert_eq!(dry["env"]["GITHUB_TOKEN"], "${{ github.token }}");
}

// Rewritten in Phase 6b. The TypeScript test said a dry run runs both tools and fails when they disagree. There is one tool now: a dry run runs
// it with `--no-write`, as one command whose exit code is the step's, with the read-only token and nothing else.
#[test]
fn a_dry_run_runs_the_one_tool_with_no_write_and_the_step_fails_with_the_tool() {
    let dry = dry_run_step();
    assert_eq!(run_of(dry), "tool/scaffold-release update-pins --no-write");
    assert!(dry["continue-on-error"].is_null());
    assert_eq!(dry["env"], json!({ "GITHUB_TOKEN": "${{ github.token }}" }));
    assert_eq!(
        runs()
            .into_iter()
            .filter(|s| run_of(s).contains("--no-write"))
            .count(),
        1
    );
}

// Rewritten in Phase 6b. The TypeScript test said a real run uses the Rust tool unless asked for the TypeScript one, and that mise is installed
// only for the TypeScript tool. A real run uses the one tool, with the App token, and no step installs mise.
#[test]
fn a_real_run_uses_the_one_tool_with_the_app_token_and_no_step_installs_mise() {
    let write = rust_write();
    assert_eq!(write["if"], NOT_DRY);
    assert_eq!(write["env"], json!({ "GITHUB_TOKEN": APP_TOKEN }));
    assert_eq!(
        run_of(write),
        "tool/scaffold-release update-pins \\\n  --freshness-out \"$RUNNER_TEMP/freshness.json\" --summary \"$RUNNER_TEMP/summary.md\"\n"
    );
    for step in all_steps() {
        assert!(
            !uses_of(step).starts_with("jdx/mise-action@"),
            "{}",
            step["name"]
        );
        assert!(!run_of(step).contains("mise "), "{}", step["name"]);
    }
}

// New in Phase 6b: nothing names a second tool, a choice between tools or the bun runtime, so the way back to the TypeScript tool is gone
// from the workflow and a change that brings it back fails here.
#[test]
fn there_is_one_tool_nothing_chooses_it_and_nothing_names_a_second_one_or_bun() {
    let named =
        Regex::new(r"implementation|update-pins\.ts|\bbun\b|\bmise\b|disagree|typescript").unwrap();
    let lines: Vec<&str> = WF.text.lines().filter(|l| named.is_match(l)).collect();
    assert!(
        lines.is_empty(),
        "the workflow still names a second tool: {lines:?}"
    );
}
