//! Ported from `runner/test/release-builtins.test.ts`: the release options (releaseShape, releaseAutomerge, versionFile, versionCommitScope and
//! seedReleaseConfig) and what the release-please, release-automerge and release-bump builtins render for them. Every error case is reached
//! through Presence.forOptions, which every aggregator reads.

use regex::Regex;
use scaffold_package_tests::pkg::{eval, read, root};
use scaffold_package_tests::repo::temp_dir;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const COMPOSITION: &str = "workflows { ...Builtins.standardWorkflows(options) }\nfiles {\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles(options)\n}";

#[derive(Clone)]
struct Rendered {
    class: String,
    mode: String,
    content: String,
}

fn try_render(extra: &str, without: &[&str]) -> Result<BTreeMap<String, Rendered>, String> {
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
    let dir = temp_dir();
    let config = dir.path().join("release.pkl");
    std::fs::write(
        &config,
        format!(
            "amends \"file://{root}/pkl/Config.pkl\"\nimport \"file://{root}/pkl/Builtins.pkl\"\noptions {{\n  toolchain = \"1.96.1\"\n  runnerVersion = \"0.1.9\"\n  skills = false\n  {crate_name}\n  {extra}\n  without {{ {list} }}\n}}\n{COMPOSITION}\n",
            root = root().display()
        ),
    )
    .unwrap();
    let contract = eval(&config)?;
    Ok(contract["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["path"].as_str().unwrap().to_owned(),
                Rendered {
                    class: f["class"].as_str().unwrap().to_owned(),
                    mode: f["mode"].as_str().unwrap().to_owned(),
                    content: f["content"].as_str().unwrap_or("").to_owned(),
                },
            )
        })
        .collect())
}

fn render(extra: &str) -> BTreeMap<String, Rendered> {
    try_render(extra, &[]).unwrap_or_else(|e| panic!("{extra}: {e}"))
}

fn text<'a>(files: &'a BTreeMap<String, Rendered>, path: &str) -> &'a str {
    &files
        .get(path)
        .unwrap_or_else(|| {
            panic!(
                "no file {path}; files: {}",
                files.keys().cloned().collect::<Vec<_>>().join(", ")
            )
        })
        .content
}

const DRAFT: &str = "binary = false; releaseShape = \"draft\"; versionFile = \"pkl/PklProject\"; versionCommitScope = \"pkl\"";
const RELEASE_PLEASE: &str = ".github/workflows/release-please.yml";

#[test]
fn the_shape_follows_binary_unless_it_is_set_and_each_shape_has_its_own_header() {
    let dist = render("binary = true");
    assert!(text(&dist, RELEASE_PLEASE).contains("starts release.yml (cargo-dist)"));
    let published = render("binary = false");
    assert!(
        text(&published, RELEASE_PLEASE).contains("creates the vX.Y.Z tag and the GitHub Release")
    );
    let draft = render(DRAFT);
    let header = text(&draft, RELEASE_PLEASE);
    assert!(header.contains("a DRAFT release"));
    assert!(header.contains(
        "The version line of pkl/PklProject is updated through the x-release-please-version"
    ));
    assert!(!header.contains("tekhne"));
    let no_file = render("binary = false; releaseShape = \"draft\"");
    assert!(!text(&no_file, RELEASE_PLEASE).contains("version line of"));
}

#[test]
fn the_draft_shape_renders_the_publishing_script_the_others_do_not() {
    let draft = render(DRAFT);
    let script = draft
        .get(".github/scripts/publish-release.sh")
        .expect("the publishing script");
    assert_eq!(
        (script.class.as_str(), script.mode.as_str()),
        ("owned", "0755")
    );
    assert!(script.content.contains("DIST_DIR:-dist"));
    assert!(!render("binary = false").contains_key(".github/scripts/publish-release.sh"));
    assert!(!render("binary = true").contains_key(".github/scripts/publish-release.sh"));
}

#[test]
fn the_seeded_release_please_files_are_seeds_differ_by_shape_and_are_skipped_by_seed_release_config_false()
 {
    for (extra, release_type, path) in [
        ("binary = true", "rust", "crates/demo"),
        ("binary = false", "simple", "."),
        (DRAFT, "simple", "."),
    ] {
        let m = render(extra);
        assert_eq!(m["release-please-config.json"].class, "seed", "{extra}");
        assert_eq!(m[".release-please-manifest.json"].class, "seed", "{extra}");
        let cfg: Value = serde_json::from_str(text(&m, "release-please-config.json")).unwrap();
        let package = &cfg["packages"][path];
        assert!(!package.is_null(), "{extra}");
        let kind = if package["release-type"].is_null() {
            &cfg["release-type"]
        } else {
            &package["release-type"]
        };
        assert_eq!(kind, release_type, "{extra}");
        let manifest: Value =
            serde_json::from_str(text(&m, ".release-please-manifest.json")).unwrap();
        assert_eq!(manifest, json!({ ".": "0.0.0" }));
        let off = render(&format!("{extra}; seedReleaseConfig = false"));
        assert!(
            !(off.contains_key("release-please-config.json")
                || off.contains_key(".release-please-manifest.json")),
            "{extra}"
        );
    }
    let dist: Value =
        serde_json::from_str(text(&render("binary = true"), "release-please-config.json")).unwrap();
    assert_eq!(dist["skip-github-release"], true);
    assert!(dist["draft"].is_null());
    let draft: Value =
        serde_json::from_str(text(&render(DRAFT), "release-please-config.json")).unwrap();
    assert_eq!(draft["draft"], true);
    assert_eq!(draft["force-tag-creation"], true);
    assert_eq!(draft["include-component-in-tag"], false);
    assert_eq!(
        draft["packages"]["."]["extra-files"],
        json!([{ "type": "generic", "path": "pkl/PklProject" }])
    );
}

