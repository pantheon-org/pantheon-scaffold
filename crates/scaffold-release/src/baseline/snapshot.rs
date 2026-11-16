//! Renders a pantheon-scaffold tree with one binary and writes what came out, one JSON file per configuration. Ported from
//! `runner/scripts/builtins-baseline/snapshot.ts`: the folder it writes is byte for byte the one the TypeScript writes.
//!
//! Configurations: the tree's own `scaffold.pkl`, every `runner/test/fixtures/*/scaffold.pkl`, and an option matrix written into
//! `<out dir>/_matrix` (skills, binary, compositeAction, features, plus error paths), all built on the tree's `pkl/` folder. Per
//! configuration: the exit code, stdout and stderr of `contract`, `render` and `check`, every file written (path, mode, sha256) and the
//! manifest as text. The tree path and the render directory are replaced by placeholders.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::json::{octal_mode, resolve, sort_like_javascript, stringify_indent_1};

pub const USAGE: &str =
    "usage: scaffold-release baseline snapshot <tree root> <binary> <out dir> [--api old|new]";

/// How the matrix configurations call the aggregators that gained a parameter: `new` is `toolingFiles(options)` and `old` is
/// `toolingFiles()`, for a tree from before the presence work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Api {
    Old,
    New,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    pub tree: PathBuf,
    pub binary: PathBuf,
    pub out: PathBuf,
    pub api: Api,
}

/// The arguments, or the message for a usage error.
pub fn parse_args(args: &[&str], cwd: &Path) -> Result<Args, String> {
    let api = match args.iter().position(|a| *a == "--api") {
        None => Api::New,
        Some(i) => match args.get(i + 1) {
            Some(&"old") => Api::Old,
            Some(&"new") => Api::New,
            _ => return Err("--api must be old or new".to_owned()),
        },
    };
    let positional: Vec<&str> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| **a != "--api" && (*i == 0 || args[*i - 1] != "--api"))
        .map(|(_, a)| *a)
        .collect();
    let [tree, binary, out, ..] = positional[..] else {
        return Err(USAGE.to_owned());
    };
    Ok(Args {
        tree: resolve(tree, cwd),
        binary: resolve(binary, cwd),
        out: resolve(out, cwd),
        api,
    })
}

struct Run {
    code: i32,
    out: String,
    err: String,
}

impl Run {
    fn to_json(&self) -> Value {
        json!({ "code": self.code, "out": self.out, "err": self.err })
    }
}

/// The paths the output must not name, replaced by placeholders in the order the TypeScript replaces them.
struct Placeholders<'a> {
    dir: &'a Path,
    out: &'a Path,
    tree: &'a Path,
}

impl Placeholders<'_> {
    fn apply(&self, text: &str) -> String {
        text.replace(&*self.dir.to_string_lossy(), "<dir>")
            .replace(&*self.out.to_string_lossy(), "<out>")
            .replace(&*self.tree.to_string_lossy(), "<tree>")
    }
}

fn run(
    binary: &Path,
    args: &[String],
    dir: &Path,
    placeholders: &Placeholders,
) -> Result<Run, String> {
    let output = Command::new(binary)
        .args(args)
        .current_dir(dir)
        .env("CI", "")
        .output()
        .map_err(|e| format!("{}: {e}", binary.display()))?;
    Ok(Run {
        code: output.status.code().unwrap_or(-1),
        out: placeholders.apply(&String::from_utf8_lossy(&output.stdout)),
        err: placeholders.apply(&String::from_utf8_lossy(&output.stderr)),
    })
}

/// Every file under `dir` as `<path> <mode> <sha256>`, sorted, leaving out the scratch folders of the runner.
fn files(dir: &Path) -> Result<Vec<String>, String> {
    fn walk(root: &Path, current: &Path, found: &mut Vec<String>) -> Result<(), String> {
        let read = |e: std::io::Error| format!("{}: {e}", current.display());
        for entry in std::fs::read_dir(current).map_err(read)? {
            let entry = entry.map_err(read)?;
            let full = entry.path();
            let relative = full
                .strip_prefix(root)
                .expect("an entry is under its root")
                .to_string_lossy()
                .into_owned();
            if [".pkl-gen/stage", ".pkl-gen/lock", ".pkl-gen/cache"].contains(&relative.as_str()) {
                continue;
            }
            if entry.file_type().map_err(read)?.is_dir() {
                walk(root, &full, found)?;
            } else {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&full).map_err(read)?.permissions().mode();
                let bytes = std::fs::read(&full).map_err(|e| format!("{}: {e}", full.display()))?;
                let sha: String = Sha256::digest(&bytes)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                found.push(format!("{relative} {} {sha}", octal_mode(mode)));
            }
        }
        Ok(())
    }
    let mut found = Vec::new();
    walk(dir, dir, &mut found)?;
    sort_like_javascript(&mut found);
    Ok(found)
}

