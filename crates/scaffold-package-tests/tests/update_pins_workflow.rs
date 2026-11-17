//! Properties of `.github/workflows/update-pins.yml`, which keeps `pkl/Pins.pkl` current, renders the repository so that a pin pull request passes its checks
//! with no commit from a person, and publishes the freshness record (decision 83). The tests read the file, so a change that breaks one fails here.
//!
//! The workflow has three jobs so that nothing that runs a tool or pkl sits next to the write token: `tool` builds with no token, `pins` updates and renders
//! with the read-only token and packs the result, and `update` holds the write token and only checks the tar against `main`, applies it and pushes. `check` is
//! the dry-run twin of `update`. The tests that pinned "one tool, no mise" were rescoped in #162 to the `update` job, where the reason for them still holds.

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
const APPLY: &str = ".github/scripts/apply-pin-update.sh";
const COLLECT: &str = ".github/scripts/collect-pin-update.sh";
const OPEN_PR: &str = ".github/scripts/open-pin-pull-request.sh";

fn steps(job: &str) -> &'static [Value] {
    steps_of(WF.job(job))
}

fn index_of(job: &str, needle: &str) -> usize {
    steps(job)
        .iter()
        .position(|s| run_of(s).contains(needle))
        .unwrap_or_else(|| panic!("no step of {job} runs {needle}"))
}

fn step_named<'a>(job: &'a str, id: &str) -> &'a Value {
    steps(job)
        .iter()
        .find(|s| s["id"] == id)
        .unwrap_or_else(|| panic!("no step {id} in {job}"))
}

fn all_runs() -> Vec<&'static Value> {
    WF.all_steps()
        .into_iter()
        .map(|(_, s)| s)
        .filter(|s| !run_of(s).is_empty())
        .collect()
}

fn all_steps() -> Vec<&'static Value> {
    WF.all_steps().into_iter().map(|(_, s)| s).collect()
}

fn text_of(job: &str) -> String {
    WF.job(job).to_string()
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
}

#[test]
fn the_app_token_asks_for_contents_pull_requests_and_workflows_and_for_nothing_else() {
    let mint = step_named("update", "app-token");
    assert_eq!(
        uses_of(mint).split('@').next(),
        Some("actions/create-github-app-token")
    );
    let asked: Vec<&String> = mint["with"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|k| k.starts_with("permission-"))
        .collect();
    assert_eq!(
        asked,
        [
            "permission-contents",
            "permission-pull-requests",
            "permission-workflows"
        ]
    );
    for key in asked {
        assert_eq!(mint["with"][key], "write", "{key}");
    }
}

#[test]
fn the_only_secret_is_the_app_key_it_is_read_in_the_update_job_only_and_the_default_token_never_writes()
 {
    let found: Vec<String> = Regex::new(r"secrets\.(\w+)")
        .unwrap()
        .captures_iter(&WF.text)
        .map(|c| c[1].to_owned())
        .collect();
    assert_eq!(found, ["GH_APP_PRIVATE_KEY"]);
    assert!(text_of("update").contains("secrets.GH_APP_PRIVATE_KEY"));
    for job in ["tool", "pins", "check"] {
        assert!(!text_of(job).contains("secrets."), "{job}");
    }
    assert!(!WF.text.contains("secrets.GITHUB_TOKEN"));
}

#[test]
fn every_action_is_pinned_by_a_40_character_sha_and_checkout_keeps_no_credentials() {
    let pinned = Regex::new(r"@[0-9a-f]{40}$").unwrap();
    for step in all_steps().into_iter().filter(|s| !uses_of(s).is_empty()) {
        assert!(pinned.is_match(uses_of(step)), "{}", uses_of(step));
    }
    let checkouts: Vec<&Value> = all_steps()
        .into_iter()
        .filter(|s| uses_of(s).starts_with("actions/checkout@"))
        .collect();
    assert_eq!(
        checkouts.len(),
        4,
        "tool, pins, check and update each check out"
    );
    for step in checkouts {
        assert_eq!(step["with"]["persist-credentials"], false);
    }
}

#[test]
fn no_run_script_interpolates_an_expression_and_each_is_15_lines_or_fewer() {
    for step in all_runs() {
        let name = &step["name"];
        assert!(!run_of(step).contains("${{"), "{name}");
        assert!(run_of(step).trim_end().lines().count() <= 15, "{name}");
    }
}

