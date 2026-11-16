//! Ported from `runner/test/presence-templates.test.ts`: presence inside templates. What mise.toml, hk.shared.pkl and the headers say when a
//! builtin is left out (options.without). The tests that run real hk and real mise need `hk` and `mise` on the PATH.

use regex::Regex;
use scaffold_package_tests::pkg::root;
use scaffold_package_tests::presence::{folders, render};
use scaffold_package_tests::repo::{git, git_init, temp_dir};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn builtins() -> std::path::PathBuf {
    root().join("pkl/Builtins")
}

/// Evaluates a pkl expression over Shared.pkl and Presence.pkl and returns the JSON it prints.
fn shared(body: &str) -> Value {
    let dir = temp_dir();
    let file = dir.path().join("shared.pkl");
    std::fs::write(
        &file,
        format!(
            "import \"file://{b}/Shared.pkl\"\nimport \"file://{b}/../Presence.pkl\"\n{body}\n",
            b = builtins().display()
        ),
    )
    .unwrap();
    let output = Command::new("pkl")
        .args(["eval", "-f", "json", "--allowed-resources", "file:,prop:"])
        .arg(&file)
        .output()
        .expect("pkl runs");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    serde_json::from_slice(&output.stdout).expect("pkl prints JSON")
}

struct Flags {
    tools: serde_json::Map<String, Value>,
    present: serde_json::Map<String, Value>,
}

fn flags(without: &[&str]) -> Flags {
    let list = without
        .iter()
        .map(|w| format!("\"{w}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let json = shared(&format!(
        "local p = new Presence.Present {{ without = List({list}) }}\ntools = p.toolFlags\npresent = p.presentFlags"
    ));
    Flags {
        tools: json["tools"].as_object().unwrap().clone(),
        present: json["present"].as_object().unwrap().clone(),
    }
}

fn tables() -> (BTreeMap<String, Vec<String>>, Vec<String>) {
    let json = shared("owners = Presence.toolOwners\nalways = Presence.alwaysTools");
    let owners = json["owners"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(tool, list)| {
            (
                tool.clone(),
                list.as_array()
                    .unwrap()
                    .iter()
                    .map(|o| o.as_str().unwrap().to_owned())
                    .collect(),
            )
        })
        .collect();
    let always = json["always"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap().to_owned())
        .collect();
    (owners, always)
}

fn sorted(mut list: Vec<String>) -> Vec<String> {
    list.sort();
    list
}

#[test]
fn the_present_mapping_has_one_entry_for_every_builtin_folder_and_the_tools_mapping_one_for_every_tool_of_the_table()
 {
    let f = flags(&[]);
    let (owners, always) = tables();
    assert_eq!(sorted(f.present.keys().cloned().collect()), folders());
    assert_eq!(
        sorted(f.tools.keys().cloned().collect()),
        sorted(
            always
                .iter()
                .cloned()
                .chain(owners.keys().cloned())
                .collect()
        )
    );
    assert!(f.present.values().all(|v| *v == true));
    assert!(f.tools.values().all(|v| *v == true));
}

#[test]
fn every_owner_in_the_table_is_a_builtin() {
    let names = folders();
    for (tool, list) in tables().0 {
        for owner in list {
            assert!(names.contains(&owner), "{tool}: {owner}");
        }
    }
}

#[test]
fn a_tool_is_present_while_any_of_its_owners_is_and_gone_when_all_are_left_out() {
    let (owners, _) = tables();
    for (tool, list) in &owners {
        let one_left: Vec<&str> = list[..list.len() - 1].iter().map(String::as_str).collect();
        assert_eq!(
            flags(&one_left).tools[tool],
            true,
            "{tool} with one owner left"
        );
    }
    let hk = flags(&["hk"]).tools;
    assert_eq!(
        (hk["hk"].clone(), hk["yq"].clone(), hk["shellcheck"].clone()),
        (false.into(), true.into(), true.into())
    );
    let no_yq = flags(&["hk", "inline-scripts"]).tools;
    assert_eq!(
        (no_yq["yq"].clone(), no_yq["jq"].clone()),
        (false.into(), true.into())
    );
    assert_eq!(flags(&["hk", "actionlint"]).tools["shellcheck"], false);
    assert_eq!(
        flags(&[
            "agnix",
            "inline-scripts",
            "release-automerge",
            "release-bump",
            "release-please"
        ])
        .tools["jq"],
        false
    );
}

