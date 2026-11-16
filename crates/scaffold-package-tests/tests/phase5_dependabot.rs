//! Ported from `runner/test/phase5-dependabot.test.ts`. Decision 86: cargo-dist is the only writer of the generated release.yml, so with
//! `binary` Dependabot ignores the two actions inside it (they change through dist-workspace.toml and `dist generate`), and without `binary`
//! it does not.

use regex::Regex;
use scaffold_package_tests::parse_yaml;
use scaffold_package_tests::pkg::{content, render};
use serde_json::{Value, json};

fn entries(text: &str) -> Value {
    let doc = parse_yaml(text).expect("dependabot.yml parses");
    let by_ecosystem: serde_json::Map<String, Value> = doc["updates"]
        .as_array()
        .expect("updates")
        .iter()
        .map(|u| {
            (
                u["package-ecosystem"].as_str().unwrap().to_owned(),
                u.clone(),
            )
        })
        .collect();
    Value::Object(by_ecosystem)
}

fn dependabot(fixture: &str) -> Value {
    entries(content(&render(fixture), ".github/dependabot.yml"))
}

macro_rules! binary_repo {
    ($name:ident, $repo:expr) => {
        #[test]
        fn $name() {
            let e = dependabot(concat!("real-workflows-", $repo));
            assert_eq!(
                e["github-actions"]["ignore"],
                json!([{ "dependency-name": "actions/upload-artifact" }, { "dependency-name": "actions/download-artifact" }])
            );
            assert_eq!(
                e["cargo"]["ignore"],
                json!([{ "dependency-name": "pahdk-core" }, { "dependency-name": "pahdk-install" }])
            );
        }
    };
}

binary_repo!(pantheon_adr_with_a_binary_dependabot_ignores_upload_artifact_and_download_artifact_in_the_github_actions_entry_only, "pantheon-adr");
binary_repo!(pantheon_journal_with_a_binary_dependabot_ignores_upload_artifact_and_download_artifact_in_the_github_actions_entry_only, "pantheon-journal");
binary_repo!(pantheon_skill_auditor_with_a_binary_dependabot_ignores_upload_artifact_and_download_artifact_in_the_github_actions_entry_only, "pantheon-skill-auditor");

// The library repository renders every workflow, so its exclusion list would cover them all and Dependabot would fail with
// dependency_file_not_found: it has no github-actions entry (options.dependabotGithubActions,
// crates/scaffold-package-tests/tests/dependabot_entry.rs).
#[test]
fn the_library_repository_with_no_binary_has_no_github_actions_entry_and_so_no_ignore_list() {
    let e = dependabot("real-workflows-pantheon-agentic-harness-dev-kit");
    assert!(e["github-actions"].is_null());
    assert!(!e["cargo"].is_null());
}

#[test]
fn the_header_says_why_the_two_actions_are_ignored() {
    let text = content(
        &render("real-workflows-pantheon-adr"),
        ".github/dependabot.yml",
    )
    .to_owned();
    assert!(text.contains("release.yml"));
    assert!(text.contains("dist generate"));
}

// Dependabot must not edit the workflow files the package renders: the runner owns them, so a bumped pin is drift (`check` exits 4). Their
// pins move through Pins.pkl (update-pins.yml) and `upgrade`. The list must be exactly the owned workflow files of the same render.
#[test]
fn real_workflows_pantheon_adr_the_github_actions_entry_excludes_exactly_the_owned_workflow_files_and_the_cargo_entry_excludes_nothing()
 {
    let contract = render("real-workflows-pantheon-adr");
    let workflow = Regex::new(r"^\.github/workflows/[^/]+\.yml$").unwrap();
    let mut owned: Vec<String> = contract["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["class"] == "owned" && workflow.is_match(f["path"].as_str().unwrap()))
        .map(|f| f["path"].as_str().unwrap().to_owned())
        .collect();
    owned.sort();
    let e = entries(content(&contract, ".github/dependabot.yml"));
    assert!(owned.len() > 10);
    let mut excluded: Vec<String> = e["github-actions"]["exclude-paths"]
        .as_array()
        .map(|l| l.iter().map(|p| p.as_str().unwrap().to_owned()).collect())
        .unwrap_or_default();
    excluded.sort();
    assert_eq!(excluded, owned);
    assert!(e["cargo"]["exclude-paths"].is_null());
}

#[test]
fn a_repository_with_skills_and_a_binary_excludes_audit_skill_yml_and_dist_check_yml() {
    let with_skills = entries(content(
        &render("real-workflows-pantheon-adr"),
        ".github/dependabot.yml",
    ));
    let excluded = with_skills["github-actions"]["exclude-paths"]
        .as_array()
        .unwrap();
    assert!(excluded.contains(&json!(".github/workflows/dist-check.yml")));
    assert!(excluded.contains(&json!(".github/workflows/audit-skill.yml")));
}