#[test]
fn every_checkout_is_of_the_commit_the_run_started_on_so_the_jobs_see_one_main() {
    for job in ["tool", "pins", "check", "update"] {
        let checkout = steps(job)
            .iter()
            .find(|s| uses_of(s).starts_with("actions/checkout@"))
            .unwrap();
        assert_eq!(checkout["with"]["ref"], "${{ github.sha }}", "{job}");
    }
}

// The jobs and their order.

#[test]
fn the_jobs_are_tool_then_pins_then_check_or_update_and_nothing_else() {
    let mut names: Vec<&str> = WF.jobs().into_iter().map(|(n, _)| n).collect();
    names.sort_unstable();
    assert_eq!(names, ["check", "pins", "tool", "update"]);
    assert!(needs(WF.job("tool")).is_empty());
    assert_eq!(needs(WF.job("pins")), ["tool"]);
    assert_eq!(needs(WF.job("check")), ["pins"]);
    assert_eq!(needs(WF.job("update")), ["pins"]);
}

#[test]
fn a_dry_run_reaches_nothing_that_writes_no_app_token_no_environment_no_push_no_pull_request_no_freshness_record()
 {
    assert_eq!(WF.job("check")["if"], DRY);
    assert_eq!(WF.job("update")["if"], NOT_DRY);
    for job in ["tool", "pins", "check"] {
        let text = text_of(job);
        for banned in [
            "app-token",
            "GH_TOKEN",
            "environment",
            "git push",
            "gh pr",
            "refs/heads/freshness",
            "permission-",
        ] {
            assert!(!text.contains(banned), "{job} names {banned}");
        }
    }
    assert!(WF.job("check")["environment"].is_null());
    let mints: Vec<&str> = WF
        .all_steps()
        .into_iter()
        .filter(|(_, s)| uses_of(s).starts_with("actions/create-github-app-token@"))
        .map(|(j, _)| j)
        .collect();
    assert_eq!(mints, ["update"]);
}

// The `tool` job.

#[test]
fn the_tools_are_built_in_a_job_that_holds_no_token() {
    let tool = WF.job("tool");
    assert_eq!(tool["permissions"], json!({ "contents": "read" }));
    assert!(tool["env"].is_null());
    let secret = Regex::new(r"github\.token|secrets\.|app-token").unwrap();
    for step in steps_of(tool) {
        assert!(!secret.is_match(&step.to_string()), "{}", step["name"]);
    }
    let build = step_named("tool", "build");
    assert!(
        run_of(build)
            .contains("cargo build --release --locked -p scaffold-release -p scaffold-cli")
    );
    let upload = steps_of(tool)
        .iter()
        .find(|s| uses_of(s).starts_with("actions/upload-artifact@"))
        .unwrap();
    assert_eq!(upload["with"]["name"], "scaffold-tools");
    assert!(
        upload["with"]["path"]
            .as_str()
            .unwrap()
            .contains("target/release/scaffold-release")
    );
    assert!(
        upload["with"]["path"]
            .as_str()
            .unwrap()
            .contains("target/release/pantheon-scaffold")
    );
    for output in ["sha256", "sha256-runner"] {
        assert!(tool["outputs"][output].is_string(), "{output}");
    }
}

// The `pins` job.

#[test]
fn the_pins_job_holds_no_token_that_can_write_and_gives_the_read_only_token_to_the_one_step_that_reads_github()
 {
    let pins = WF.job("pins");
    assert!(pins["env"]["GITHUB_TOKEN"].is_null() && pins["env"]["GH_TOKEN"].is_null());
    assert_eq!(pins["permissions"], json!({ "contents": "read" }));
    let update = &steps("pins")[index_of("pins", "update-pins")];
    assert_eq!(
        update["env"],
        json!({ "GITHUB_TOKEN": "${{ github.token }}" })
    );
    for step in steps("pins").iter().filter(|s| !std::ptr::eq(*s, update)) {
        assert!(step["env"]["GITHUB_TOKEN"].is_null(), "{}", step["name"]);
    }
    assert!(!text_of("pins").contains("app-token"));
}

