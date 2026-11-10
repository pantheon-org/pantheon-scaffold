//! Compares the render with the files on disk and writes nothing. Ported from `runner/src/check.ts`.

use std::path::Path;

use crate::contract::{Contract, FileClass, Mode};
use crate::paths::resolve_inside;
use crate::render::{final_content, short_diff};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Missing,
    Different,
    Mode,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Drift {
    pub path: String,
    pub reason: Reason,
    pub diff: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Checked {
    pub drift: Vec<Drift>,
    pub checked: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CheckError(pub String);

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for CheckError {}

/// Seed and external files are skipped: only owned files can drift.
pub fn check(contract: &Contract, root: &Path) -> Result<Checked, CheckError> {
    use std::os::unix::fs::PermissionsExt;
    let mut drift = Vec::new();
    let mut checked = 0;
    for f in contract
        .files
        .iter()
        .filter(|f| f.class == FileClass::Owned)
    {
        checked += 1;
        let abs = resolve_inside(root, &f.path).map_err(|e| CheckError(e.to_string()))?;
        let expected = final_content(f, &contract.pins).map_err(|e| CheckError(e.to_string()))?;
        if !abs.exists() {
            drift.push(Drift {
                path: f.path.clone(),
                reason: Reason::Missing,
                diff: String::new(),
            });
            continue;
        }
        let bytes = std::fs::read(&abs).map_err(|e| CheckError(format!("{}: {e}", f.path)))?;
        let actual = String::from_utf8_lossy(&bytes);
        if actual != expected {
            drift.push(Drift {
                path: f.path.clone(),
                reason: Reason::Different,
                diff: short_diff(&actual, &expected, 6),
            });
            continue;
        }
        let wants_exec = f.mode == Mode::Executable;
        let mode = std::fs::metadata(&abs)
            .map_err(|e| CheckError(format!("{}: {e}", f.path)))?
            .permissions()
            .mode();
        let is_exec = mode & 0o111 != 0;
        if wants_exec != is_exec {
            let name = |exec: bool| if exec { "executable" } else { "not executable" };
            drift.push(Drift {
                path: f.path.clone(),
                reason: Reason::Mode,
                diff: format!("    expected {}, found {}", name(wants_exec), name(is_exec)),
            });
        }
    }
    Ok(Checked { drift, checked })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Mode, Pin};
    use crate::testutil::{contract_of, external, owned, put, seed};
    use std::os::unix::fs::PermissionsExt;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    /// What a render leaves behind: every owned file with its content and mode.
    fn rendered(root: &Path, contract: &Contract) {
        for f in &contract.files {
            if let Some(content) = &f.content {
                put(root, &f.path, content);
                let mode = if f.mode == Mode::Executable {
                    0o755
                } else {
                    0o644
                };
                std::fs::set_permissions(root.join(&f.path), std::fs::Permissions::from_mode(mode))
                    .unwrap();
            }
        }
    }

    #[test]
    fn no_drift_after_a_render() {
        let dir = tmp();
        let c = contract_of(vec![
            owned("a.txt", "one\n", Mode::Regular),
            seed("s.md", "# s\n"),
            external("m.lock"),
        ]);
        rendered(dir.path(), &c);
        assert_eq!(
            check(&c, dir.path()).unwrap(),
            Checked {
                drift: vec![],
                checked: 1
            }
        );
    }

    #[test]
    fn a_hand_edit_is_drift_with_a_diff() {
        let dir = tmp();
        let c = contract_of(vec![owned("a.txt", "one\n", Mode::Regular)]);
        rendered(dir.path(), &c);
        put(dir.path(), "a.txt", "edited\n");
        assert_eq!(
            check(&c, dir.path()).unwrap().drift,
            [Drift {
                path: "a.txt".into(),
                reason: Reason::Different,
                diff: "    line 1 on disk:  edited\n    line 1 rendered: one".into()
            }]
        );
    }

    #[test]
    fn a_stale_file_is_drift() {
        let dir = tmp();
        rendered(
            dir.path(),
            &contract_of(vec![owned("a.txt", "one\n", Mode::Regular)]),
        );
        let newer = contract_of(vec![owned("a.txt", "two\n", Mode::Regular)]);
        assert_eq!(check(&newer, dir.path()).unwrap().drift.len(), 1);
    }

    #[test]
    fn a_missing_file_is_drift() {
        let c = contract_of(vec![owned("a.txt", "one\n", Mode::Regular)]);
        assert_eq!(
            check(&c, tmp().path()).unwrap().drift,
            [Drift {
                path: "a.txt".into(),
                reason: Reason::Missing,
                diff: String::new()
            }]
        );
    }

    #[test]
    fn seed_and_external_files_are_skipped() {
        let c = contract_of(vec![seed("s.md", "x\n"), external("m.lock")]);
        assert_eq!(
            check(&c, tmp().path()).unwrap(),
            Checked {
                drift: vec![],
                checked: 0
            }
        );
    }

    #[test]
    fn check_writes_nothing_not_even_the_tool_folder() {
        let dir = tmp();
        check(
            &contract_of(vec![owned("a.txt", "one\n", Mode::Regular)]),
            dir.path(),
        )
        .unwrap();
        assert!(!dir.path().join(".pkl-gen").exists());
        assert!(!dir.path().join("a.txt").exists());
    }

    #[test]
    fn check_uses_the_text_with_version_comments_so_a_rendered_repo_is_clean() {
        let dir = tmp();
        let sha = "3d3c42e5aac5ba805825da76410c181273ba90b1";
        let mut c = contract_of(vec![owned(
            ".github/workflows/x.yml",
            &format!("steps:\n  - uses: actions/checkout@{sha}\n"),
            Mode::Regular,
        )]);
        c.pins = vec![Pin {
            action: "actions/checkout".into(),
            sha: sha.into(),
            version: "v7.0.1".into(),
        }];
        put(
            dir.path(),
            ".github/workflows/x.yml",
            &format!("steps:\n  - uses: actions/checkout@{sha} # v7.0.1\n"),
        );
        assert_eq!(check(&c, dir.path()).unwrap().drift, []);
        put(
            dir.path(),
            ".github/workflows/x.yml",
            &format!("steps:\n  - uses: actions/checkout@{sha}\n"),
        );
        assert_eq!(check(&c, dir.path()).unwrap().drift.len(), 1);
    }

    #[test]
    fn a_missing_exec_bit_is_drift_and_so_is_an_exec_bit_the_contract_does_not_ask_for() {
        let dir = tmp();
        rendered(
            dir.path(),
            &contract_of(vec![owned("run.sh", "#!/bin/sh\n", Mode::Regular)]),
        );
        let wants_exec = contract_of(vec![owned("run.sh", "#!/bin/sh\n", Mode::Executable)]);
        assert_eq!(
            check(&wants_exec, dir.path()).unwrap().drift,
            [Drift {
                path: "run.sh".into(),
                reason: Reason::Mode,
                diff: "    expected executable, found not executable".into()
            }]
        );
        rendered(dir.path(), &wants_exec);
        assert_eq!(check(&wants_exec, dir.path()).unwrap().drift, []);
        let plain = contract_of(vec![owned("run.sh", "#!/bin/sh\n", Mode::Regular)]);
        assert_eq!(
            check(&plain, dir.path()).unwrap().drift,
            [Drift {
                path: "run.sh".into(),
                reason: Reason::Mode,
                diff: "    expected not executable, found executable".into()
            }]
        );
    }

    #[test]
    fn any_exec_bit_counts_as_executable() {
        let dir = tmp();
        put(dir.path(), "run.sh", "x\n");
        std::fs::set_permissions(
            dir.path().join("run.sh"),
            std::fs::Permissions::from_mode(0o610),
        )
        .unwrap();
        let wants_exec = contract_of(vec![owned("run.sh", "x\n", Mode::Executable)]);
        assert_eq!(check(&wants_exec, dir.path()).unwrap().drift, []);
    }

    #[test]
    fn a_path_that_escapes_the_repository_is_an_error() {
        let c = contract_of(vec![owned("../x", "x\n", Mode::Regular)]);
        assert_eq!(
            check(&c, tmp().path()).unwrap_err().to_string(),
            "../x: escapes the repository"
        );
    }

    #[test]
    fn a_workflow_with_an_unknown_pin_is_an_error_naming_the_file() {
        let sha = "3d3c42e5aac5ba805825da76410c181273ba90b1";
        let c = contract_of(vec![owned(
            ".github/workflows/x.yml",
            &format!("steps:\n  - uses: a/b@{sha}\n"),
            Mode::Regular,
        )]);
        let err = check(&c, tmp().path()).unwrap_err().to_string();
        assert!(
            err.starts_with(".github/workflows/x.yml: line 2: no version known"),
            "{err}"
        );
    }
}
