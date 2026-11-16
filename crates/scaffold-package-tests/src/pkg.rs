//! The package under test: where it is, how a config is evaluated against it, and the few helpers the tests share.

use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::Value;

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
        .join("runner/test/fixtures")
        .join(name)
        .join("scaffold.pkl")
}

/// Evaluates a config with the core library, as the runner does, with the package in `pkl/` allowed as a local package. The error is the
/// message of the runner.
pub fn eval(config: &Path) -> Result<Value, String> {
    let opts = EvalOptions {
        allow_files_under: vec![root().join("pkl")],
        ..EvalOptions::default()
    };
    eval_contract_json(config, &root(), &opts).map_err(|e| e.to_string())
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
