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

    /// The paths of every drifted file, in report order: what `--accept-builtins` replaces.
    pub fn drifted_paths(&self) -> Vec<&str> {
        self.drifted().iter().map(|e| e.path.as_str()).collect()
    }

    /// [`Report::adopt`] plus every drifted file: the files a write may overwrite when the owner has accepted the Builtins'
    /// version of each drifted one. A new file is never in it, and neither is a file the render does not list.
    pub fn adopt_accepting_drifted(&self) -> BTreeSet<String> {
        let mut paths = self.adopt();
        paths.extend(self.drifted_paths().into_iter().map(str::to_owned));
        paths
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

/// A Preset that keeps one drifted file as it is: `path` is where it goes (under `presets/`), `target` the file it replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetFile {
    pub path: String,
    pub target: String,
    pub text: String,
}

/// The name of a Preset for a file: its stem, with anything a Preset path does not allow turned into a dash.
fn stem(target: &str) -> String {
    let name = target.rsplit('/').next().unwrap_or(target);
    let name = name.split_once('.').map_or(name, |(first, rest)| {
        if first.is_empty() {
            rest.split('.').next().unwrap_or(rest)
        } else {
            first
        }
    });
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let clean = clean.trim_matches('-');
    if clean.is_empty() {
        "file".to_owned()
    } else {
        clean.to_owned()
    }
}

/// A pkl raw string holding `text` exactly. The delimiter has more `#` than any run in the text, so nothing in the text can end the
/// string or start an interpolation. A multi-line string leaves out the line break before its closing delimiter, so one is added.
fn raw_string(text: &str) -> String {
    let longest = text
        .split(|c| c != '#')
        .map(str::len)
        .max()
        .unwrap_or_default();
    let hashes = "#".repeat(longest + 1);
    format!("{hashes}\"\"\"\n{text}\n\"\"\"{hashes}")
}

fn preset_text(config_import: &str, target: &str, text: &str, differences: &[String]) -> String {
    let mut out = format!(
        "// Written by `pantheon-scaffold migrate` from {target}, which differed from the Builtin.\n// The whole file, kept as it was. It does not follow the Builtin when the package changes.\n"
    );
    if !differences.is_empty() {
        out.push_str("// What differed when this was written:\n");
        for line in differences {
            out.push_str(&format!("//   {}\n", line.replace(['\n', '\r'], " ")));
        }
    }
    out.push_str(&format!(
        "import \"{config_import}\"\n\nfunction apply(files: Mapping<String, Config.GenFile>): Mapping<String, Config.GenFile> =\n  (files) {{\n    [\"{}\"] = new Config.GenFile {{ content = {} }}\n  }}\n",
        target.replace('\\', "\\\\").replace('"', "\\\""),
        raw_string(text)
    ));
    out
}

