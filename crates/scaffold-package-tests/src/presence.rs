//! Renders the full composition with builtins left out (`options.without`), for the presence tests.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;

use crate::pkg::{eval, root};
use crate::repo::temp_dir;

pub const COMPOSITION: &str = "workflows { ...Builtins.standardWorkflows(options) }\nfiles {\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles(options)\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}";

/// Writes a scaffold.pkl with the given composition, the options (skills and binary on unless set) and the builtins left out, and returns its
/// path with the folder that keeps it.
pub fn config(
    without: &[&str],
    extra: &str,
    composition: &str,
    imports: &str,
) -> (tempfile::TempDir, PathBuf) {
    let list = without
        .iter()
        .map(|w| format!("\"{w}\""))
        .collect::<Vec<_>>()
        .join("; ");
    let dir = temp_dir();
    let path = dir.path().join("presence.pkl");
    std::fs::write(
        &path,
        format!(
            "amends \"file://{root}/pkl/Config.pkl\"\nimport \"file://{root}/pkl/Builtins.pkl\"\n{imports}\noptions {{\n  toolchain = \"1.96.1\"\n  runnerVersion = \"0.1.9\"\n  skillName = \"demo-skill\"\n  crateName = \"demo\"\n  {extra}\n  without {{ {list} }}\n}}\n{composition}\n",
            root = root().display()
        ),
    )
    .unwrap();
    (dir, path)
}

pub fn eval_config(
    without: &[&str],
    extra: &str,
    composition: &str,
    imports: &str,
) -> Result<Value, String> {
    let (_keep, path) = config(without, extra, composition, imports);
    eval(&path)
}

/// The full composition rendered with these builtins left out: path to content (empty for an external file).
pub fn render(without: &[&str], extra: &str) -> BTreeMap<String, String> {
    let contract = eval_config(without, extra, COMPOSITION, "").expect("the configuration renders");
    contract["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["path"].as_str().unwrap().to_owned(),
                f["content"].as_str().unwrap_or("").to_owned(),
            )
        })
        .collect()
}

/// The names of the folders under `pkl/Builtins`, sorted.
pub fn folders() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root().join("pkl/Builtins"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Like `eval_config`, but the runner reports that the repository has (or has no) `.coderabbit.yaml`, which `reviewBot = "auto"` reads.
pub fn eval_config_detecting(coderabbit_yaml: bool, extra: &str) -> Result<Value, String> {
    let (_keep, path) = config(&[], extra, COMPOSITION, "");
    crate::pkg::eval_detecting(&path, coderabbit_yaml)
}
