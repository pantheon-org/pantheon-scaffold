//! The `pantheon.pkl` that `init` writes for a new project: the amends line of one release (or of a local package folder), the options,
//! and the composition of Builtins that renders a complete, working set (verified in a fresh repository: it renders, `check` passes and
//! a second render writes nothing). `init` writes it once; from then on `upgrade` edits the amends line and the imports of the package.

use crate::semver::parse_semver;

#[derive(Debug, PartialEq, Eq)]
pub struct InitError(pub String);

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for InitError {}

/// What the project asks for. Everything else is the same for every project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitOptions {
    pub toolchain: String,
    pub binary: bool,
    pub crate_name: Option<String>,
    pub skills: bool,
    pub skill_name: Option<String>,
    /// The repository is the skill auditor itself: its own action runs the audit and zizmor also scans `action.yml`.
    pub composite_action: bool,
    /// Git dependencies Dependabot cannot resolve, which it must leave alone (`gitDependencyIgnore`).
    pub git_dependency_ignore: Vec<String>,
    /// The package's `features = "all"`: the Rust CI workflow and the hooks build, lint and test with `--all-features`.
    /// `default` is the package's own default, so it is not written.
    pub features_all: bool,
}

/// Which package the config amends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Package {
    /// A published release, with the checksum of its package file (verified by the caller).
    Release { version: String, checksum: String },
    /// A folder holding the package, for working on it. It has no version, so the runner pin is given.
    Local { dir: String, runner_version: String },
}

fn refuse<T>(message: impl Into<String>) -> Result<T, InitError> {
    Err(InitError(message.into()))
}

fn not_a_version(text: &str) -> InitError {
    InitError(format!(
        "{} is not a version",
        serde_json::to_string(text).expect("a string serialises")
    ))
}

/// A name ends up inside a pkl string and in file contents, so it holds only what a crate or skill name can.
fn check_name(flag: &str, name: &str) -> Result<(), InitError> {
    let fine = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if fine {
        Ok(())
    } else {
        refuse(format!(
            "{flag} {name:?} may hold only letters, digits, dots, dashes and underscores"
        ))
    }
}

/// The problems with what was asked for, as the message to show.
pub fn validate(opts: &InitOptions) -> Result<(), InitError> {
    validate_for("init", opts)
}

/// `validate`, with the name of the command that was run in the message: `migrate` takes the same options as `init`.
pub fn validate_for(command: &str, opts: &InitOptions) -> Result<(), InitError> {
    if opts.toolchain.is_empty() {
        return refuse(format!(
            "{command} needs --toolchain, for example --toolchain 1.96.1"
        ));
    }
    if parse_semver(&opts.toolchain).is_none() {
        return refuse(format!(
            "--toolchain {:?} is not a version like 1.96.1",
            opts.toolchain
        ));
    }
    match (opts.binary, &opts.crate_name) {
        (true, None) => return refuse("--binary needs --crate-name"),
        (false, Some(_)) => return refuse("--crate-name needs --binary"),
        (true, Some(name)) => check_name("--crate-name", name)?,
        (false, None) => {}
    }
    match (opts.skills, &opts.skill_name) {
        (true, None) => return refuse("--skills needs --skill-name"),
        (false, Some(_)) => return refuse("--skill-name needs --skills"),
        (true, Some(name)) => check_name("--skill-name", name)?,
        (false, None) => {}
    }
    for (i, name) in opts.git_dependency_ignore.iter().enumerate() {
        check_name("--git-dependency-ignore", name)?;
        if opts.git_dependency_ignore[..i].contains(name) {
            return refuse(format!("--git-dependency-ignore {name:?} is given twice"));
        }
    }
    Ok(())
}

