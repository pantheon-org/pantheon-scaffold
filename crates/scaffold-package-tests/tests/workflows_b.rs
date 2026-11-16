//! Ported from `runner/test/workflows-b.test.ts`: the second wave of workflows, for three kinds of repository, each compared with the pinned
//! copy of the repository it stands for.

use regex::Regex;
use scaffold_package_tests::pkg::{
    actionlint, content, expect_pins, git_blob_sha, jobs, paths, read, render, root, unpin,
};
use scaffold_package_tests::{compare_directives, compare_workflows};
use serde_json::Value;
use std::sync::LazyLock;

static ADR: LazyLock<Value> = LazyLock::new(|| render("workflows-b"));
static KIT: LazyLock<Value> = LazyLock::new(|| render("workflows-b-kit"));
static AUDITOR: LazyLock<Value> = LazyLock::new(|| render("workflows-b-auditor"));

fn text(rel: &str) -> String {
    read(&format!("crates/scaffold-package-tests/upstream/{rel}"))
}

// The pinned copies: pantheon-adr at 1e2fd46 (the other two repositories with a binary and a skill have the same blobs, except for a comment
// in release-please.yml), the library repository at 42f48b4, and pantheon-skill-auditor at b9e706e.
const ADR_BLOBS: [(&str, &str); 5] = [
    ("release-please", "8b04f6aa"),
    ("zizmor", "c7c1412f"),
    ("agnix", "8c3d0ba0"),
    ("aislop", "1d1b8344"),
    ("dependabot", "349959db"),
];
const KIT_BLOBS: [(&str, &str); 6] = [
    ("release-please", "26950f15"),
    ("zizmor", "e26564d1"),
    ("agnix", "b34c6731"),
    ("aislop", "a6b31de4"),
    ("rust-ci", "6e1725bb"),
    ("dependabot", "e8a1d9f4"),
];

#[test]
fn the_pinned_copies_are_the_repositories_files_at_the_pinned_commits() {
    for (name, blob) in ADR_BLOBS {
        assert_eq!(
            &git_blob_sha(&text(&format!("workflows/{name}.yml")))[..8],
            blob,
            "adr {name}"
        );
    }
    for (name, blob) in KIT_BLOBS {
        assert_eq!(
            &git_blob_sha(&text(&format!("workflows-kit/{name}.yml")))[..8],
            blob,
            "kit {name}"
        );
    }
    assert_eq!(
        &git_blob_sha(&text("workflows-auditor/zizmor.yml"))[..8],
        "a63f097a"
    );
}

fn sorted(contract: &Value) -> Vec<String> {
    let mut got = paths(contract);
    got.sort();
    got
}

#[test]
fn the_files_each_kind_of_repository_gets() {
    assert_eq!(
        sorted(&ADR),
        [
            ".agnix.toml",
            ".github/dependabot.yml",
            ".github/workflows/agnix.yml",
            ".github/workflows/aislop.yml",
            ".github/workflows/release-please.yml",
            ".github/workflows/rust-ci.yml",
            ".github/workflows/zizmor.yml",
            ".github/zizmor.yml",
        ]
    );
    // No binary: no cargo-dist release.yml to configure zizmor for, so no .github/zizmor.yml.
    assert_eq!(
        sorted(&KIT),
        [
            ".agnix.toml",
            ".github/dependabot.yml",
            ".github/workflows/agnix.yml",
            ".github/workflows/aislop.yml",
            ".github/workflows/release-please.yml",
            ".github/workflows/rust-ci.yml",
            ".github/workflows/zizmor.yml",
        ]
    );
    assert_eq!(
        sorted(&AUDITOR),
        [
            ".agnix.toml",
            ".github/dependabot.yml",
            ".github/workflows/zizmor.yml",
            ".github/zizmor.yml"
        ]
    );
}

// Each rendering is compared with the pinned copy of the repository it stands for: P.1, the comments a tool acts on, job ids and names.

fn equals_under_p1(contract: &Value, path: &str, upstream: &str) {
    let result = compare_workflows(&unpin(upstream), &unpin(content(contract, path))).unwrap();
    assert!(result.equal, "{path}: {:?}", result.differences);
    assert!(result.duplicate_keys.is_empty(), "{path}");
}

fn keeps_directives_and_jobs(contract: &Value, path: &str, upstream: &str) {
    assert_eq!(
        compare_directives(upstream, content(contract, path)),
        Vec::<String>::new(),
        "{path}"
    );
    assert_eq!(jobs(content(contract, path)), jobs(upstream), "{path}");
}

