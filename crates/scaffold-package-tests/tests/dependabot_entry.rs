//! Ported from `runner/test/dependabot-entry.test.ts`. options.dependabotGithubActions: the github-actions entry of dependabot.yml is
//! rendered unless nothing is left for it to scan. Its exclude-paths lists every workflow the package renders, so a repository whose
//! workflows are all rendered has nothing outside it and Dependabot fails with dependency_file_not_found (dev-kit, 16-11-2026). `auto`
//! renders the entry for a binary, a composite action, the draft release shape and workflow Builtins left out with `without`; `on` and
//! `off` force it.

use regex::Regex;
use scaffold_package_tests::parse_yaml;
use scaffold_package_tests::pkg::{eval, file, read, root};
use scaffold_package_tests::repo::temp_dir;
use serde_json::Value;

const TEMPLATE: &str = "amends \"file://@ROOT@/pkl/Config.pkl\"\nimport \"file://@ROOT@/pkl/Builtins.pkl\"\noptions {\n  toolchain = \"1.96.1\"\n  runnerVersion = \"0.4.0\"\n  skills = false\n  @CRATE@\n  @EXTRA@\n  without { @LIST@ }\n}\nworkflows { ...Builtins.standardWorkflows(options) }\nfiles { ...Builtins.standardFiles(options) }\n";

fn render(extra: &str, without: &[&str]) -> Result<Value, String> {
    let list = without
        .iter()
        .map(|w| format!("\"{w}\""))
        .collect::<Vec<_>>()
        .join("; ");
    let crate_name = if extra.contains("crateName") {
        ""
    } else {
        "crateName = \"demo\""
    };
    let text = TEMPLATE
        .replace("@ROOT@", &root().to_string_lossy())
        .replace("@CRATE@", crate_name)
        .replace("@EXTRA@", extra)
        .replace("@LIST@", &list);
    let dir = temp_dir();
    let config = dir.path().join("dependabot.pkl");
    std::fs::write(&config, text).unwrap();
    eval(&config)
}

struct Dependabot {
    text: String,
    updates: Vec<Value>,
}

impl Dependabot {
    fn entry(&self, ecosystem: &str) -> Option<&Value> {
        self.updates
            .iter()
            .find(|u| u["package-ecosystem"] == ecosystem)
    }
}

fn dependabot(contract: &Value) -> Dependabot {
    let text = file(contract, ".github/dependabot.yml")["content"]
        .as_str()
        .expect("the file has content")
        .to_owned();
    let doc = parse_yaml(&text).expect("dependabot.yml parses");
    assert_eq!(doc["version"], 2);
    let updates = doc["updates"].as_array().expect("updates").clone();
    assert!(!updates.is_empty());
    Dependabot { text, updates }
}