#[test]
fn pkl_and_the_runner_are_always_pinned() {
    let tools = flags(&[
        "hk",
        "rumdl",
        "agnix",
        "actionlint",
        "zizmor",
        "inline-scripts",
        "release-automerge",
    ])
    .tools;
    assert_eq!(tools["pkl"], true);
    assert_eq!(tools["runner"], true);
}

// A script or template that names a tool needs it through mise, so its builtin must be an owner in the table. Comments and workflow run blocks
// do not count (a runner has its own jq and yq), and mise.toml and hk.shared.pkl are the two files the table governs.
#[test]
fn every_tool_a_script_names_outside_its_comments_is_owned_by_that_scripts_builtin() {
    let (owners, _) = tables();
    let mut names: Vec<String> = owners.keys().filter(|t| *t != "rust").cloned().collect();
    names.push("hk".to_owned());
    let comment = Regex::new(r"^\s*#").unwrap();
    let mut found = Vec::new();
    for builtin in folders() {
        let dir = builtins().join(&builtin).join("assets");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut scripts: Vec<String> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".sh"))
            .collect();
        scripts.sort();
        for script in scripts {
            let code: String = std::fs::read_to_string(dir.join(&script))
                .unwrap()
                .split('\n')
                .filter(|l| !comment.is_match(l))
                .collect::<Vec<_>>()
                .join("\n");
            for tool in &names {
                let mentions = Regex::new(&format!(r"\b{}\b", regex::escape(tool)))
                    .unwrap()
                    .is_match(&code);
                let owned = owners.get(tool).is_none_or(|list| list.contains(&builtin));
                if mentions && !owned {
                    found.push(format!("{builtin}/{script} names {tool}"));
                }
            }
        }
    }
    assert_eq!(found, Vec::<String>::new());
}

// ---- rendering with builtins left out ----

fn text<'a>(files: &'a BTreeMap<String, String>, path: &str) -> &'a str {
    files.get(path).unwrap_or_else(|| {
        panic!(
            "{path} was not rendered. Files: {}",
            files.keys().cloned().collect::<Vec<_>>().join(", ")
        )
    })
}

#[test]
fn rumdl_toml_names_mise_run_lint_only_when_mise_is_present_and_keeps_the_pull_request_template_ignore_either_way()
 {
    let full = render(&[], "");
    assert!(
        text(&full, ".rumdl.toml")
            .contains("Lint everything with: mise run lint   (or: rumdl check .)")
    );
    for without in [
        vec!["mise"],
        vec!["mise", "hk"],
        vec!["pull-request-template"],
    ] {
        let files = render(&without, "");
        let t = text(&files, ".rumdl.toml");
        assert_eq!(
            t.contains("mise run lint"),
            !without.contains(&"mise"),
            "{}",
            without.join(",")
        );
        assert!(
            t.contains("\".github/pull_request_template.md\" = [\"MD041\"]"),
            "{}",
            without.join(",")
        );
    }
    let no_mise = render(&["mise"], "");
    assert_eq!(
        text(&no_mise, ".rumdl.toml").split('\n').next().unwrap(),
        "# rumdl configuration. Lint everything with: rumdl check ."
    );
}

// ---- mise.toml ----

fn mise(without: &[&str]) -> String {
    text(&render(without, ""), "mise.toml").to_owned()
}

/// The tool keys of the [tools] table, in file order.
fn tool_keys(t: &str) -> Vec<String> {
    let table: toml::Table = t.parse().expect("mise.toml parses");
    table["tools"].as_table().unwrap().keys().cloned().collect()
}

/// The table headers, read from the text.
fn table_names(t: &str) -> Vec<String> {
    Regex::new(r"(?m)^\[([^\]]+)\]$")
        .unwrap()
        .captures_iter(t)
        .map(|c| c[1].to_owned())
        .collect()
}

fn tasks_of(t: &str) -> Vec<String> {
    let table: toml::Table = t.parse().expect("mise.toml parses");
    table
        .get("tasks")
        .and_then(toml::Value::as_table)
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default()
}

