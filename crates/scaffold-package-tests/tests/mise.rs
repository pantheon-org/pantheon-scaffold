//! Ported from `runner/test/mise.test.ts`: the mise files the package renders, against the pinned mise.toml of the repositories, and real
//! mise reading them. The last test needs `mise` on the PATH.

use regex::Regex;
use scaffold_core::lockcheck::SUPPORTED_PLATFORMS;
use scaffold_package_tests::pkg::{
    content, eval, file, fixture, git_blob_sha, line_diff, mode, read, render,
};
use scaffold_package_tests::repo::temp_dir;
use serde_json::Value;
use std::process::Command;
use std::sync::LazyLock;

static CONTRACT: LazyLock<Value> = LazyLock::new(|| render("mise"));

fn upstream() -> String {
    read("crates/scaffold-package-tests/upstream/mise.toml")
}

#[test]
fn the_mise_files_are_the_owned_mise_toml_the_external_mise_lock_and_the_seeded_per_repo_config() {
    let mut classes: Vec<String> = CONTRACT["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            format!(
                "{}:{}",
                f["class"].as_str().unwrap(),
                f["path"].as_str().unwrap()
            )
        })
        .collect();
    classes.sort();
    assert_eq!(
        classes,
        [
            "external:mise.lock",
            "owned:mise.toml",
            "seed:.mise/conf.d/repo.toml"
        ]
    );
    assert!(file(&CONTRACT, "mise.lock").get("content").is_none());
    for f in CONTRACT["files"].as_array().unwrap() {
        let path = f["path"].as_str().unwrap();
        assert_eq!(mode(&CONTRACT, path), "0644", "{path}");
    }
}

#[test]
fn the_pinned_upstream_copy_is_the_pantheon_adr_mise_toml_at_1e2fd46_git_blob_16c264c() {
    assert_eq!(
        git_blob_sha(&upstream()),
        "16c264c54f1408330819ecd81131de90eb9b49e4"
    );
}

#[test]
fn mise_toml_differs_from_the_repositories_current_file_only_by_the_named_additions_and_the_two_comments()
 {
    let (removed, added) = line_diff(&upstream(), content(&CONTRACT, "mise.toml"));
    assert_eq!(
        removed,
        [
            "# Workflow security linter. Keep in sync with `version` in",
            "# .github/workflows/zizmor.yml.",
            "# Lints skill frontmatter and agent files. Keep in sync with AGNIX_VERSION in",
            "# .github/workflows/agnix.yml.",
        ]
    );
    assert_eq!(
        added,
        [
            "# Workflow security linter. Pinned in the scaffold package (Tools.pkl).",
            "# Lints skill frontmatter and agent files. Pinned in the scaffold package (Tools.pkl).",
            "# The scaffold runner, released together with the package that rendered this file.",
            "\"github:pantheon-org/pantheon-scaffold\" = \"0.1.2\"",
            "# Record every tool in mise.lock with its checksum, and refuse a tool whose provenance",
            "# attestation does not verify. Limited to the platforms the runner is built for: with no",
            "# list mise locks every platform and names the package zip for the ones with no runner.",
            "lockfile = true",
            "locked_verify_provenance = true",
            "lockfile_platforms = [\"linux-x64\", \"linux-arm64\", \"macos-arm64\"]",
        ]
    );
}

#[test]
fn the_tool_specifiers_are_the_ones_the_repositories_use_today_exact_stays_exact_and_latest_stays_latest_decision_q14_option_a()
 {
    let text = content(&CONTRACT, "mise.toml");
    for pin in [
        "actionlint = \"1.7.12\"",
        "zizmor = \"1.32.0\"",
        "rumdl = \"0.2.77\"",
        "\"github:agent-sh/agnix\" = \"0.56.6\"",
        "hk = \"latest\"",
        "pkl = \"latest\"",
        "yq = \"latest\"",
        "jq = \"latest\"",
        "shellcheck = \"latest\"",
        "minimum_release_age = \"7d\"",
    ] {
        assert!(text.contains(pin), "{pin}");
    }
}

