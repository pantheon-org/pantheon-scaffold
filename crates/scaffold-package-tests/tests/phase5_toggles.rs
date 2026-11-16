//! Ported from `runner/test/phase5-toggles.test.ts`. Phase 5 wave C: the four toggles, each flipped from a base configuration, with the exact
//! set of files it removes and changes, and what the runner does with the files a toggle drops. The expected sets were read from the render and
//! checked against what each toggle owns.

use regex::Regex;
use scaffold_package_tests::cli::run;
use scaffold_package_tests::pkg::{eval, root};
use scaffold_package_tests::repo::{commit_all, get, git_repo, has, put, temp_dir};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

#[derive(Clone)]
struct Toggles {
    skills: bool,
    binary: bool,
    features: &'static str,
    composite_action: bool,
    skill_name: Option<&'static str>,
    crate_name: Option<&'static str>,
}

const BASE: Toggles = Toggles {
    skills: true,
    binary: true,
    features: "default",
    composite_action: false,
    skill_name: Some("demo-skill"),
    crate_name: Some("demo"),
};

/// Writes a scaffold.pkl for these options that uses the standard workflows and files. The folder is returned with the path, to keep it.
fn config(t: &Toggles) -> (tempfile::TempDir, PathBuf) {
    let dir = temp_dir();
    let names = [
        t.skill_name
            .map(|n| format!("skillName = \"{n}\""))
            .unwrap_or_default(),
        t.crate_name
            .map(|n| format!("crateName = \"{n}\""))
            .unwrap_or_default(),
    ]
    .join("\n  ");
    let text = format!(
        "amends \"file://{root}/pkl/Config.pkl\"\nimport \"file://{root}/pkl/Builtins.pkl\"\noptions {{\n  toolchain = \"1.96.1\"\n  {names}\n  skills = {skills}\n  binary = {binary}\n  features = \"{features}\"\n  compositeAction = {composite}\n}}\nworkflows {{ ...Builtins.standardWorkflows(options) }}\nfiles {{ ...Builtins.standardFiles(options) }}\n",
        root = root().display(),
        skills = t.skills,
        binary = t.binary,
        features = t.features,
        composite = t.composite_action,
    );
    let path = dir.path().join("toggles.pkl");
    std::fs::write(&path, text).unwrap();
    (dir, path)
}

type Files = BTreeMap<String, Value>;