/// A rendered file is tidy: no empty table, no comment that is followed by a blank line or a table, no two blank lines in a row, and it ends
/// in one newline.
fn expect_tidy(t: &str, label: &str) {
    let table_header = Regex::new(r"^\[.+\]$").unwrap();
    let empty_or_table = Regex::new(r"^(\[.*|)$").unwrap();
    let lines: Vec<&str> = t.split('\n').collect();
    for (i, l) in lines.iter().enumerate() {
        let next = lines.get(i + 1).copied().unwrap_or("");
        if table_header.is_match(l) {
            assert!(!empty_or_table.is_match(next), "{label}: empty table {l}");
        }
        if l.starts_with('#') {
            assert!(
                !empty_or_table.is_match(next),
                "{label}: orphan comment at line {}",
                i + 1
            );
        }
        if l.is_empty() {
            assert_ne!(
                lines.get(i + 1).copied(),
                Some(""),
                "{label}: two blank lines at {}",
                i + 1
            );
        }
    }
    assert!(
        t.ends_with('\n') && !t.ends_with("\n\n"),
        "{label}: ends in one newline"
    );
}

const ALL_TOOLS: [&str; 11] = [
    "rust",
    "actionlint",
    "zizmor",
    "hk",
    "pkl",
    "yq",
    "jq",
    "shellcheck",
    "rumdl",
    "github:agent-sh/agnix",
    "github:pantheon-org/pantheon-scaffold",
];
const ALL_TASKS: [&str; 5] = ["setup", "lint", "lint-fix", "fmt", "check"];

fn all_tools_except(left: &[&str]) -> Vec<String> {
    ALL_TOOLS
        .iter()
        .filter(|t| !left.contains(t))
        .map(|t| (*t).to_owned())
        .collect()
}

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn by_default_mise_toml_pins_the_eleven_tools_and_has_the_five_tasks_the_env_and_both_hooks() {
    let t = mise(&[]);
    assert_eq!(tool_keys(&t), strings(&ALL_TOOLS));
    assert_eq!(tasks_of(&t), strings(&ALL_TASKS));
    assert_eq!(
        table_names(&t),
        [
            "tools",
            "settings",
            "env",
            "hooks",
            "tasks.setup",
            "tasks.lint",
            "tasks.lint-fix",
            "tasks.fmt",
            "tasks.check"
        ]
    );
    expect_tidy(&t, "default");
}

fn mise_case(without: &[&str], tools: Vec<String>, tasks: &[&str], env: bool, postinstall: bool) {
    let t = mise(without);
    assert_eq!(tool_keys(&t), tools);
    assert_eq!(tasks_of(&t), strings(tasks));
    assert_eq!(t.contains("HK_MISE"), env, "HK_MISE");
    assert_eq!(t.contains("postinstall"), postinstall, "postinstall");
    assert!(t.contains("enter = \"mise install -q\""), "enter stays");
    assert!(
        t.contains("\"github:pantheon-org/pantheon-scaffold\" ="),
        "the runner pin stays"
    );
    expect_tidy(&t, &without.join(","));
}

#[test]
fn mise_toml_with_hk_left_out_pins_10_tools_and_keeps_the_tasks_of_what_is_present() {
    mise_case(
        &["hk"],
        all_tools_except(&["hk"]),
        &["lint", "lint-fix", "fmt"],
        false,
        false,
    );
}

#[test]
fn mise_toml_with_hk_and_inline_scripts_left_out_pins_9_tools_and_keeps_the_tasks_of_what_is_present()
 {
    mise_case(
        &["hk", "inline-scripts"],
        all_tools_except(&["hk", "yq"]),
        &["lint", "lint-fix", "fmt"],
        false,
        false,
    );
}

#[test]
fn mise_toml_with_hk_and_actionlint_left_out_pins_8_tools_and_keeps_the_tasks_of_what_is_present() {
    mise_case(
        &["hk", "actionlint"],
        all_tools_except(&["hk", "actionlint", "shellcheck"]),
        &["lint", "lint-fix", "fmt"],
        false,
        false,
    );
}

#[test]
fn mise_toml_with_rumdl_left_out_pins_10_tools_and_keeps_the_tasks_of_what_is_present() {
    mise_case(
        &["rumdl"],
        all_tools_except(&["rumdl"]),
        &["setup", "fmt", "check"],
        true,
        true,
    );
}

#[test]
fn mise_toml_with_rust_ci_left_out_pins_11_tools_and_keeps_the_tasks_of_what_is_present() {
    mise_case(&["rust-ci"], all_tools_except(&[]), &ALL_TASKS, true, true);
}

#[test]
fn mise_toml_with_rust_ci_and_rust_toolchain_left_out_pins_10_tools_and_keeps_the_tasks_of_what_is_present()
 {
    mise_case(
        &["rust-ci", "rust-toolchain"],
        all_tools_except(&["rust"]),
        &["setup", "lint", "lint-fix", "check"],
        true,
        true,
    );
}

