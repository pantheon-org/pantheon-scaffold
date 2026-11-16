//! Ported from `runner/test/phase5-verbatim.test.ts`. Phase 5 wave A: the verbatim files of the `binary` toggle and the one `.gitignore`. Git
//! blobs are those of pantheon-adr at 1e2fd46; the same three binary files are identical in pantheon-journal and pantheon-skill-auditor, and
//! .gitignore in all three (survey, 08-11-2026).

use scaffold_package_tests::cli::run;
use scaffold_package_tests::pkg::{
    content, file, fixture, git_blob_sha, mode, paths, read, render, root,
};
use scaffold_package_tests::repo::{has, temp_dir};
use serde_json::Value;
use std::sync::LazyLock;

static BINARY: LazyLock<Value> = LazyLock::new(|| render("phase5-binary"));
static LIBRARY: LazyLock<Value> = LazyLock::new(|| render("phase5-library"));

fn sorted(contract: &Value) -> Vec<String> {
    let mut got = paths(contract);
    got.sort();
    got
}

#[test]
fn a_repository_with_a_binary_gets_gitignore_actionlint_yaml_dist_workspace_toml_and_release_yml() {
    assert_eq!(
        sorted(&BINARY),
        [
            ".github/actionlint.yaml",
            ".github/workflows/release.yml",
            ".gitignore",
            "dist-workspace.toml"
        ]
    );
}

#[test]
fn a_repository_without_a_binary_gets_gitignore_only() {
    assert_eq!(paths(&LIBRARY), [".gitignore"]);
}

/// The one TypeScript test of the one entry of its `BLOBS`.
#[test]
fn actionlint_yaml_is_byte_for_byte_the_pinned_pantheon_adr_file_owned_and_not_executable() {
    let path = ".github/actionlint.yaml";
    assert_eq!(
        git_blob_sha(content(&BINARY, path)),
        "010277fad034ccbc7ec5b08827de18b22c23b703"
    );
    assert_eq!(file(&BINARY, path)["class"], "owned");
    assert_eq!(mode(&BINARY, path), "0644");
}

// Decision 85: Dependabot bumped upload-artifact and download-artifact inside the generated release.yml on 04-11-2026 and
// dist-workspace.toml, which tells dist which commits to write, was not changed, so `dist generate --check` fails on the repositories' main.
// The file is rendered with the two commits release.yml uses, v7.0.2 and v8.0.2, and is otherwise the pantheon-adr file (blob e08f6a7 at
// 1e2fd46).
#[test]
fn dist_workspace_toml_is_the_pantheon_adr_file_with_exactly_the_two_action_commits_that_release_yml_uses()
 {
    let upstream = read("runner/test/upstream/phase5/pantheon-adr/dist-workspace.toml");
    assert_eq!(
        git_blob_sha(&upstream),
        "e08f6a76e797aebe650685131f709dc53e0dcb20"
    );
    let expected = upstream
        .replacen(
            "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
            "cf430e030ddbb5b0abf93d22962f4752f3646cd9",
            1,
        )
        .replacen(
            "3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c",
            "9000827ccba6bdab643e8b6fd33ac0654aef8333",
            1,
        );
    assert_eq!(content(&BINARY, "dist-workspace.toml"), expected);
    assert_eq!(file(&BINARY, "dist-workspace.toml")["class"], "owned");
    assert_eq!(mode(&BINARY, "dist-workspace.toml"), "0644");
}

// Decision 87: the runner keeps its package cache in .pkl-gen/cache, which must not be committed, so the shipped .gitignore is the
// pantheon-adr file (blob 66e8098 at 1e2fd46) plus the line `.pkl-gen/cache/`.
#[test]
fn gitignore_is_the_pantheon_adr_file_plus_exactly_the_line_pkl_gen_cache_owned_and_not_executable()
{
    let upstream = read("runner/test/upstream/phase5/pantheon-adr/.gitignore");
    assert_eq!(
        git_blob_sha(&upstream),
        "66e8098804d3b4b7ddc5cf4f00cfa8dd94d2c49f"
    );
    assert_eq!(
        content(&BINARY, ".gitignore"),
        format!("{upstream}.pkl-gen/cache/\n")
    );
    assert_eq!(file(&BINARY, ".gitignore")["class"], "owned");
    assert_eq!(mode(&BINARY, ".gitignore"), "0644");
}

#[test]
fn gitignore_is_the_same_in_both_kinds_of_repository_target_is_the_decided_pick_decision_74() {
    assert_eq!(
        content(&LIBRARY, ".gitignore"),
        content(&BINARY, ".gitignore")
    );
    assert_eq!(
        content(&LIBRARY, ".gitignore").split('\n').next().unwrap(),
        "target/"
    );
}

#[test]
fn release_yml_is_external_listed_with_no_content_because_dist_generate_writes_it() {
    let f = file(&BINARY, ".github/workflows/release.yml");
    assert_eq!(f["class"], "external");
    assert!(f.get("content").is_none());
}

#[test]
fn an_external_release_yml_is_never_written_and_an_existing_one_is_left_alone() {
    let dir = temp_dir();
    let config = fixture("phase5-binary");
    let pkl = root().join("pkl");
    let rendered = run(
        &[
            "render",
            &config.to_string_lossy(),
            "--root",
            &dir.path().to_string_lossy(),
            "--allow-local-package",
            &pkl.to_string_lossy(),
        ],
        &root(),
    );
    assert_eq!(rendered.code, 0, "{}", rendered.err);
    assert!(!has(dir.path(), ".github/workflows/release.yml"));
    assert!(has(dir.path(), "dist-workspace.toml"));
}