#[test]
fn the_pins_job_checks_both_binaries_before_it_runs_either_and_in_this_order_updates_renders_collects()
 {
    let download = steps("pins")
        .iter()
        .position(|s| {
            uses_of(s).starts_with("actions/download-artifact@")
                && s["with"]["name"] == "scaffold-tools"
        })
        .expect("the download");
    let verify = index_of("pins", "sha256sum --check --strict");
    assert!(verify > download);
    let verify_step = &steps("pins")[verify];
    assert_eq!(
        verify_step["env"]["EXPECTED"],
        "${{ needs.tool.outputs.sha256 }}"
    );
    assert_eq!(
        verify_step["env"]["EXPECTED_RUNNER"],
        "${{ needs.tool.outputs.sha256-runner }}"
    );
    // Both binaries are checked, each against its own digest (found by the breakage comparison: dropping the runner's line failed no test when this
    // only looked for the names, which the `chmod` line below also holds).
    let checks: Vec<&str> = run_of(verify_step)
        .lines()
        .filter(|l| l.contains("sha256sum --check --strict"))
        .collect();
    assert_eq!(
        checks,
        [
            r#"echo "$EXPECTED  tool/scaffold-release" | sha256sum --check --strict -"#,
            r#"echo "$EXPECTED_RUNNER  tool/pantheon-scaffold" | sha256sum --check --strict -"#,
        ]
    );
    for (i, step) in steps("pins").iter().enumerate().take(verify) {
        assert!(
            !run_of(step).contains("tool/scaffold-") && !run_of(step).contains("tool/pantheon-"),
            "step {i} runs a binary before the check"
        );
    }
    let update = index_of("pins", "tool/scaffold-release update-pins");
    let render = index_of("pins", "tool/pantheon-scaffold render");
    let check = index_of("pins", "tool/pantheon-scaffold check");
    let collect = index_of("pins", COLLECT);
    assert!(verify < update && update < render && render <= check && check < collect);
    let block = run_of(&steps("pins")[render]);
    assert!(
        block.find("pantheon-scaffold render") < block.find("pantheon-scaffold check"),
        "the check comes after the render, in one step"
    );
    assert!(
        run_of(&steps("pins")[update]).contains("--freshness-out")
            && run_of(&steps("pins")[update]).contains("--summary")
    );
    assert!(!run_of(&steps("pins")[update]).contains("--no-write"));
    assert!(
        run_of(&steps("pins")[render]).contains("git diff --quiet pkl/Pins.pkl"),
        "the render is skipped when no pin moved"
    );
}

#[test]
fn the_pins_job_installs_the_pinned_tools_from_the_lock_file_and_runs_the_runner_under_them() {
    let install = index_of("pins", "mise install --locked");
    let render = index_of("pins", "tool/pantheon-scaffold render");
    assert!(install < render);
    assert!(run_of(&steps("pins")[render]).contains(
        "mise x -- tool/pantheon-scaffold render scaffold.pkl --allow-local-package pkl"
    ));
    assert!(
        run_of(&steps("pins")[index_of("pins", "tool/pantheon-scaffold check")]).contains(
            "mise x -- tool/pantheon-scaffold check scaffold.pkl --allow-local-package pkl"
        )
    );
}

#[test]
fn the_tar_is_uploaded_with_hidden_files_and_fails_when_there_is_none_and_its_digest_is_an_output()
{
    let upload = steps("pins")
        .iter()
        .find(|s| uses_of(s).starts_with("actions/upload-artifact@"))
        .unwrap();
    assert_eq!(upload["with"]["name"], "pin-update");
    assert_eq!(upload["with"]["include-hidden-files"], true);
    assert_eq!(upload["with"]["if-no-files-found"], "error");
    let pack = step_named("pins", "pack");
    assert!(run_of(pack).contains(COLLECT) && run_of(pack).contains("sha256sum"));
    assert_eq!(
        WF.job("pins")["outputs"]["sha256"],
        "${{ steps.pack.outputs.sha256 }}"
    );
}

// The `check` and `update` jobs.

#[test]
fn the_dry_run_check_runs_the_apply_script_from_the_checkout_in_check_only_mode_on_the_digest_of_the_tar()
 {
    let step = &steps("check")[index_of("check", APPLY)];
    assert_eq!(
        step["env"],
        json!({ "DIGEST": "${{ needs.pins.outputs.sha256 }}" })
    );
    assert!(
        run_of(step).trim_end().ends_with("--check-only"),
        "{}",
        run_of(step)
    );
    assert!(run_of(step).starts_with(&format!("bash {APPLY} ")));
    let checkout = steps("check")
        .iter()
        .position(|s| uses_of(s).starts_with("actions/checkout@"))
        .unwrap();
    assert!(checkout < index_of("check", APPLY));
}

