//! Ported from `runner/test/templates.test.ts`: the assets the builtins ship are templates, and no builtin patches rendered text.

use regex::Regex;
use scaffold_package_tests::pkg::root;
use std::path::PathBuf;

struct Asset {
    builtin: String,
    file: String,
    path: PathBuf,
}

fn builtins() -> PathBuf {
    root().join("pkl/Builtins")
}

fn assets() -> Vec<Asset> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(builtins()).unwrap().flatten() {
        let dir = entry.path().join("assets");
        if !entry.path().is_dir() || !dir.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&dir).unwrap().flatten() {
            found.push(Asset {
                builtin: entry.file_name().to_string_lossy().into_owned(),
                file: file.file_name().to_string_lossy().into_owned(),
                path: file.path(),
            });
        }
    }
    found.sort_by(|a, b| (&a.builtin, &a.file).cmp(&(&b.builtin, &b.file)));
    found
}

/// The one named exception: a seed with comments and an empty [tools] table, so there is no value to pass (docs/templates-variables.md,
/// question 2).
const STATIC_STRUCTURED: [&str; 1] = ["mise/mise-repo.toml"];

#[test]
fn every_toml_yaml_and_json_asset_a_builtin_ships_is_a_jinja_template_apart_from_the_one_named_exception()
 {
    let structured = Regex::new(r"\.(toml|ya?ml|json)$").unwrap();
    let plain: Vec<String> = assets()
        .iter()
        .filter(|a| structured.is_match(&a.file))
        .map(|a| format!("{}/{}", a.builtin, a.file))
        .filter(|name| !STATIC_STRUCTURED.contains(&name.as_str()))
        .collect();
    assert_eq!(plain, Vec::<String>::new());
    assert!(!assets().is_empty(), "the builtins ship assets");
}

#[test]
fn no_asset_still_uses_an_at_at_name_at_at_placeholder() {
    let placeholder = Regex::new(r"@@[A-Z_]+@@").unwrap();
    let found: Vec<String> = assets()
        .iter()
        .filter(|a| placeholder.is_match(&std::fs::read_to_string(&a.path).unwrap_or_default()))
        .map(|a| format!("{}/{}", a.builtin, a.file))
        .collect();
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn no_builtin_patches_rendered_text_with_replace_all_or_replace_first_a_variable_and_a_conditional_do_that_in_the_template()
 {
    let patch = Regex::new(r"\.replace(All|First)\(").unwrap();
    let mut modules: Vec<PathBuf> = std::fs::read_dir(builtins())
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.path().join("Builtin.pkl"))
        .filter(|p| p.exists())
        .collect();
    modules.sort();
    assert!(!modules.is_empty(), "the builtins have modules");
    let found: Vec<&PathBuf> = modules
        .iter()
        .filter(|p| patch.is_match(&std::fs::read_to_string(p).unwrap()))
        .collect();
    assert_eq!(found, Vec::<&PathBuf>::new());
}
