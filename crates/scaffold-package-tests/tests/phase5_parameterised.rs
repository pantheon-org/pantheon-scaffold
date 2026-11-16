//! Ported from `runner/test/phase5-parameterised.test.ts`. Phase 5 wave B: the files that differ between the four repositories only in a
//! parameter (crate name, skill name) or a toggle. Each is compared with the repository's own pinned copy (main at 1e2fd46, 05c3d84,
//! b9e706e, 42f48b4, survey of 08-11-2026).

use regex::Regex;
use scaffold_package_tests::pkg::{content, file, git_blob_sha, mode, paths, read, render};
use scaffold_package_tests::{compare_directives, compare_workflows};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::LazyLock;

const REPOS: [&str; 4] = [
    "pantheon-adr",
    "pantheon-journal",
    "pantheon-skill-auditor",
    "pantheon-agentic-harness-dev-kit",
];

static RENDERED: LazyLock<BTreeMap<&'static str, Value>> = LazyLock::new(|| {
    REPOS
        .iter()
        .map(|r| (*r, render(&format!("real-workflows-{r}"))))
        .collect()
});

fn rendered(repo: &str) -> &'static Value {
    &RENDERED[repo]
}

fn upstream(repo: &str, path: &str) -> String {
    read(&format!("runner/test/upstream/phase5/{repo}/{path}"))
}

const BLOBS: [(&str, &str, &str); 14] = [
    (
        "pantheon-adr",
        ".coderabbit.yaml",
        "da97d156ddec4d82c8c33ecfb24bf04ba606ee7c",
    ),
    (
        "pantheon-adr",
        ".github/scripts/tag-release.sh",
        "1314873bd8eb1a24b8d8ed43a9cf0b44a88f3165",
    ),
    (
        "pantheon-adr",
        ".github/workflows/audit-skill.yml",
        "f94eca2d6a47c6acc5f1e13e85e19c0d87ff5fb8",
    ),
    (
        "pantheon-adr",
        "SECURITY.md",
        "40fde387ed9022b214a1671ee6d37ab3309afb9b",
    ),
    (
        "pantheon-agentic-harness-dev-kit",
        ".coderabbit.yaml",
        "c482c34cea153aa8bbff2e803c58f98800d6c1e5",
    ),
    (
        "pantheon-agentic-harness-dev-kit",
        "SECURITY.md",
        "d9211075bd406c305f04c52bbde484b4f53e7069",
    ),
    (
        "pantheon-journal",
        ".coderabbit.yaml",
        "68c7bc809afefc78044a07eb8ac43be1d6fd3041",
    ),
    (
        "pantheon-journal",
        ".github/scripts/tag-release.sh",
        "9990db7f936290fdd6b724150f9dc6b8a9096ea1",
    ),
    (
        "pantheon-journal",
        ".github/workflows/audit-skill.yml",
        "e59e14d2b8c367666864eed5a76cda0b58c2ef13",
    ),
    (
        "pantheon-journal",
        "SECURITY.md",
        "1b768601a8cb5661b27572ea0ea426f05f110076",
    ),
    (
        "pantheon-skill-auditor",
        ".coderabbit.yaml",
        "8c316238a9f367ae59f88ee4c601e917ce0af325",
    ),
    (
        "pantheon-skill-auditor",
        ".github/scripts/tag-release.sh",
        "b8202c2ef298b8c070a6ee4ef2750ca926d67036",
    ),
    (
        "pantheon-skill-auditor",
        ".github/workflows/audit-skill.yml",
        "5b42f69e602badd05ddcf5f8862ca47360b18957",
    ),
    (
        "pantheon-skill-auditor",
        "SECURITY.md",
        "49e58f97f7562f4e605fe2d9269d7076812c9f33",
    ),
];

#[test]
fn the_pinned_upstream_copies_are_the_repositories_files_at_the_surveyed_commits() {
    for (repo, path, blob) in BLOBS {
        assert_eq!(git_blob_sha(&upstream(repo, path)), blob, "{repo} {path}");
    }
}

/// The two tests the TypeScript generated for each repository with a skill.
macro_rules! with_skill {
    ($module:ident, $repo:expr) => {
        mod $module {
            use super::*;

            #[test]
            fn audit_skill_yml_equals_the_repositorys_file_under_p1_with_the_directive_comments_and_job_names() {
                let mine = content(rendered($repo), ".github/workflows/audit-skill.yml");
                let theirs = upstream($repo, ".github/workflows/audit-skill.yml");
                let result = compare_workflows(&theirs, mine).unwrap();
                assert!(result.equal, "{:?}", result.differences);
                assert!(result.duplicate_keys.is_empty());
                assert_eq!(compare_directives(&theirs, mine), Vec::<String>::new());
            }

            #[test]
            fn tag_release_sh_is_byte_for_byte_the_repositorys_file_and_executable() {
                let path = ".github/scripts/tag-release.sh";
                assert_eq!(content(rendered($repo), path), upstream($repo, path));
                assert_eq!(mode(rendered($repo), path), "0755");
                assert_eq!(file(rendered($repo), path)["class"], "owned");
            }
        }
    };
}

