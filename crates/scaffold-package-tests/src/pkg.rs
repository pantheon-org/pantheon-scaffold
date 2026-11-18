//! The package under test: where it is, how a config is evaluated against it, and the few helpers the tests share.

use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::Value;

use scaffold_core::contract::parse_contract;
use scaffold_core::pkl::{EvalOptions, eval_contract_json};

/// The root of the repository.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

/// Text of a file, named from the root of the repository.
pub fn read(rel: &str) -> String {
    let path = root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The `scaffold.pkl` of a fixture.
pub fn fixture(name: &str) -> PathBuf {
    root()
        .join("crates/scaffold-package-tests/fixtures")
        .join(name)
        .join("scaffold.pkl")
}

/// The contract of a config, as `pantheon-scaffold contract --json` prints it: evaluated with the core library, with the package in
/// `pkl/` allowed as a local package, then parsed, which renders the files that are templates, and serialized again. The error is the
/// message of the runner.
pub fn eval(config: &Path) -> Result<Value, String> {
    eval_detecting(config, false)
}

/// `eval`, with the answer the runner gives to "does the repository have a .coderabbit.yaml?". The tests do not ask the checkout, which has one
/// until it renders pr-agent.
pub fn eval_detecting(config: &Path, coderabbit_yaml: bool) -> Result<Value, String> {
    let opts = EvalOptions {
        allow_files_under: vec![root().join("pkl")],
        coderabbit_yaml: Some(coderabbit_yaml),
        ..EvalOptions::default()
    };
    let json = eval_contract_json(config, &root(), &opts).map_err(|e| e.to_string())?;
    let contract = parse_contract(&json).map_err(|e| e.to_string())?;
    serde_json::to_value(&contract).map_err(|e| e.to_string())
}

/// The files of a contract, by path, in the order the contract has them.
pub fn paths(contract: &Value) -> Vec<String> {
    contract["files"]
        .as_array()
        .expect("a contract has files")
        .iter()
        .map(|f| f["path"].as_str().expect("a file has a path").to_owned())
        .collect()
}

/// The file of a contract with this path.
pub fn file<'a>(contract: &'a Value, path: &str) -> &'a Value {
    contract["files"]
        .as_array()
        .expect("a contract has files")
        .iter()
        .find(|f| f["path"] == path)
        .unwrap_or_else(|| panic!("the contract has no file {path}"))
}

/// Replaces the commit of every pinned action with a marker, so a pin moving to a newer release does not break a comparison with a pinned
/// upstream copy.
pub fn unpin(text: &str) -> String {
    Regex::new(r"@[0-9a-f]{40}\b")
        .expect("valid")
        .replace_all(text, "@<pin>")
        .into_owned()
}

/// The package version, from the one `version = "..."` line of `pkl/PklProject`.
pub fn pkl_project_version() -> String {
    let text = read("pkl/PklProject");
    let line = Regex::new(r#"(?m)^[ \t]*version[ \t]*=[ \t]*"([^"]+)""#).expect("valid");
    let hits: Vec<_> = line.captures_iter(&text).collect();
    assert_eq!(hits.len(), 1, "PklProject has one version line");
    hits[0][1].to_owned()
}

/// The contract of a fixture, evaluated once for the process: a test file reads it from a `static`.
pub fn render(fixture_name: &str) -> Value {
    eval(&fixture(fixture_name)).unwrap_or_else(|e| panic!("{fixture_name}: {e}"))
}

/// The text of a file of a contract. Panics when the file has no content (an external file).
pub fn content<'a>(contract: &'a Value, path: &str) -> &'a str {
    file(contract, path)["content"]
        .as_str()
        .unwrap_or_else(|| panic!("{path} has no content"))
}

/// The mode of a file of a contract, as the contract writes it (`"0644"`).
pub fn mode<'a>(contract: &'a Value, path: &str) -> &'a str {
    file(contract, path)["mode"]
        .as_str()
        .unwrap_or_else(|| panic!("{path} has no mode"))
}

