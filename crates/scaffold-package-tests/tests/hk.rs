//! Ported from `runner/test/hk.test.ts`: the hook files the package renders, against the pinned hk.pkl of the repositories, and real hk on
//! the render. The tests that run hk need `hk` on the PATH (mise puts it there), and the last one needs `shellcheck`.

use regex::Regex;
use scaffold_package_tests::pkg::{content, git_blob_sha, line_diff, paths, read, render};
use scaffold_package_tests::repo::{git, git_init};
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

static DEF: LazyLock<Value> = LazyLock::new(|| render("hk"));
static ALL: LazyLock<Value> = LazyLock::new(|| render("hk-all"));

fn upstream() -> String {
    read("crates/scaffold-package-tests/upstream/hk.pkl")
}

fn kit() -> String {
    read("crates/scaffold-package-tests/upstream/hk-kit.pkl")
}

/// The text between the line that starts a Mapping and the first closing brace at column 0.
fn mapping_body(text: &str, starts_with: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with(starts_with))
        .unwrap_or_else(|| panic!("no line starts with {starts_with}"));
    let end = lines
        .iter()
        .enumerate()
        .position(|(i, l)| i > start && *l == "}")
        .expect("a closing brace");
    lines[start + 1..end].join("\n")
}

fn up_pre(t: &str) -> String {
    mapping_body(t, "local preCommit")
}
fn up_push(t: &str) -> String {
    mapping_body(t, "local prePush")
}
fn sh_pre(t: &str) -> String {
    mapping_body(t, "function preCommit()")
}
fn sh_push(t: &str) -> String {
    mapping_body(t, "function prePush()")
}

#[test]
fn the_pinned_upstream_copies_are_the_hk_pkl_files_of_pantheon_adr_at_1e2fd46_and_of_the_library_repo_at_42f48b4()
 {
    assert_eq!(
        git_blob_sha(&upstream()),
        "e4bbde76e0d09f3bbc329a01e38a62deb7473513"
    );
    assert_eq!(
        git_blob_sha(&kit()),
        "37ed8613e5f7f095333fc558bbddecf4d5748f2d"
    );
}

#[test]
fn the_hook_files_are_the_owned_shared_steps_module_and_the_seeded_hk_pkl() {
    let mut classes: Vec<String> = DEF["files"]
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
        .filter(|p| p.contains("hk"))
        .collect();
    classes.sort();
    assert_eq!(classes, ["owned:hk.shared.pkl", "seed:hk.pkl"]);
    let mut all = paths(&ALL);
    all.sort();
    assert_eq!(all, ["hk.pkl", "hk.shared.pkl"]);
}

#[test]
fn the_pre_commit_steps_are_byte_for_byte_the_ones_in_pantheon_adrs_hk_pkl() {
    assert_eq!(sh_pre(content(&DEF, "hk.shared.pkl")), up_pre(&upstream()));
}

#[test]
fn the_pre_push_steps_with_the_default_features_are_byte_for_byte_the_ones_in_pantheon_adrs_hk_pkl()
{
    assert_eq!(
        sh_push(content(&DEF, "hk.shared.pkl")),
        up_push(&upstream())
    );
}

#[test]
fn the_pre_push_steps_with_all_features_are_byte_for_byte_the_ones_in_the_library_repos_hk_pkl() {
    assert_eq!(sh_push(content(&ALL, "hk.shared.pkl")), up_push(&kit()));
}

#[test]
fn the_pre_commit_steps_differ_from_the_library_repos_only_by_one_comment_which_is_wording_tied_to_github_actionlint_yaml()
 {
    let (removed, added) = line_diff(&up_pre(&kit()), &sh_pre(content(&ALL, "hk.shared.pkl")));
    assert_eq!(
        removed,
        [
            "    // the workflow files so it only runs when one is staged. It is a local gate:",
            "    // there is no actionlint workflow, so it is not a required status check.",
        ]
    );
    assert_eq!(
        added,
        [
            "    // the workflow files so it only runs when one is staged. Reads",
            "    // .github/actionlint.yaml. It is a local gate: there is no actionlint",
            "    // workflow, so it is not a required status check.",
        ]
    );
}

#[test]
fn the_seeded_hk_pkl_is_pantheon_adrs_hooks_block_with_the_steps_taken_from_the_shared_module() {
    let hooks = |t: &str| t[t.find("\nhooks {\n").expect("a hooks block")..].to_owned();
    let expected = hooks(&upstream())
        .replacen("steps = prePush", "steps = shared.prePush()", 1)
        .replace("steps = preCommit", "steps = shared.preCommit()");
    assert_eq!(hooks(content(&DEF, "hk.pkl")), expected);
}