/// One Preset per drifted file, each keeping the file as it is. `config_import` is the address of the package's `Config.pkl`. A file
/// that is not text cannot be kept this way. A Preset never takes the name of a file that is already under `presets/`.
pub fn verbatim_presets(
    report: &Report,
    root: &Path,
    config_import: &str,
) -> Result<Vec<PresetFile>, MigrateError> {
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    for entry in report.drifted() {
        let abs = resolve_inside(root, &entry.path).map_err(|e| MigrateError(e.to_string()))?;
        let text = std::fs::read(&abs)
            .map_err(|e| MigrateError(format!("{} cannot be read: {e}", entry.path)))
            .and_then(|bytes| {
                String::from_utf8(bytes).map_err(|_| {
                    MigrateError(format!(
                        "{} cannot be kept as a Preset: it is not UTF-8 text",
                        entry.path
                    ))
                })
            })?;
        let base = stem(&entry.path);
        let path = std::iter::once(format!("presets/{base}.pkl"))
            .chain((2..100).map(|n| format!("presets/{base}-{n}.pkl")))
            .find(|path| !taken.contains(path) && !root.join(path).exists())
            .ok_or_else(|| {
                MigrateError(format!("{base}: there are too many Presets with this name"))
            })?;
        taken.insert(path.clone());
        out.push(PresetFile {
            text: preset_text(config_import, &entry.path, &text, &entry.differences),
            path,
            target: entry.path.clone(),
        });
    }
    Ok(out)
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
            "schemaVersion": 2, "files": files, "pins": [],
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
    fn accepting_the_builtins_adds_the_drifted_files_to_the_ones_a_write_may_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "same.toml", "x = 1\n");
        put(dir.path(), "old.toml", "x = 0\n");
        put(dir.path(), "z.toml", "z = 0\n");
        put(
            dir.path(),
            ".github/workflows/ci.yml",
            "name: \"CI\"\n\"on\": push\njobs: {test: {timeout-minutes: 10}}\n",
        );
        put(dir.path(), "own.md", "mine\n");
        let c = contract(&[
            ("same.toml", "owned", "x = 1\n"),
            ("old.toml", "owned", "x = 1\n"),
            ("z.toml", "owned", "z = 1\n"),
            (".github/workflows/ci.yml", "owned", WORKFLOW),
            ("fresh.toml", "owned", "n = 1\n"),
        ]);
        let r = classify(&c, dir.path()).unwrap();
        assert_eq!(r.drifted_paths(), vec!["old.toml", "z.toml"]);
        assert_eq!(
            r.adopt_accepting_drifted().into_iter().collect::<Vec<_>>(),
            vec![
                ".github/workflows/ci.yml".to_owned(),
                "old.toml".to_owned(),
                "z.toml".to_owned()
            ],
            "the text-only YAML file and both drifted files, and neither the identical nor the new file"
        );
        assert_eq!(
            r.adopt().into_iter().collect::<Vec<_>>(),
            vec![".github/workflows/ci.yml".to_owned()],
            "adopt() itself is unchanged"
        );
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

    #[test]
    fn a_preset_is_named_after_the_file_stem_and_never_after_a_file_that_is_there() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), ".github/workflows/ci.yml", "x: 1\n");
        put(dir.path(), "ci.toml", "x = 1\n");
        put(dir.path(), ".gitignore", "a\n");
        put(dir.path(), "presets/ci.pkl", "mine\n");
        let c = contract(&[
            (".github/workflows/ci.yml", "owned", "x: 2\n"),
            ("ci.toml", "owned", "x = 2\n"),
            (".gitignore", "owned", "b\n"),
        ]);
        let r = classify(&c, dir.path()).unwrap();
        let presets = verbatim_presets(&r, dir.path(), "PKG#/Config.pkl").unwrap();
        let paths: Vec<&str> = presets.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "presets/ci-2.pkl",
                "presets/ci-3.pkl",
                "presets/gitignore.pkl"
            ]
        );
        assert_eq!(presets[0].target, ".github/workflows/ci.yml");
    }

    #[test]
    fn a_preset_holds_the_file_exactly_and_says_what_differed() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "a.toml", "x = 1\n");
        let c = contract(&[("a.toml", "owned", "x = 2\n")]);
        let r = classify(&c, dir.path()).unwrap();
        let text = verbatim_presets(&r, dir.path(), "PKG#/Config.pkl")
            .unwrap()
            .remove(0)
            .text;
        assert!(
            text.starts_with("// Written by `pantheon-scaffold migrate` from a.toml"),
            "{text}"
        );
        assert!(
            text.contains(
                "// What differed when this was written:\n//       line 1 on disk:  x = 1\n"
            ),
            "{text}"
        );
        assert!(text.contains("import \"PKG#/Config.pkl\"\n"), "{text}");
        assert!(
            text.contains(
                "[\"a.toml\"] = new Config.GenFile { content = #\"\"\"\nx = 1\n\n\"\"\"# }"
            ),
            "{text}"
        );
    }

    #[test]
    fn the_raw_string_outgrows_any_run_of_hashes_in_the_text() {
        assert_eq!(raw_string("a\n"), "#\"\"\"\na\n\n\"\"\"#");
        assert_eq!(raw_string("a"), "#\"\"\"\na\n\"\"\"#");
        assert_eq!(raw_string("x \"\"\"# y"), "##\"\"\"\nx \"\"\"# y\n\"\"\"##");
        assert_eq!(raw_string("###"), "####\"\"\"\n###\n\"\"\"####");
        assert_eq!(raw_string("#a##b#"), "###\"\"\"\n#a##b#\n\"\"\"###");
    }

    #[test]
    fn a_file_that_is_not_utf8_cannot_be_kept_as_a_preset() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), [0xff, 0xfe, 0x00]).unwrap();
        let c = contract(&[("a.bin", "owned", "x\n")]);
        let r = classify(&c, dir.path()).unwrap();
        let e = verbatim_presets(&r, dir.path(), "PKG#/Config.pkl").unwrap_err();
        assert_eq!(
            e.to_string(),
            "a.bin cannot be kept as a Preset: it is not UTF-8 text"
        );
    }

    #[test]
    fn only_drifted_files_become_presets_and_the_import_is_one_upgrade_can_move() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "same.toml", "x = 1\n");
        put(dir.path(), "a.toml", "x = 1\n");
        let c = contract(&[
            ("same.toml", "owned", "x = 1\n"),
            ("a.toml", "owned", "x = 2\n"),
        ]);
        let r = classify(&c, dir.path()).unwrap();
        let pkg = format!(
            "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v0.2.0/pantheon-scaffold@0.2.0::sha256:{}#/Config.pkl",
            "c".repeat(64)
        );
        let presets = verbatim_presets(&r, dir.path(), &pkg).unwrap();
        assert_eq!(presets.len(), 1);
        let checksum = "d".repeat(64);
        let next = crate::pins::NewPins {
            version: "0.3.0",
            checksum: &checksum,
        };
        let moved = crate::pins::moved_imports(&presets[0].text, &next).unwrap();
        assert_eq!(moved.map(|(_, count)| count), Some(1));
    }

    #[test]
    fn the_stem_drops_the_directory_the_dots_and_what_a_preset_path_does_not_allow() {
        assert_eq!(stem(".github/workflows/sync-labels.yml"), "sync-labels");
        assert_eq!(stem(".gitignore"), "gitignore");
        assert_eq!(stem("Dockerfile"), "Dockerfile");
        assert_eq!(stem("a b.toml"), "a-b");
        assert_eq!(stem("...."), "file");
        assert_eq!(stem("my.file.name.yml"), "my");
    }
}
