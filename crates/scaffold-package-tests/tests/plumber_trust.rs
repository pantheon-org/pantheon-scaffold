//! Plumber's rule ISSUE-713 ("action comes from an unauthorized source", control `githubActionMustComeFromAuthorizedSources`) passes
//! GitHub's own actions, same-organisation actions (`trustSameOrgActions`) and the names in `trustedGithubActions`. This test keeps the
//! trusted list and the actions the rendered workflows use in step, in both directions, so a third-party action added to the package
//! without a Plumber decision fails here instead of raising an alert on every repository that renders it (jdx/mise-action did, 16-11-2026).
//!
//! `trustedOwners` belongs to another control (`actionsMustBePinnedByCommitSha`, a pinning exemption) and is not a source rule.

use scaffold_package_tests::parse_yaml;
use scaffold_package_tests::pkg::{content, eval, paths, render, root};
use scaffold_package_tests::repo::temp_dir;
use serde_json::Value;
use std::collections::BTreeSet;

/// Owners whose actions Plumber trusts without a list entry: GitHub's own (`trustGithubOfficialActions`).
const OFFICIAL: [&str; 2] = ["actions", "github"];

/// Same-organisation actions (`trustSameOrgActions`): trusted in pantheon-org repositories only, which is every repository that renders
/// this package today. `pantheon-org/pantheon-skill-auditor` is the audit-skill workflow's action.
const SAME_ORG_PREFIX: &str = "pantheon-org/";

/// Entries of the trusted list that no workflow of the renders below uses, each with its reason. None today.
const UNUSED_ON_PURPOSE: [(&str, &str); 0] = [];

fn workflow_texts(contract: &Value) -> Vec<String> {
    paths(contract)
        .into_iter()
        .filter(|p| p.starts_with(".github/workflows/") && p.ends_with(".yml"))
        .filter(|p| contract_has_content(contract, p))
        .map(|p| content(contract, &p).to_string())
        .collect()
}

fn contract_has_content(contract: &Value, path: &str) -> bool {
    contract["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"] == path && f["content"].is_string())
}

/// Every `uses:` of a workflow, as `owner/name` without the ref. Local actions (`./`) and `docker://` are skipped.
fn used_actions(workflow: &str) -> BTreeSet<String> {
    let doc = parse_yaml(workflow).expect("the workflow parses");
    let mut found = BTreeSet::new();
    collect_uses(&doc, &mut found);
    found
}

fn collect_uses(value: &Value, found: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, v) in map {
                if key == "uses"
                    && let Some(uses) = v.as_str()
                    && !uses.starts_with("./")
                    && !uses.starts_with("docker://")
                {
                    found.insert(uses.split('@').next().unwrap().to_string());
                }
                collect_uses(v, found);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_uses(v, found)),
        _ => {}
    }
}

fn trusted(contract: &Value) -> BTreeSet<String> {
    let doc = parse_yaml(content(contract, ".plumber.yaml")).expect(".plumber.yaml parses");
    doc["github"]["controls"]["githubActionMustComeFromAuthorizedSources"]["trustedGithubActions"]
        .as_array()
        .expect("a trusted list")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

/// The upgrade workflow is not in `standardWorkflows`: a repository adds it itself, so it is rendered here on its own.
fn upgrade_workflow() -> String {
    let text = format!(
        "amends \"file://{r}/pkl/Config.pkl\"\nimport \"file://{r}/pkl/Builtins.pkl\"\noptions {{\n  toolchain = \"1.96.1\"\n  runnerVersion = \"0.4.0\"\n  skills = false\n  binary = false\n}}\nworkflows {{ [\"upgrade\"] = Builtins.upgradeWorkflow(options) }}\n",
        r = root().to_string_lossy()
    );
    let dir = temp_dir();
    let config = dir.path().join("upgrade.pkl");
    std::fs::write(&config, text).unwrap();
    let contract = eval(&config).expect("the upgrade workflow renders");
    content(&contract, ".github/workflows/upgrade.yml").to_string()
}

fn all_used() -> BTreeSet<String> {
    // Every workflow builtin set up like pantheon-adr (skills and a binary), plus the upgrade workflow.
    let adr = render("real-workflows-pantheon-adr");
    let mut used = BTreeSet::new();
    for text in workflow_texts(&adr) {
        used.extend(used_actions(&text));
    }
    used.extend(used_actions(&upgrade_workflow()));
    used
}

#[test]
fn every_action_a_rendered_workflow_uses_is_official_same_org_or_in_the_trusted_list() {
    let adr = render("real-workflows-pantheon-adr");
    let list = trusted(&adr);
    let untrusted: Vec<String> = all_used()
        .into_iter()
        .filter(|a| {
            let owner = a.split('/').next().unwrap();
            !OFFICIAL.contains(&owner) && !a.starts_with(SAME_ORG_PREFIX) && !list.contains(a)
        })
        .collect();
    assert!(
        untrusted.is_empty(),
        "Plumber would flag ISSUE-713 for {untrusted:?}: add each to Values.plumberTrustedActions with a comment (a deliberate choice), or stop using it"
    );
}

#[test]
fn every_trusted_action_is_used_by_a_rendered_workflow_or_has_a_reason() {
    let adr = render("real-workflows-pantheon-adr");
    let used = all_used();
    let unused: Vec<String> = trusted(&adr)
        .into_iter()
        .filter(|a| !used.contains(a) && !UNUSED_ON_PURPOSE.iter().any(|(name, _)| name == a))
        .collect();
    assert!(
        unused.is_empty(),
        "trusted but not used by any rendered workflow: {unused:?}: remove them from Values.plumberTrustedActions, or list them in UNUSED_ON_PURPOSE with a reason"
    );
}

#[test]
fn the_mise_action_is_among_the_actions_in_use_and_is_trusted_by_name() {
    let adr = render("real-workflows-pantheon-adr");
    assert!(all_used().contains("jdx/mise-action"));
    assert!(trusted(&adr).contains("jdx/mise-action"));
}