#[test]
fn mise_toml_with_zizmor_left_out_pins_10_tools_and_keeps_the_tasks_of_what_is_present() {
    mise_case(
        &["zizmor"],
        all_tools_except(&["zizmor"]),
        &ALL_TASKS,
        true,
        true,
    );
}

#[test]
fn mise_toml_with_agnix_left_out_pins_10_tools_and_keeps_the_tasks_of_what_is_present() {
    mise_case(
        &["agnix"],
        all_tools_except(&["github:agent-sh/agnix"]),
        &ALL_TASKS,
        true,
        true,
    );
}

#[test]
fn jq_goes_only_when_all_five_of_its_owners_are_left_out() {
    assert!(tool_keys(&mise(&["agnix", "inline-scripts"])).contains(&"jq".to_owned()));
    assert!(
        tool_keys(&mise(&["agnix", "inline-scripts", "release-automerge"]))
            .contains(&"jq".to_owned())
    );
    assert!(
        !tool_keys(&mise(&[
            "agnix",
            "inline-scripts",
            "release-automerge",
            "release-bump",
            "release-please"
        ]))
        .contains(&"jq".to_owned())
    );
}

#[test]
fn the_rust_comment_names_check_toolchain_version_sh_only_with_rust_ci_and_the_actionlint_comment_names_zizmor_only_with_zizmor()
 {
    assert!(mise(&[]).contains("check-toolchain-version.sh"));
    let no_ci = mise(&["rust-ci"]);
    assert!(!no_ci.contains("check-toolchain-version.sh"));
    assert!(no_ci.contains("# Same toolchain as rust-toolchain.toml.\nrust = "));
    let no_zizmor = mise(&["zizmor"]);
    assert!(!no_zizmor.contains("like zizmor"));
    assert!(
        no_zizmor.contains("# so a new release cannot fail a commit that did not touch a rule.")
    );
}

#[test]
fn real_mise_reads_a_reduced_mise_toml_and_lists_the_tools_that_are_left() {
    let holder = temp_dir();
    let dir = holder.path();
    std::fs::create_dir(dir.join("data")).unwrap();
    let empty = dir.join("empty.toml");
    std::fs::write(&empty, "").unwrap();
    std::fs::write(
        dir.join("mise.toml"),
        mise(&["hk", "inline-scripts", "rumdl", "agnix"]),
    )
    .unwrap();
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
    let listed: Value = serde_json::from_slice(&output.stdout).unwrap();
    let listed = sorted(listed.as_object().unwrap().keys().cloned().collect());
    assert_eq!(
        listed,
        sorted(strings(&[
            "actionlint",
            "github:pantheon-org/pantheon-scaffold",
            "jq",
            "pkl",
            "rust",
            "shellcheck",
            "zizmor"
        ]))
    );
}

// ---- hk.shared.pkl, with real hk ----

/// Runs hk in a directory with the `HK_*` variables removed and mise turned off. Returns the exit code and its output without colours.
fn hk(dir: &Path, args: &[&str]) -> (i32, String) {
    let mut command = Command::new("hk");
    command.args(args).current_dir(dir);
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
    (
        output.status.code().unwrap_or(-1),
        Regex::new("\x1b\\[[0-9;]*m")
            .unwrap()
            .replace_all(&text, "")
            .into_owned(),
    )
}

/// Writes the rendered files into a git repository, as a repository adopting the scaffold has them.
fn repo_of(files: &BTreeMap<String, String>) -> tempfile::TempDir {
    let dir = git_init();
    for (path, content) in files {
        let target = dir.path().join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, content).unwrap();
    }
    git(dir.path(), &["add", "-A"]);
    dir
}

const STEPS: [&str; 7] = [
    "rustfmt",
    "rumdl",
    "yamllint",
    "shellcheck",
    "actionlint",
    "clippy",
    "test",
];

fn steps_in(shared: &str) -> Vec<&'static str> {
    STEPS
        .iter()
        .copied()
        .filter(|s| shared.contains(&format!("[\"{s}\"]")))
        .collect()
}

fn steps_except(left: &[&str]) -> Vec<&'static str> {
    STEPS
        .iter()
        .copied()
        .filter(|s| !left.contains(s))
        .collect()
}