/// A pkl string literal's contents: a quote or a backslash would end or bend the string.
fn pkl_escaped(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Where the package is, as the start of a module address, and the runner pin a folder needs because it has no version.
fn locate(package: &Package) -> Result<(String, Option<&String>), InitError> {
    Ok(match package {
        Package::Release { version, checksum } => {
            if parse_semver(version).is_none() {
                return Err(not_a_version(version));
            }
            if checksum.len() != 64
                || !checksum
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            {
                return refuse("the checksum is not 64 hex characters");
            }
            (
                format!(
                    "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{version}/pantheon-scaffold@{version}::sha256:{checksum}#"
                ),
                None,
            )
        }
        Package::Local {
            dir,
            runner_version,
        } => {
            if dir.is_empty() {
                return refuse("the package folder is empty");
            }
            if dir.contains(['\n', '\r']) {
                return refuse("the package folder holds a line break");
            }
            if parse_semver(runner_version).is_none() {
                return Err(not_a_version(runner_version));
            }
            (pkl_escaped(dir), Some(runner_version))
        }
    })
}

/// The address of a module of the package, for an `import` line.
pub fn module_address(package: &Package, module: &str) -> Result<String, InitError> {
    let (base, _) = locate(package)?;
    Ok(format!("{base}/{module}"))
}

/// The text of `pantheon.pkl`.
pub fn config_text(opts: &InitOptions, package: &Package) -> Result<String, InitError> {
    config_text_with(opts, package, &[])
}

/// The text of `pantheon.pkl` with the Presets at these paths (under `presets/`) applied to the files, in order.
pub fn config_text_with(
    opts: &InitOptions,
    package: &Package,
    presets: &[String],
) -> Result<String, InitError> {
    validate(opts)?;
    let (base, runner) = locate(package)?;
    let join = |module: &str| format!("{base}/{module}");
    let mut options = format!("  toolchain = \"{}\"\n", opts.toolchain);
    if let Some(runner) = runner {
        options.push_str(&format!("  runnerVersion = \"{runner}\"\n"));
    }
    options.push_str(&format!("  skills = {}\n", opts.skills));
    if let Some(name) = &opts.skill_name {
        options.push_str(&format!("  skillName = \"{name}\"\n"));
    }
    options.push_str(&format!("  binary = {}\n", opts.binary));
    if let Some(name) = &opts.crate_name {
        options.push_str(&format!("  crateName = \"{name}\"\n"));
    }
    let imports: String = presets
        .iter()
        .enumerate()
        .map(|(i, path)| format!("import \"{}\" as preset{i}\n", pkl_escaped(path)))
        .collect();
    let listed: String = if presets.is_empty() {
        String::new()
    } else {
        let paths: String = presets
            .iter()
            .map(|path| format!("  \"{}\"\n", pkl_escaped(path)))
            .collect();
        format!("presets {{\n{paths}}}\n\n")
    };
    let files = if presets.is_empty() {
        "files {\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles(options)\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}\n".to_owned()
    } else {
        let applied = (0..presets.len()).fold("standard".to_owned(), |inner, i| {
            format!("preset{i}.apply({inner})")
        });
        format!(
            "local standard = new Mapping {{\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles(options)\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}}\n\nfiles = {applied}\n"
        )
    };
    if opts.composite_action {
        options.push_str("  compositeAction = true\n");
    }
    if !opts.git_dependency_ignore.is_empty() {
        let names: Vec<String> = opts
            .git_dependency_ignore
            .iter()
            .map(|n| format!("\"{n}\""))
            .collect();
        options.push_str(&format!(
            "  gitDependencyIgnore {{ {} }}\n",
            names.join("; ")
        ));
    }
    if opts.features_all {
        options.push_str("  features = \"all\"\n");
    }
    Ok(format!(
        "// Written by `pantheon-scaffold init`. From here on `pantheon-scaffold upgrade` edits the amends line and the imports of the package, and everything else is yours.\namends \"{}\"\n\nimport \"{}\"\n{imports}\noptions {{\n{options}}}\n\n{listed}workflows {{ ...Builtins.standardWorkflows(options) }}\n\n{files}",
        join("Config.pkl"),
        join("Builtins.pkl")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pins::{RUNNER_TOOL, count_package_imports, read_pins};
    use crate::testutil::put;

    fn opts() -> InitOptions {
        InitOptions {
            toolchain: "1.96.1".into(),
            ..InitOptions::default()
        }
    }

    fn release() -> Package {
        Package::Release {
            version: "0.2.0".into(),
            checksum: "c".repeat(64),
        }
    }

    fn local() -> Package {
        Package::Local {
            dir: "/work/pkl".into(),
            runner_version: "0.2.0".into(),
        }
    }

    fn problem(o: &InitOptions) -> String {
        validate(o).unwrap_err().to_string()
    }

    const COMPOSITION: &str = "workflows { ...Builtins.standardWorkflows(options) }\n\nfiles {\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles(options)\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}\n";

    #[test]
    fn a_release_config_amends_the_release_with_its_checksum_and_imports_the_builtins_from_it() {
        let pkg = format!(
            "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v0.2.0/pantheon-scaffold@0.2.0::sha256:{}",
            "c".repeat(64)
        );
        assert_eq!(
            config_text(&opts(), &release()).unwrap(),
            format!(
                "// Written by `pantheon-scaffold init`. From here on `pantheon-scaffold upgrade` edits the amends line and the imports of the package, and everything else is yours.\namends \"{pkg}#/Config.pkl\"\n\nimport \"{pkg}#/Builtins.pkl\"\n\noptions {{\n  toolchain = \"1.96.1\"\n  skills = false\n  binary = false\n}}\n\n{COMPOSITION}"
            )
        );
    }

    #[test]
    fn a_local_config_amends_the_folder_and_gives_the_runner_pin_because_a_folder_has_no_version() {
        assert_eq!(
            config_text(&opts(), &local()).unwrap(),
            format!(
                "// Written by `pantheon-scaffold init`. From here on `pantheon-scaffold upgrade` edits the amends line and the imports of the package, and everything else is yours.\namends \"/work/pkl/Config.pkl\"\n\nimport \"/work/pkl/Builtins.pkl\"\n\noptions {{\n  toolchain = \"1.96.1\"\n  runnerVersion = \"0.2.0\"\n  skills = false\n  binary = false\n}}\n\n{COMPOSITION}"
            )
        );
    }

    #[test]
    fn the_options_a_project_turns_on_are_written_with_their_names() {
        let o = InitOptions {
            binary: true,
            crate_name: Some("my-tool".into()),
            skills: true,
            skill_name: Some("my-skill".into()),
            ..opts()
        };
        let text = config_text(&o, &release()).unwrap();
        assert!(
            text.contains("options {\n  toolchain = \"1.96.1\"\n  skills = true\n  skillName = \"my-skill\"\n  binary = true\n  crateName = \"my-tool\"\n}"),
            "{text}"
        );
    }

    #[test]
    fn a_path_with_a_quote_or_a_backslash_is_escaped_so_it_stays_one_pkl_string() {
        let package = Package::Local {
            dir: "/work/a\"b\\c".into(),
            runner_version: "0.2.0".into(),
        };
        let text = config_text(&opts(), &package).unwrap();
        assert!(
            text.contains("amends \"/work/a\\\"b\\\\c/Config.pkl\""),
            "{text}"
        );
    }

    #[test]
    fn the_text_is_what_upgrade_reads_so_a_later_upgrade_can_move_it() {
        let dir = tempfile::tempdir().unwrap();
        put(
            dir.path(),
            "pantheon.pkl",
            &config_text(&opts(), &release()).unwrap(),
        );
        put(
            dir.path(),
            "mise.toml",
            &format!("[tools]\n\"{RUNNER_TOOL}\" = \"0.2.0\"\n"),
        );
        let pins = read_pins(dir.path(), "pantheon.pkl").unwrap();
        assert_eq!(pins.package_version, "0.2.0");
        assert_eq!(pins.checksum.as_deref(), Some("c".repeat(64).as_str()));
        assert_eq!(
            count_package_imports(dir.path(), "pantheon.pkl").unwrap(),
            1
        );
    }

    #[test]
    fn a_valid_request_has_no_problem() {
        assert_eq!(validate(&opts()), Ok(()));
        let full = InitOptions {
            binary: true,
            crate_name: Some("a_b.c-1".into()),
            skills: true,
            skill_name: Some("skill".into()),
            ..opts()
        };
        assert_eq!(validate(&full), Ok(()));
    }

    #[test]
    fn the_composite_action_and_the_ignored_git_dependencies_are_written_after_the_other_options() {
        let o = InitOptions {
            composite_action: true,
            git_dependency_ignore: vec!["pahdk-core".into(), "pahdk-install".into()],
            ..opts()
        };
        let text = config_text(&o, &local()).unwrap();
        assert!(
            text.contains("  skills = false\n  binary = false\n  compositeAction = true\n  gitDependencyIgnore { \"pahdk-core\"; \"pahdk-install\" }\n}\n"),
            "{text}"
        );
        let plain = config_text(&opts(), &local()).unwrap();
        assert!(!plain.contains("compositeAction") && !plain.contains("gitDependencyIgnore"));
    }

    #[test]
    fn features_all_is_written_last_after_the_other_options_and_default_writes_nothing() {
        let o = InitOptions {
            composite_action: true,
            git_dependency_ignore: vec!["pahdk-core".into()],
            features_all: true,
            ..opts()
        };
        let text = config_text(&o, &local()).unwrap();
        assert!(
            text.contains(
                "  compositeAction = true\n  gitDependencyIgnore { \"pahdk-core\" }\n  features = \"all\"\n}\n"
            ),
            "{text}"
        );
        let plain = config_text(&opts(), &local()).unwrap();
        assert!(!plain.contains("features"), "{plain}");
        assert!(
            plain.contains("  skills = false\n  binary = false\n}\n"),
            "default adds nothing after the other options: {plain}"
        );
    }

    #[test]
    fn features_all_is_also_written_when_presets_are_listed() {
        let o = InitOptions {
            features_all: true,
            ..opts()
        };
        let text = config_text_with(&o, &local(), &["presets/ci.pkl".to_owned()]).unwrap();
        assert!(
            text.contains("  binary = false\n  features = \"all\"\n}\n"),
            "{text}"
        );
        assert!(
            text.contains("presets {\n  \"presets/ci.pkl\"\n}"),
            "{text}"
        );
    }

    #[test]
    fn an_ignored_dependency_is_a_name_and_is_given_once() {
        let with = |names: &[&str]| InitOptions {
            git_dependency_ignore: names.iter().map(|n| (*n).to_owned()).collect(),
            ..opts()
        };
        assert_eq!(validate(&with(&["a", "b.c_d-e"])), Ok(()));
        assert_eq!(
            problem(&with(&["a", "b", "a"])),
            "--git-dependency-ignore \"a\" is given twice"
        );
        assert_eq!(
            problem(&with(&["a b"])),
            "--git-dependency-ignore \"a b\" may hold only letters, digits, dots, dashes and underscores"
        );
        assert_eq!(
            problem(&with(&[""])),
            "--git-dependency-ignore \"\" may hold only letters, digits, dots, dashes and underscores"
        );
    }

    #[test]
    fn the_message_names_the_command_that_was_run() {
        assert_eq!(
            validate_for("migrate", &InitOptions::default())
                .unwrap_err()
                .to_string(),
            "migrate needs --toolchain, for example --toolchain 1.96.1"
        );
    }

    #[test]
    fn presets_are_imported_listed_and_applied_in_order_to_the_files() {
        let text = config_text_with(
            &opts(),
            &local(),
            &["presets/a.pkl".to_owned(), "presets/b.pkl".to_owned()],
        )
        .unwrap();
        assert!(text.contains("import \"/work/pkl/Builtins.pkl\"\nimport \"presets/a.pkl\" as preset0\nimport \"presets/b.pkl\" as preset1\n\noptions {"), "{text}");
        assert!(
            text.contains(
                "}\n\npresets {\n  \"presets/a.pkl\"\n  \"presets/b.pkl\"\n}\n\nworkflows {"
            ),
            "{text}"
        );
        assert!(
            text.contains("local standard = new Mapping {\n  ...Builtins.standardFiles(options)\n"),
            "{text}"
        );
        assert!(
            text.ends_with("}\n\nfiles = preset1.apply(preset0.apply(standard))\n"),
            "{text}"
        );
        assert!(!text.contains("\nfiles {"), "{text}");
    }

    #[test]
    fn with_no_presets_config_text_with_is_config_text() {
        assert_eq!(
            config_text_with(&opts(), &release(), &[]).unwrap(),
            config_text(&opts(), &release()).unwrap()
        );
    }

    #[test]
    fn the_address_of_a_module_is_the_package_address_with_the_module() {
        assert_eq!(
            module_address(&local(), "Config.pkl").unwrap(),
            "/work/pkl/Config.pkl"
        );
        let release = module_address(&release(), "Config.pkl").unwrap();
        assert!(
            release.ends_with(&format!("::sha256:{}#/Config.pkl", "c".repeat(64))),
            "{release}"
        );
    }

    #[test]
    fn the_toolchain_is_required_and_must_be_a_version() {
        assert_eq!(
            problem(&InitOptions::default()),
            "init needs --toolchain, for example --toolchain 1.96.1"
        );
        for bad in ["stable", "1.96", "v1.96.1x", " 1.96.1"] {
            let o = InitOptions {
                toolchain: bad.into(),
                ..InitOptions::default()
            };
            assert_eq!(
                problem(&o),
                format!("--toolchain {bad:?} is not a version like 1.96.1"),
                "{bad}"
            );
        }
    }

    #[test]
    fn binary_needs_a_crate_name_and_a_crate_name_needs_binary() {
        let o = InitOptions {
            binary: true,
            ..opts()
        };
        assert_eq!(problem(&o), "--binary needs --crate-name");
        let o = InitOptions {
            crate_name: Some("x".into()),
            ..opts()
        };
        assert_eq!(problem(&o), "--crate-name needs --binary");
    }

    #[test]
    fn skills_needs_a_skill_name_and_a_skill_name_needs_skills() {
        let o = InitOptions {
            skills: true,
            ..opts()
        };
        assert_eq!(problem(&o), "--skills needs --skill-name");
        let o = InitOptions {
            skill_name: Some("x".into()),
            ..opts()
        };
        assert_eq!(problem(&o), "--skill-name needs --skills");
    }

    #[test]
    fn a_name_is_letters_digits_dots_dashes_and_underscores_only() {
        for bad in ["", "a b", "a\"b", "a/b", "a\\b", "a\nb"] {
            let o = InitOptions {
                binary: true,
                crate_name: Some(bad.into()),
                ..opts()
            };
            assert_eq!(
                problem(&o),
                format!(
                    "--crate-name {bad:?} may hold only letters, digits, dots, dashes and underscores"
                ),
                "{bad:?}"
            );
            let o = InitOptions {
                skills: true,
                skill_name: Some(bad.into()),
                ..opts()
            };
            assert_eq!(
                problem(&o),
                format!(
                    "--skill-name {bad:?} may hold only letters, digits, dots, dashes and underscores"
                ),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_package_that_is_not_usable_is_refused() {
        let bad_version = Package::Release {
            version: "banana".into(),
            checksum: "c".repeat(64),
        };
        assert_eq!(
            config_text(&opts(), &bad_version).unwrap_err().to_string(),
            "\"banana\" is not a version"
        );
        let bad_sum = Package::Release {
            version: "0.2.0".into(),
            checksum: "abc".into(),
        };
        assert_eq!(
            config_text(&opts(), &bad_sum).unwrap_err().to_string(),
            "the checksum is not 64 hex characters"
        );
        let bad_runner = Package::Local {
            dir: "/x".into(),
            runner_version: "latest".into(),
        };
        assert_eq!(
            config_text(&opts(), &bad_runner).unwrap_err().to_string(),
            "\"latest\" is not a version"
        );
        let empty_dir = Package::Local {
            dir: String::new(),
            runner_version: "0.2.0".into(),
        };
        assert_eq!(
            config_text(&opts(), &empty_dir).unwrap_err().to_string(),
            "the package folder is empty"
        );
        let newline = Package::Local {
            dir: "/x\ny".into(),
            runner_version: "0.2.0".into(),
        };
        assert_eq!(
            config_text(&opts(), &newline).unwrap_err().to_string(),
            "the package folder holds a line break"
        );
    }

    #[test]
    fn a_config_is_refused_when_the_options_are_not_valid() {
        assert!(config_text(&InitOptions::default(), &release()).is_err());
    }

    #[test]
    fn the_checks_that_parse_versions_agree_with_the_runner() {
        assert!(parse_semver("1.96.1").is_some());
    }
}
