//! The delete guard. It is evaluated once, before the runner writes anything, and it only ever answers "refuse, and
//! why". git is a required tool of the runner, so a root that is not a git work tree cannot delete. Ported from
//! `runner/src/guard.ts`.

use std::path::Path;

/// Runs git in `root` with every GIT_* variable removed, so a hook or a linked worktree cannot redirect it. `None` when
/// git fails or cannot be started.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(root).args(args);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Returns the reasons deletion is refused, empty when it is allowed. The tracked tree must be clean, a sparse or
/// partial checkout counts as not clean, and a file git does not track is never deleted. A shallow checkout is fine.
pub fn delete_guard(root: &Path, paths: &[&str]) -> Vec<String> {
    if git(root, &["rev-parse", "--is-inside-work-tree"]).is_none_or(|out| out.trim() != "true") {
        return vec!["not a git repository: orphan files cannot be deleted without git to confirm the tree is clean".to_owned()];
    }
    let mut reasons = Vec::new();
    let sparse = git(root, &["config", "--get", "core.sparseCheckout"])
        .is_some_and(|out| out.trim() == "true");
    let partial = git(root, &["config", "--get", "extensions.partialClone"])
        .is_some_and(|out| !out.trim().is_empty());
    if sparse || partial {
        reasons.push(
            "a sparse or partial checkout is treated as not clean, so nothing is deleted"
                .to_owned(),
        );
    }
    match git(root, &["status", "--porcelain", "--untracked-files=no"]) {
        None => reasons.push("git status failed, so the tree cannot be confirmed clean".to_owned()),
        Some(out) if !out.trim().is_empty() => {
            let lines: Vec<String> = out
                .trim_end()
                .split('\n')
                .map(|line| format!("    {line}"))
                .collect();
            reasons.push(format!(
                "the tracked tree is not clean, commit or stash first:\n{}",
                lines.join("\n")
            ));
        }
        Some(_) => {}
    }
    for path in paths {
        if git(root, &["ls-files", "--error-unmatch", "--", path]).is_none() {
            reasons.push(format!(
                "{path}: not tracked by git, so it is never deleted"
            ));
        }
    }
    reasons
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{git, git_repo, put};

    fn joined(root: &Path, paths: &[&str]) -> String {
        delete_guard(root, paths).join("\n")
    }

    #[test]
    fn a_clean_tracked_tree_with_a_tracked_file_passes() {
        let repo = git_repo(&[("a.txt", "a\n")]);
        assert!(delete_guard(&repo.root, &["a.txt"]).is_empty());
    }

    #[test]
    fn a_modified_tracked_file_makes_the_tree_not_clean_and_is_listed_indented() {
        let repo = git_repo(&[("a.txt", "a\n")]);
        put(&repo.root, "a.txt", "changed\n");
        assert_eq!(
            joined(&repo.root, &["a.txt"]),
            "the tracked tree is not clean, commit or stash first:\n     M a.txt"
        );
    }

    #[test]
    fn a_staged_change_makes_the_tree_not_clean() {
        let repo = git_repo(&[("a.txt", "a\n"), ("b.txt", "b\n")]);
        put(&repo.root, "b.txt", "changed\n");
        git(&repo.root, &["add", "b.txt"]);
        assert!(joined(&repo.root, &["a.txt"]).contains("not clean"));
    }

    #[test]
    fn an_untracked_file_is_never_deleted_and_does_not_block_a_tracked_one() {
        let repo = git_repo(&[("a.txt", "a\n")]);
        put(&repo.root, "new.txt", "u\n");
        assert_eq!(
            joined(&repo.root, &["new.txt"]),
            "new.txt: not tracked by git, so it is never deleted"
        );
        assert!(delete_guard(&repo.root, &["a.txt"]).is_empty());
    }

    #[test]
    fn a_directory_that_is_not_a_git_repository_refuses() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "a.txt", "a\n");
        assert_eq!(
            joined(dir.path(), &["a.txt"]),
            "not a git repository: orphan files cannot be deleted without git to confirm the tree is clean"
        );
    }

    #[test]
    fn a_sparse_checkout_counts_as_not_clean() {
        let repo = git_repo(&[("a.txt", "a\n")]);
        git(&repo.root, &["config", "core.sparseCheckout", "true"]);
        assert!(
            joined(&repo.root, &["a.txt"])
                .contains("a sparse or partial checkout is treated as not clean")
        );
    }

    #[test]
    fn a_partial_clone_counts_as_not_clean() {
        let repo = git_repo(&[("a.txt", "a\n")]);
        git(&repo.root, &["config", "extensions.partialClone", "origin"]);
        assert!(joined(&repo.root, &["a.txt"]).contains("sparse or partial"));
    }

    #[test]
    fn a_shallow_checkout_is_not_refused() {
        let source = git_repo(&[("a.txt", "a\n")]);
        put(&source.root, "a.txt", "two\n");
        git(&source.root, &["commit", "-qam", "second"]);
        let parent = tempfile::tempdir().unwrap();
        let clone = parent.path().join("clone");
        git(
            &source.root,
            &[
                "clone",
                "-q",
                "--depth",
                "1",
                &format!("file://{}", source.root.display()),
                clone.to_str().unwrap(),
            ],
        );
        assert_eq!(
            git(&clone, &["rev-parse", "--is-shallow-repository"]).trim(),
            "true"
        );
        assert!(delete_guard(&clone, &["a.txt"]).is_empty());
    }

    #[test]
    fn a_tracked_file_in_a_subfolder_is_found() {
        let repo = git_repo(&[("dir/a.txt", "a\n")]);
        assert!(delete_guard(&repo.root, &["dir/a.txt"]).is_empty());
    }

    #[test]
    fn a_missing_file_is_not_tracked() {
        let repo = git_repo(&[("a.txt", "a\n")]);
        assert_eq!(
            joined(&repo.root, &["gone.txt"]),
            "gone.txt: not tracked by git, so it is never deleted"
        );
    }

    #[test]
    fn a_git_dir_in_the_environment_cannot_redirect_the_guard() {
        // Run the guard in a child process with GIT_DIR pointing at another repository, because changing this
        // process's environment would affect the other tests.
        let other = git_repo(&[("x.txt", "x\n")]);
        let repo = git_repo(&[("a.txt", "a\n")]);
        let exe = std::env::current_exe().unwrap();
        let output = std::process::Command::new(exe)
            .args([
                "guard::tests::child_for_the_git_dir_test",
                "--exact",
                "--nocapture",
                "--include-ignored",
            ])
            .env("SCAFFOLD_GUARD_CHILD_ROOT", &repo.root)
            .env("GIT_DIR", other.root.join(".git"))
            .env("GIT_WORK_TREE", &other.root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    #[ignore = "run only by the test above, in a child process"]
    fn child_for_the_git_dir_test() {
        let root = std::env::var("SCAFFOLD_GUARD_CHILD_ROOT").unwrap();
        assert!(delete_guard(Path::new(&root), &["a.txt"]).is_empty());
    }
}
