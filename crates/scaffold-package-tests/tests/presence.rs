//! Ported from `runner/test/presence.test.ts`: options.without, builtins left out by name. The aggregators in Builtins.pkl skip a left-out
//! builtin; a builtin that cannot work without another fails with an error naming both. The default render (nothing left out) is compared with
//! the baseline of pull request 116 by `scaffold-release baseline`, not here.

use regex::Regex;
use scaffold_package_tests::cli::run;
use scaffold_package_tests::pkg::root;
use scaffold_package_tests::presence::{COMPOSITION, config, eval_config, folders, render};
use scaffold_package_tests::repo::{commit_all, get, git_repo, has, put, temp_dir};
use std::process::Command;

#[test]
fn the_registry_of_builtin_names_in_shared_pkl_is_the_list_of_folders_under_pkl_builtins() {
    let dir = temp_dir();
    let file = dir.path().join("names.pkl");
    std::fs::write(
        &file,
        format!(
            "import \"file://{}/pkl/Presence.pkl\"\nnames = Presence.builtinNames\n",
            root().display()
        ),
    )
    .unwrap();
    let output = Command::new("pkl")
        .args(["eval", "-f", "json", "--allowed-resources", "file:,prop:"])
        .arg(&file)
        .output()
        .expect("pkl runs");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let mut names: Vec<String> = json["names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap().to_owned())
        .collect();
    names.sort();
    assert_eq!(names, folders());
    assert_eq!(folders().len(), 29);
}

// What leaving each builtin out removes from the full render (skills and binary on), read from the render and checked against what the builtin
// owns. A builtin that others need is left out together with them. dependabot.yml changes whenever a workflow goes, because its exclude list
// follows the workflows that are rendered; nothing else changes and nothing is added.
const DEPENDABOT: &str = ".github/dependabot.yml";

struct Expected {
    name: &'static str,
    also: &'static [&'static str],
    removed: Vec<String>,
    dependabot: bool,
}

