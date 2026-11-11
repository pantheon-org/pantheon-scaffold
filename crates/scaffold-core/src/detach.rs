//! `detach`: removes the manifest and the rendered files the package marked `removeOnDetach` (the consumer upgrade
//! workflow, which would fail on every run once detached), and leaves every other file as a plain file. It needs no
//! pantheon.pkl and no network. Deleting the flagged files goes through the same delete guard as a render. Ported from
//! `runner/src/detach.ts`.

use std::path::Path;

use crate::apply::ApplyError;
use crate::guard::delete_guard;
use crate::lock::{LockOptions, acquire_lock};
use crate::manifest::{MANIFEST_PATH, ManifestError, TOOL_DIR, read_manifest, sha256};
use crate::paths::resolve_inside;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetachKind {
    Delete,
    Forget,
    Manifest,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DetachAction {
    pub kind: DetachKind,
    pub path: String,
}

impl DetachAction {
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            DetachKind::Delete => "delete",
            DetachKind::Forget => "forget",
            DetachKind::Manifest => "manifest",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct DetachReport {
    pub actions: Vec<DetachAction>,
    pub refusals: Vec<String>,
    pub dry_run: bool,
}

#[derive(Default)]
pub struct DetachOptions {
    pub dry_run: bool,
    pub force: bool,
    pub lock: LockOptions,
}

pub fn detach(root: &Path, opts: &DetachOptions) -> Result<DetachReport, ApplyError> {
    if !root.join(MANIFEST_PATH).exists() {
        return Err(ManifestError(format!(
            "{MANIFEST_PATH} not found, there is nothing to detach"
        ))
        .into());
    }
    let lock = if opts.dry_run {
        None
    } else {
        Some(acquire_lock(root, &opts.lock)?)
    };
    let manifest = read_manifest(root)?;
    let mut actions = Vec::new();
    let mut refusals = Vec::new();
    let mut deletes: Vec<String> = Vec::new();
    // JavaScript sorts keys by UTF-16 code unit, so this does too.
    let mut entries: Vec<_> = manifest
        .files
        .iter()
        .filter(|(_, e)| e.remove_on_detach)
        .collect();
    entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    for (path, entry) in entries {
        let abs = resolve_inside(root, path)?;
        let action = |kind| DetachAction {
            kind,
            path: path.clone(),
        };
        if !abs.exists() {
            actions.push(action(DetachKind::Forget));
            continue;
        }
        let bytes = std::fs::read(&abs).map_err(|e| ApplyError::Io(format!("{path}: {e}")))?;
        let edited = entry
            .hash
            .as_deref()
            .is_some_and(|hash| sha256(&String::from_utf8_lossy(&bytes)) != hash);
        if edited && !opts.force {
            refusals.push(format!("{path}: edited by hand since the last render, so detach will not delete it (use --force)"));
        } else {
            actions.push(action(DetachKind::Delete));
            deletes.push(path.clone());
        }
    }
    if !deletes.is_empty() {
        let paths: Vec<&str> = deletes.iter().map(String::as_str).collect();
        refusals.extend(delete_guard(root, &paths));
    }
    actions.push(DetachAction {
        kind: DetachKind::Manifest,
        path: MANIFEST_PATH.to_owned(),
    });
    if !refusals.is_empty() || opts.dry_run {
        return Ok(DetachReport {
            actions,
            refusals,
            dry_run: opts.dry_run,
        });
    }

    for path in &deletes {
        std::fs::remove_file(resolve_inside(root, path)?)
            .map_err(|e| ApplyError::Io(format!("{path}: {e}")))?;
    }
    let tool = root.join(TOOL_DIR);
    for name in ["manifest.json", "stage", "cache"] {
        let target = tool.join(name);
        let removed = if target.is_dir() {
            std::fs::remove_dir_all(&target)
        } else {
            std::fs::remove_file(&target)
        };
        if let Err(e) = removed
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(ApplyError::Io(format!("{TOOL_DIR}/{name}: {e}")));
        }
    }
    drop(lock);
    // The folder holds something the runner did not make: leave it.
    let _ = std::fs::remove_dir(&tool);
    Ok(DetachReport {
        actions,
        refusals,
        dry_run: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply::{Options, apply};
    use crate::contract::{Contract, ContractFile, Mode};
    use crate::testutil::{
        Repo, commit_all, contract_of, external, get, git_repo, has, owned, put, seed,
    };

    fn upgrade_workflow() -> ContractFile {
        ContractFile {
            remove_on_detach: Some(true),
            ..owned(
                ".github/workflows/upgrade.yml",
                "name: upgrade\n",
                Mode::Regular,
            )
        }
    }

    fn with_upgrade() -> Contract {
        contract_of(vec![
            upgrade_workflow(),
            owned("deny.toml", "# deny\n", Mode::Regular),
            seed("pantheon.pkl", "# mine\n"),
            external("mise.lock"),
        ])
    }

    fn render(c: &Contract, root: &Path) {
        apply(c, root, &Options::default()).unwrap();
    }

    fn managed() -> Repo {
        let repo = git_repo(&[(
            ".gitignore",
            ".pkl-gen/lock\n.pkl-gen/stage/\n.pkl-gen/cache/\n",
        )]);
        render(&with_upgrade(), &repo.root);
        put(&repo.root, "mise.lock", "lock\n");
        commit_all(&repo.root, "render");
        repo
    }

    fn go(root: &Path) -> DetachReport {
        detach(root, &DetachOptions::default()).unwrap()
    }

    fn kinds(r: &DetachReport) -> Vec<String> {
        r.actions
            .iter()
            .map(|a| format!("{}:{}", a.kind_name(), a.path))
            .collect()
    }

    #[test]
    fn detach_removes_the_manifest_and_the_flagged_upgrade_workflow_and_leaves_every_other_file_untouched()
     {
        let repo = managed();
        let before = (
            get(&repo.root, "deny.toml"),
            get(&repo.root, "pantheon.pkl"),
            get(&repo.root, "mise.lock"),
        );
        let r = go(&repo.root);
        assert_eq!(
            kinds(&r),
            [
                "delete:.github/workflows/upgrade.yml",
                "manifest:.pkl-gen/manifest.json"
            ]
        );
        assert!(r.refusals.is_empty() && !r.dry_run);
        assert!(!has(&repo.root, ".github/workflows/upgrade.yml"));
        assert!(!has(&repo.root, ".pkl-gen"));
        assert_eq!(
            (
                get(&repo.root, "deny.toml"),
                get(&repo.root, "pantheon.pkl"),
                get(&repo.root, "mise.lock")
            ),
            before
        );
    }

    #[test]
    fn the_cache_stage_and_lock_folders_go_with_the_manifest() {
        let repo = managed();
        put(&repo.root, ".pkl-gen/cache/known-bad.json", "{}");
        put(&repo.root, ".pkl-gen/stage/x", "x");
        go(&repo.root);
        assert!(!has(&repo.root, ".pkl-gen"));
    }

    #[test]
    fn a_folder_that_holds_something_the_runner_did_not_make_is_left() {
        let repo = managed();
        put(&repo.root, ".pkl-gen/notes.txt", "mine");
        go(&repo.root);
        assert!(has(&repo.root, ".pkl-gen/notes.txt"));
        assert!(!has(&repo.root, ".pkl-gen/manifest.json"));
    }

    #[test]
    fn dry_run_lists_the_plan_and_changes_nothing() {
        let repo = managed();
        let r = detach(
            &repo.root,
            &DetachOptions {
                dry_run: true,
                ..DetachOptions::default()
            },
        )
        .unwrap();
        assert!(kinds(&r).contains(&"delete:.github/workflows/upgrade.yml".to_owned()));
        assert!(r.dry_run);
        assert!(has(&repo.root, ".github/workflows/upgrade.yml"));
        assert!(has(&repo.root, ".pkl-gen/manifest.json"));
    }

    #[test]
    fn a_hand_edited_upgrade_workflow_is_refused_and_nothing_is_removed_and_force_removes_it() {
        let repo = managed();
        put(&repo.root, ".github/workflows/upgrade.yml", "name: mine\n");
        commit_all(&repo.root, "edit");
        let refused = go(&repo.root);
        assert_eq!(
            refused.refusals,
            [
                ".github/workflows/upgrade.yml: edited by hand since the last render, so detach will not delete it (use --force)"
            ]
        );
        assert!(has(&repo.root, ".pkl-gen/manifest.json"));
        assert!(has(&repo.root, ".github/workflows/upgrade.yml"));
        let forced = detach(
            &repo.root,
            &DetachOptions {
                force: true,
                ..DetachOptions::default()
            },
        )
        .unwrap();
        assert!(kinds(&forced).contains(&"delete:.github/workflows/upgrade.yml".to_owned()));
        assert!(!has(&repo.root, ".pkl-gen"));
    }

    #[test]
    fn a_dirty_tracked_tree_refuses_the_delete_guard_applies_to_detach_too() {
        let repo = managed();
        put(&repo.root, "deny.toml", "# changed\n");
        let r = go(&repo.root);
        assert!(r.refusals.join("\n").contains("not clean"));
        assert!(has(&repo.root, ".pkl-gen/manifest.json"));
    }

    #[test]
    fn a_workflow_that_is_already_gone_is_forgotten_and_the_manifest_still_goes() {
        let repo = managed();
        std::fs::remove_file(repo.root.join(".github/workflows/upgrade.yml")).unwrap();
        commit_all(&repo.root, "removed by hand");
        assert_eq!(
            kinds(&go(&repo.root)),
            [
                "forget:.github/workflows/upgrade.yml",
                "manifest:.pkl-gen/manifest.json"
            ]
        );
        assert!(!has(&repo.root, ".pkl-gen"));
    }

    #[test]
    fn with_no_flagged_file_git_is_not_needed() {
        let dir = tempfile::tempdir().unwrap();
        render(
            &contract_of(vec![owned("a.txt", "x\n", Mode::Regular)]),
            dir.path(),
        );
        assert_eq!(kinds(&go(dir.path())), ["manifest:.pkl-gen/manifest.json"]);
        assert_eq!(get(dir.path(), "a.txt"), "x\n");
    }

    #[test]
    fn flagged_files_are_visited_in_sorted_order() {
        let dir = tempfile::tempdir().unwrap();
        let flag = |p: &str| ContractFile {
            remove_on_detach: Some(true),
            ..owned(p, "x\n", Mode::Regular)
        };
        render(
            &contract_of(vec![flag("b.yml"), flag("a.yml"), flag("c.yml")]),
            dir.path(),
        );
        for name in ["a.yml", "b.yml", "c.yml"] {
            std::fs::remove_file(dir.path().join(name)).unwrap();
        }
        assert_eq!(
            kinds(&go(dir.path())),
            [
                "forget:a.yml",
                "forget:b.yml",
                "forget:c.yml",
                "manifest:.pkl-gen/manifest.json"
            ]
        );
    }

    #[test]
    fn a_repository_with_no_manifest_has_nothing_to_detach() {
        let dir = tempfile::tempdir().unwrap();
        let message = detach(dir.path(), &DetachOptions::default())
            .unwrap_err()
            .to_string();
        assert_eq!(
            message,
            ".pkl-gen/manifest.json not found, there is nothing to detach"
        );
    }

    #[test]
    fn detach_refuses_while_another_run_holds_the_lock() {
        let repo = managed();
        let started = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        put(
            &repo.root,
            ".pkl-gen/lock",
            &format!(
                "{{\"pid\":{},\"started\":\"{started}\"}}",
                std::process::id()
            ),
        );
        let opts = DetachOptions {
            lock: LockOptions {
                ci: false,
                ..LockOptions::default()
            },
            ..DetachOptions::default()
        };
        assert!(
            detach(&repo.root, &opts)
                .unwrap_err()
                .to_string()
                .contains("is held by process")
        );
        assert!(has(&repo.root, ".github/workflows/upgrade.yml"));
    }

    #[test]
    fn dry_run_does_not_take_the_lock() {
        let repo = managed();
        let started = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        put(
            &repo.root,
            ".pkl-gen/lock",
            &format!(
                "{{\"pid\":{},\"started\":\"{started}\"}}",
                std::process::id()
            ),
        );
        let opts = DetachOptions {
            dry_run: true,
            lock: LockOptions {
                ci: false,
                ..LockOptions::default()
            },
            ..DetachOptions::default()
        };
        assert!(detach(&repo.root, &opts).is_ok());
    }

    #[test]
    fn after_a_detach_the_files_can_be_adopted_again_by_a_fresh_render() {
        let repo = managed();
        go(&repo.root);
        let r = apply(&with_upgrade(), &repo.root, &Options::default()).unwrap();
        assert!(r.refusals.is_empty());
        assert!(
            r.actions.iter().any(
                |a| a.kind == crate::apply::Kind::Adopt || a.kind == crate::apply::Kind::Create
            )
        );
    }

    #[test]
    fn a_manifest_path_that_escapes_the_repository_is_an_error() {
        let repo = managed();
        let mut m = crate::manifest::read_manifest(&repo.root).unwrap();
        m.files.insert(
            "../evil.txt".into(),
            crate::manifest::ManifestEntry {
                class: crate::contract::FileClass::Owned,
                hash: Some("x".into()),
                remove_on_detach: true,
            },
        );
        put(
            &repo.root,
            ".pkl-gen/manifest.json",
            &crate::manifest::serialise_manifest(&m),
        );
        assert!(
            detach(&repo.root, &DetachOptions::default())
                .unwrap_err()
                .to_string()
                .contains("escapes")
        );
    }
}