with_skill!(pantheon_adr_skill, "pantheon-adr");
with_skill!(pantheon_journal_skill, "pantheon-journal");
with_skill!(pantheon_skill_auditor_skill, "pantheon-skill-auditor");

/// The test the TypeScript generated for each of the four repositories.
macro_rules! coderabbit {
    ($name:ident, $repo:expr) => {
        #[test]
        fn $name() {
            assert_eq!(
                content(rendered($repo), ".coderabbit.yaml"),
                upstream($repo, ".coderabbit.yaml")
            );
        }
    };
}

coderabbit!(
    pantheon_adr_coderabbit_yaml_is_byte_for_byte_the_repositorys_file,
    "pantheon-adr"
);
coderabbit!(
    pantheon_journal_coderabbit_yaml_is_byte_for_byte_the_repositorys_file,
    "pantheon-journal"
);
coderabbit!(
    pantheon_skill_auditor_coderabbit_yaml_is_byte_for_byte_the_repositorys_file,
    "pantheon-skill-auditor"
);
coderabbit!(
    pantheon_agentic_harness_dev_kit_coderabbit_yaml_is_byte_for_byte_the_repositorys_file,
    "pantheon-agentic-harness-dev-kit"
);

#[test]
fn security_md_is_seeded_so_a_repository_that_words_its_scope_differently_keeps_its_own_text() {
    for repo in REPOS {
        assert_eq!(
            file(rendered(repo), "SECURITY.md")["class"],
            "seed",
            "{repo}"
        );
    }
}

macro_rules! security {
    ($name:ident, $repo:expr) => {
        #[test]
        fn $name() {
            assert_eq!(
                content(rendered($repo), "SECURITY.md"),
                upstream($repo, "SECURITY.md")
            );
        }
    };
}

security!(
    pantheon_adr_security_md_is_byte_for_byte_the_repositorys_file,
    "pantheon-adr"
);
security!(
    pantheon_journal_security_md_is_byte_for_byte_the_repositorys_file,
    "pantheon-journal"
);
security!(
    pantheon_agentic_harness_dev_kit_security_md_is_byte_for_byte_the_repositorys_file,
    "pantheon-agentic-harness-dev-kit"
);

#[test]
fn pantheon_skill_auditor_security_md_differs_only_in_the_three_scope_lines_that_name_its_crates_skill_and_composite_action()
 {
    let mine = content(rendered("pantheon-skill-auditor"), "SECURITY.md");
    let theirs = upstream("pantheon-skill-auditor", "SECURITY.md");
    let theirs: Vec<&str> = theirs.split('\n').collect();
    let different: Vec<&str> = mine.split('\n').filter(|l| !theirs.contains(l)).collect();
    assert!(
        different.iter().all(|l| l.starts_with("- The ")),
        "{different:?}"
    );
    assert!(different.len() <= 3, "{different:?}");
}

#[test]
fn the_library_repository_with_no_binary_and_no_skill_gets_no_audit_skill_yml_and_no_tag_release_sh()
 {
    let got = paths(rendered("pantheon-agentic-harness-dev-kit"));
    assert!(!got.contains(&".github/workflows/audit-skill.yml".to_owned()));
    assert!(!got.contains(&".github/scripts/tag-release.sh".to_owned()));
}

#[test]
fn the_composite_action_variant_runs_the_repositorys_own_action_and_the_bootstrap_binary_the_others_use_the_released_action()
 {
    let auditor = content(
        rendered("pantheon-skill-auditor"),
        ".github/workflows/audit-skill.yml",
    );
    assert!(auditor.contains("uses: ./"));
    assert!(auditor.contains("download-url:"));
    let adr = content(
        rendered("pantheon-adr"),
        ".github/workflows/audit-skill.yml",
    );
    assert!(
        Regex::new(r"uses: pantheon-org/pantheon-skill-auditor@[0-9a-f]{40}\n")
            .unwrap()
            .is_match(adr)
    );
    assert!(
        rendered("pantheon-adr")["pins"]
            .as_array()
            .unwrap()
            .contains(&json!({
                "action": "pantheon-org/pantheon-skill-auditor",
                "sha": "a71472ec078f54ff13b5b74b7e3786779ef28df9",
                "version": "v0.3.1"
            }))
    );
    assert!(!adr.contains("download-url"));
}

#[test]
fn every_other_file_is_unchanged_by_the_new_builtins_the_rendered_set_grows_by_exactly_the_new_files()
 {
    let adr = paths(rendered("pantheon-adr"));
    for path in [
        ".coderabbit.yaml",
        "SECURITY.md",
        ".github/scripts/tag-release.sh",
        ".github/workflows/audit-skill.yml",
        ".gitignore",
    ] {
        assert!(adr.contains(&path.to_owned()), "{path}");
    }
}
