//! Every step that mints a token of the GitHub App names the permissions it asks for, and only one workflow asks for `workflows`.
//!
//! The App is shared: a permission granted to it applies to every installation, and a mint step that passes no `permission-*` input gets every permission the
//! installation holds. So a new permission on the App is safe only if each mint step, here and in what the Builtins render for other repositories, asks for
//! exactly what it needs. This test is that check; it runs on every pull request.

use scaffold_package_tests::pkg::root;
use scaffold_package_tests::presence::render;
use scaffold_package_tests::workflow::{Workflow, uses_of};
use serde_json::Value;

const MINT: &str = "actions/create-github-app-token@";
/// The one workflow that may ask for the `workflows` permission: it pushes the rendered workflow files of a pin update.
const MAY_WRITE_WORKFLOWS: &str = ".github/workflows/update-pins.yml";

/// The `permission-*` inputs of a mint step.
fn asked(step: &Value) -> Vec<String> {
    step["with"]
        .as_object()
        .map(|w| {
            w.keys()
                .filter(|k| k.starts_with("permission-"))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn mint_steps(workflow: &Workflow) -> Vec<Value> {
    workflow
        .all_steps()
        .into_iter()
        .map(|(_, s)| s.clone())
        .filter(|s| uses_of(s).starts_with(MINT))
        .collect()
}

fn check(name: &str, steps: &[Value], may_write_workflows: bool) {
    for step in steps {
        let asked = asked(step);
        assert!(
            !asked.is_empty(),
            "{name}: a mint step passes no permission-* input, so it gets every permission of the installation"
        );
        assert!(
            asked
                .iter()
                .all(|p| p != "permission-administration" && p != "permission-secrets"),
            "{name}: {asked:?}"
        );
        if !may_write_workflows {
            assert!(
                !asked.contains(&"permission-workflows".to_owned()),
                "{name}: only {MAY_WRITE_WORKFLOWS} may ask for the workflows permission"
            );
        }
    }
}

#[test]
fn every_mint_step_of_the_workflows_of_this_repository_names_its_permissions() {
    let mut seen = 0;
    for entry in std::fs::read_dir(root().join(".github/workflows"))
        .unwrap()
        .flatten()
    {
        let name = format!(".github/workflows/{}", entry.file_name().to_string_lossy());
        if !name.ends_with(".yml") {
            continue;
        }
        let steps = mint_steps(&Workflow::load(&name));
        seen += steps.len();
        check(&name, &steps, name == MAY_WRITE_WORKFLOWS);
    }
    assert!(
        seen >= 5,
        "the mint steps of the five workflows that have one were found: {seen}"
    );
}

#[test]
fn every_mint_step_that_the_builtins_render_for_other_repositories_names_its_permissions() {
    let files = render(&[], "");
    let mut seen = 0;
    for (path, text) in files
        .iter()
        .filter(|(p, _)| p.starts_with(".github/workflows/") && p.ends_with(".yml"))
    {
        if !text.contains(MINT) {
            continue;
        }
        let doc =
            scaffold_package_tests::parse_yaml(text).unwrap_or_else(|e| panic!("{path}: {e}"));
        let workflow = Workflow {
            text: text.clone(),
            doc,
        };
        let steps = mint_steps(&workflow);
        seen += steps.len();
        check(path, &steps, false);
    }
    assert!(
        seen >= 3,
        "the rendered workflows that mint a token were found: {seen}"
    );
}

#[test]
fn the_shared_mint_step_of_the_builtins_asks_for_contents_and_pull_requests_only() {
    let text = std::fs::read_to_string(root().join("pkl/Builtins/Shared.pkl")).unwrap();
    let start = text
        .find("function mintToken")
        .expect("the shared mint step");
    let body = &text[start
        ..text[start..]
            .find("\n}\n")
            .map_or(text.len(), |e| start + e)];
    let asked: Vec<&str> = body
        .lines()
        .filter_map(|l| l.trim().strip_prefix("[\"permission-"))
        .map(|l| l.split('"').next().unwrap())
        .collect();
    assert_eq!(asked, ["contents", "pull-requests"]);
}