fn wf(name: &str) -> String {
    format!(".github/workflows/{name}")
}
fn sc(name: &str) -> String {
    format!(".github/scripts/{name}")
}
fn paths(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn expected() -> Vec<Expected> {
    let e = |name: &'static str,
             also: &'static [&'static str],
             removed: Vec<String>,
             dependabot: bool| Expected {
        name,
        also,
        removed,
        dependabot,
    };
    vec![
        e(
            "actionlint",
            &[],
            paths(&[".github/actionlint.yaml"]),
            false,
        ),
        e(
            "agnix",
            &[],
            vec![
                ".agnix.toml".into(),
                sc("agnix-assemble-comment.sh"),
                sc("agnix-file-issues.sh"),
                sc("agnix-introduced-errors.sh"),
                wf("agnix.yml"),
            ],
            true,
        ),
        e("aislop", &[], vec![wf("aislop.yml")], true),
        e("audit-skill", &[], vec![wf("audit-skill.yml")], true),
        e(
            "cargo-deny",
            &["rust-ci"],
            vec![
                sc("cargo-deny.sh"),
                sc("check-toolchain-version.sh"),
                wf("cargo-deny.yml"),
                wf("rust-ci.yml"),
                "deny.toml".into(),
            ],
            true,
        ),
        e(
            "claude-settings",
            &[],
            paths(&[".claude/settings.json"]),
            false,
        ),
        e("coderabbit", &[], paths(&[".coderabbit.yaml"]), false),
        e(
            "commit-types",
            &[],
            vec![
                ".github/commit-types.txt".into(),
                sc("check-commit-types.sh"),
                wf("commit-types.yml"),
            ],
            true,
        ),
        e(
            "conventional-commits",
            &["commit-types"],
            vec![
                ".github/commit-types.txt".into(),
                sc("check-commit-types.sh"),
                wf("commit-types.yml"),
                wf("conventional-commits.yml"),
            ],
            true,
        ),
        e("dependabot", &[], vec![DEPENDABOT.into()], false),
        e("gitignore", &[], paths(&[".gitignore"]), false),
        e("hk", &[], paths(&["hk.pkl", "hk.shared.pkl"]), false),
        e(
            "inline-scripts",
            &[],
            vec![sc("check-inline-scripts.sh"), wf("inline-scripts.yml")],
            true,
        ),
        e(
            "mise",
            &[],
            paths(&[".mise/conf.d/repo.toml", "mise.lock", "mise.toml"]),
            false,
        ),
        e(
            "plumber",
            &[],
            vec![".plumber.yaml".into(), wf("plumber.yml")],
            true,
        ),
        e(
            "pr-auto-rebase",
            &[],
            vec![sc("auto-rebase.sh"), wf("pr-auto-rebase.yml")],
            true,
        ),
        e(
            "pull-request-template",
            &[],
            paths(&[".github/pull_request_template.md"]),
            false,
        ),
        e(
            "release",
            &[],
            vec![
                wf("dist-check.yml"),
                wf("release.yml"),
                "dist-workspace.toml".into(),
            ],
            true,
        ),
        e(
            "release-automerge",
            &[],
            vec![sc("release-automerge.sh"), wf("release-automerge.yml")],
            true,
        ),
        // Not rendered unless options.versionFile is set (the full render here does not set it), so leaving it out changes nothing;
        // release_builtins.rs renders it.
        e("release-bump", &[], vec![], false),
        e(
            "release-please",
            &[],
            vec![
                sc("tag-release.sh"),
                wf("release-please.yml"),
                ".release-please-manifest.json".into(),
                "release-please-config.json".into(),
            ],
            true,
        ),
        e("rumdl", &[], paths(&[".rumdl.toml"]), false),
        e(
            "rust-ci",
            &[],
            vec![sc("check-toolchain-version.sh"), wf("rust-ci.yml")],
            true,
        ),
        e(
            "rust-toolchain",
            &["rust-ci"],
            vec![
                sc("check-toolchain-version.sh"),
                wf("rust-ci.yml"),
                "rust-toolchain.toml".into(),
            ],
            true,
        ),
        e("scaffold-check", &[], vec![wf("scaffold-check.yml")], true),
        e("security", &[], paths(&["SECURITY.md"]), false),
        e(
            "sync-labels",
            &[],
            vec![
                ".github/labels.txt".into(),
                sc("sync-labels.sh"),
                wf("sync-labels.yml"),
            ],
            true,
        ),
        // Not rendered by the aggregators (it is a consumer workflow a repository adds itself), so leaving it out changes nothing.
        e("upgrade", &[], vec![], false),
        e(
            "zizmor",
            &[],
            vec![".github/zizmor.yml".into(), wf("zizmor.yml")],
            true,
        ),
    ]
}

#[test]
fn every_builtin_has_an_entry_in_the_table_of_what_leaving_it_out_removes() {
    let mut names: Vec<&str> = expected().iter().map(|e| e.name).collect();
    names.sort_unstable();
    assert_eq!(names, folders());
}

// mise.toml, hk.shared.pkl, the seeded hk.pkl, .rumdl.toml and the zizmor workflow header read which builtins are present and change too;
// presence_templates.rs asserts exactly how.
const READ_PRESENCE: [&str; 5] = [
    "mise.toml",
    "hk.shared.pkl",
    "hk.pkl",
    ".rumdl.toml",
    ".github/workflows/zizmor.yml",
];

#[test]
fn leaving_a_builtin_out_removes_what_it_owns_changes_only_dependabot_yml_and_the_files_that_read_presence_and_adds_nothing()
 {
    let full = render(&[], "");
    assert_eq!(full.len(), 51);
    for entry in expected() {
        let mut without = vec![entry.name];
        without.extend(entry.also);
        let r = render(&without, "");
        let mut gone: Vec<String> = full
            .keys()
            .filter(|p| !r.contains_key(*p))
            .cloned()
            .collect();
        gone.sort();
        let mut removed = entry.removed.clone();
        removed.sort();
        assert_eq!(gone, removed, "{}", entry.name);
        let mut changed: Vec<String> = full
            .keys()
            .filter(|p| {
                r.get(*p).is_some_and(|c| c != &full[*p]) && !READ_PRESENCE.contains(&p.as_str())
            })
            .cloned()
            .collect();
        changed.sort();
        let want: Vec<String> = if entry.dependabot {
            vec![DEPENDABOT.into()]
        } else {
            vec![]
        };
        assert_eq!(changed, want, "{}", entry.name);
        let added: Vec<&String> = r.keys().filter(|p| !full.contains_key(*p)).collect();
        assert!(added.is_empty(), "{}: {added:?}", entry.name);
    }
}

