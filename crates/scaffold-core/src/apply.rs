//! Writes the render into a repository: plans every action first, refuses before writing anything when a file was edited
//! by hand, then writes each file atomically and the manifest last. Ported from `runner/src/apply.ts`.

use std::path::Path;

use crate::contract::{Contract, ContractError, ContractFile, FileClass, Mode};
use crate::guard::delete_guard;
use crate::lock::{LockError, LockOptions, acquire_lock};
use crate::manifest::{
    MANIFEST_PATH, Manifest, ManifestEntry, ManifestError, TOOL_DIR, read_manifest,
    serialise_manifest, sha256,
};
use crate::paths::{PathError, find_case_collisions, resolve_inside};
use crate::render::{final_content, short_diff};

#[derive(Default)]
pub struct Options {
    pub force: bool,
    pub dry_run: bool,
    pub repair: bool,
    pub lock: LockOptions,
    pub package_version: Option<String>,
    /// The Preset files this render applies, by path, with their hashes (see `presets`). Recorded in the manifest, which keeps
    /// only the last render's: a render that applies none clears them.
    pub presets: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Create,
    Update,
    Unchanged,
    Adopt,
    Seed,
    SeedKept,
    External,
    Chmod,
    Delete,
    Forget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub kind: Kind,
    pub path: String,
}

impl Kind {
    /// The name the TypeScript runner prints for the action.
    pub fn name(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Unchanged => "unchanged",
            Self::Adopt => "adopt",
            Self::Seed => "seed",
            Self::SeedKept => "seed-kept",
            Self::External => "external",
            Self::Chmod => "chmod",
            Self::Delete => "delete",
            Self::Forget => "forget",
        }
    }
}

impl std::fmt::Display for Action {
    /// `kind:path`, with the kind spelled as the TypeScript runner spells it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.kind.name(), self.path)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub actions: Vec<Action>,
    pub refusals: Vec<String>,
    pub written: usize,
    pub deleted: usize,
    pub dry_run: bool,
}

