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