/// The git blob hash of a text, as git computes it.
pub fn git_blob_sha(text: &str) -> String {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("git")
        .args(["hash-object", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("git runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(text.as_bytes())
        .expect("write to git");
    let output = child.wait_with_output().expect("git finishes");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A JSON file of the pinned upstream copies, such as `blobs.json`, read from `crates/scaffold-package-tests/upstream`.
pub fn upstream_json(name: &str) -> Value {
    serde_json::from_str(&read(&format!(
        "crates/scaffold-package-tests/upstream/{name}"
    )))
    .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The repository-relative path of an asset the package ships, found by name under `pkl/Builtins/<builtin>/assets/`. Panics when there is
/// no copy, or more than one, and names what it found.
pub fn asset_path(name: &str) -> String {
    let builtins = root().join("pkl/Builtins");
    let mut found: Vec<String> = std::fs::read_dir(&builtins)
        .expect("the builtins folder exists")
        .flatten()
        .filter(|e| e.path().join("assets").join(name).exists())
        .map(|e| {
            format!(
                "pkl/Builtins/{}/assets/{name}",
                e.file_name().to_string_lossy()
            )
        })
        .collect();
    found.sort();
    assert_eq!(
        found.len(),
        1,
        "asset {name}: expected one copy under pkl/Builtins/*/assets, found {}: {}",
        found.len(),
        found.join(", ")
    );
    found.remove(0)
}

/// The pins are exactly these actions, each a 40 character commit with a version tag. The values move with the pin update workflow, so
/// they are not asserted.
pub fn expect_pins(pins: &Value, actions: &[&str]) {
    let list = pins.as_array().expect("pins is a list");
    let mut got: Vec<&str> = list.iter().map(|p| p["action"].as_str().unwrap()).collect();
    got.sort_unstable();
    let mut want = actions.to_vec();
    want.sort_unstable();
    assert_eq!(got, want);
    let sha = Regex::new(r"^[0-9a-f]{40}$").unwrap();
    let version = Regex::new(r"^v\d+(\.\d+){0,2}$").unwrap();
    for pin in list {
        let action = pin["action"].as_str().unwrap();
        assert!(sha.is_match(pin["sha"].as_str().unwrap()), "{action}");
        assert!(
            version.is_match(pin["version"].as_str().unwrap()),
            "{action}"
        );
    }
}

/// Runs `actionlint` over workflow files in a temporary repository and returns what it printed. Empty means clean.
pub fn actionlint(files: &[(&str, &str)]) -> String {
    let dir = crate::repo::temp_dir();
    for (path, text) in files {
        let target = dir.path().join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, text).unwrap();
    }
    let output = std::process::Command::new("actionlint")
        .args(files.iter().map(|(path, _)| path))
        .current_dir(dir.path())
        .output()
        .expect("actionlint runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The job ids of a workflow with their names, in the order the workflow has them.
pub fn jobs(workflow: &str) -> Vec<(String, Option<String>)> {
    let doc = crate::parse_yaml(workflow).expect("the workflow parses");
    doc["jobs"]
        .as_object()
        .expect("a workflow has jobs")
        .iter()
        .map(|(id, job)| (id.clone(), job["name"].as_str().map(str::to_owned)))
        .collect()
}

/// The lines that differ between two texts, from `diff -U0`: the ones only in `a` and the ones only in `b`.
pub fn line_diff(a: &str, b: &str) -> (Vec<String>, Vec<String>) {
    let dir = crate::repo::temp_dir();
    std::fs::write(dir.path().join("a"), a).unwrap();
    std::fs::write(dir.path().join("b"), b).unwrap();
    let output = std::process::Command::new("diff")
        .arg("-U0")
        .arg(dir.path().join("a"))
        .arg(dir.path().join("b"))
        .output()
        .expect("diff runs");
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let pick = |sign: &str, header: &str| -> Vec<String> {
        text.lines()
            .filter(|l| l.starts_with(sign) && !l.starts_with(header))
            .map(|l| l[1..].to_owned())
            .collect()
    };
    (pick("-", "---"), pick("+", "+++"))
}