fn hk_case(without: &[&str], steps: &[&str]) {
    let files = render(without, "");
    let shared = text(&files, "hk.shared.pkl");
    assert_eq!(steps_in(shared), steps);
    let lines: Vec<&str> = shared.split('\n').collect();
    assert!(
        lines
            .windows(2)
            .all(|w| !(w[0].is_empty() && w[1].is_empty())),
        "no two blank lines"
    );
    let repo = repo_of(&files);
    let (code, out) = hk(repo.path(), &["validate"]);
    assert_eq!(code, 0, "{out}");
    let (code, plan) = hk(repo.path(), &["check", "--all", "--plan"]);
    assert_eq!(code, 0, "{plan}");
    for step in steps.iter().filter(|s| **s != "clippy" && **s != "test") {
        assert!(plan.contains(step), "{step}: {plan}");
    }
    for step in STEPS
        .iter()
        .filter(|s| !steps.contains(s) && **s != "clippy" && **s != "test")
    {
        assert!(!plan.contains(step), "no {step}: {plan}");
    }
}

#[test]
fn hk_shared_pkl_with_nothing_left_out_has_all_the_steps_and_real_hk_accepts_it_with_the_seeded_hk_pkl()
 {
    hk_case(&[], &STEPS);
}

#[test]
fn hk_shared_pkl_with_rumdl_left_out_has_the_steps_without_rumdl_and_real_hk_accepts_it_with_the_seeded_hk_pkl()
 {
    hk_case(&["rumdl"], &steps_except(&["rumdl"]));
}

#[test]
fn hk_shared_pkl_with_actionlint_left_out_has_the_steps_without_actionlint_and_real_hk_accepts_it_with_the_seeded_hk_pkl()
 {
    hk_case(&["actionlint"], &steps_except(&["actionlint"]));
}

#[test]
fn hk_shared_pkl_with_rust_ci_left_out_has_the_steps_without_clippy_and_test_and_real_hk_accepts_it_with_the_seeded_hk_pkl()
 {
    hk_case(&["rust-ci"], &steps_except(&["clippy", "test"]));
}

#[test]
fn hk_shared_pkl_with_rust_ci_and_rust_toolchain_left_out_has_the_steps_without_the_rust_ones_and_real_hk_accepts_it_with_the_seeded_hk_pkl()
 {
    hk_case(
        &["rust-ci", "rust-toolchain"],
        &steps_except(&["clippy", "test", "rustfmt"]),
    );
}

#[test]
fn hk_shared_pkl_with_rumdl_actionlint_rust_ci_and_rust_toolchain_left_out_has_two_steps_and_real_hk_accepts_it_with_the_seeded_hk_pkl()
 {
    hk_case(
        &["rumdl", "actionlint", "rust-ci", "rust-toolchain"],
        &["yamllint", "shellcheck"],
    );
}

// ---- text that names another builtin ----
const MISE_LINE: &str = "Tools are pinned in mise.toml; HK_MISE=1 runs each hook under `mise x`.";

#[test]
fn the_hk_files_say_where_tools_are_pinned_only_when_mise_is_present() {
    let full = render(&[], "");
    let no_mise_files = render(&["mise"], "");
    for path in ["hk.shared.pkl", "hk.pkl"] {
        assert!(text(&full, path).contains(MISE_LINE), "{path}");
        assert!(
            !text(&no_mise_files, path).contains("mise"),
            "{path} without mise"
        );
    }
}

#[test]
fn the_pre_push_comment_names_the_rust_ci_workflow_only_when_rust_ci_is_present() {
    assert!(
        text(&render(&[], ""), "hk.shared.pkl")
            .contains("/// The steps of the pre-push hook, matching the rust-ci workflow.\n")
    );
    let no_ci = render(&["rust-ci"], "");
    let no_ci = text(&no_ci, "hk.shared.pkl");
    assert!(no_ci.contains("/// The steps of the pre-push hook.\n"));
    assert!(!no_ci.contains("rust-ci"));
}

#[test]
fn the_zizmor_workflow_header_runs_zizmor_through_mise_and_says_it_is_pinned_there_only_when_mise_is_present()
 {
    let path = ".github/workflows/zizmor.yml";
    let full = render(&[], "");
    assert!(text(&full, path).contains("(zizmor is pinned in mise.toml)"));
    assert!(text(&full, path).contains("mise exec -- zizmor --min-severity high"));
    let no_mise = render(&["mise"], "");
    assert!(!text(&no_mise, path).contains("mise"));
    assert!(text(&no_mise, path).contains("Run it locally with the same findings:"));
    assert!(text(&no_mise, path).contains("GH_TOKEN=$(gh auth token) zizmor --min-severity high"));
}
