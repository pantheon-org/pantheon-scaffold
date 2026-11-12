//! `migrate`: classifies the files of an existing repository against a render, and writes nothing. Each file the render lists
//! is `identical` (the same as the Builtin), `drifted` (the same path with other content) or `new` (not there yet), with
//! seeds that exist and external files named as such. Files the render does not list, in `.github/` and at the top of the
//! repository, are `per-repo`. A `.yml` or `.yaml` file is compared by the YAML equality rule of `check`, so quoting,
//! comments and layout do not count; any other file must match byte for byte.

use std::collections::BTreeSet;
use std::path::Path;

use crate::contract::{Contract, FileClass};
use crate::equality::compare_workflows;
use crate::paths::resolve_inside;
use crate::render::{final_content, short_diff};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Identical,
    Drifted,
    New,
    SeedKept,
    External,
    PerRepo,
}

impl Class {
    pub fn label(self) -> &'static str {
        match self {
            Self::Identical => "identical",
            Self::Drifted => "drifted",
            Self::New => "new",
            Self::SeedKept => "seed-kept",
            Self::External => "external",
            Self::PerRepo => "per-repo",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub class: Class,
    pub path: String,
    /// What differs, one line each: for a YAML file the keys, for any other file the first differing lines.
    pub differences: Vec<String>,
    /// An `identical` YAML file whose text is not the render's: writing the render would change its comments or layout.
    pub text_differs: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub entries: Vec<Entry>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MigrateError(pub String);

impl std::fmt::Display for MigrateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for MigrateError {}

impl Report {
    pub fn count(&self, class: Class) -> usize {
        self.entries.iter().filter(|e| e.class == class).count()
    }

    pub fn drifted(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.class == Class::Drifted)
            .collect()
    }

    /// The files a write may overwrite: identical as YAML, different as text. Nothing else that exists is ever overwritten.
    pub fn adopt(&self) -> BTreeSet<String> {
        self.entries
            .iter()
            .filter(|e| e.class == Class::Identical && e.text_differs)
            .map(|e| e.path.clone())
            .collect()
    }
}

fn is_yaml(path: &str) -> bool {
    path.ends_with(".yml") || path.ends_with(".yaml")
}

fn entry(class: Class, path: &str) -> Entry {
    Entry {
        class,
        path: path.to_owned(),
        differences: Vec::new(),
        text_differs: false,
    }
}

fn drifted(path: &str, differences: Vec<String>) -> Entry {
    Entry {
        differences,
        ..entry(Class::Drifted, path)
    }
}

fn compare(path: &str, existing: &[u8], rendered: &str) -> Entry {
    let text = String::from_utf8_lossy(existing);
    if existing == rendered.as_bytes() {
        return entry(Class::Identical, path);
    }
    if !is_yaml(path) {
        let lines = short_diff(&text, rendered, 6)
            .lines()
            .map(str::to_owned)
            .collect();
        return drifted(path, lines);
    }
    match compare_workflows(rendered, &text) {
        Err(e) => drifted(path, vec![format!("not valid YAML: {e}")]),
        Ok(c) if c.equal => Entry {
            text_differs: true,
            ..entry(Class::Identical, path)
        },
        Ok(c) => drifted(
            path,
            c.differences.into_iter().chain(c.duplicate_keys).collect(),
        ),
    }
}

/// The regular files at the top of the repository and everywhere under `.github/`, as paths relative to the root. Symbolic
/// links are listed as files and never followed.
fn candidates(root: &Path) -> Result<Vec<String>, MigrateError> {
    fn walk(
        dir: &Path,
        prefix: &str,
        deep: bool,
        out: &mut Vec<String>,
    ) -> Result<(), MigrateError> {
        let read =
            std::fs::read_dir(dir).map_err(|e| MigrateError(format!("{}: {e}", dir.display())))?;
        for item in read {
            let item = item.map_err(|e| MigrateError(format!("{}: {e}", dir.display())))?;
            let name = item.file_name().to_string_lossy().into_owned();
            let rel = format!("{prefix}{name}");
            let kind = item
                .file_type()
                .map_err(|e| MigrateError(format!("{rel}: {e}")))?;
            if kind.is_dir() {
                if deep || name == ".github" {
                    walk(&item.path(), &format!("{rel}/"), true, out)?;
                }
            } else {
                out.push(rel);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, "", false, &mut out)?;
    out.sort();
    Ok(out)
}

pub fn classify(contract: &Contract, root: &Path) -> Result<Report, MigrateError> {
    let mut entries = Vec::new();
    for file in &contract.files {
        let abs = resolve_inside(root, &file.path).map_err(|e| MigrateError(e.to_string()))?;
        if file.class == FileClass::External {
            entries.push(entry(Class::External, &file.path));
            continue;
        }
        let existing = match std::fs::read(&abs) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                entries.push(drifted(&file.path, vec![format!("cannot be read: {e}")]));
                continue;
            }
        };
        entries.push(match (existing, file.class) {
            (None, _) => entry(Class::New, &file.path),
            (Some(_), FileClass::Seed) => entry(Class::SeedKept, &file.path),
            (Some(bytes), _) => {
                let rendered =
                    final_content(file, &contract.pins).map_err(|e| MigrateError(e.to_string()))?;
                compare(&file.path, &bytes, &rendered)
            }
        });
    }
    let listed: BTreeSet<&str> = contract.files.iter().map(|f| f.path.as_str()).collect();
    for path in candidates(root)? {
        if !listed.contains(path.as_str()) {
            entries.push(entry(Class::PerRepo, &path));
        }
    }
    Ok(Report { entries })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::parse_contract;
    use crate::testutil::put;
    use serde_json::json;

    fn contract(files: &[(&str, &str, &str)]) -> Contract {
        let files: Vec<_> = files
            .iter()
            .map(|(path, class, content)| {
                let mut f = json!({"path": path, "class": class, "mode": "0644"});
                if *class != "external" {
                    f["content"] = json!(content);
                }
                f
            })
            .collect();
        parse_contract(&json!({
            "schemaVersion": 1, "files": files, "pins": [],
            "escapeHatch": {"uses": 0, "constructs": 0}
        }))
        .expect("a valid contract")
    }

    fn find<'a>(report: &'a Report, path: &str) -> &'a Entry {
        report
            .entries
            .iter()
            .find(|e| e.path == path)
            .unwrap_or_else(|| panic!("{path} is not in {report:?}"))
    }

    fn classes(report: &Report) -> Vec<(String, &'static str)> {
        report
            .entries
            .iter()
            .map(|e| (e.path.clone(), e.class.label()))
            .collect()
    }

    const WORKFLOW: &str =
        "# a comment\nname: CI\n'on': push\njobs:\n  test:\n    timeout-minutes: 10\n";

    #[test]
    fn a_file_with_the_same_bytes_is_identical_and_a_missing_one_is_new() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "a.toml", "x = 1\n");
        let c = contract(&[
            ("a.toml", "owned", "x = 1\n"),
            ("b.toml", "owned", "y = 2\n"),
        ]);
        let r = classify(&c, dir.path()).unwrap();
        assert_eq!(
            classes(&r),
            vec![("a.toml".into(), "identical"), ("b.toml".into(), "new")]
        );
        assert!(!find(&r, "a.toml").text_differs);
        assert!(r.adopt().is_empty(), "identical bytes need no overwrite");
    }

    #[test]
    fn a_file_that_is_not_yaml_is_not_compared_as_yaml_even_when_it_would_parse_as_it() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "a.toml", "# note\na: 1\n");
        let c = contract(&[("a.toml", "owned", "a: 1\n")]);
        let r = classify(&c, dir.path()).unwrap();
        assert_eq!(find(&r, "a.toml").class, Class::Drifted);
    }

    #[test]
    fn a_yaml_file_is_recognised_by_either_extension() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "a.yaml", "# note\na: 1\n");
        put(dir.path(), "b.yml", "# note\nb: 1\n");
        let c = contract(&[("a.yaml", "owned", "a: 1\n"), ("b.yml", "owned", "b: 1\n")]);
        let r = classify(&c, dir.path()).unwrap();
        assert_eq!(find(&r, "a.yaml").class, Class::Identical);
        assert_eq!(find(&r, "b.yml").class, Class::Identical);
    }

    #[test]
    fn a_file_that_is_not_yaml_must_match_byte_for_byte() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "a.toml", "x   = 1\n");
        let c = contract(&[("a.toml", "owned", "x = 1\n")]);
        let r = classify(&c, dir.path()).unwrap();
        let e = find(&r, "a.toml");
        assert_eq!(e.class, Class::Drifted);
        assert!(!e.differences.is_empty());
    }

    #[test]
    fn a_yaml_file_that_differs_only_in_quoting_comments_and_layout_is_identical_and_may_be_adopted()
     {
        let dir = tempfile::tempdir().unwrap();
        put(
            dir.path(),
            ".github/workflows/ci.yml",
            "name: \"CI\"\n\"on\": push\njobs: {test: {timeout-minutes: 10}}\n",
        );
        let c = contract(&[(".github/workflows/ci.yml", "owned", WORKFLOW)]);
        let r = classify(&c, dir.path()).unwrap();
        let e = find(&r, ".github/workflows/ci.yml");
        assert_eq!(e.class, Class::Identical);
        assert!(e.text_differs);
        assert_eq!(
            r.adopt().into_iter().collect::<Vec<_>>(),
            vec![".github/workflows/ci.yml".to_owned()]
        );
    }

    #[test]
    fn a_yaml_file_with_one_changed_value_is_drifted_and_says_which_key() {
        let dir = tempfile::tempdir().unwrap();
        put(
            dir.path(),
            ".github/workflows/ci.yml",
            &WORKFLOW.replace("10", "20"),
        );
        let c = contract(&[(".github/workflows/ci.yml", "owned", WORKFLOW)]);
        let r = classify(&c, dir.path()).unwrap();
        let e = find(&r, ".github/workflows/ci.yml");
        assert_eq!(e.class, Class::Drifted);
        assert_eq!(
            e.differences,
            vec!["$.jobs.test.timeout-minutes: 10 vs 20".to_owned()]
        );
        assert!(r.adopt().is_empty());
        assert_eq!(r.drifted().len(), 1);
    }

    #[test]
    fn a_yaml_file_that_does_not_parse_is_drifted_with_the_reason() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "a.yml", "a: [1\n");
        let c = contract(&[("a.yml", "owned", "a: 1\n")]);
        let r = classify(&c, dir.path()).unwrap();
        let e = find(&r, "a.yml");
        assert_eq!(e.class, Class::Drifted);
        assert!(e.differences[0].starts_with("not valid YAML: "), "{e:?}");
    }

    #[test]
    fn a_yaml_file_with_a_duplicate_key_is_drifted() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "a.yml", "a: 1\na: 1\n");
        let c = contract(&[("a.yml", "owned", "a: 1\n")]);
        let r = classify(&c, dir.path()).unwrap();
        assert_eq!(find(&r, "a.yml").class, Class::Drifted);
        assert!(r.adopt().is_empty());
    }

    #[test]
    fn a_seed_that_exists_is_kept_and_one_that_does_not_is_new_and_an_external_file_is_not_compared()
     {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "SECURITY.md", "mine\n");
        let c = contract(&[
            ("SECURITY.md", "seed", "theirs\n"),
            ("CONTRIBUTING.md", "seed", "x\n"),
            (".github/workflows/release.yml", "external", ""),
        ]);
        let r = classify(&c, dir.path()).unwrap();
        assert_eq!(
            classes(&r),
            vec![
                ("SECURITY.md".into(), "seed-kept"),
                ("CONTRIBUTING.md".into(), "new"),
                (".github/workflows/release.yml".into(), "external"),
            ]
        );
        assert!(r.drifted().is_empty() && r.adopt().is_empty());
    }

    #[test]
    fn per_repo_files_are_those_under_github_and_at_the_top_that_the_render_does_not_list() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "README.md", "r\n");
        put(dir.path(), "a.toml", "x = 1\n");
        put(dir.path(), "src/main.rs", "fn main() {}\n");
        put(dir.path(), ".github/workflows/ci.yml", "name: CI\n");
        put(dir.path(), ".github/scripts/deep/run.sh", "echo\n");
        put(dir.path(), ".git/config", "[core]\n");
        put(dir.path(), ".pkl-gen/manifest.json", "{}\n");
        let c = contract(&[("a.toml", "owned", "x = 1\n")]);
        let r = classify(&c, dir.path()).unwrap();
        let per_repo: Vec<&str> = r
            .entries
            .iter()
            .filter(|e| e.class == Class::PerRepo)
            .map(|e| e.path.as_str())
            .collect();
        assert_eq!(
            per_repo,
            vec![
                ".github/scripts/deep/run.sh",
                ".github/workflows/ci.yml",
                "README.md"
            ]
        );
        assert_eq!(r.count(Class::PerRepo), 3);
        assert_eq!(r.count(Class::Identical), 1);
    }

    #[test]
    fn a_path_that_is_a_directory_is_drifted_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "a.toml/inner", "x\n");
        let c = contract(&[("a.toml", "owned", "x = 1\n")]);
        let r = classify(&c, dir.path()).unwrap();
        assert_eq!(find(&r, "a.toml").class, Class::Drifted);
    }

    #[test]
    fn a_path_that_escapes_the_repository_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let c = contract(&[("../x", "owned", "x\n")]);
        let e = classify(&c, dir.path()).unwrap_err();
        assert!(e.to_string().contains("escapes the repository"), "{e}");
    }

    #[test]
    fn the_labels_are_the_words_of_the_report() {
        let all = [
            (Class::Identical, "identical"),
            (Class::Drifted, "drifted"),
            (Class::New, "new"),
            (Class::SeedKept, "seed-kept"),
            (Class::External, "external"),
            (Class::PerRepo, "per-repo"),
        ];
        for (class, label) in all {
            assert_eq!(class.label(), label);
        }
    }
}
