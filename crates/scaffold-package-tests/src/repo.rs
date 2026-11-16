//! Temporary repositories and git for the tests.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// An empty temporary directory, removed when it is dropped.
pub fn temp_dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("scaffold-test-")
        .tempdir()
        .expect("a temporary directory")
}

/// Runs git in a directory with the `GIT_*` variables removed, so a hook or a worktree environment cannot redirect it. Returns stdout.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args);
    for (key, _) in std::env::vars() {
        if key.starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let output = command.output().expect("git runs");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A temporary directory that is a git repository on `main`.
pub fn git_init() -> tempfile::TempDir {
    let dir = temp_dir();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    dir
}

pub fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
}

/// The file modes git has in its index, by path.
pub fn modes(dir: &Path) -> BTreeMap<String, String> {
    git(dir, &["ls-files", "-s"])
        .lines()
        .map(|line| {
            let (meta, path) = line.split_once('\t').expect("a tab before the path");
            let mode = meta.split(' ').next().expect("a mode");
            (path.to_owned(), mode.to_owned())
        })
        .collect()
}

/// A fresh clone of a repository, in a temporary directory that is kept as long as the first value of the pair.
pub fn clone_of(source: &Path) -> (tempfile::TempDir, PathBuf) {
    let holder = temp_dir();
    let target = holder.path().join("clone");
    git(
        source,
        &[
            "clone",
            "-q",
            &source.to_string_lossy(),
            &target.to_string_lossy(),
        ],
    );
    (holder, target)
}

/// The files git tracks in the repository, relative to its root, leaving out `archive/`.
pub fn tracked_files() -> Vec<String> {
    let root = crate::pkg::root();
    git(&root, &["ls-files", "-z"])
        .split('\0')
        .filter(|f| !f.is_empty() && !f.starts_with("archive/"))
        .map(str::to_owned)
        .collect()
}

/// Writes a file under a directory, making the folders it needs.
pub fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("a parent folder")).unwrap();
    std::fs::write(path, text).unwrap();
}

pub fn get(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

pub fn has(root: &Path, rel: &str) -> bool {
    root.join(rel).exists()
}

/// A temporary directory that is a git repository with one commit of every file given.
pub fn git_repo(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = git_init();
    for (rel, text) in files {
        put(dir.path(), rel, text);
    }
    commit_all(dir.path(), "init");
    dir
}

/// Writes an executable script and waits until it can be run. Running a file fails with ETXTBSY ("text file busy") while any process
/// still holds it open for writing, and a test thread that forks at the wrong moment holds a copy of the descriptor until its own exec. So
/// the script gets a guard line that makes a probe run exit at once, and the probe is repeated until it is no longer refused.
pub fn write_script(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    let (shebang, rest) = body
        .split_once('\n')
        .expect("a script starts with a shebang line");
    std::fs::write(
        path,
        format!("{shebang}\n[ -z \"${{SCRIPT_PROBE:-}}\" ] || exit 0\n{rest}"),
    )
    .unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    for _ in 0..500 {
        let probe = Command::new(path)
            .env("SCRIPT_PROBE", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match probe {
            Err(e) if e.raw_os_error() == Some(26) => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            other => {
                other.unwrap();
                return;
            }
        }
    }
    panic!("{} stayed busy for five seconds", path.display());
}