#[test]
fn the_seed_for_this_repositorys_options_is_the_committed_release_please_config_json() {
    let m = render(
        "binary = false; crateName = \"pantheon-scaffold\"; releaseShape = \"draft\"; versionFile = \"pkl/PklProject\"",
    );
    assert_eq!(
        text(&m, "release-please-config.json"),
        read("release-please-config.json")
    );
}

#[test]
fn release_automerge_manual_by_default_with_a_notice_when_the_schedule_is_off_todays_schedule_with_always()
 {
    let path = ".github/workflows/release-automerge.yml";
    let gated_files = render("binary = false");
    let gated = text(&gated_files, path);
    assert!(gated.contains("RELEASE_AUTOMERGE_SCHEDULE"));
    assert!(gated.contains("steps.gate.outputs.run == 'true'"));
    assert!(gated.contains("Scheduled run skipped"));
    assert!(gated.contains("workflow_dispatch"));
    let always_files = render("binary = false; releaseAutomerge = \"always\"");
    let always = text(&always_files, path);
    assert!(!always.contains("RELEASE_AUTOMERGE_SCHEDULE"));
    assert!(!always.contains("gate"));
    assert!(always.contains("cron: 37 8 * * 1"));
}

#[test]
fn release_bump_renders_only_with_version_file_as_a_workflow_and_a_script_that_name_the_file_and_the_scope()
 {
    let none = render("binary = false");
    assert!(
        !(none.contains_key(".github/workflows/release-bump.yml")
            || none.contains_key(".github/scripts/release-bump.sh"))
    );
    let m = render(DRAFT);
    let wf = text(&m, ".github/workflows/release-bump.yml");
    assert!(wf.contains("workflows:\n    - release"));
    assert!(wf.contains("zizmor: ignore[dangerous-triggers]"));
    assert!(wf.contains("timeout-minutes: 45"));
    let script = text(&m, ".github/scripts/release-bump.sh");
    assert!(script.contains("version_file='pkl/PklProject'"));
    assert!(script.contains("chore(pkl): start $next"));
    assert!(script.contains("max_polls=\"${MAX_POLLS:-60}\""));
    assert_eq!(m[".github/scripts/release-bump.sh"].mode, "0755");
    assert!(text(&m, ".github/dependabot.yml").contains(".github/workflows/release-bump.yml"));
    let unscoped =
        render("binary = false; releaseShape = \"draft\"; versionFile = \"version.toml\"");
    assert!(text(&unscoped, ".github/scripts/release-bump.sh").contains("\"chore: start $next\""));
}

fn refused(extra: &str, without: &[&str], pattern: &str) {
    let error = try_render(extra, without)
        .err()
        .unwrap_or_else(|| panic!("{extra} was not refused"));
    assert!(
        Regex::new(pattern).unwrap().is_match(&error),
        "{extra}: {error}"
    );
}

#[test]
fn the_release_options_are_refused_with_a_message_that_names_what_is_wrong_whatever_is_rendered() {
    refused(
        "binary = true; releaseShape = \"draft\"",
        &[],
        r#"releaseShape = "draft" cannot be used with options\.binary = true"#,
    );
    refused(
        "versionCommitScope = \"pkl\"",
        &[],
        r"versionCommitScope is set but options\.versionFile is not",
    );
    refused(
        "binary = false; versionFile = \"pkl/PklProject\"",
        &[],
        r#"(?s)versionFile needs options\.releaseShape = "draft".*"published""#,
    );
    refused(DRAFT, &["release-bump"], r#"leaves out "release-bump""#);
    for bad in ["/etc/passwd", "../up", "it's", "a/../b"] {
        refused(
            &format!("binary = false; releaseShape = \"draft\"; versionFile = \"{bad}\""),
            &[],
            r"versionFile must be a plain path",
        );
    }
}

#[test]
fn release_bump_waits_for_the_workflow_that_the_release_shape_names() {
    let release = read(".github/workflows/release.yml");
    assert!(
        Regex::new(r"(?m)^name: release$")
            .unwrap()
            .is_match(&release)
    );
}