#[derive(Clone)]
struct Opts {
    skills: bool,
    binary: bool,
    composite_action: bool,
    features: &'static str,
    skill_name: Option<&'static str>,
    crate_name: Option<&'static str>,
    runner_version: Option<&'static str>,
    ignore: Option<Vec<&'static str>>,
    without: Option<Vec<&'static str>>,
}

const BASE: Opts = Opts {
    skills: true,
    binary: true,
    composite_action: false,
    features: "default",
    skill_name: Some("demo-skill"),
    crate_name: Some("demo"),
    runner_version: Some("0.1.9"),
    ignore: None,
    without: None,
};

/// The option matrix, in the order the TypeScript builds it.
fn matrix() -> Vec<(String, Opts)> {
    let mut all = Vec::new();
    for skills in [true, false] {
        for binary in [true, false] {
            for composite_action in [false, true] {
                for features in ["default", "all"] {
                    all.push((
                        format!("m-skills-{skills}-binary-{binary}-composite-{composite_action}-features-{features}"),
                        Opts { skills, binary, composite_action, features, ..BASE },
                    ));
                }
            }
        }
    }
    all.push((
        "m-ignore".to_owned(),
        Opts {
            ignore: Some(vec!["serde", "tokio"]),
            ..BASE
        },
    ));
    // Builtins left out (options.without): what mise.toml, hk.shared.pkl, .rumdl.toml and the zizmor header say when something is missing.
    for (name, without) in [
        ("w-hk", vec!["hk"]),
        ("w-rumdl", vec!["rumdl"]),
        ("w-rust-ci", vec!["rust-ci"]),
        ("w-rust-ci-and-toolchain", vec!["rust-ci", "rust-toolchain"]),
        ("w-mise", vec!["mise"]),
        ("w-hk-and-mise", vec!["hk", "mise"]),
    ] {
        all.push((
            name.to_owned(),
            Opts {
                without: Some(without),
                ..BASE
            },
        ));
    }
    all.push((
        "e-no-skill-name".to_owned(),
        Opts {
            skill_name: None,
            ..BASE
        },
    ));
    all.push((
        "e-no-crate-name".to_owned(),
        Opts {
            crate_name: None,
            ..BASE
        },
    ));
    all.push((
        "e-no-runner-version".to_owned(),
        Opts {
            runner_version: None,
            ..BASE
        },
    ));
    all
}

