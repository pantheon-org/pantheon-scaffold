//! Ported from `runner/test/workflows-a.test.ts`: the first six workflows and the four config files they use, against the pinned
//! pantheon-adr copies.

use scaffold_package_tests::pkg::{
    actionlint, content, expect_pins, file, git_blob_sha, jobs, mode, paths, read, render, unpin,
    upstream_json,
};
use scaffold_package_tests::{compare_directives, compare_workflows, parse_toml};
use serde_json::Value;
use std::sync::LazyLock;

static CONTRACT: LazyLock<Value> = LazyLock::new(|| render("workflows-a"));

const WORKFLOWS: [(&str, &str); 6] = [
    ("plumber", "a3468d27b20b77bb39909a2dbf22b173bf059d13"),
    ("sync-labels", "b81cb37d260a91399939b25198803c339992418a"),
    ("pr-auto-rebase", "a9931a88d908eba56fe03f54d31a711f5c593e27"),
    (
        "release-automerge",
        "fd5d20a13f93b26d153222769188299204b93fc9",
    ),
    ("commit-types", "f507416d9636d72b9091220542d906188093c670"),
    (
        "conventional-commits",
        "e3e06519eaa74194c0052eeec358e6fcbf4d265c",
    ),
];

fn upstream(name: &str) -> String {
    read(&format!("runner/test/upstream/workflows/{name}.yml"))
}

fn rendered(name: &str) -> &'static str {
    content(&CONTRACT, &format!(".github/workflows/{name}.yml"))
}

#[test]
fn the_pinned_upstream_copies_are_the_pantheon_adr_workflows_at_1e2fd46_identical_in_all_four_repositories()
 {
    for (name, blob) in WORKFLOWS {
        assert_eq!(git_blob_sha(&upstream(name)), blob, "{name}");
    }
}

#[test]
fn wave_a_renders_the_six_workflows_and_the_four_config_files_they_use() {
    let mut got = paths(&CONTRACT);
    got.sort();
    assert_eq!(
        got,
        [
            ".github/commit-types.txt",
            ".github/labels.txt",
            ".github/pull_request_template.md",
            ".github/workflows/commit-types.yml",
            ".github/workflows/conventional-commits.yml",
            ".github/workflows/plumber.yml",
            ".github/workflows/pr-auto-rebase.yml",
            ".github/workflows/release-automerge.yml",
            ".github/workflows/sync-labels.yml",
            ".plumber.yaml",
        ]
    );
}

#[test]
fn the_one_per_file_ignore_in_the_rendered_rumdl_toml_is_for_the_pull_request_template_this_contract_renders()
 {
    let tooling = render("tooling");
    let toml = parse_toml(content(&tooling, ".rumdl.toml"));
    let ignored: Vec<&String> = toml["per-file-ignores"]
        .as_table()
        .expect("per-file-ignores")
        .keys()
        .collect();
    assert_eq!(ignored.len(), 1);
    assert!(file(&CONTRACT, ignored[0]).is_object());
    assert_eq!(ignored[0], ".github/pull_request_template.md");
}

/// The three tests the TypeScript generated for each of the six workflows.
macro_rules! workflow {
    ($module:ident, $name:expr) => {
        mod $module {
            use super::*;

            #[test]
            fn yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule() {
                let result =
                    compare_workflows(&unpin(&upstream($name)), &unpin(rendered($name))).unwrap();
                assert!(result.equal, "{:?}", result.differences);
                assert!(result.duplicate_keys.is_empty());
            }

            #[test]
            fn yml_keeps_every_comment_a_tool_acts_on() {
                assert_eq!(
                    compare_directives(&upstream($name), rendered($name)),
                    Vec::<String>::new()
                );
            }

            #[test]
            fn yml_keeps_its_job_ids_and_job_names_which_the_required_status_checks_depend_on() {
                assert_eq!(jobs(rendered($name)), jobs(&upstream($name)));
            }
        }
    };
}

workflow!(plumber, "plumber");
workflow!(sync_labels, "sync-labels");
workflow!(pr_auto_rebase, "pr-auto-rebase");
workflow!(release_automerge, "release-automerge");
workflow!(commit_types, "commit-types");
workflow!(conventional_commits, "conventional-commits");

#[test]
fn the_zizmor_ignore_comment_on_the_pull_request_target_trigger_is_rendered_on_the_on_line() {
    let text = rendered("conventional-commits");
    assert!(
        text.contains("\n'on': # zizmor: ignore[dangerous-triggers] reads only the PR title; checks out and runs no PR code\n"),
        "{text}"
    );
}

#[test]
fn the_four_config_files_are_byte_for_byte_the_pinned_pantheon_adr_files_and_not_executable() {
    let blobs = upstream_json("workflow-files-blobs.json");
    let blobs = blobs.as_object().unwrap();
    assert_eq!(blobs.len(), 4);
    for (path, blob) in blobs {
        assert_eq!(
            git_blob_sha(content(&CONTRACT, path)),
            blob.as_str().unwrap(),
            "{path}"
        );
        assert_eq!(mode(&CONTRACT, path), "0644", "{path}");
    }
}

#[test]
fn the_pins_are_the_three_actions_the_workflows_use_each_a_commit_with_a_version_tag() {
    expect_pins(
        &CONTRACT["pins"],
        &[
            "actions/checkout",
            "actions/create-github-app-token",
            "getplumber/plumber",
        ],
    );
}

#[test]
fn wave_a_uses_no_escape_hatch() {
    assert_eq!(CONTRACT["escapeHatch"]["uses"], 0);
}

#[test]
fn the_app_client_id_is_an_option_with_the_pantheon_ai_bot_id_as_the_default() {
    for name in ["pr-auto-rebase", "release-automerge"] {
        assert!(
            rendered(name).contains("client-id: Iv23ligzn16vkDCWfHgc"),
            "{name}"
        );
    }
    let custom = render("workflows-a-client-id");
    for name in ["pr-auto-rebase", "release-automerge"] {
        let text = content(&custom, &format!(".github/workflows/{name}.yml"));
        assert!(text.contains("client-id: Iv-test-client-id"), "{name}");
        assert!(!text.contains("Iv23ligzn16vkDCWfHgc"), "{name}");
    }
}

/// actionlint over the render (plan task 4.4). It reads the workflow files and, when shellcheck is on the PATH, also checks the run scripts.
#[test]
fn actionlint_is_clean_on_the_six_rendered_workflows() {
    let workflows: Vec<(&str, &str)> = CONTRACT["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| {
            f["path"]
                .as_str()
                .unwrap()
                .starts_with(".github/workflows/")
        })
        .map(|f| (f["path"].as_str().unwrap(), f["content"].as_str().unwrap()))
        .collect();
    assert_eq!(workflows.len(), 6);
    assert_eq!(actionlint(&workflows), "");
}

/// Phase 7 benefit test (task 7.6): the one canonical change released as 0.1.4. plumber.yml says it is generated, in its header comment, so
/// the change reaches every repository as a comment-only difference, which the proof already accepts.
const NOTICE: &str =
    "# Generated by pantheon-scaffold from scaffold.pkl: change it there, not in this file.\n";

#[test]
fn plumber_yml_ends_its_header_with_the_notice_that_it_is_generated_and_no_other_workflow_carries_it()
 {
    let text = rendered("plumber");
    assert!(text.starts_with("# CI/CD security scan"));
    assert!(text.contains(&format!("#\n{NOTICE}name: Plumber\n")));
    for (name, _) in WORKFLOWS.iter().filter(|(n, _)| *n != "plumber") {
        assert!(!rendered(name).contains(NOTICE), "{name}");
    }
}