#[test]
fn the_seeded_hk_pkl_pins_the_hk_config_package_imports_the_shared_module_and_holds_no_step_of_its_own()
 {
    let seed = content(&DEF, "hk.pkl");
    assert_eq!(
        seed.split('\n').next().unwrap(),
        "amends \"package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Config.pkl\""
    );
    assert!(seed.contains("import \"hk.shared.pkl\" as shared"));
    assert!(!seed.contains("[\"rustfmt\"]"));
    assert!(seed.contains("seeded once"));
}

#[test]
fn the_shared_module_says_it_is_rendered_and_imports_the_same_hk_config_package() {
    let shared = content(&DEF, "hk.shared.pkl");
    assert!(
        Regex::new(r"^// Rendered by pantheon-scaffold")
            .unwrap()
            .is_match(shared)
    );
    assert!(shared.contains(
        "import \"package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Config.pkl\" as hk"
    ));
}

// Real hk on the render. The plan only lists steps, so no tool is needed to run these.

/// A git repository with the files of a contract written to it (executable where the contract says so), the extra files, and everything
/// staged.
fn repo_with(contract: &Value, extra: &[(&str, &str)]) -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    let dir = git_init();
    for f in contract["files"].as_array().unwrap() {
        let path = dir.path().join(f["path"].as_str().unwrap());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, f["content"].as_str().unwrap()).unwrap();
        let mode = if f["mode"] == "0755" { 0o755 } else { 0o644 };
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    for (rel, text) in extra {
        std::fs::write(dir.path().join(rel), text).unwrap();
    }
    git(dir.path(), &["add", "-A"]);
    dir
}

/// Runs hk in a directory with the `HK_*` variables removed and mise turned off. Returns the exit code and its output without colours.
fn hk(dir: &Path, args: &[&str]) -> (i32, String) {
    let mut command = Command::new("hk");
    command.args(args).current_dir(dir).env("HK_MISE", "0");
    for (key, _) in std::env::vars() {
        if key.starts_with("HK_") {
            command.env_remove(key);
        }
    }
    command.env("HK_MISE", "0");
    let output = command.output().expect("hk runs: is it on the PATH?");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let colours = Regex::new("\x1b\\[[0-9;]*m").unwrap();
    (
        output.status.code().unwrap_or(-1),
        colours.replace_all(&text, "").into_owned(),
    )
}

#[test]
fn real_hk_validates_the_render_with_the_default_features_and_with_all_features() {
    for contract in [&*DEF, &*ALL] {
        let repo = repo_with(contract, &[]);
        let (code, out) = hk(repo.path(), &["validate"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("is valid"), "{out}");
    }
}

#[test]
fn real_hk_plans_the_five_shared_steps_and_a_step_a_repository_adds_to_the_seeded_file_next_to_them()
 {
    // A repository adds a step by editing a hook in the seeded file: a module can define hooks only once.
    let seed = content(&DEF, "hk.pkl");
    let marker = "steps = shared.preCommit()";
    let at = seed.rfind(marker).expect("the pre-commit hook");
    let edited = format!(
        "{}steps {{ ...shared.preCommit(); [\"my-local-step\"] {{ glob = List(\"*.txt\"); check = \"true\" }} }}{}",
        &seed[..at],
        &seed[at + marker.len()..]
    );
    let repo = repo_with(&DEF, &[("hk.pkl", &edited), ("notes.txt", "x\n")]);
    let (code, out) = hk(repo.path(), &["check", "--all", "--plan"]);
    assert_eq!(code, 0, "{out}");
    for step in [
        "rustfmt",
        "rumdl",
        "yamllint",
        "shellcheck",
        "actionlint",
        "my-local-step",
    ] {
        assert!(out.contains(step), "{step}: {out}");
    }
}

/// The hook fixers over a fresh render (plan task 3.1). Only shellcheck has files to look at: the render holds shell scripts, TOML and JSON,
/// no Rust, Markdown or workflow files. It needs shellcheck on the PATH, which the GitHub Ubuntu runners have.
#[test]
fn the_hook_fixers_and_checks_over_a_fresh_render_pass_and_change_nothing() {
    let repo = repo_with(&DEF, &[]);
    let before = git(repo.path(), &["status", "--porcelain"]);
    let (code, out) = hk(repo.path(), &["fix", "--all"]);
    assert_eq!(code, 0, "{out}");
    let (code, out) = hk(repo.path(), &["check", "--all"]);
    assert_eq!(code, 0, "{out}");
    git(repo.path(), &["add", "-A"]);
    assert_eq!(git(repo.path(), &["status", "--porcelain"]), before);
    assert_eq!(git(repo.path(), &["diff", "--stat"]), "");
}