/// Anything that stops a run before or while it writes. Refusals are not errors: they come back in the report.
#[derive(Debug, PartialEq, Eq)]
pub enum ApplyError {
    Path(PathError),
    Contract(ContractError),
    Lock(LockError),
    Manifest(ManifestError),
    Io(String),
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(e) => e.fmt(f),
            Self::Contract(e) => e.fmt(f),
            Self::Lock(e) => e.fmt(f),
            Self::Manifest(e) => e.fmt(f),
            Self::Io(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for ApplyError {}

impl From<PathError> for ApplyError {
    fn from(e: PathError) -> Self {
        Self::Path(e)
    }
}
impl From<ContractError> for ApplyError {
    fn from(e: ContractError) -> Self {
        Self::Contract(e)
    }
}
impl From<LockError> for ApplyError {
    fn from(e: LockError) -> Self {
        Self::Lock(e)
    }
}
impl From<ManifestError> for ApplyError {
    fn from(e: ManifestError) -> Self {
        Self::Manifest(e)
    }
}

/// Everything a run decided, before anything is written.
pub struct Plan {
    pub actions: Vec<Action>,
    pub refusals: Vec<String>,
    pub next: Manifest,
    pub writes: Vec<ContractFile>,
    pub chmods: Vec<ContractFile>,
    pub deletes: Vec<String>,
}

fn io_error(what: &str, e: &std::io::Error) -> ApplyError {
    ApplyError::Io(format!("{what}: {e}"))
}

fn wants_exec(f: &ContractFile) -> bool {
    f.mode == Mode::Executable
}

fn has_exec(abs: &Path) -> Result<bool, ApplyError> {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(abs).map_err(|e| io_error(&abs.display().to_string(), &e))?;
    Ok(meta.permissions().mode() & 0o111 != 0)
}

/// The text of a file, or `None` when it does not exist.
fn read(abs: &Path) -> Result<Option<String>, ApplyError> {
    if !abs.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(abs).map_err(|e| io_error(&abs.display().to_string(), &e))?;
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

fn entry(class: FileClass, hash: Option<String>, remove_on_detach: bool) -> ManifestEntry {
    ManifestEntry {
        class,
        hash,
        remove_on_detach,
    }
}

/// The content is already right: only the executable bit may still differ (set and cleared as the contract says).
fn settle(plan: &mut Plan, f: &ContractFile, abs: &Path, adopt: bool) -> Result<(), ApplyError> {
    let drift = has_exec(abs)? != wants_exec(f);
    if drift {
        plan.chmods.push(f.clone());
    }
    let action = |kind| Action {
        kind,
        path: f.path.clone(),
    };
    if adopt {
        plan.actions.push(action(Kind::Adopt));
    }
    if drift {
        plan.actions.push(action(Kind::Chmod));
    } else if !adopt {
        plan.actions.push(action(Kind::Unchanged));
    }
    Ok(())
}

/// Plans every action without touching the disk. Refusals are collected so the run can stop before writing anything.
pub fn plan(
    contract: &Contract,
    root: &Path,
    manifest: &Manifest,
    opts: &Options,
) -> Result<Plan, ApplyError> {
    let mut next = Manifest::empty();
    next.package_version = opts
        .package_version
        .clone()
        .or_else(|| manifest.package_version.clone());
    next.presets = opts.presets.clone();
    let mut plan = Plan {
        actions: Vec::new(),
        refusals: Vec::new(),
        next,
        writes: Vec::new(),
        chmods: Vec::new(),
        deletes: Vec::new(),
    };

    let paths: Vec<&str> = contract.files.iter().map(|f| f.path.as_str()).collect();
    let collisions = find_case_collisions(&paths);
    if !collisions.is_empty() {
        return Err(PathError(collisions.join("; ")).into());
    }

    for f in &contract.files {
        let abs = resolve_inside(root, &f.path)?;
        let prev = manifest.files.get(&f.path);
        let current = read(&abs)?;
        let action = |kind| Action {
            kind,
            path: f.path.clone(),
        };

        if f.class == FileClass::External {
            plan.next
                .files
                .insert(f.path.clone(), entry(FileClass::External, None, false));
            plan.actions.push(action(Kind::External));
            continue;
        }
        let content = final_content(f, &contract.pins)?;
        let hash = sha256(&content);
        let to_write = || ContractFile {
            content: Some(content.clone()),
            ..f.clone()
        };

        if f.class == FileClass::Seed {
            match current {
                None => {
                    plan.writes.push(to_write());
                    plan.actions.push(action(Kind::Seed));
                    plan.next
                        .files
                        .insert(f.path.clone(), entry(FileClass::Seed, Some(hash), false));
                }
                Some(text) => {
                    plan.actions.push(action(Kind::SeedKept));
                    let kept = prev
                        .filter(|p| p.class == FileClass::Seed)
                        .and_then(|p| p.hash.clone())
                        .unwrap_or_else(|| sha256(&text));
                    plan.next
                        .files
                        .insert(f.path.clone(), entry(FileClass::Seed, Some(kept), false));
                }
            }
            continue;
        }

        // owned
        plan.next.files.insert(
            f.path.clone(),
            entry(
                FileClass::Owned,
                Some(hash),
                f.remove_on_detach == Some(true),
            ),
        );
        let prev_seed_hash = prev
            .filter(|p| p.class == FileClass::Seed)
            .and_then(|p| p.hash.as_deref());
        let prev_owned_hash = prev
            .filter(|p| p.class == FileClass::Owned)
            .and_then(|p| p.hash.as_deref());
        if let (Some(text), Some(seeded)) = (&current, prev_seed_hash)
            && sha256(text) != seeded
            && !opts.force
        {
            plan.refusals.push(format!(
                "{}: was seeded and has been edited, turning it into an owned file would overwrite your edits (use --force)",
                f.path
            ));
            continue;
        }
        let Some(text) = current else {
            plan.writes.push(to_write());
            plan.actions.push(action(Kind::Create));
            continue;
        };
        if let Some(owned_hash) = prev_owned_hash {
            if sha256(&text) != owned_hash && !opts.force {
                plan.refusals.push(format!(
                    "{}: was edited by hand since the last render (use --force to overwrite)\n{}",
                    f.path,
                    short_diff(&text, &content, 6)
                ));
            } else if text == content {
                settle(&mut plan, f, &abs, false)?;
            } else {
                plan.writes.push(to_write());
                plan.actions.push(action(Kind::Update));
            }
        } else if prev_seed_hash.is_some_and(|seeded| sha256(&text) == seeded) {
            // An untouched seed becomes owned: it is still exactly what was seeded, so overwriting loses nothing.
            if text == content {
                settle(&mut plan, f, &abs, false)?;
            } else {
                plan.writes.push(to_write());
                plan.actions.push(action(Kind::Update));
            }
        } else if text == content {
            settle(&mut plan, f, &abs, true)?;
        } else if opts.force {
            plan.writes.push(to_write());
            plan.actions.push(action(Kind::Update));
        } else {
            plan.refusals.push(format!(
                "{}: already exists and differs from the render, so the runner will not adopt it (use --force to overwrite)\n{}",
                f.path,
                short_diff(&text, &content, 6)
            ));
        }
    }

    plan_orphans(contract, root, manifest, opts, &mut plan)?;
    Ok(plan)
}

/// Orphans: owned files the contract no longer lists. Seed and external files are never deleted, only dropped from the
/// manifest.
fn plan_orphans(
    contract: &Contract,
    root: &Path,
    manifest: &Manifest,
    opts: &Options,
    plan: &mut Plan,
) -> Result<(), ApplyError> {
    for (path, e) in &manifest.files {
        if e.class != FileClass::Owned || contract.files.iter().any(|f| f.path == *path) {
            continue;
        }
        // On a case-insensitive filesystem the old and the new name are one file, and deleting the orphan would delete
        // the render.
        if let Some(renamed) = contract
            .files
            .iter()
            .find(|f| f.path.to_lowercase() == path.to_lowercase())
        {
            plan.refusals.push(format!(
                "{path} and {} differ only by case, which is one file on a case-insensitive filesystem. Rename it by hand and render again",
                renamed.path
            ));
            continue;
        }
        match read(&resolve_inside(root, path)?)? {
            None => plan.actions.push(Action { kind: Kind::Forget, path: path.clone() }),
            Some(text) if e.hash.as_deref().is_some_and(|h| sha256(&text) != h) && !opts.force => plan.refusals.push(format!(
                "{path}: no longer rendered, but it was edited by hand since the last render, so it is not deleted (use --force)"
            )),
            Some(_) => {
                plan.deletes.push(path.clone());
                plan.actions.push(Action { kind: Kind::Delete, path: path.clone() });
            }
        }
    }
    Ok(())
}

/// Atomic write: the file is written inside the tool folder, on the same filesystem, then renamed into place.
fn write_atomic(root: &Path, f: &ContractFile) -> Result<(), ApplyError> {
    use std::os::unix::fs::PermissionsExt;
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let abs = resolve_inside(root, &f.path)?;
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_error(&f.path, &e))?;
    }
    let stage = root.join(TOOL_DIR).join("stage");
    std::fs::create_dir_all(&stage).map_err(|e| io_error(".pkl-gen/stage", &e))?;
    let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = stage.join(format!("{}-{unique}", std::process::id()));
    std::fs::write(&tmp, f.content.as_deref().unwrap_or("")).map_err(|e| io_error(&f.path, &e))?;
    std::fs::set_permissions(
        &tmp,
        std::fs::Permissions::from_mode(if wants_exec(f) { 0o755 } else { 0o644 }),
    )
    .map_err(|e| io_error(&f.path, &e))?;
    std::fs::rename(&tmp, &abs).map_err(|e| io_error(&f.path, &e))
}

pub fn apply(contract: &Contract, root: &Path, opts: &Options) -> Result<Report, ApplyError> {
    if !root.is_dir() {
        return Err(PathError(format!("{}: not a directory", root.display())).into());
    }
    // The lock is released when `_lock` goes out of scope, on success and on error alike.
    let _lock = if opts.dry_run {
        None
    } else {
        Some(acquire_lock(root, &opts.lock)?)
    };
    run(contract, root, opts)
}

fn write_manifest(
    root: &Path,
    manifest: &Manifest,
    only_if_changed: bool,
) -> Result<(), ApplyError> {
    std::fs::create_dir_all(root.join(TOOL_DIR)).map_err(|e| io_error(TOOL_DIR, &e))?;
    let path = root.join(MANIFEST_PATH);
    let text = serialise_manifest(manifest);
    if only_if_changed && std::fs::read_to_string(&path).is_ok_and(|existing| existing == text) {
        return Ok(());
    }
    std::fs::write(&path, text).map_err(|e| io_error(MANIFEST_PATH, &e))
}

fn run(contract: &Contract, root: &Path, opts: &Options) -> Result<Report, ApplyError> {
    if opts.repair {
        return repair(contract, root, opts);
    }
    let manifest = read_manifest(root)?;
    let Plan {
        actions,
        mut refusals,
        next,
        writes,
        chmods,
        deletes,
    } = plan(contract, root, &manifest, opts)?;
    // The delete guard runs once, here, before anything is written. --force does not override it.
    if !deletes.is_empty() {
        let paths: Vec<&str> = deletes.iter().map(String::as_str).collect();
        refusals.extend(delete_guard(root, &paths));
    }
    if !refusals.is_empty() || opts.dry_run {
        return Ok(Report {
            actions,
            refusals,
            written: 0,
            deleted: 0,
            dry_run: opts.dry_run,
        });
    }

    use std::os::unix::fs::PermissionsExt;
    for f in &writes {
        write_atomic(root, f)?;
    }
    for f in &chmods {
        let abs = resolve_inside(root, &f.path)?;
        std::fs::set_permissions(
            &abs,
            std::fs::Permissions::from_mode(if wants_exec(f) { 0o755 } else { 0o644 }),
        )
        .map_err(|e| io_error(&f.path, &e))?;
    }
    for path in &deletes {
        std::fs::remove_file(resolve_inside(root, path)?).map_err(|e| io_error(path, &e))?;
    }
    let stage = root.join(TOOL_DIR).join("stage");
    if stage.exists() {
        std::fs::remove_dir_all(&stage).map_err(|e| io_error(".pkl-gen/stage", &e))?;
    }
    write_manifest(root, &next, true)?;
    Ok(Report {
        actions,
        refusals,
        written: writes.len(),
        deleted: deletes.len(),
        dry_run: false,
    })
}

/// Rebuilds the manifest from a clean render. Files that differ from the render are left out, never adopted.
fn repair(contract: &Contract, root: &Path, opts: &Options) -> Result<Report, ApplyError> {
    let mut next = Manifest::empty();
    next.package_version = opts.package_version.clone();
    next.presets = opts.presets.clone();
    let mut refusals = Vec::new();
    for f in &contract.files {
        let current = read(&resolve_inside(root, &f.path)?)?;
        if f.class == FileClass::External {
            next.files
                .insert(f.path.clone(), entry(FileClass::External, None, false));
            continue;
        }
        let content = final_content(f, &contract.pins)?;
        if current.as_deref() == Some(content.as_str()) {
            next.files.insert(
                f.path.clone(),
                entry(
                    f.class,
                    Some(sha256(&content)),
                    f.remove_on_detach == Some(true),
                ),
            );
        } else {
            let why = if current.is_none() {
                "missing"
            } else {
                "differs from the render"
            };
            refusals.push(format!(
                "{}: {why}, left out of the repaired manifest",
                f.path
            ));
        }
    }
    if !opts.dry_run {
        write_manifest(root, &next, false)?;
    }
    Ok(Report {
        actions: Vec::new(),
        refusals,
        written: 0,
        deleted: 0,
        dry_run: opts.dry_run,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::Mode;
    use crate::manifest::{ManifestEntry, read_manifest, sha256};
    use crate::testutil::{
        commit_all, contract_of, external, get, git, git_repo, has, mode_of, owned, put, seed,
        set_mode,
    };

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn run(c: &Contract, root: &Path) -> Report {
        apply(c, root, &Options::default()).unwrap()
    }

    fn run_with(c: &Contract, root: &Path, opts: Options) -> Report {
        apply(c, root, &opts).unwrap()
    }

    fn fails(c: &Contract, root: &Path, opts: &Options) -> String {
        apply(c, root, opts).unwrap_err().to_string()
    }

    fn kinds(r: &Report) -> Vec<String> {
        r.actions.iter().map(ToString::to_string).collect()
    }

    fn refusals(r: &Report) -> String {
        r.refusals.join("\n")
    }

    fn file(path: &str, content: &str) -> ContractFile {
        owned(path, content, Mode::Regular)
    }

    fn one(path: &str, content: &str) -> Contract {
        contract_of(vec![file(path, content)])
    }

    fn force() -> Options {
        Options {
            force: true,
            ..Options::default()
        }
    }

    fn dry() -> Options {
        Options {
            dry_run: true,
            ..Options::default()
        }
    }

    fn entry(root: &Path, path: &str) -> ManifestEntry {
        read_manifest(root).unwrap().files[path].clone()
    }

    fn manifest_paths(root: &Path) -> Vec<String> {
        read_manifest(root).unwrap().files.into_keys().collect()
    }

    const SHA: &str = "3d3c42e5aac5ba805825da76410c181273ba90b1";

    #[test]
    fn a_fresh_run_creates_owned_files_with_the_contract_mode_and_writes_the_manifest_last() {
        let dir = tmp();
        let c = contract_of(vec![
            file("a.txt", "one\n"),
            owned("s/run.sh", "#!/bin/sh\n", Mode::Executable),
        ]);
        let r = run(&c, dir.path());
        assert_eq!(kinds(&r), ["create:a.txt", "create:s/run.sh"]);
        assert_eq!(get(dir.path(), "a.txt"), "one\n");
        assert_eq!(mode_of(dir.path(), "s/run.sh"), "0755");
        assert_eq!(mode_of(dir.path(), "a.txt"), "0644");
        assert_eq!(
            entry(dir.path(), "a.txt"),
            ManifestEntry {
                class: FileClass::Owned,
                hash: Some(sha256("one\n")),
                remove_on_detach: false
            }
        );
        assert_eq!(r.written, 2);
    }

    #[test]
    fn a_second_run_changes_nothing() {
        let dir = tmp();
        let c = one("a.txt", "one\n");
        run(&c, dir.path());
        let before = get(dir.path(), ".pkl-gen/manifest.json");
        let r = run(&c, dir.path());
        assert_eq!(kinds(&r), ["unchanged:a.txt"]);
        assert_eq!(r.written, 0);
        assert_eq!(get(dir.path(), ".pkl-gen/manifest.json"), before);
    }

    #[test]
    fn no_stage_files_are_left_behind() {
        let dir = tmp();
        run(&one("a.txt", "x\n"), dir.path());
        assert!(!has(dir.path(), ".pkl-gen/stage"));
        let names: Vec<_> = std::fs::read_dir(dir.path().join(".pkl-gen"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["manifest.json"]);
    }

    #[test]
    fn an_unmodified_owned_file_is_updated_when_the_render_changes() {
        let dir = tmp();
        run(&one("a.txt", "one\n"), dir.path());
        let r = run(&one("a.txt", "two\n"), dir.path());
        assert_eq!(kinds(&r), ["update:a.txt"]);
        assert_eq!(get(dir.path(), "a.txt"), "two\n");
    }

    #[test]
    fn a_hand_edited_owned_file_is_refused_and_nothing_else_is_written() {
        let dir = tmp();
        run(
            &contract_of(vec![file("a.txt", "one\n"), file("b.txt", "b1\n")]),
            dir.path(),
        );
        put(dir.path(), "a.txt", "edited\n");
        let r = run(
            &contract_of(vec![file("a.txt", "one\n"), file("b.txt", "b2\n")]),
            dir.path(),
        );
        assert_eq!(r.refusals.len(), 1);
        assert!(r.refusals[0].contains("a.txt: was edited by hand"));
        assert!(r.refusals[0].contains("line 1 on disk:  edited"));
        assert_eq!(r.written, 0);
        assert_eq!(get(dir.path(), "a.txt"), "edited\n");
        assert_eq!(get(dir.path(), "b.txt"), "b1\n");
    }

    #[test]
    fn the_refusal_for_a_hand_edit_has_the_exact_text() {
        let dir = tmp();
        run(&one("a.txt", "one\n"), dir.path());
        put(dir.path(), "a.txt", "edited\n");
        let r = run(&one("a.txt", "one\n"), dir.path());
        assert_eq!(
            r.refusals,
            [
                "a.txt: was edited by hand since the last render (use --force to overwrite)\n    line 1 on disk:  edited\n    line 1 rendered: one"
            ]
        );
    }

    #[test]
    fn force_overwrites_a_hand_edited_owned_file() {
        let dir = tmp();
        run(&one("a.txt", "one\n"), dir.path());
        put(dir.path(), "a.txt", "edited\n");
        let r = run_with(&one("a.txt", "one\n"), dir.path(), force());
        assert!(r.refusals.is_empty());
        assert_eq!(get(dir.path(), "a.txt"), "one\n");
    }

    #[test]
    fn first_adoption_an_identical_existing_file_is_adopted_not_rewritten() {
        let dir = tmp();
        put(dir.path(), "a.txt", "same\n");
        let r = run(&one("a.txt", "same\n"), dir.path());
        assert_eq!(kinds(&r), ["adopt:a.txt"]);
        assert_eq!(r.written, 0);
        assert_eq!(entry(dir.path(), "a.txt").hash, Some(sha256("same\n")));
    }

    #[test]
    fn first_adoption_a_differing_existing_file_is_refused_with_a_diff() {
        let dir = tmp();
        put(dir.path(), "a.txt", "mine\n");
        let r = run(&one("a.txt", "theirs\n"), dir.path());
        assert_eq!(
            r.refusals,
            [
                "a.txt: already exists and differs from the render, so the runner will not adopt it (use --force to overwrite)\n    line 1 on disk:  mine\n    line 1 rendered: theirs"
            ]
        );
        assert_eq!(get(dir.path(), "a.txt"), "mine\n");
        assert!(!has(dir.path(), ".pkl-gen/manifest.json"));
    }

    #[test]
    fn first_adoption_force_overwrites_a_differing_existing_file() {
        let dir = tmp();
        put(dir.path(), "a.txt", "mine\n");
        let r = run_with(&one("a.txt", "theirs\n"), dir.path(), force());
        assert_eq!(kinds(&r), ["update:a.txt"]);
        assert_eq!(get(dir.path(), "a.txt"), "theirs\n");
    }

    #[test]
    fn seed_is_written_once_then_left_alone_even_when_the_render_changes() {
        let dir = tmp();
        let first = run(
            &contract_of(vec![seed("README.md", "# first\n")]),
            dir.path(),
        );
        assert_eq!(kinds(&first), ["seed:README.md"]);
        put(dir.path(), "README.md", "# mine\n");
        let r = run(
            &contract_of(vec![seed("README.md", "# second\n")]),
            dir.path(),
        );
        assert_eq!(kinds(&r), ["seed-kept:README.md"]);
        assert_eq!(get(dir.path(), "README.md"), "# mine\n");
    }

    #[test]
    fn a_kept_seed_remembers_the_hash_it_was_first_seeded_with() {
        let dir = tmp();
        run(
            &contract_of(vec![seed("README.md", "# first\n")]),
            dir.path(),
        );
        put(dir.path(), "README.md", "# mine\n");
        run(
            &contract_of(vec![seed("README.md", "# second\n")]),
            dir.path(),
        );
        assert_eq!(
            entry(dir.path(), "README.md").hash,
            Some(sha256("# first\n"))
        );
    }

    #[test]
    fn a_seed_with_no_recorded_hash_takes_the_hash_of_what_is_on_disk() {
        let dir = tmp();
        put(dir.path(), "README.md", "# mine\n");
        run(
            &contract_of(vec![seed("README.md", "# first\n")]),
            dir.path(),
        );
        assert_eq!(
            entry(dir.path(), "README.md").hash,
            Some(sha256("# mine\n"))
        );
    }

    #[test]
    fn seed_to_owned_is_refused_for_an_edited_seed() {
        let dir = tmp();
        run(
            &contract_of(vec![seed("README.md", "# first\n")]),
            dir.path(),
        );
        put(dir.path(), "README.md", "# mine\n");
        let r = run(&one("README.md", "# first\n"), dir.path());
        assert!(r.refusals[0].contains("was seeded and has been edited"));
        assert_eq!(get(dir.path(), "README.md"), "# mine\n");
    }

    #[test]
    fn seed_to_owned_with_force_overwrites_an_edited_seed() {
        let dir = tmp();
        run(
            &contract_of(vec![seed("README.md", "# first\n")]),
            dir.path(),
        );
        put(dir.path(), "README.md", "# mine\n");
        let r = run_with(&one("README.md", "# first\n"), dir.path(), force());
        assert!(r.refusals.is_empty());
        assert_eq!(get(dir.path(), "README.md"), "# first\n");
    }

    #[test]
    fn seed_to_owned_is_allowed_for_an_untouched_seed() {
        let dir = tmp();
        run(
            &contract_of(vec![seed("README.md", "# first\n")]),
            dir.path(),
        );
        let r = run(&one("README.md", "# second\n"), dir.path());
        assert!(r.refusals.is_empty());
        assert_eq!(kinds(&r), ["update:README.md"]);
        assert_eq!(get(dir.path(), "README.md"), "# second\n");
    }

    #[test]
    fn an_untouched_seed_that_already_matches_becomes_owned_without_a_write() {
        let dir = tmp();
        run(
            &contract_of(vec![seed("README.md", "# first\n")]),
            dir.path(),
        );
        let r = run(&one("README.md", "# first\n"), dir.path());
        assert_eq!(kinds(&r), ["unchanged:README.md"]);
        assert_eq!(entry(dir.path(), "README.md").class, FileClass::Owned);
    }

    #[test]
    fn owned_to_seed_leaves_the_file_and_records_it_as_a_seed() {
        let dir = tmp();
        run(&one("a.txt", "one\n"), dir.path());
        run(&contract_of(vec![seed("a.txt", "one\n")]), dir.path());
        assert_eq!(get(dir.path(), "a.txt"), "one\n");
        assert_eq!(entry(dir.path(), "a.txt").class, FileClass::Seed);
    }

    #[test]
    fn an_external_file_is_listed_never_written_hashed_or_touched() {
        let dir = tmp();
        put(dir.path(), "mise.lock", "lock\n");
        let r = run(&contract_of(vec![external("mise.lock")]), dir.path());
        assert_eq!(kinds(&r), ["external:mise.lock"]);
        assert_eq!(get(dir.path(), "mise.lock"), "lock\n");
        assert_eq!(
            entry(dir.path(), "mise.lock"),
            ManifestEntry {
                class: FileClass::External,
                hash: None,
                remove_on_detach: false
            }
        );
    }

    #[test]
    fn dry_run_plans_and_writes_nothing() {
        let dir = tmp();
        let r = run_with(&one("a.txt", "one\n"), dir.path(), dry());
        assert_eq!(kinds(&r), ["create:a.txt"]);
        assert!(r.dry_run);
        assert!(!has(dir.path(), "a.txt"));
        assert!(!has(dir.path(), ".pkl-gen"));
    }

    #[test]
    fn a_path_outside_the_repository_stops_the_run_before_anything_is_written() {
        let dir = tmp();
        let c = contract_of(vec![file("a.txt", "x\n"), file("../evil.txt", "x\n")]);
        assert!(fails(&c, dir.path(), &Options::default()).contains("escapes"));
        assert!(!has(dir.path(), "a.txt"));
    }

    #[test]
    fn paths_that_differ_only_by_case_stop_the_run() {
        let c = contract_of(vec![file("A.txt", "x\n"), file("a.txt", "y\n")]);
        assert_eq!(
            fails(&c, tmp().path(), &Options::default()),
            "A.txt and a.txt differ only by case"
        );
    }

    #[test]
    fn a_write_through_a_symlink_that_leaves_the_repository_is_refused() {
        let (dir, outside) = (tmp(), tmp());
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        assert!(
            fails(&one("link/x.txt", "x\n"), dir.path(), &Options::default()).contains("symlink")
        );
        assert!(!has(outside.path(), "x.txt"));
    }

    #[test]
    fn a_root_that_is_not_a_directory_is_an_error() {
        let dir = tmp();
        put(dir.path(), "f", "x");
        let message = fails(
            &one("a.txt", "x\n"),
            &dir.path().join("f"),
            &Options::default(),
        );
        assert!(message.ends_with("f: not a directory"), "{message}");
        let message = fails(
            &one("a.txt", "x\n"),
            &dir.path().join("missing"),
            &Options::default(),
        );
        assert!(message.ends_with("missing: not a directory"), "{message}");
    }

    #[test]
    fn repair_rebuilds_the_manifest_from_files_equal_to_the_render_and_never_adopts_a_differing_file()
     {
        let dir = tmp();
        put(dir.path(), "same.txt", "s\n");
        put(dir.path(), "diff.txt", "mine\n");
        let c = contract_of(vec![
            file("same.txt", "s\n"),
            file("diff.txt", "theirs\n"),
            file("gone.txt", "g\n"),
            external("m.lock"),
        ]);
        let r = run_with(
            &c,
            dir.path(),
            Options {
                repair: true,
                ..Options::default()
            },
        );
        assert_eq!(manifest_paths(dir.path()), ["m.lock", "same.txt"]);
        assert_eq!(
            r.refusals,
            [
                "diff.txt: differs from the render, left out of the repaired manifest",
                "gone.txt: missing, left out of the repaired manifest"
            ]
        );
        assert_eq!(get(dir.path(), "diff.txt"), "mine\n");
        assert!(r.actions.is_empty());
    }

    #[test]
    fn repair_after_a_broken_manifest_lets_a_normal_run_continue() {
        let dir = tmp();
        let c = one("a.txt", "one\n");
        run(&c, dir.path());
        put(dir.path(), ".pkl-gen/manifest.json", "<<<<<<< x\n");
        assert!(fails(&c, dir.path(), &Options::default()).contains("--repair"));
        run_with(
            &c,
            dir.path(),
            Options {
                repair: true,
                ..Options::default()
            },
        );
        assert_eq!(kinds(&run(&c, dir.path())), ["unchanged:a.txt"]);
    }

    #[test]
    fn repair_with_dry_run_writes_nothing() {
        let dir = tmp();
        put(dir.path(), "a.txt", "one\n");
        let opts = Options {
            repair: true,
            dry_run: true,
            ..Options::default()
        };
        let r = run_with(&one("a.txt", "one\n"), dir.path(), opts);
        assert!(r.dry_run);
        assert!(!has(dir.path(), ".pkl-gen"));
    }

    #[test]
    fn a_readable_existing_file_is_adopted() {
        let dir = tmp();
        put(dir.path(), "ok.txt", "x\n");
        set_mode(dir.path(), "ok.txt", 0o644);
        assert_eq!(
            kinds(&run(&one("ok.txt", "x\n"), dir.path())),
            ["adopt:ok.txt"]
        );
    }

    #[test]
    fn pinned_uses_lines_get_their_version_comment_before_hashing_and_a_second_run_is_a_no_op() {
        let dir = tmp();
        let mut c = one(
            ".github/workflows/x.yml",
            &format!("jobs:\n  a:\n    steps:\n      - uses: actions/checkout@{SHA}\n"),
        );
        c.pins = vec![crate::contract::Pin {
            action: "actions/checkout".into(),
            sha: SHA.into(),
            version: "v7.0.1".into(),
        }];
        run(&c, dir.path());
        assert!(
            get(dir.path(), ".github/workflows/x.yml")
                .contains(&format!("actions/checkout@{SHA} # v7.0.1"))
        );
        assert_eq!(
            entry(dir.path(), ".github/workflows/x.yml").hash,
            Some(sha256(&get(dir.path(), ".github/workflows/x.yml")))
        );
        assert_eq!(
            kinds(&run(&c, dir.path())),
            ["unchanged:.github/workflows/x.yml"]
        );
    }

    #[test]
    fn a_pinned_uses_line_with_no_known_version_stops_the_run() {
        let c = one(
            ".github/workflows/x.yml",
            &format!("steps:\n  - uses: other/thing@{SHA}\n"),
        );
        assert!(
            fails(&c, tmp().path(), &Options::default())
                .contains("x.yml: line 2: no version known")
        );
    }

    #[test]
    fn version_comments_are_not_added_to_files_outside_workflows() {
        let dir = tmp();
        run(
            &one("notes.md", &format!("  - uses: actions/checkout@{SHA}\n")),
            dir.path(),
        );
        assert!(!get(dir.path(), "notes.md").contains('#'));
    }

    /// A git repository holding a render of `files`, committed.
    fn rendered(files: Vec<ContractFile>) -> crate::testutil::Repo {
        let repo = git_repo(&[(".gitignore", ".pkl-gen/lock\n.pkl-gen/stage/\n")]);
        run(&contract_of(files), &repo.root);
        commit_all(&repo.root, "render");
        repo
    }

    #[test]
    fn a_file_dropped_from_the_contract_is_deleted_when_the_tree_is_clean_and_the_file_is_tracked()
    {
        let repo = rendered(vec![file("a.txt", "one\n"), file("b.txt", "two\n")]);
        let r = run(&one("a.txt", "one\n"), &repo.root);
        assert!(kinds(&r).contains(&"delete:b.txt".to_owned()));
        assert!(!has(&repo.root, "b.txt"));
        assert_eq!(manifest_paths(&repo.root), ["a.txt"]);
        assert_eq!(r.deleted, 1);
    }

    #[test]
    fn dry_run_shows_the_deletion_and_removes_nothing() {
        let repo = rendered(vec![file("a.txt", "one\n"), file("b.txt", "two\n")]);
        let r = run_with(&one("a.txt", "one\n"), &repo.root, dry());
        assert!(kinds(&r).contains(&"delete:b.txt".to_owned()));
        assert!(has(&repo.root, "b.txt"));
        assert_eq!(r.deleted, 0);
    }

    #[test]
    fn a_dirty_tracked_tree_refuses_the_deletion_and_nothing_else_is_written() {
        let repo = rendered(vec![file("a.txt", "one\n"), file("b.txt", "two\n")]);
        put(&repo.root, "other.md", "dirty\n");
        git(&repo.root, &["add", "other.md"]);
        let r = run(
            &contract_of(vec![file("a.txt", "NEW\n"), file("c.txt", "three\n")]),
            &repo.root,
        );
        assert!(refusals(&r).contains("not clean"));
        assert!(has(&repo.root, "b.txt"));
        assert!(!has(&repo.root, "c.txt"));
        assert_eq!(get(&repo.root, "a.txt"), "one\n");
    }

    #[test]
    fn dry_run_reports_a_guard_refusal_too() {
        let repo = rendered(vec![file("a.txt", "one\n"), file("b.txt", "two\n")]);
        put(&repo.root, "a.txt", "edited\n");
        let r = run_with(
            &one("a.txt", "one\n"),
            &repo.root,
            Options {
                dry_run: true,
                force: true,
                ..Options::default()
            },
        );
        assert!(refusals(&r).contains("not clean"));
    }

    #[test]
    fn an_untracked_orphan_is_never_deleted() {
        let repo = git_repo(&[(".gitignore", ".pkl-gen/lock\n.pkl-gen/stage/\n")]);
        run(
            &contract_of(vec![file("a.txt", "one\n"), file("b.txt", "two\n")]),
            &repo.root,
        );
        git(&repo.root, &["add", "a.txt", ".gitignore"]);
        git(&repo.root, &["commit", "-qm", "only a"]);
        let r = run(&one("a.txt", "one\n"), &repo.root);
        assert!(refusals(&r).contains("b.txt: not tracked by git"));
        assert!(has(&repo.root, "b.txt"));
    }

    #[test]
    fn an_orphan_edited_by_hand_and_committed_is_refused_and_force_deletes_it() {
        let repo = rendered(vec![file("a.txt", "one\n"), file("b.txt", "two\n")]);
        put(&repo.root, "b.txt", "mine\n");
        commit_all(&repo.root, "hand edit");
        let refused = run(&one("a.txt", "one\n"), &repo.root);
        assert_eq!(
            refused.refusals,
            [
                "b.txt: no longer rendered, but it was edited by hand since the last render, so it is not deleted (use --force)"
            ]
        );
        assert!(has(&repo.root, "b.txt"));
        let forced = run_with(&one("a.txt", "one\n"), &repo.root, force());
        assert!(kinds(&forced).contains(&"delete:b.txt".to_owned()));
        assert!(!has(&repo.root, "b.txt"));
    }

    #[test]
    fn an_orphan_that_is_already_gone_is_simply_dropped_from_the_manifest() {
        let repo = rendered(vec![file("a.txt", "one\n"), file("b.txt", "two\n")]);
        git(&repo.root, &["rm", "-q", "b.txt"]);
        git(&repo.root, &["commit", "-qm", "removed by hand"]);
        let r = run(&one("a.txt", "one\n"), &repo.root);
        assert!(r.refusals.is_empty());
        assert!(kinds(&r).contains(&"forget:b.txt".to_owned()));
        assert_eq!(manifest_paths(&repo.root), ["a.txt"]);
    }

    #[test]
    fn a_seed_or_external_file_dropped_from_the_contract_is_left_alone_and_dropped_from_the_manifest()
     {
        let repo = rendered(vec![file("a.txt", "one\n")]);
        run(
            &contract_of(vec![
                file("a.txt", "one\n"),
                seed("s.txt", "seed\n"),
                external("mise.lock"),
            ]),
            &repo.root,
        );
        put(&repo.root, "mise.lock", "lock\n");
        commit_all(&repo.root, "seed and lock");
        let r = run(&one("a.txt", "one\n"), &repo.root);
        assert!(kinds(&r).iter().all(|k| !k.starts_with("delete")));
        assert_eq!(get(&repo.root, "s.txt"), "seed\n");
        assert_eq!(get(&repo.root, "mise.lock"), "lock\n");
        assert_eq!(manifest_paths(&repo.root), ["a.txt"]);
    }

    #[test]
    fn without_orphans_git_is_not_needed_at_all() {
        let dir = tmp();
        assert!(run(&one("a.txt", "one\n"), dir.path()).refusals.is_empty());
    }

    #[test]
    fn an_orphan_in_a_directory_that_is_not_a_git_repository_is_refused() {
        let dir = tmp();
        run(
            &contract_of(vec![file("a.txt", "one\n"), file("b.txt", "two\n")]),
            dir.path(),
        );
        let r = run(&one("a.txt", "one\n"), dir.path());
        assert!(refusals(&r).contains("not a git repository"));
        assert!(has(dir.path(), "b.txt"));
    }

    #[test]
    fn a_file_outside_the_repo_cannot_be_deleted_through_a_path_in_the_manifest() {
        let repo = rendered(vec![file("a.txt", "one\n")]);
        let mut m = read_manifest(&repo.root).unwrap();
        m.files.insert(
            "../evil.txt".into(),
            ManifestEntry {
                class: FileClass::Owned,
                hash: Some(sha256("x")),
                remove_on_detach: false,
            },
        );
        put(
            &repo.root,
            ".pkl-gen/manifest.json",
            &crate::manifest::serialise_manifest(&m),
        );
        commit_all(&repo.root, "tampered manifest");
        assert!(fails(&one("a.txt", "one\n"), &repo.root, &Options::default()).contains("escapes"));
    }

    #[test]
    fn the_exec_bit_is_set_when_the_contract_says_so_and_the_content_is_unchanged() {
        let dir = tmp();
        run(&one("run.sh", "#!/bin/sh\n"), dir.path());
        let exec = contract_of(vec![owned("run.sh", "#!/bin/sh\n", Mode::Executable)]);
        assert_eq!(kinds(&run(&exec, dir.path())), ["chmod:run.sh"]);
        assert_eq!(mode_of(dir.path(), "run.sh"), "0755");
        assert_eq!(kinds(&run(&exec, dir.path())), ["unchanged:run.sh"]);
    }

    #[test]
    fn the_exec_bit_is_cleared_when_the_contract_drops_it() {
        let dir = tmp();
        run(
            &contract_of(vec![owned("run.sh", "#!/bin/sh\n", Mode::Executable)]),
            dir.path(),
        );
        assert_eq!(
            kinds(&run(&one("run.sh", "#!/bin/sh\n"), dir.path())),
            ["chmod:run.sh"]
        );
        assert_eq!(mode_of(dir.path(), "run.sh"), "0644");
    }

    #[test]
    fn an_exec_bit_removed_by_hand_is_restored_on_the_next_render() {
        let dir = tmp();
        let exec = contract_of(vec![owned("run.sh", "#!/bin/sh\n", Mode::Executable)]);
        run(&exec, dir.path());
        set_mode(dir.path(), "run.sh", 0o644);
        run(&exec, dir.path());
        assert_eq!(mode_of(dir.path(), "run.sh"), "0755");
    }

    #[test]
    fn an_adopted_file_with_a_wrong_exec_bit_is_adopted_and_fixed() {
        let dir = tmp();
        put(dir.path(), "run.sh", "#!/bin/sh\n");
        let exec = contract_of(vec![owned("run.sh", "#!/bin/sh\n", Mode::Executable)]);
        assert_eq!(
            kinds(&run(&exec, dir.path())),
            ["adopt:run.sh", "chmod:run.sh"]
        );
        assert_eq!(mode_of(dir.path(), "run.sh"), "0755");
    }

    #[test]
    fn dry_run_reports_the_mode_change_and_does_not_make_it() {
        let dir = tmp();
        run(&one("run.sh", "#!/bin/sh\n"), dir.path());
        let exec = contract_of(vec![owned("run.sh", "#!/bin/sh\n", Mode::Executable)]);
        assert_eq!(kinds(&run_with(&exec, dir.path(), dry())), ["chmod:run.sh"]);
        assert_eq!(mode_of(dir.path(), "run.sh"), "0644");
    }

    #[test]
    fn a_seed_file_keeps_the_mode_the_user_gave_it() {
        let dir = tmp();
        run(&contract_of(vec![seed("s.sh", "#!/bin/sh\n")]), dir.path());
        set_mode(dir.path(), "s.sh", 0o755);
        assert_eq!(
            kinds(&run(
                &contract_of(vec![seed("s.sh", "#!/bin/sh\n")]),
                dir.path()
            )),
            ["seed-kept:s.sh"]
        );
        assert_eq!(mode_of(dir.path(), "s.sh"), "0755");
    }

    fn held_lock(dir: &Path) {
        put(
            dir,
            ".pkl-gen/lock",
            &format!(
                "{{\"pid\":{},\"started\":\"{}\"}}",
                std::process::id(),
                now_iso()
            ),
        );
    }

    fn now_iso() -> String {
        time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    }

    #[test]
    fn a_render_refuses_to_run_while_another_run_holds_the_lock_and_writes_nothing() {
        let dir = tmp();
        held_lock(dir.path());
        let opts = Options {
            lock: LockOptions {
                ci: false,
                ..LockOptions::default()
            },
            ..Options::default()
        };
        assert!(fails(&one("a.txt", "one\n"), dir.path(), &opts).contains("is held by process"));
        assert!(!has(dir.path(), "a.txt"));
    }

    #[test]
    fn the_lock_is_released_after_a_run_and_after_a_refusal() {
        let dir = tmp();
        run(&one("a.txt", "one\n"), dir.path());
        assert!(!has(dir.path(), ".pkl-gen/lock"));
        put(dir.path(), "a.txt", "hand\n");
        let r = run(&one("a.txt", "two\n"), dir.path());
        assert_eq!(r.refusals.len(), 1);
        assert!(!has(dir.path(), ".pkl-gen/lock"));
    }

    #[test]
    fn the_lock_is_released_after_an_error() {
        let dir = tmp();
        let c = contract_of(vec![file("a.txt", "x\n"), file("../evil.txt", "x\n")]);
        assert!(apply(&c, dir.path(), &Options::default()).is_err());
        assert!(!has(dir.path(), ".pkl-gen/lock"));
    }

    #[test]
    fn dry_run_does_not_take_the_lock() {
        let dir = tmp();
        held_lock(dir.path());
        assert_eq!(
            kinds(&run_with(&one("a.txt", "one\n"), dir.path(), dry())),
            ["create:a.txt"]
        );
    }

    #[test]
    fn the_package_version_is_recorded_in_the_manifest_and_kept_when_a_later_run_has_none() {
        let dir = tmp();
        let c = one("a.txt", "one\n");
        let with = |v: &str| Options {
            package_version: Some(v.into()),
            ..Options::default()
        };
        run_with(&c, dir.path(), with("0.2.0"));
        assert_eq!(
            read_manifest(dir.path())
                .unwrap()
                .package_version
                .as_deref(),
            Some("0.2.0")
        );
        run(&c, dir.path());
        assert_eq!(
            read_manifest(dir.path())
                .unwrap()
                .package_version
                .as_deref(),
            Some("0.2.0")
        );
        run_with(&c, dir.path(), with("0.3.0"));
        assert_eq!(
            read_manifest(dir.path())
                .unwrap()
                .package_version
                .as_deref(),
            Some("0.3.0")
        );
    }

    fn presets_of(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn the_presets_of_a_render_are_recorded_and_a_render_with_none_clears_them() {
        let dir = tmp();
        let c = one("a.txt", "one\n");
        let with = |p: &[(&str, &str)]| Options {
            presets: presets_of(p),
            ..Options::default()
        };
        run_with(
            &c,
            dir.path(),
            with(&[("presets/a.pkl", "h1"), ("presets/b.pkl", "h2")]),
        );
        assert_eq!(
            read_manifest(dir.path()).unwrap().presets,
            presets_of(&[("presets/a.pkl", "h1"), ("presets/b.pkl", "h2")])
        );
        run_with(&c, dir.path(), with(&[("presets/a.pkl", "h3")]));
        assert_eq!(
            read_manifest(dir.path()).unwrap().presets,
            presets_of(&[("presets/a.pkl", "h3")])
        );
        run(&c, dir.path());
        assert!(read_manifest(dir.path()).unwrap().presets.is_empty());
    }

    #[test]
    fn a_dry_run_and_a_refused_render_do_not_record_presets() {
        let dir = tmp();
        let c = one("a.txt", "one\n");
        let opts = |dry_run: bool| Options {
            dry_run,
            presets: presets_of(&[("presets/a.pkl", "h1")]),
            ..Options::default()
        };
        run_with(&c, dir.path(), opts(true));
        assert!(!dir.path().join(".pkl-gen/manifest.json").exists());
        put(dir.path(), "a.txt", "mine\n");
        let r = run_with(&c, dir.path(), opts(false));
        assert!(!refusals(&r).is_empty());
        assert!(!dir.path().join(".pkl-gen/manifest.json").exists());
    }

    #[test]
    fn repair_records_the_presets_too() {
        let dir = tmp();
        let c = one("a.txt", "one\n");
        let opts = |repair: bool| Options {
            repair,
            presets: presets_of(&[("presets/a.pkl", "h1")]),
            ..Options::default()
        };
        run_with(&c, dir.path(), opts(false));
        run_with(&c, dir.path(), opts(true));
        assert_eq!(
            read_manifest(dir.path()).unwrap().presets,
            presets_of(&[("presets/a.pkl", "h1")])
        );
    }

    #[test]
    fn the_detach_flag_of_an_owned_file_is_recorded_in_the_manifest() {
        let dir = tmp();
        let flagged = ContractFile {
            remove_on_detach: Some(true),
            ..file("up.yml", "x\n")
        };
        run(
            &contract_of(vec![flagged, file("a.txt", "one\n")]),
            dir.path(),
        );
        assert_eq!(
            entry(dir.path(), "up.yml"),
            ManifestEntry {
                class: FileClass::Owned,
                hash: Some(sha256("x\n")),
                remove_on_detach: true
            }
        );
        assert!(!entry(dir.path(), "a.txt").remove_on_detach);
    }

    #[test]
    fn repair_keeps_the_package_version_and_the_detach_flag() {
        let dir = tmp();
        let c = contract_of(vec![ContractFile {
            remove_on_detach: Some(true),
            ..file("up.yml", "x\n")
        }]);
        let opts = || Options {
            package_version: Some("0.2.0".into()),
            ..Options::default()
        };
        run_with(&c, dir.path(), opts());
        run_with(
            &c,
            dir.path(),
            Options {
                repair: true,
                ..opts()
            },
        );
        let m = read_manifest(dir.path()).unwrap();
        assert_eq!(m.package_version.as_deref(), Some("0.2.0"));
        assert!(m.files["up.yml"].remove_on_detach);
    }

    #[test]
    fn a_case_only_rename_is_refused_with_nothing_written_and_the_old_file_intact() {
        let dir = tmp();
        run(&one("readme.md", "one\n"), dir.path());
        let r = run(&one("README.md", "two\n"), dir.path());
        assert!(refusals(&r).contains("readme.md and README.md differ only by case"));
        assert_eq!(get(dir.path(), "readme.md"), "one\n");
        assert_eq!(r.written, 0);
    }

    #[test]
    fn force_does_not_override_a_case_only_rename() {
        let dir = tmp();
        run(&one("readme.md", "one\n"), dir.path());
        let r = run_with(&one("README.md", "two\n"), dir.path(), force());
        assert!(refusals(&r).contains("differ only by case"));
        assert_eq!(get(dir.path(), "readme.md"), "one\n");
    }

    #[test]
    fn dry_run_reports_a_case_only_rename_as_a_refusal_too() {
        let dir = tmp();
        run(&one("readme.md", "one\n"), dir.path());
        assert!(
            refusals(&run_with(&one("README.md", "two\n"), dir.path(), dry()))
                .contains("differ only by case")
        );
    }

    #[test]
    fn a_rename_to_a_different_name_is_still_an_ordinary_orphan() {
        let repo = rendered(vec![file("readme.md", "one\n")]);
        let r = run(&one("docs/readme.md", "one\n"), &repo.root);
        assert!(r.refusals.is_empty());
        assert!(kinds(&r).contains(&"delete:readme.md".to_owned()));
    }

    #[test]
    fn action_text_uses_the_typescript_names() {
        let all = [
            (Kind::Create, "create"),
            (Kind::Update, "update"),
            (Kind::Unchanged, "unchanged"),
            (Kind::Adopt, "adopt"),
            (Kind::Seed, "seed"),
            (Kind::SeedKept, "seed-kept"),
            (Kind::External, "external"),
            (Kind::Chmod, "chmod"),
            (Kind::Delete, "delete"),
            (Kind::Forget, "forget"),
        ];
        for (kind, name) in all {
            assert_eq!(
                Action {
                    kind,
                    path: "p".into()
                }
                .to_string(),
                format!("{name}:p")
            );
        }
    }
}