/// The two TypeScript tests of a workflow: it equals the pinned copy under P.1, and it keeps the comments a tool acts on and its job ids and
/// names.
macro_rules! workflow {
    ($module:ident, $contract:expr, $path:expr, $upstream:expr) => {
        mod $module {
            use super::*;

            #[test]
            fn equals_the_pinned_copy_under_p1() {
                equals_under_p1(&$contract, $path, &$upstream);
            }

            #[test]
            fn keeps_the_comments_a_tool_acts_on_and_its_job_ids_and_names() {
                keeps_directives_and_jobs(&$contract, $path, &$upstream);
            }
        }
    };
}

workflow!(
    adr_like_release_please,
    ADR,
    ".github/workflows/release-please.yml",
    text("workflows/release-please.yml")
);
workflow!(
    adr_like_zizmor,
    ADR,
    ".github/workflows/zizmor.yml",
    text("workflows/zizmor.yml")
);
workflow!(
    adr_like_agnix,
    ADR,
    ".github/workflows/agnix.yml",
    text("workflows/agnix.yml")
);
workflow!(
    adr_like_aislop,
    ADR,
    ".github/workflows/aislop.yml",
    text("workflows/aislop.yml")
);
workflow!(
    adr_like_rust_ci,
    ADR,
    ".github/workflows/rust-ci.yml",
    text("workflows/rust-ci.yml")
);
workflow!(
    library_like_release_please,
    KIT,
    ".github/workflows/release-please.yml",
    text("workflows-kit/release-please.yml")
);
workflow!(
    library_like_zizmor,
    KIT,
    ".github/workflows/zizmor.yml",
    text("workflows-kit/zizmor.yml")
);
workflow!(
    library_like_agnix,
    KIT,
    ".github/workflows/agnix.yml",
    text("workflows-kit/agnix.yml")
);
workflow!(
    library_like_aislop,
    KIT,
    ".github/workflows/aislop.yml",
    text("workflows-kit/aislop.yml")
);
workflow!(
    library_like_rust_ci,
    KIT,
    ".github/workflows/rust-ci.yml",
    text("workflows-kit/rust-ci.yml")
);
workflow!(
    auditor_like_zizmor,
    AUDITOR,
    ".github/workflows/zizmor.yml",
    text("workflows-auditor/zizmor.yml")
);

// Decision 86: a repository with a binary also tells Dependabot to leave the two artifact actions of the generated release.yml alone.
// The only difference from the repository's own file is that ignore list, added here to the pinned copy.
fn with_dist_ignore(upstream: &str) -> String {
    upstream.replacen(
        "    cooldown:\n      default-days: 7\n    labels:\n      - \"dependencies\"\n      - \"github-actions\"",
        "    cooldown:\n      default-days: 7\n    ignore:\n      - dependency-name: \"actions/upload-artifact\"\n      - dependency-name: \"actions/download-artifact\"\n    labels:\n      - \"dependencies\"\n      - \"github-actions\"",
        1,
    )
}

// The package also tells Dependabot to leave the workflow files it renders alone (`exclude-paths`), which the repositories' own files do not
// have. The list is every standard workflow of the repository's options, in the order of Builtins.standardWorkflows, not only the files this
// fixture renders.
const STANDARD: [&str; 13] = [
    "plumber",
    "sync-labels",
    "pr-auto-rebase",
    "release-automerge",
    "commit-types",
    "conventional-commits",
    "release-please",
    "zizmor",
    "agnix",
    "aislop",
    "rust-ci",
    "cargo-deny",
    "inline-scripts",
];

fn with_excluded(upstream: &str, skills: bool, binary: bool) -> String {
    let mut names: Vec<&str> = STANDARD.to_vec();
    if skills {
        names.push("audit-skill");
    }
    names.push("scaffold-check");
    if binary {
        names.push("dist-check");
    }
    let at = upstream
        .find("package-ecosystem: \"github-actions\"")
        .expect("a github-actions entry");
    let cooldown = "    cooldown:\n      default-days: 7\n";
    let insert = at + upstream[at..].find(cooldown).expect("a cooldown") + cooldown.len();
    let list: String = names
        .iter()
        .map(|n| format!("      - \".github/workflows/{n}.yml\"\n"))
        .collect();
    format!(
        "{}    exclude-paths:\n{list}{}",
        &upstream[..insert],
        &upstream[insert..]
    )
}