fn render(t: &Toggles) -> Result<Files, String> {
    let (_keep, path) = config(t);
    let contract = eval(&path)?;
    Ok(contract["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["path"].as_str().unwrap().to_owned(), f.clone()))
        .collect())
}

static BASE_RENDER: LazyLock<Files> = LazyLock::new(|| render(&BASE).expect("the base renders"));

/// Flips one toggle and returns what was removed, changed and added, each sorted.
fn compare(flip: &Toggles) -> (Vec<String>, Vec<String>, Vec<String>) {
    let other = render(flip).expect("the flipped configuration renders");
    let removed = BASE_RENDER
        .keys()
        .filter(|p| !other.contains_key(*p))
        .cloned()
        .collect();
    let added = other
        .keys()
        .filter(|p| !BASE_RENDER.contains_key(*p))
        .cloned()
        .collect();
    let changed = BASE_RENDER
        .keys()
        .filter(|p| other.get(*p).is_some_and(|o| *o != BASE_RENDER[*p]))
        .cloned()
        .collect();
    (removed, changed, added)
}

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn skills_off_removes_and_changes_exactly_the_files_that_toggle_owns_and_adds_none() {
    let (removed, changed, added) = compare(&Toggles {
        skills: false,
        ..BASE
    });
    assert_eq!(removed, strings(&[".github/workflows/audit-skill.yml"]));
    assert_eq!(
        changed,
        strings(&[
            ".agnix.toml",
            ".coderabbit.yaml",
            ".github/dependabot.yml",
            ".github/workflows/agnix.yml",
            "SECURITY.md"
        ])
    );
    assert!(added.is_empty());
}

#[test]
fn binary_off_removes_and_changes_exactly_the_files_that_toggle_owns_and_adds_none() {
    let (removed, changed, added) = compare(&Toggles {
        binary: false,
        ..BASE
    });
    assert_eq!(
        removed,
        strings(&[
            ".github/actionlint.yaml",
            ".github/scripts/tag-release.sh",
            ".github/workflows/dist-check.yml",
            ".github/workflows/release.yml",
            ".github/zizmor.yml",
            "dist-workspace.toml",
        ])
    );
    assert_eq!(
        changed,
        strings(&[
            ".github/dependabot.yml",
            ".github/workflows/release-please.yml",
            ".github/workflows/zizmor.yml",
            "SECURITY.md",
            "release-please-config.json",
        ])
    );
    assert!(added.is_empty());
}

#[test]
fn features_all_removes_and_changes_exactly_the_files_that_toggle_owns_and_adds_none() {
    let (removed, changed, added) = compare(&Toggles {
        features: "all",
        ..BASE
    });
    assert!(removed.is_empty());
    assert_eq!(changed, strings(&[".github/workflows/rust-ci.yml"]));
    assert!(added.is_empty());
}

#[test]
fn composite_action_on_removes_and_changes_exactly_the_files_that_toggle_owns_and_adds_none() {
    let (removed, changed, added) = compare(&Toggles {
        composite_action: true,
        ..BASE
    });
    assert!(removed.is_empty());
    assert_eq!(
        changed,
        strings(&[
            ".github/workflows/audit-skill.yml",
            ".github/workflows/zizmor.yml",
            ".github/zizmor.yml"
        ])
    );
    assert!(added.is_empty());
}

#[test]
fn a_base_configuration_renders_the_same_twice_the_toggles_have_no_hidden_input() {
    let again = render(&BASE).unwrap();
    assert_eq!(
        again.keys().collect::<Vec<_>>(),
        BASE_RENDER.keys().collect::<Vec<_>>()
    );
    for (path, row) in BASE_RENDER.iter() {
        assert_eq!(&again[path], row, "{path}");
    }
}

#[test]
fn skills_on_without_skill_name_is_a_clear_error_that_names_the_option_and_the_way_out() {
    let error = render(&Toggles {
        skill_name: None,
        ..BASE
    })
    .expect_err("an error");
    assert!(
        Regex::new(
            r"(?s)options\.skillName is required when options\.skills is true.*skills = false"
        )
        .unwrap()
        .is_match(&error),
        "{error}"
    );
}

#[test]
fn skills_off_needs_no_skill_name_and_binary_off_needs_no_crate_name() {
    let r = render(&Toggles {
        skills: false,
        binary: false,
        skill_name: None,
        crate_name: None,
        ..BASE
    })
    .unwrap();
    assert!(r.contains_key("SECURITY.md"));
    assert!(!r.contains_key(".github/workflows/audit-skill.yml"));
}

#[test]
fn binary_on_without_crate_name_is_a_clear_error_that_names_the_option_and_the_way_out() {
    let error = render(&Toggles {
        crate_name: None,
        ..BASE
    })
    .expect_err("an error");
    assert!(
        Regex::new(
            r"(?s)options\.crateName is required when options\.binary is true.*binary = false"
        )
        .unwrap()
        .is_match(&error),
        "{error}"
    );
}

/// Renders a configuration into a repository with the CLI, the way a consumer would.
fn cli_render(repo: &Path, t: &Toggles) -> i32 {
    let (_keep, path) = config(t);
    let pkl = root().join("pkl");
    run(
        &[
            "render",
            &path.to_string_lossy(),
            "--root",
            &repo.to_string_lossy(),
            "--allow-local-package",
            &pkl.to_string_lossy(),
        ],
        &root(),
    )
    .code
}

fn listing(repo: &Path, sub: &str) -> Vec<String> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(repo.join(sub)).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == ".git" || name == ".pkl-gen" {
            continue;
        }
        let rel = if sub.is_empty() {
            name
        } else {
            format!("{sub}/{name}")
        };
        if entry.path().is_dir() {
            found.extend(listing(repo, &rel));
        } else {
            found.push(rel);
        }
    }
    found
}

#[test]
fn turning_skills_and_binary_off_deletes_only_the_files_they_owned_and_never_touches_the_external_release_yml()
 {
    let repo = git_repo(&[("README.md", "x\n")]);
    assert_eq!(cli_render(repo.path(), &BASE), 0);
    put(
        repo.path(),
        ".github/workflows/release.yml",
        "# written by dist generate\n",
    );
    commit_all(repo.path(), "render, and the generated release.yml");
    let before = listing(repo.path(), "");
    assert_eq!(
        cli_render(
            repo.path(),
            &Toggles {
                skills: false,
                binary: false,
                ..BASE
            }
        ),
        0
    );
    let after = listing(repo.path(), "");
    let mut deleted: Vec<String> = before.into_iter().filter(|p| !after.contains(p)).collect();
    deleted.sort();
    assert_eq!(
        deleted,
        strings(&[
            ".github/actionlint.yaml",
            ".github/scripts/tag-release.sh",
            ".github/workflows/audit-skill.yml",
            ".github/workflows/dist-check.yml",
            ".github/zizmor.yml",
            "dist-workspace.toml",
        ])
    );
    assert_eq!(
        get(repo.path(), ".github/workflows/release.yml"),
        "# written by dist generate\n"
    );
    assert!(has(repo.path(), ".github/workflows/plumber.yml"));
}

#[test]
fn turning_a_toggle_back_on_writes_its_files_again() {
    let repo = git_repo(&[("README.md", "x\n")]);
    cli_render(
        repo.path(),
        &Toggles {
            skills: false,
            ..BASE
        },
    );
    assert!(!has(repo.path(), ".github/workflows/audit-skill.yml"));
    commit_all(repo.path(), "skills off");
    assert_eq!(cli_render(repo.path(), &BASE), 0);
    assert!(has(repo.path(), ".github/workflows/audit-skill.yml"));
}