fn quoted_list(items: &[&str]) -> String {
    items
        .iter()
        .map(|x| format!("\"{x}\""))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The text of a matrix configuration.
fn matrix_config(opts: &Opts, tree: &Path, api: Api) -> String {
    let mut lines = vec![r#"toolchain = "1.96.1""#.to_owned()];
    if let Some(v) = opts.skill_name {
        lines.push(format!("skillName = \"{v}\""));
    }
    if let Some(v) = opts.crate_name {
        lines.push(format!("crateName = \"{v}\""));
    }
    if let Some(v) = opts.runner_version {
        lines.push(format!("runnerVersion = \"{v}\""));
    }
    lines.push(format!("skills = {}", opts.skills));
    lines.push(format!("binary = {}", opts.binary));
    lines.push(format!("features = \"{}\"", opts.features));
    lines.push(format!("compositeAction = {}", opts.composite_action));
    if let Some(without) = &opts.without {
        lines.push(format!("without {{ {} }}", quoted_list(without)));
    }
    if let Some(ignore) = &opts.ignore {
        lines.push(format!("gitDependencyIgnore {{ {} }}", quoted_list(ignore)));
    }
    let tree = tree.display();
    let tooling = if api == Api::New { "options" } else { "" };
    format!(
        "amends \"file://{tree}/pkl/Config.pkl\"\nimport \"file://{tree}/pkl/Builtins.pkl\"\noptions {{\n  {}\n}}\nworkflows {{ ...Builtins.standardWorkflows(options) }}\nfiles {{\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles({tooling})\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}}\n",
        lines.join("\n  ")
    )
}

#[cfg(test)]
pub(super) fn matrix_names() -> Vec<String> {
    matrix().into_iter().map(|(name, _)| name).collect()
}

#[cfg(test)]
pub(super) fn matrix_text(name: &str, tree: &Path, api: Api) -> String {
    let (_, opts) = matrix()
        .into_iter()
        .find(|(n, _)| n == name)
        .expect("a configuration of that name");
    matrix_config(&opts, tree, api)
}

#[cfg(test)]
pub(super) fn files_for_test(dir: &Path) -> Vec<String> {
    files(dir).expect("the files can be listed")
}

#[cfg(test)]
pub(super) fn placeholders_for_test(text: &str) -> String {
    Placeholders {
        dir: Path::new("/t/out/dir"),
        out: Path::new("/t/out"),
        tree: Path::new("/t/tree"),
    }
    .apply(text)
}

/// Renders every configuration and writes the snapshot. Returns the exit code.
pub fn snapshot(args: &Args, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match snapshot_inner(args, out) {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}

fn snapshot_inner(args: &Args, out: &mut dyn Write) -> Result<(), String> {
    let Args {
        tree,
        binary,
        out: out_dir,
        api,
    } = args;
    let io = |path: &Path| {
        let path = path.to_owned();
        move |e: std::io::Error| format!("{}: {e}", path.display())
    };
    std::fs::create_dir_all(out_dir).map_err(io(out_dir))?;
    let allow = [
        "--allow-local-package".to_owned(),
        tree.join("pkl").to_string_lossy().into_owned(),
    ];

    let mut configs: Vec<(String, PathBuf)> = Vec::new();
    if tree.join("scaffold.pkl").exists() {
        configs.push(("root".to_owned(), tree.join("scaffold.pkl")));
    }
    let fixtures = tree.join("runner/test/fixtures");
    let mut names: Vec<String> = std::fs::read_dir(&fixtures)
        .map_err(io(&fixtures))?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    sort_like_javascript(&mut names);
    for name in names {
        let config = fixtures.join(&name).join("scaffold.pkl");
        if config.exists() {
            configs.push((format!("fixture-{name}"), config));
        }
    }
    for (name, opts) in matrix() {
        let dir = out_dir.join("_matrix").join(&name);
        std::fs::create_dir_all(&dir).map_err(io(&dir))?;
        let config = dir.join("scaffold.pkl");
        std::fs::write(&config, matrix_config(&opts, tree, *api)).map_err(io(&config))?;
        configs.push((name, config));
    }

    for (name, config) in &configs {
        let scratch = tempfile::Builder::new()
            .prefix("builtins-snap-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        let dir = scratch.path();
        let placeholders = Placeholders {
            dir,
            out: out_dir,
            tree,
        };
        let config_text = config.to_string_lossy().into_owned();
        let dir_text = dir.to_string_lossy().into_owned();
        let contract = run(
            binary,
            &[
                vec!["contract".to_owned(), config_text.clone()],
                allow.to_vec(),
            ]
            .concat(),
            dir,
            &placeholders,
        )?;
        let root_args = |command: &str| {
            [
                vec![
                    command.to_owned(),
                    config_text.clone(),
                    "--root".to_owned(),
                    dir_text.clone(),
                ],
                allow.to_vec(),
            ]
            .concat()
        };
        let render = run(binary, &root_args("render"), dir, &placeholders)?;
        let check = run(binary, &root_args("check"), dir, &placeholders)?;
        let manifest_path = dir.join(".pkl-gen/manifest.json");
        let manifest = if manifest_path.exists() {
            placeholders
                .apply(&std::fs::read_to_string(&manifest_path).map_err(io(&manifest_path))?)
        } else {
            "(none)".to_owned()
        };
        let listed = files(dir)?;
        let mut result = Map::new();
        result.insert("contract".to_owned(), contract.to_json());
        result.insert("render".to_owned(), render.to_json());
        result.insert("check".to_owned(), check.to_json());
        result.insert("files".to_owned(), json!(listed));
        result.insert("manifest".to_owned(), json!(manifest));
        let file = out_dir.join(format!("{name}.json"));
        std::fs::write(&file, stringify_indent_1(&Value::Object(result))).map_err(io(&file))?;
        writeln!(
            out,
            "{name}: contract {}, render {}, check {}, {} files",
            contract.code,
            render.code,
            check.code,
            listed.len()
        )
        .map_err(|e| e.to_string())?;
    }
    writeln!(
        out,
        "\n{} configurations written to {}",
        configs.len(),
        out_dir.display()
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod run_tests {
    use super::*;

    #[test]
    fn a_runner_that_is_killed_has_no_exit_code_and_is_recorded_as_exit_minus_1() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("runner");
        std::fs::write(&script, "#!/bin/sh\nkill -9 $$\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let placeholders = Placeholders {
            dir: dir.path(),
            out: dir.path(),
            tree: dir.path(),
        };
        let result = run(&script, &[], dir.path(), &placeholders).unwrap();
        assert_eq!((result.code, result.out.as_str()), (-1, ""));
    }
}