#[test]
fn dependabots_exclude_list_follows_the_workflows_that_are_rendered() {
    let full = render(&[], "");
    let r = render(&["plumber"], "");
    assert!(full[DEPENDABOT].contains(&wf("plumber.yml")));
    assert!(!r[DEPENDABOT].contains(&wf("plumber.yml")));
    assert!(r[DEPENDABOT].contains(&wf("rust-ci.yml")));
}

fn error_of(without: &[&str]) -> String {
    eval_config(without, "", COMPOSITION, "").expect_err("the render is refused")
}

#[test]
fn a_name_that_is_not_a_builtin_is_an_error_that_lists_the_builtins_and_so_is_an_empty_or_a_repeated_name()
 {
    assert!(
        Regex::new(r#"(?s)"plumbr", which is not a builtin of this package version\. Builtins: actionlint, agnix, aislop.*plumber.*zizmor"#)
            .unwrap()
            .is_match(&error_of(&["plumbr"]))
    );
    assert!(
        Regex::new(r#""rumdl ", "", which are not a builtin of this package version"#)
            .unwrap()
            .is_match(&error_of(&["rumdl ", ""]))
    );
    assert!(
        Regex::new(r"names a builtin more than once: rumdl")
            .unwrap()
            .is_match(&error_of(&["rumdl", "mise", "rumdl"]))
    );
}

#[test]
fn leaving_out_a_builtin_that_another_needs_fails_and_names_both_with_the_way_out() {
    for (needs, dependent, reason) in [
        (
            "conventional-commits",
            "commit-types",
            r"check-commit-types\.sh reads the copy of the commit types",
        ),
        (
            "rust-toolchain",
            "rust-ci",
            r"reads the channel from rust-toolchain\.toml",
        ),
        (
            "cargo-deny",
            "rust-ci",
            r"checks the toolchain lines of \.github/workflows/cargo-deny\.yml",
        ),
    ] {
        let message = error_of(&[needs]);
        assert!(
            message.contains(&format!(
                "options.without leaves out \"{needs}\", which \"{dependent}\" needs"
            )),
            "{message}"
        );
        assert!(Regex::new(reason).unwrap().is_match(&message), "{message}");
        assert!(
            message.contains(&format!(
                "Leave \"{dependent}\" out too (options.without), or keep \"{needs}\"."
            )),
            "{message}"
        );
        // leaving the dependent out as well is the way out, and it renders
        let ok = eval_config(&[needs, dependent], "", COMPOSITION, "").unwrap();
        assert!(!ok["files"].as_array().unwrap().is_empty());
    }
}

#[test]
fn mise_and_hk_are_not_hard_requirements_a_repository_can_supply_mise_toml_and_hk_pkl_itself() {
    let r = render(&["mise", "hk"], "");
    assert!(!r.contains_key("mise.toml"));
    assert!(!r.contains_key("hk.shared.pkl"));
    assert!(r.contains_key(&wf("scaffold-check.yml")));
    assert!(r.contains_key(&wf("rust-ci.yml")));
}

#[test]
fn leaving_every_builtin_out_renders_nothing_and_is_not_an_error() {
    let names = folders();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let contract = eval_config(&refs, "", COMPOSITION, "").unwrap();
    assert_eq!(contract["files"], serde_json::json!([]));
}

#[test]
fn a_name_that_a_toggle_already_removes_is_fine_and_the_render_equals_the_toggle_alone() {
    let toggle_skills = render(&[], "skills = false");
    assert_eq!(render(&["audit-skill"], "skills = false"), toggle_skills);
    let toggle_binary = render(&[], "binary = false");
    assert_eq!(render(&["release"], "binary = false"), toggle_binary);
    assert!(!toggle_skills.contains_key(&wf("audit-skill.yml")));
}

#[test]
fn features_all_with_the_rust_builtins_left_out_still_renders_with_no_placeholder_left_in_any_file()
{
    let r = render(
        &["rust-ci", "rust-toolchain", "cargo-deny"],
        "features = \"all\"",
    );
    assert!(!r.contains_key(&wf("rust-ci.yml")));
    for (path, content) in &r {
        assert!(!content.contains("@@"), "{path}");
    }
}

#[test]
fn a_preset_that_amends_a_left_out_builtin_fails_and_says_options_without_may_have_left_it_out() {
    let composition = COMPOSITION.replace(
        "...Builtins.standardWorkflows(options)",
        "...Presets.amendWorkflow(Builtins.standardWorkflows(options), \"rust-ci\", (w) -> w)",
    );
    let imports = format!("import \"file://{}/pkl/Presets.pkl\"", root().display());
    let error =
        eval_config(&["rust-ci"], "", &composition, &imports).expect_err("the render is refused");
    assert!(
        Regex::new(r#"(?s)preset: workflow "rust-ci" is not a builtin of this package version\. Builtins: .*A builtin that options\.without leaves out is not in the mapping either\."#)
            .unwrap()
            .is_match(&error),
        "{error}"
    );
}

#[test]
fn a_single_builtin_function_still_renders_when_its_builtin_is_left_out_without_only_affects_the_aggregators()
 {
    let composition = "workflows { [\"rust-ci\"] = Builtins.rustCi(options) }\nfiles {}";
    let contract = eval_config(&["rust-ci"], "", composition, "").unwrap();
    let got: Vec<&str> = contract["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(got, [wf("rust-ci.yml")]);
}

/// Renders a configuration into a repository with the CLI, the way a consumer would; returns the exit code and what it printed.
fn cli_render(repo: &std::path::Path, without: &[&str]) -> (i32, String) {
    let (_keep, path) = config(without, "", COMPOSITION, "");
    let pkl = root().join("pkl");
    let out = run(
        &[
            "render",
            &path.to_string_lossy(),
            "--root",
            &repo.to_string_lossy(),
            "--allow-local-package",
            &pkl.to_string_lossy(),
        ],
        &root(),
    );
    (out.code, out.err)
}

#[test]
fn in_an_adopted_repository_leaving_a_builtin_out_deletes_the_files_it_rendered_and_a_seeded_file_stays()
 {
    let repo = git_repo(&[("README.md", "x\n")]);
    assert_eq!(cli_render(repo.path(), &[]).0, 0);
    commit_all(repo.path(), "render");
    assert!(has(repo.path(), ".plumber.yaml"));
    assert!(has(repo.path(), "hk.pkl"));
    assert_eq!(cli_render(repo.path(), &["plumber", "hk"]).0, 0);
    assert!(!has(repo.path(), ".plumber.yaml"));
    assert!(!has(repo.path(), &wf("plumber.yml")));
    assert!(!has(repo.path(), "hk.shared.pkl"));
    // hk.pkl was seeded: it is never deleted, and it now imports a file that is gone, which the docs say.
    assert!(has(repo.path(), "hk.pkl"));
    assert!(has(repo.path(), &wf("rust-ci.yml")));
}

#[test]
fn in_an_adopted_repository_a_file_of_a_left_out_builtin_that_was_edited_by_hand_blocks_the_render()
{
    let repo = git_repo(&[("README.md", "x\n")]);
    assert_eq!(cli_render(repo.path(), &[]).0, 0);
    put(repo.path(), ".plumber.yaml", "mine\n");
    commit_all(repo.path(), "edited by hand");
    let (code, err) = cli_render(repo.path(), &["plumber"]);
    assert_eq!(code, 3, "{err}");
    assert!(err.contains("it is not deleted"), "{err}");
    assert_eq!(get(repo.path(), ".plumber.yaml"), "mine\n");
}

#[test]
fn turning_a_builtin_back_on_writes_its_files_again() {
    let repo = git_repo(&[("README.md", "x\n")]);
    cli_render(repo.path(), &["plumber"]);
    assert!(!has(repo.path(), ".plumber.yaml"));
    commit_all(repo.path(), "plumber out");
    assert_eq!(cli_render(repo.path(), &[]).0, 0);
    assert!(has(repo.path(), ".plumber.yaml"));
}