// The library repository's files are all rendered, so the github-actions entry would have nothing to scan (Dependabot fails with
// dependency_file_not_found) and is left out: the expected file is the repository's own with that entry cut off.
fn cargo_only(upstream: &str) -> String {
    let at = upstream
        .find("  # Keeps the SHA pins")
        .expect("the upstream dependabot.yml has no github-actions entry to cut");
    format!("{}\n", upstream[..at].trim_end())
}

#[test]
fn adr_like_dependabot_yml_equals_the_pinned_copy_under_p1() {
    let expected = with_excluded(
        &with_dist_ignore(&text("workflows/dependabot.yml")),
        true,
        true,
    );
    equals_under_p1(&ADR, ".github/dependabot.yml", &expected);
}

#[test]
fn library_like_dependabot_yml_equals_the_pinned_copy_under_p1() {
    equals_under_p1(
        &KIT,
        ".github/dependabot.yml",
        &cargo_only(&text("workflows-kit/dependabot.yml")),
    );
}

#[test]
fn the_verbatim_config_files_are_byte_for_byte_the_repositories_files() {
    assert_eq!(
        git_blob_sha(content(&ADR, ".github/zizmor.yml")),
        "152581e2ee6553c9d6a4f83838763e478f24d399"
    );
    assert_eq!(
        git_blob_sha(content(&AUDITOR, ".github/zizmor.yml")),
        "9e9c78bbc3117929af57b362572d7714f7eebad6"
    );
    assert_eq!(
        git_blob_sha(content(&ADR, ".agnix.toml")),
        "760a2847b7a01acb4af786c3ca75796ee78b9ac5"
    );
    assert_eq!(
        git_blob_sha(content(&KIT, ".agnix.toml")),
        "0a08924f0c41e390032e8a493aaf0291e7d8da41"
    );
}

#[test]
fn a_repository_without_a_skill_differs_from_one_with_a_skill_only_in_the_first_line_of_agnix_toml()
{
    let a: Vec<&str> = content(&ADR, ".agnix.toml").split('\n').collect();
    let k: Vec<&str> = content(&KIT, ".agnix.toml").split('\n').collect();
    assert_eq!(k[1..], a[1..]);
    assert_ne!(k[0], a[0]);
}

#[test]
fn the_tool_versions_the_workflows_pin_are_the_ones_in_tools_pkl_the_same_ones_mise_toml_pins() {
    let tools = std::fs::read_to_string(root().join("pkl/Tools.pkl")).unwrap();
    let tool = |name: &str| -> String {
        Regex::new(&format!(r#"const {name}: String = "([^"]+)""#))
            .unwrap()
            .captures(&tools)
            .unwrap_or_else(|| panic!("no {name} in Tools.pkl"))[1]
            .to_owned()
    };
    let agnix = content(&ADR, ".github/workflows/agnix.yml");
    assert!(agnix.contains(&format!("AGNIX_VERSION: {}", tool("agnix"))));
    assert!(agnix.contains(&tool("agnixSha256")));
    assert!(
        content(&ADR, ".github/workflows/zizmor.yml")
            .contains(&format!("version: {}", tool("zizmor")))
    );
    assert!(
        content(&ADR, ".github/workflows/aislop.yml")
            .contains(&format!("aislop@{}", tool("aislop")))
    );
}

#[test]
fn the_new_pins_are_each_a_commit_with_a_version_tag_and_the_workflows_that_use_them_report_them() {
    let wanted = [
        "googleapis/release-please-action",
        "zizmorcore/zizmor-action",
        "actions/setup-node",
        "peter-evans/find-comment",
        "peter-evans/create-or-update-comment",
    ];
    let pins: Vec<Value> = ADR["pins"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| wanted.contains(&p["action"].as_str().unwrap()))
        .cloned()
        .collect();
    expect_pins(&Value::Array(pins), &wanted);
}

#[test]
fn wave_b_uses_no_escape_hatch_in_any_of_the_three_renders() {
    for contract in [&*ADR, &*KIT, &*AUDITOR] {
        assert_eq!(contract["escapeHatch"]["uses"], 0);
    }
}

#[test]
fn actionlint_is_clean_on_the_rendered_workflows_of_a_repository_with_a_binary_and_of_one_without()
{
    for (label, contract) in [("adr-like", &*ADR), ("library-like", &*KIT)] {
        let workflows: Vec<(&str, &str)> = contract["files"]
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
        assert!(!workflows.is_empty(), "{label}");
        assert_eq!(actionlint(&workflows), "", "{label} actionlint output");
    }
}
