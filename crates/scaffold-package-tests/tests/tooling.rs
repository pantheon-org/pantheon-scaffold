//! Ported from `runner/test/tooling.test.ts`: the tooling files the package renders for a repository, against the pinned upstream files.

use scaffold_package_tests::parse_toml;
use scaffold_package_tests::pkg::{
    content, file, git_blob_sha, mode, paths, render, upstream_json,
};
use serde_json::Value;
use std::sync::LazyLock;

static CONTRACT: LazyLock<Value> = LazyLock::new(|| render("tooling"));
static PINNED: LazyLock<Value> = LazyLock::new(|| upstream_json("tooling-blobs.json"));

/// The one deliberate difference from pantheon-adr's files: .rumdl.toml ends with this table. The pull request template starts with an
/// HTML comment and a level 2 heading, so MD041 can never pass for it, and the rendered hook would fail the first commit of every project.
/// The same text is appended to pkl/Builtins/rumdl/assets/rumdl.toml. Keep it last in the file, because the test below strips it from the end.
const RUMDL_ADDITION: &str = "\n# The pull request template starts with an HTML comment and a level 2 heading, so MD041 can never pass for it.\n# Every other file is still checked by MD041.\n[per-file-ignores]\n\".github/pull_request_template.md\" = [\"MD041\"]\n";

/// The nine scripts the four repos share (baseline SP-0.1): two come from baseFiles, seven from toolingFiles.
const NINE: [&str; 9] = [
    "agnix-assemble-comment",
    "agnix-file-issues",
    "agnix-introduced-errors",
    "auto-rebase",
    "cargo-deny",
    "check-commit-types",
    "check-inline-scripts",
    "release-automerge",
    "sync-labels",
];

fn nine() -> Vec<String> {
    NINE.iter()
        .map(|n| format!(".github/scripts/{n}.sh"))
        .collect()
}

#[test]
fn the_tooling_files_are_the_nine_shared_scripts_check_toolchain_version_sh_the_two_configs_and_the_base_files()
 {
    let mut got = paths(&CONTRACT);
    got.sort();
    assert_eq!(
        got,
        [
            ".claude/settings.json",
            ".github/scripts/agnix-assemble-comment.sh",
            ".github/scripts/agnix-file-issues.sh",
            ".github/scripts/agnix-introduced-errors.sh",
            ".github/scripts/auto-rebase.sh",
            ".github/scripts/cargo-deny.sh",
            ".github/scripts/check-commit-types.sh",
            ".github/scripts/check-inline-scripts.sh",
            ".github/scripts/check-toolchain-version.sh",
            ".github/scripts/release-automerge.sh",
            ".github/scripts/sync-labels.sh",
            ".rumdl.toml",
            "deny.toml",
            "rust-toolchain.toml",
        ]
    );
}

#[test]
fn all_nine_shared_scripts_render_executable_and_every_config_renders_not_executable() {
    for path in nine() {
        assert_eq!(mode(&CONTRACT, &path), "0755", "{path}");
    }
    for path in [
        ".rumdl.toml",
        ".claude/settings.json",
        "deny.toml",
        "rust-toolchain.toml",
    ] {
        assert_eq!(mode(&CONTRACT, path), "0644", "{path}");
    }
}

#[test]
fn every_file_is_an_owned_file_with_content_and_the_phase_3_files_use_no_escape_hatch() {
    for f in CONTRACT["files"].as_array().unwrap() {
        let path = f["path"].as_str().unwrap();
        assert_eq!(f["class"], "owned", "{path}");
        assert!(f.get("content").is_some(), "{path}");
    }
    assert_eq!(CONTRACT["escapeHatch"]["uses"], 0);
}

/// The nine TypeScript tests, one for each entry of `tooling-blobs.json`: every file is the pinned pantheon-adr file byte for byte,
/// and `.rumdl.toml` is that file plus the one per-file-ignores table.
#[test]
fn each_pinned_file_renders_byte_for_byte_as_the_pinned_pantheon_adr_file_and_rumdl_toml_with_its_one_table()
 {
    let pinned = PINNED.as_object().unwrap();
    assert_eq!(pinned.len(), 9);
    for (path, entry) in pinned {
        let blob = entry["blob"].as_str().unwrap();
        let text = content(&CONTRACT, path);
        if path != ".rumdl.toml" {
            assert_eq!(git_blob_sha(text), blob, "{path}");
            continue;
        }
        assert!(text.ends_with(RUMDL_ADDITION));
        assert_eq!(text.matches(RUMDL_ADDITION).count(), 1);
        assert_eq!(
            git_blob_sha(&text[..text.len() - RUMDL_ADDITION.len()]),
            blob
        );
    }
}

#[test]
fn the_rendered_rumdl_toml_ignores_md041_for_the_pull_request_template_and_nowhere_else() {
    let toml = parse_toml(content(&CONTRACT, ".rumdl.toml"));
    let ignores = toml["per-file-ignores"]
        .as_table()
        .expect("per-file-ignores");
    assert_eq!(
        ignores.keys().collect::<Vec<_>>(),
        [".github/pull_request_template.md"]
    );
    assert_eq!(
        ignores[".github/pull_request_template.md"]
            .as_array()
            .unwrap(),
        &vec![toml::Value::String("MD041".to_owned())]
    );
    let disabled = toml["global"]["disable"]
        .as_array()
        .expect("global.disable");
    assert!(!disabled.iter().any(|r| r.as_str() == Some("MD041")));
}

#[test]
fn the_pinned_list_covers_the_seven_new_scripts_and_both_configs_and_the_nine_scripts_are_all_accounted_for()
 {
    let pinned = PINNED.as_object().unwrap();
    assert_eq!(pinned.len(), 9);
    let shared = nine()
        .into_iter()
        .filter(|p| {
            pinned.contains_key(p)
                || ["cargo-deny", "check-inline-scripts"]
                    .iter()
                    .any(|n| p.ends_with(&format!("{n}.sh")))
        })
        .count();
    assert_eq!(shared, 9);
}

#[test]
fn the_one_script_that_is_not_executable_upstream_is_rendered_executable_on_purpose_and_it_is_the_only_one()
 {
    let differs: Vec<&str> = PINNED
        .as_object()
        .unwrap()
        .iter()
        .filter(|(p, entry)| {
            p.ends_with(".sh")
                && entry["upstreamMode"] != "100755"
                && file(&CONTRACT, p)["mode"] == "0755"
        })
        .map(|(p, _)| p.as_str())
        .collect();
    assert_eq!(differs, [".github/scripts/sync-labels.sh"]);
}