#[test]
fn lockfile_platforms_is_exactly_the_platforms_the_runner_is_built_for() {
    let line = Regex::new(r"lockfile_platforms = \[(.*)\]").unwrap();
    let text = content(&CONTRACT, "mise.toml");
    let listed: Vec<String> = line.captures(text).expect("a lockfile_platforms line")[1]
        .split(',')
        .map(|s| s.trim().replace('"', ""))
        .collect();
    let supported: Vec<&str> = SUPPORTED_PLATFORMS
        .iter()
        .map(|(platform, _)| *platform)
        .collect();
    assert_eq!(listed, supported);
}

#[test]
fn the_toolchain_option_reaches_the_rust_entry() {
    assert!(
        content(&CONTRACT, "mise.toml")
            .contains("rust = { version = \"1.96.1\", components = \"rustfmt,clippy\" }")
    );
}

#[test]
fn the_seeded_per_repo_config_says_what_it_is_for_and_holds_no_tools() {
    let seed = content(&CONTRACT, ".mise/conf.d/repo.toml");
    assert!(Regex::new(r"(?i)seeded once").unwrap().is_match(seed));
    assert!(seed.contains("[tools]"));
    assert!(!Regex::new(r#"(?m)^[a-z"].*="#).unwrap().is_match(seed));
}

#[test]
fn using_the_mise_builtin_without_a_runner_version_and_not_as_a_released_package_fails_with_a_clear_message()
 {
    let error = eval(&fixture("mise-no-runner")).expect_err("the render is refused");
    assert!(error.contains("runnerVersion"), "{error}");
}

/// Decision 29 with the real mise: the per-repo file adds tools, and the owned file wins a conflict. The data directory is empty and only the
/// tools the config asks for are listed (--current), so the result does not depend on what is installed on the machine.
#[test]
fn real_mise_reads_both_files_adds_the_repos_own_tool_and_lets_the_owned_mise_toml_win_a_conflict()
{
    let holder = temp_dir();
    let dir = holder.path();
    std::fs::create_dir_all(dir.join(".mise/conf.d")).unwrap();
    std::fs::create_dir(dir.join("data")).unwrap();
    let empty = dir.join("empty.toml");
    std::fs::write(&empty, "").unwrap();
    std::fs::write(dir.join("mise.toml"), content(&CONTRACT, "mise.toml")).unwrap();
    // The seed ends with an empty [tools] section, so a repository adds its tools right after it.
    std::fs::write(
        dir.join(".mise/conf.d/repo.toml"),
        format!(
            "{}jq = \"1.6\"\ntokei = \"13.0.0\"\n",
            content(&CONTRACT, ".mise/conf.d/repo.toml")
        ),
    )
    .unwrap();
    // MISE_DISABLE_TOOLS is cleared because the workflows set it to skip Rust in the locked install, and this test needs Rust listed.
    let output = Command::new("mise")
        .args(["ls", "--current", "--json"])
        .current_dir(dir)
        .env("MISE_DISABLE_TOOLS", "")
        .env("MISE_DATA_DIR", dir.join("data"))
        .env("MISE_CEILING_PATHS", dir.parent().unwrap())
        .env("MISE_TRUSTED_CONFIG_PATHS", dir)
        .env("MISE_GLOBAL_CONFIG_FILE", &empty)
        .env("MISE_SYSTEM_CONFIG_FILE", &empty)
        .output()
        .expect("mise runs: is it on the PATH?");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tools: Value = serde_json::from_slice(&output.stdout).expect("mise prints JSON");
    let tools = tools.as_object().unwrap();
    let requested = |name: &str| {
        tools[name][0]["requested_version"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let source = |name: &str| -> String {
        let path = tools[name][0]["source"]["path"].as_str().unwrap();
        if path.ends_with("/.mise/conf.d/repo.toml") {
            ".mise/conf.d/repo.toml".to_owned()
        } else if path.ends_with("/mise.toml") {
            "mise.toml".to_owned()
        } else {
            path.to_owned()
        }
    };
    assert_eq!(
        (requested("jq"), source("jq")),
        ("latest".to_owned(), "mise.toml".to_owned())
    );
    assert_eq!(
        (requested("tokei"), source("tokei")),
        ("13.0.0".to_owned(), ".mise/conf.d/repo.toml".to_owned())
    );
    assert_eq!(requested("rust"), "1.96.1");
    assert_eq!(requested("github:pantheon-org/pantheon-scaffold"), "0.1.2");
    assert_eq!(tools.len(), 12);
}