#[test]
fn the_update_job_holds_the_token_in_the_protected_environment_and_mints_it_first() {
    let update = WF.job("update");
    assert_eq!(update["environment"], "update-pins");
    let first = &steps("update")[0];
    assert_eq!(first["id"], "app-token");
    assert!(
        first["if"].is_null(),
        "the job itself is skipped on a dry run, the mint has no condition of its own"
    );
    assert!(update["env"]["GITHUB_TOKEN"].is_null() && update["env"]["GH_TOKEN"].is_null());
}

#[test]
fn the_update_job_runs_no_tool_no_pkl_no_mise_and_no_cargo() {
    for step in steps("update") {
        let run = run_of(step);
        for banned in [
            "tool/",
            "cargo",
            "mise",
            "pkl ",
            "pantheon-scaffold",
            "scaffold-release",
        ] {
            assert!(!run.contains(banned), "{} runs {banned}", step["name"]);
        }
        assert!(
            !uses_of(step).starts_with("jdx/mise-action@")
                && !uses_of(step).starts_with("actions/download-artifact@")
                || step["with"]["name"] == "pin-update"
        );
    }
}

#[test]
fn the_apply_script_comes_from_the_checkout_and_runs_on_the_digest_before_anything_is_written() {
    let apply = index_of("update", APPLY);
    let step = &steps("update")[apply];
    assert_eq!(
        step["env"],
        json!({ "DIGEST": "${{ needs.pins.outputs.sha256 }}" })
    );
    assert!(
        run_of(step).starts_with(&format!("bash {APPLY} "))
            && !run_of(step).contains("--check-only")
    );
    let checkout = steps("update")
        .iter()
        .position(|s| uses_of(s).starts_with("actions/checkout@"))
        .unwrap();
    let download = steps("update")
        .iter()
        .position(|s| {
            uses_of(s).starts_with("actions/download-artifact@")
                && s["with"]["name"] == "pin-update"
        })
        .unwrap();
    assert!(checkout < apply && download < apply);
    let open_pr = index_of("update", OPEN_PR);
    let freshness = index_of("update", "refs/heads/freshness");
    assert!(
        apply < open_pr && open_pr < freshness,
        "the freshness record is written only after the apply and the pull request step"
    );
    for (i, step) in steps("update").iter().enumerate().take(apply) {
        assert!(
            !run_of(step).contains("git push") && !run_of(step).contains("git commit"),
            "step {i} writes before the apply"
        );
    }
}

#[test]
fn the_pull_request_and_the_record_use_the_app_token_and_the_record_goes_to_the_freshness_branch_never_main()
 {
    let pr = &steps("update")[index_of("update", OPEN_PR)];
    assert_eq!(pr["env"]["GH_TOKEN"], APP_TOKEN);
    assert_eq!(
        pr["env"]["SUMMARY"],
        "${{ runner.temp }}/applied/summary.md"
    );
    assert_eq!(run_of(pr), format!("bash {OPEN_PR}"));
    let publish = &steps("update")[index_of("update", "refs/heads/freshness")];
    assert_eq!(publish["env"]["GH_TOKEN"], APP_TOKEN);
    assert!(!run_of(publish).contains("refs/heads/main"));
    assert!(
        run_of(publish).contains("freshness.json")
            && run_of(publish).contains("$RUNNER_TEMP/applied/freshness.json")
    );
}

#[test]
fn the_pull_request_script_does_nothing_when_no_pin_changed_and_opens_a_conventional_commit_title()
{
    let script =
        std::fs::read_to_string(scaffold_package_tests::pkg::root().join(OPEN_PR)).unwrap();
    assert!(script.contains("git diff --quiet"));
    assert!(script.contains("title=\"chore(deps): update pinned actions\""));
    assert!(
        script.contains("--force-with-lease"),
        "a push that raced is refused, not forced"
    );
}

// New in #162: nothing names a second tool, a choice between tools or the bun runtime.

#[test]
fn there_is_one_tool_nothing_chooses_it_and_nothing_names_a_second_one_or_bun() {
    let named = Regex::new(r"implementation|update-pins\.ts|\bbun\b|disagree|typescript").unwrap();
    let lines: Vec<&str> = WF.text.lines().filter(|l| named.is_match(l)).collect();
    assert!(
        lines.is_empty(),
        "the workflow still names a second tool: {lines:?}"
    );
}
