//! Helpers for the tests of the modules that touch the file system and git.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Runs git in `root` with every GIT_* variable removed, so a hook or a linked worktree cannot redirect it.
pub fn git(root: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).args(args);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let output = command.output().expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A temporary git repository with one commit holding `files`. The folder is deleted when the value is dropped.
pub struct Repo {
    _dir: tempfile::TempDir,
    pub root: PathBuf,
}

pub fn git_repo(files: &[(&str, &str)]) -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "user.name", "test"]);
    git(&root, &["config", "user.email", "test@example.invalid"]);
    for (path, text) in files {
        put(&root, path, text);
    }
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "initial"]);
    Repo { _dir: dir, root }
}

use crate::contract::{Contract, ContractFile, EscapeHatch, FileClass, Mode};

pub fn owned(path: &str, content: &str, mode: Mode) -> ContractFile {
    ContractFile {
        path: path.into(),
        class: FileClass::Owned,
        mode,
        content: Some(content.into()),
        template: None,
        context: None,
        remove_on_detach: None,
    }
}

pub fn seed(path: &str, content: &str) -> ContractFile {
    ContractFile {
        path: path.into(),
        class: FileClass::Seed,
        mode: Mode::Regular,
        content: Some(content.into()),
        template: None,
        context: None,
        remove_on_detach: None,
    }
}

pub fn external(path: &str) -> ContractFile {
    ContractFile {
        path: path.into(),
        class: FileClass::External,
        mode: Mode::Regular,
        content: None,
        template: None,
        context: None,
        remove_on_detach: None,
    }
}

pub fn contract_of(files: Vec<ContractFile>) -> Contract {
    Contract {
        schema_version: 2,
        files,
        pins: Vec::new(),
        escape_hatch: EscapeHatch {
            uses: 0,
            constructs: 0,
        },
        package_uri: None,
        requires: None,
        security_builtins: None,
        presets: None,
    }
}

pub fn get(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap()
}

pub fn has(root: &Path, rel: &str) -> bool {
    root.join(rel).exists()
}

/// The permission bits as an octal string such as `0755`.
pub fn mode_of(root: &Path, rel: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    format!(
        "{:04o}",
        std::fs::metadata(root.join(rel))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    )
}

pub fn set_mode(root: &Path, rel: &str, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(root.join(rel), std::fs::Permissions::from_mode(mode)).unwrap();
}

pub fn commit_all(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", message]);
}