/// The workflows the package renders as owned files.
fn owned(contract: &Value) -> Vec<String> {
    let workflow = Regex::new(r"^\.github/workflows/[^/]+\.yml$").unwrap();
    let mut paths: Vec<String> = contract["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["class"] == "owned" && workflow.is_match(f["path"].as_str().unwrap()))
        .map(|f| f["path"].as_str().unwrap().to_owned())
        .collect();
    paths.sort();
    paths
}

fn check_case(extra: &str, without: &[&str], entry: bool) {
    let contract = render(extra, without).unwrap();
    let d = dependabot(&contract);
    assert!(d.entry("cargo").is_some(), "cargo entry");
    assert_eq!(d.entry("github-actions").is_some(), entry);
    if let Some(actions) = d.entry("github-actions") {
        let mut excluded: Vec<String> = actions
            .get("exclude-paths")
            .and_then(Value::as_array)
            .map(|l| l.iter().map(|p| p.as_str().unwrap().to_owned()).collect())
            .unwrap_or_default();
        excluded.sort();
        assert_eq!(excluded, owned(&contract), "exclude-paths");
    }
    assert_eq!(d.text.contains("There is no github-actions entry"), !entry);
    assert_eq!(
        d.text.contains("release.yml caveat"),
        entry && extra.contains("binary = true"),
        "the dist note belongs to the entry with a binary"
    );
}

macro_rules! case {
    ($name:ident, $extra:expr, $without:expr, $entry:expr) => {
        #[test]
        fn $name() {
            check_case($extra, &$without, $entry);
        }
    };
}

case!(
    a_binary_renders_the_github_actions_entry_and_the_cargo_entry,
    "binary = true",
    [],
    true
);
case!(
    a_composite_action_renders_the_github_actions_entry_and_the_cargo_entry,
    "binary = false; compositeAction = true",
    [],
    true
);
case!(
    the_draft_release_shape_renders_the_github_actions_entry_and_the_cargo_entry,
    "binary = false; releaseShape = \"draft\"",
    [],
    true
);
case!(
    builtins_left_out_one_workflow_render_the_github_actions_entry_and_the_cargo_entry,
    "binary = false",
    ["rust-ci"],
    true
);
case!(
    a_tool_builtin_left_out_that_renders_no_workflow_leaves_the_github_actions_entry_out,
    "binary = false",
    ["rumdl"],
    false
);
case!(
    a_builtin_the_toggles_already_removed_leaves_the_github_actions_entry_out,
    "binary = false",
    ["release"],
    false
);
case!(
    everything_a_workflow_builtin_could_be_left_out_renders_the_github_actions_entry,
    "binary = false",
    [
        "plumber",
        "sync-labels",
        "pr-auto-rebase",
        "release-automerge",
        "release-please",
        "zizmor",
        "agnix",
        "aislop",
        "rust-ci",
        "cargo-deny",
        "inline-scripts",
        "scaffold-check"
    ],
    true
);
case!(
    a_plain_library_the_dev_kit_leaves_the_github_actions_entry_out,
    "binary = false; features = \"all\"",
    [],
    false
);
case!(
    on_on_a_plain_library_renders_the_github_actions_entry,
    "binary = false; dependabotGithubActions = \"on\"",
    [],
    true
);
case!(
    off_on_a_binary_leaves_the_github_actions_entry_out,
    "binary = true; dependabotGithubActions = \"off\"",
    [],
    false
);
case!(
    auto_spelt_out_on_a_plain_library_leaves_the_github_actions_entry_out,
    "binary = false; dependabotGithubActions = \"auto\"",
    [],
    false
);

#[test]
fn the_header_has_its_four_variants_and_none_claims_what_the_file_does_not_do() {
    let text = |extra: &str| dependabot(&render(extra, &[]).unwrap()).text;
    let plain = text("binary = false");
    assert!(plain.contains("Keeps Cargo dependencies current."));
    assert!(!plain.contains("SHA-pinned GitHub Actions"));
    assert!(plain.contains("options.dependabotGithubActions = \"on\""));
    assert!(plain.contains("pantheon-agentic-harness-dev-kit crates are git dependencies"));
    let plain_ignore = text("binary = false; gitDependencyIgnore = new Listing { \"pahdk-core\" }");
    assert!(plain_ignore.contains("pahdk-core and pahdk-install are git dependencies"));
    assert!(!plain_ignore.contains("SHA-pinned GitHub Actions"));
    let entry = text("binary = true");
    assert!(entry.contains("Keeps Cargo dependencies and the SHA-pinned GitHub Actions current."));
    assert!(!entry.contains("There is no github-actions entry"));
    let entry_ignore = text("binary = true; gitDependencyIgnore = new Listing { \"pahdk-core\" }");
    assert!(entry_ignore.contains("pahdk-core and pahdk-install are git dependencies"));
    assert!(entry_ignore.contains("SHA-pinned GitHub Actions"));
}

#[test]
fn this_repositorys_options_render_the_committed_dependabot_yml_so_check_stays_clean() {
    let contract = render(
        "binary = false; crateName = \"pantheon-scaffold\"; releaseShape = \"draft\"; versionFile = \"pkl/PklProject\"; versionCommitScope = \"pkl\"; releaseAutomerge = \"always\"; features = \"default\"",
        &[],
    )
    .unwrap();
    let rendered = dependabot(&contract).text;
    let committed = read(".github/dependabot.yml");
    assert_eq!(
        parse_yaml(&rendered).unwrap(),
        parse_yaml(&committed).unwrap()
    );
}

#[test]
fn a_misspelt_value_is_refused_by_the_render() {
    let error = render("binary = false; dependabotGithubActions = \"yes\"", &[]).unwrap_err();
    assert!(
        error.contains("dependabotGithubActions") || error.contains("yes"),
        "{error}"
    );
}
