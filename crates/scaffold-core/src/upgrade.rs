//! `upgrade <version>`: rewrites the amends URI, its checksum and the runner pin, refreshes mise, and renders with the new
//! runner. Nothing is written until the release's checksums file has been downloaded and its attestation verified. If mise
//! fails, every file it or this command touched is put back. Ported from `runner/src/upgrade.ts`.

use std::cmp::Ordering;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::checksums::{checksum_asset_url, find_checksum};
use crate::contract::FileClass;
use crate::lock::{LockError, LockOptions, acquire_lock};
use crate::lockcheck::check_runner_lock;
use crate::manifest::{
    MANIFEST_PATH, Manifest, ManifestEntry, ManifestError, read_manifest, serialise_manifest,
    sha256,
};
use crate::pins::{NewPins, PinError, RUNNER_TOOL, count_package_imports, read_pins, write_pins};
use crate::semver::{compare_semver, parse_semver};

pub const REPO: &str = "pantheon-org/pantheon-scaffold";
pub const RELEASE_BASE: &str =
    "https://github.com/pantheon-org/pantheon-scaffold/releases/download";

#[derive(Debug, PartialEq, Eq)]
pub struct UpgradeError(pub String);

impl std::fmt::Display for UpgradeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for UpgradeError {}

/// The three things upgrade reaches outside the repository for. Each can be replaced, which is how the tests run without a
/// release.
pub trait Deps {
    fn fetch_text(&self, url: &str) -> Result<String, String>;
    /// Fails unless the file has a valid build provenance attestation from the repository.
    fn verify_attestation(&self, file: &Path, repo: &str) -> Result<(), String>;
    /// Runs a program and returns its exit code. The error is a program that could not be started.
    fn exec(&self, cmd: &[&str], cwd: &Path) -> Result<i32, String>;
}

/// The real thing: the release over HTTPS, `gh attestation verify`, and `mise`.
pub struct SystemDeps {
    pub gh: OsString,
    pub mise: OsString,
}

impl Default for SystemDeps {
    fn default() -> Self {
        Self {
            gh: "gh".into(),
            mise: "mise".into(),
        }
    }
}

impl Deps for SystemDeps {
    fn fetch_text(&self, url: &str) -> Result<String, String> {
        crate::http::get_text(url, std::time::Duration::from_secs(15))
    }

    fn verify_attestation(&self, file: &Path, repo: &str) -> Result<(), String> {
        let output = std::process::Command::new(&self.gh)
            .args(["attestation", "verify"])
            .arg(file)
            .args(["--repo", repo])
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| format!("could not run gh: {e}"))?;
        if output.status.success() {
            return Ok(());
        }
        let said = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if said.is_empty() {
            format!(
                "gh attestation verify exited {}",
                output.status.code().unwrap_or(-1)
            )
        } else {
            said
        })
    }

    fn exec(&self, cmd: &[&str], cwd: &Path) -> Result<i32, String> {
        let (program, args) = cmd
            .split_first()
            .ok_or_else(|| "no command to run".to_owned())?;
        let program: OsString = if *program == "mise" {
            self.mise.clone()
        } else {
            (*program).into()
        };
        let status = std::process::Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        Ok(status.code().unwrap_or(-1))
    }
}

#[derive(Default)]
pub struct UpgradeOptions {
    pub config: Option<String>,
    pub dry_run: bool,
    pub force: bool,
    pub no_render: bool,
    pub base_url: Option<String>,
    pub lock: LockOptions,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UpgradeReport {
    pub from: String,
    pub to: String,
    pub changes: Vec<String>,
    pub dry_run: bool,
    pub render_exit: Option<i32>,
}

impl From<PinError> for UpgradeError {
    fn from(e: PinError) -> Self {
        Self(e.0)
    }
}
impl From<LockError> for UpgradeError {
    fn from(e: LockError) -> Self {
        Self(e.0)
    }
}
impl From<ManifestError> for UpgradeError {
    fn from(e: ManifestError) -> Self {
        Self(e.0)
    }
}

fn json(text: &str) -> String {
    serde_json::to_string(text).expect("a string serialises")
}

/// A file as it was, to put back: its path and its bytes, or `None` when it did not exist.
struct Backup {
    path: PathBuf,
    bytes: Option<Vec<u8>>,
}

fn backup(path: PathBuf) -> Backup {
    let bytes = std::fs::read(&path).ok();
    Backup { path, bytes }
}

/// Puts every file back. Returns what could not be put back, so a failure to restore is never silent.
fn restore(saved: &[Backup]) -> Vec<String> {
    let mut failed = Vec::new();
    for b in saved {
        let result = match &b.bytes {
            Some(bytes) => std::fs::write(&b.path, bytes),
            None => match std::fs::remove_file(&b.path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            },
        };
        if let Err(e) = result {
            failed.push(format!("restoring {} failed: {e}", b.path.display()));
        }
    }
    failed
}

/// The manifest entry of `mise.toml` when it is an owned file whose text still matches, so the pin edit can be recorded
/// there and the render that follows does not read it as a hand edit. A `mise.toml` that was edited by hand, a seeded one,
/// or no manifest: nothing is recorded, and the render judges the file as it did before.
struct Seal {
    manifest: Manifest,
    entry: ManifestEntry,
}

fn sealed_mise(root: &Path) -> Result<Option<Seal>, UpgradeError> {
    let mise = root.join("mise.toml");
    // A repository with no manifest reads as an empty one, which has no entry for mise.toml, so nothing is sealed.
    let manifest = read_manifest(root)?;
    let text = String::from_utf8_lossy(
        &std::fs::read(&mise).map_err(|e| UpgradeError(format!("mise.toml: {e}")))?,
    )
    .into_owned();
    Ok(manifest
        .files
        .get("mise.toml")
        .filter(|e| {
            e.class == FileClass::Owned && e.hash.as_deref() == Some(sha256(&text).as_str())
        })
        .cloned()
        .map(|entry| Seal { manifest, entry }))
}

fn record_seal(root: &Path, seal: Seal) -> Result<(), UpgradeError> {
    let Seal {
        mut manifest,
        entry,
    } = seal;
    let text = std::fs::read(root.join("mise.toml"))
        .map_err(|e| UpgradeError(format!("mise.toml: {e}")))?;
    manifest.files.insert(
        "mise.toml".to_owned(),
        ManifestEntry {
            hash: Some(sha256(&String::from_utf8_lossy(&text))),
            ..entry
        },
    );
    std::fs::write(root.join(MANIFEST_PATH), serialise_manifest(&manifest))
        .map_err(|e| UpgradeError(format!("{MANIFEST_PATH}: {e}")))
}

/// Downloads the release's checksums file and returns the package checksum, after the attestation says the file is genuine.
fn verified_checksum(version: &str, base: &str, deps: &dyn Deps) -> Result<String, UpgradeError> {
    let url = checksum_asset_url(base, version);
    let text = deps
        .fetch_text(&url)
        .map_err(|e| UpgradeError(format!("could not fetch {url}: {e}")))?;
    {
        // The file lives in a folder only this run can use, and is removed when the check is over, passed or not.
        let dir = tempfile::Builder::new()
            .prefix("scaffold-checksums-")
            .tempdir()
            .map_err(|e| UpgradeError(format!("could not make a temporary folder: {e}")))?;
        let file = dir.path().join("checksums.txt");
        std::fs::write(&file, &text)
            .map_err(|e| UpgradeError(format!("could not write {}: {e}", file.display())))?;
        deps.verify_attestation(&file, REPO)
            .map_err(|e| UpgradeError(format!("attestation check failed for {url}: {e}")))?;
    }
    find_checksum(&text, &format!("pantheon-scaffold@{version}")).map_err(UpgradeError)
}

pub fn upgrade(
    root: &Path,
    requested: &str,
    opts: &UpgradeOptions,
    deps: &dyn Deps,
) -> Result<UpgradeReport, UpgradeError> {
    let to = requested.strip_prefix('v').unwrap_or(requested);
    if parse_semver(to).is_none() {
        return Err(UpgradeError(format!(
            "{} is not a version",
            json(requested)
        )));
    }
    let config = opts.config.as_deref().unwrap_or("pantheon.pkl");
    let pins = read_pins(root, config)?;
    let from = pins.package_version.clone();
    match compare_semver(to, &from).map_err(UpgradeError)? {
        Ordering::Equal => return Err(UpgradeError(format!("already at {to}"))),
        Ordering::Less if !opts.force => {
            return Err(UpgradeError(format!(
                "{to} is older than {from}, which is pinned now (use --force to go back)"
            )));
        }
        _ => {}
    }

    let checksum = verified_checksum(to, opts.base_url.as_deref().unwrap_or(RELEASE_BASE), deps)?;
    let imports = count_package_imports(root, config)?;
    let imports_note = match imports {
        0 => String::new(),
        1 => ", 1 import of the package".to_owned(),
        n => format!(", {n} imports of the package"),
    };
    let changes = vec![
        format!(
            "{config}: {from} -> {to}, checksum {}...{imports_note}",
            &checksum[..12]
        ),
        format!("mise.toml: {RUNNER_TOOL} {} -> {to}", pins.runner_version),
    ];
    let report = |render_exit: Option<i32>, dry_run: bool| UpgradeReport {
        from: from.clone(),
        to: to.to_owned(),
        changes: changes.clone(),
        dry_run,
        render_exit,
    };
    if opts.dry_run {
        return Ok(report(None, true));
    }

    let lock = acquire_lock(root, &opts.lock)?;
    let saved = [
        backup(root.join(config)),
        backup(root.join("mise.toml")),
        backup(root.join("mise.lock")),
        backup(root.join(MANIFEST_PATH)),
    ];
    let outcome = move_pins_and_refresh_mise(root, config, to, &checksum, deps);
    if let Err(mut error) = outcome {
        // Whatever went wrong, the files are put back, and a file that cannot be put back is named.
        for failure in restore(&saved) {
            error.0.push_str(&format!("; {failure}"));
        }
        return Err(error);
    }
    // The lock is let go before the new runner renders, so its own render can take it.
    drop(lock);
    if opts.no_render {
        return Ok(report(None, false));
    }
    let root_text = root.display().to_string();
    let render = [
        "mise",
        "x",
        "--",
        "pantheon-scaffold",
        "render",
        config,
        "--root",
        root_text.as_str(),
    ];
    let code = deps.exec(&render, root).map_err(|e| {
        UpgradeError(format!(
            "the pins are updated, but the render could not be started ({e})"
        ))
    })?;
    Ok(report(Some(code), false))
}

/// Writes the pins, runs `mise install` and `mise lock`, checks what the lock recorded, and records the new `mise.toml` in
/// the manifest. Any failure is an error for the caller to undo.
fn move_pins_and_refresh_mise(
    root: &Path,
    config: &str,
    version: &str,
    checksum: &str,
    deps: &dyn Deps,
) -> Result<(), UpgradeError> {
    // The package owns mise.toml when it renders it, so the pin edit below must not look like a hand edit to the render that follows.
    let seal = sealed_mise(root)?;
    write_pins(root, config, &NewPins { version, checksum })?;
    for step in ["install", "lock"] {
        match deps.exec(&["mise", step], root) {
            Ok(0) => {}
            Ok(code) => {
                return Err(UpgradeError(format!(
                    "mise {step} failed (exit {code}), the pins and the lock file were put back"
                )));
            }
            Err(e) => {
                return Err(UpgradeError(format!(
                    "mise {step} could not be started ({e}), the pins and the lock file were put back"
                )));
            }
        }
    }
    // mise locks every platform it knows unless told which, and for one with no runner it picks whatever asset it can.
    let lock_path = root.join("mise.lock");
    let problems = if lock_path.exists() {
        let text =
            std::fs::read(&lock_path).map_err(|e| UpgradeError(format!("mise.lock: {e}")))?;
        check_runner_lock(&String::from_utf8_lossy(&text))
    } else {
        Vec::new()
    };
    if !problems.is_empty() {
        return Err(UpgradeError(format!(
            "mise lock recorded the wrong runner for some platforms, so the pins and the lock file were put back:\n  {}\nLimit the platforms in mise.toml: [settings] lockfile_platforms = [\"linux-x64\", \"linux-arm64\", \"macos-arm64\"]",
            problems.join("\n  ")
        )));
    }
    if let Some(seal) = seal {
        record_seal(root, seal)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::FileClass;
    use crate::manifest::{Manifest, ManifestEntry, serialise_manifest, sha256};
    use crate::pins::RUNNER_TOOL;
    use crate::testutil::{get, has, put};
    use std::cell::RefCell;
    use std::collections::HashMap;

    fn h(c: char) -> String {
        c.to_string().repeat(64)
    }

    const BASE: &str = "https://example.test/releases/download";

    fn amends(v: &str, sum: &str) -> String {
        format!(
            "amends \"package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{v}/pantheon-scaffold@{v}::sha256:{sum}#/Config.pkl\"\n\noptions {{ toolchain = \"1.96.1\" }}\n"
        )
    }

    fn mise(v: &str) -> String {
        format!("[tools]\nbun = \"1.4.2\"\n\"{RUNNER_TOOL}\" = \"{v}\"\n")
    }

    fn repo(v: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "pantheon.pkl", &amends(v, &h('a')));
        put(dir.path(), "mise.toml", &mise(v));
        put(dir.path(), "mise.lock", "# lock for 0.1.0\n");
        dir
    }

    /// What a fake runs when a command is "executed": a test uses it to make mise do something to the repository.
    type OnExec = Box<dyn Fn(&str)>;

    /// Fakes for the three things upgrade reaches outside the repository for: the checksums download, the attestation check
    /// and mise.
    #[derive(Default)]
    struct Fake {
        calls: RefCell<Vec<String>>,
        verified: RefCell<Vec<String>>,
        text: Option<String>,
        fail_fetch: bool,
        fail_verify: bool,
        exits: HashMap<String, i32>,
        cannot_start: Option<String>,
        on_exec: Option<OnExec>,
    }

    impl Deps for Fake {
        fn fetch_text(&self, url: &str) -> Result<String, String> {
            self.calls.borrow_mut().push(format!("fetch {url}"));
            if self.fail_fetch {
                return Err("HTTP 404".to_owned());
            }
            Ok(self.text.clone().unwrap_or_else(|| {
                format!(
                    "{}  pantheon-scaffold@0.2.0\n{}  runner-linux-x64\n",
                    h('c'),
                    h('d')
                )
            }))
        }

        fn verify_attestation(&self, file: &Path, repo: &str) -> Result<(), String> {
            self.calls.borrow_mut().push(format!("verify {repo}"));
            self.verified
                .borrow_mut()
                .push(std::fs::read_to_string(file).unwrap());
            if self.fail_verify {
                Err("no valid attestation".to_owned())
            } else {
                Ok(())
            }
        }

        fn exec(&self, cmd: &[&str], _cwd: &Path) -> Result<i32, String> {
            let line = cmd.join(" ");
            self.calls.borrow_mut().push(format!("exec {line}"));
            if let Some(on_exec) = &self.on_exec {
                on_exec(&line);
            }
            if self.cannot_start.as_deref() == Some(line.as_str()) {
                return Err("No such file or directory".to_owned());
            }
            Ok(self.exits.get(&line).copied().unwrap_or(0))
        }
    }

    fn fake() -> Fake {
        Fake::default()
    }

    fn with_text(text: &str) -> Fake {
        Fake {
            text: Some(text.to_owned()),
            ..Fake::default()
        }
    }

    fn failing(command: &str) -> Fake {
        Fake {
            exits: HashMap::from([(command.to_owned(), 1)]),
            ..Fake::default()
        }
    }

    fn snapshot(root: &Path) -> (String, String, String) {
        (
            get(root, "pantheon.pkl"),
            get(root, "mise.toml"),
            get(root, "mise.lock"),
        )
    }

    fn opts() -> UpgradeOptions {
        UpgradeOptions {
            base_url: Some(BASE.to_owned()),
            lock: LockOptions {
                ci: false,
                ..LockOptions::default()
            },
            ..UpgradeOptions::default()
        }
    }

    fn run(root: &Path, f: &Fake, version: &str) -> Result<UpgradeReport, UpgradeError> {
        upgrade(root, version, &opts(), f)
    }

    fn run_with(
        root: &Path,
        f: &Fake,
        version: &str,
        o: UpgradeOptions,
    ) -> Result<UpgradeReport, UpgradeError> {
        upgrade(
            root,
            version,
            &UpgradeOptions {
                base_url: Some(BASE.to_owned()),
                ..o
            },
            f,
        )
    }

    fn calls(f: &Fake) -> Vec<String> {
        f.calls.borrow().clone()
    }

    fn render_line(root: &Path, config: &str) -> String {
        format!(
            "mise x -- pantheon-scaffold render {config} --root {}",
            root.display()
        )
    }

    #[test]
    fn upgrade_rewrites_the_pins_refreshes_mise_and_re_renders_with_the_new_runner_in_that_order() {
        let dir = repo("0.1.0");
        let f = fake();
        let r = run(dir.path(), &f, "0.2.0").unwrap();
        assert_eq!(get(dir.path(), "pantheon.pkl"), amends("0.2.0", &h('c')));
        assert_eq!(get(dir.path(), "mise.toml"), mise("0.2.0"));
        assert_eq!(
            calls(&f),
            [
                format!("fetch {BASE}/v0.2.0/checksums.txt"),
                "verify pantheon-org/pantheon-scaffold".to_owned(),
                "exec mise install".to_owned(),
                "exec mise lock".to_owned(),
                format!("exec {}", render_line(dir.path(), "pantheon.pkl")),
            ]
        );
        assert_eq!(
            (r.from.as_str(), r.to.as_str(), r.dry_run, r.render_exit),
            ("0.1.0", "0.2.0", false, Some(0))
        );
    }

    #[test]
    fn the_report_lists_what_changed() {
        let dir = repo("0.1.0");
        let r = run(dir.path(), &fake(), "0.2.0").unwrap();
        assert_eq!(
            r.changes,
            [
                format!(
                    "pantheon.pkl: 0.1.0 -> 0.2.0, checksum {}...",
                    &h('c')[..12]
                ),
                format!("mise.toml: {RUNNER_TOOL} 0.1.0 -> 0.2.0"),
            ]
        );
    }

    #[test]
    fn the_attestation_is_checked_on_the_exact_text_that_was_downloaded_before_any_file_is_touched()
    {
        let dir = repo("0.1.0");
        let f = Fake {
            fail_verify: true,
            ..fake()
        };
        let before = snapshot(dir.path());
        let message = run(dir.path(), &f, "0.2.0").unwrap_err().to_string();
        assert_eq!(
            message,
            format!(
                "attestation check failed for {BASE}/v0.2.0/checksums.txt: no valid attestation"
            )
        );
        assert!(f.verified.borrow()[0].contains(&format!("{}  pantheon-scaffold@0.2.0", h('c'))));
        assert_eq!(snapshot(dir.path()), before);
        assert!(!calls(&f).iter().any(|c| c.starts_with("exec")));
    }

    #[test]
    fn the_downloaded_file_is_removed_after_the_check_whether_it_passed_or_not() {
        for fail in [false, true] {
            let dir = repo("0.1.0");
            let seen = std::rc::Rc::new(RefCell::new(None));
            struct Spy(std::rc::Rc<RefCell<Option<std::path::PathBuf>>>, bool);
            impl Deps for Spy {
                fn fetch_text(&self, _: &str) -> Result<String, String> {
                    Ok(format!("{}  pantheon-scaffold@0.2.0\n", "c".repeat(64)))
                }
                fn verify_attestation(&self, file: &Path, _: &str) -> Result<(), String> {
                    *self.0.borrow_mut() = Some(file.to_owned());
                    if self.1 {
                        Err("bad".to_owned())
                    } else {
                        Ok(())
                    }
                }
                fn exec(&self, _: &[&str], _: &Path) -> Result<i32, String> {
                    Ok(0)
                }
            }
            let _ = upgrade(dir.path(), "0.2.0", &opts(), &Spy(seen.clone(), fail));
            let file = seen.borrow().clone().expect("the check ran");
            assert!(file.ends_with("checksums.txt"));
            assert!(!file.exists(), "the file is still there (fail = {fail})");
            assert!(
                !file.parent().unwrap().exists(),
                "the folder is still there (fail = {fail})"
            );
        }
    }

    #[test]
    fn a_checksum_for_the_package_that_is_missing_repeated_or_malformed_stops_the_upgrade_with_nothing_written()
     {
        let texts = [
            format!("{}  runner-linux-x64\n", h('d')),
            format!(
                "{}  pantheon-scaffold@0.2.0\n{}  pantheon-scaffold@0.2.0\n",
                h('c'),
                h('e')
            ),
            "garbage\n".to_owned(),
        ];
        for text in texts {
            let dir = repo("0.1.0");
            let before = snapshot(dir.path());
            assert!(run(dir.path(), &with_text(&text), "0.2.0").is_err());
            assert_eq!(snapshot(dir.path()), before);
        }
    }

    #[test]
    fn a_checksum_problem_names_the_file_and_the_cause() {
        let dir = repo("0.1.0");
        let message = run(
            dir.path(),
            &with_text(&format!("{}  other\n", h('d'))),
            "0.2.0",
        )
        .unwrap_err()
        .to_string();
        assert_eq!(
            message,
            "no checksum for pantheon-scaffold@0.2.0 in the release's checksums file"
        );
    }

    #[test]
    fn a_download_that_fails_stops_the_upgrade_closed() {
        let dir = repo("0.1.0");
        let before = snapshot(dir.path());
        let f = Fake {
            fail_fetch: true,
            ..fake()
        };
        let message = run(dir.path(), &f, "0.2.0").unwrap_err().to_string();
        assert_eq!(
            message,
            format!("could not fetch {BASE}/v0.2.0/checksums.txt: HTTP 404")
        );
        assert_eq!(snapshot(dir.path()), before);
    }

    #[test]
    fn when_mise_install_fails_every_file_is_put_back_exactly_as_it_was() {
        let dir = repo("0.1.0");
        let before = snapshot(dir.path());
        let message = run(dir.path(), &failing("mise install"), "0.2.0")
            .unwrap_err()
            .to_string();
        assert_eq!(
            message,
            "mise install failed (exit 1), the pins and the lock file were put back"
        );
        assert_eq!(snapshot(dir.path()), before);
    }

    #[test]
    fn when_mise_lock_fails_the_lock_file_mise_rewrote_is_restored_too() {
        let dir = repo("0.1.0");
        let before = snapshot(dir.path());
        let root = dir.path().to_owned();
        let f = Fake {
            exits: HashMap::from([("mise lock".to_owned(), 1)]),
            on_exec: Some(Box::new(move |c| {
                if c == "mise lock" {
                    put(&root, "mise.lock", "# half written\n");
                }
            })),
            ..fake()
        };
        let message = run(dir.path(), &f, "0.2.0").unwrap_err().to_string();
        assert_eq!(
            message,
            "mise lock failed (exit 1), the pins and the lock file were put back"
        );
        assert_eq!(snapshot(dir.path()), before);
    }

    #[test]
    fn a_lock_file_that_did_not_exist_before_is_removed_again_on_failure() {
        let dir = repo("0.1.0");
        std::fs::remove_file(dir.path().join("mise.lock")).unwrap();
        let root = dir.path().to_owned();
        let f = Fake {
            exits: HashMap::from([("mise lock".to_owned(), 1)]),
            on_exec: Some(Box::new(move |c| {
                if c == "mise lock" {
                    put(&root, "mise.lock", "# new\n");
                }
            })),
            ..fake()
        };
        assert!(run(dir.path(), &f, "0.2.0").is_err());
        assert!(!has(dir.path(), "mise.lock"));
    }

    #[test]
    fn a_file_that_cannot_be_put_back_is_named_in_the_error() {
        let dir = repo("0.1.0");
        let root = dir.path().to_owned();
        let f = Fake {
            exits: HashMap::from([("mise lock".to_owned(), 1)]),
            on_exec: Some(Box::new(move |c| {
                if c == "mise lock" {
                    // mise.lock becomes a folder, so writing the old text back to it fails.
                    std::fs::remove_file(root.join("mise.lock")).unwrap();
                    std::fs::create_dir(root.join("mise.lock")).unwrap();
                }
            })),
            ..fake()
        };
        let message = run(dir.path(), &f, "0.2.0").unwrap_err().to_string();
        assert!(
            message.starts_with(
                "mise lock failed (exit 1), the pins and the lock file were put back; restoring "
            ),
            "{message}"
        );
        assert!(message.contains("mise.lock failed:"), "{message}");
        assert_eq!(
            get(dir.path(), "mise.toml"),
            mise("0.1.0"),
            "the other files were still put back"
        );
    }

    #[test]
    fn a_file_that_did_not_exist_and_still_does_not_is_not_a_failed_restore() {
        let dir = repo("0.1.0");
        std::fs::remove_file(dir.path().join("mise.lock")).unwrap();
        let message = run(dir.path(), &failing("mise install"), "0.2.0")
            .unwrap_err()
            .to_string();
        assert_eq!(
            message,
            "mise install failed (exit 1), the pins and the lock file were put back"
        );
        assert!(!has(dir.path(), "mise.lock"));
    }

    #[test]
    fn a_file_that_did_not_exist_and_cannot_be_removed_is_named_in_the_error() {
        let dir = repo("0.1.0");
        std::fs::remove_file(dir.path().join("mise.lock")).unwrap();
        let root = dir.path().to_owned();
        let f = Fake {
            exits: HashMap::from([("mise lock".to_owned(), 1)]),
            on_exec: Some(Box::new(move |c| {
                if c == "mise lock" {
                    // mise.lock appears as a folder, which removing a file cannot remove.
                    std::fs::create_dir(root.join("mise.lock")).unwrap();
                }
            })),
            ..fake()
        };
        let message = run(dir.path(), &f, "0.2.0").unwrap_err().to_string();
        assert!(
            message.starts_with(
                "mise lock failed (exit 1), the pins and the lock file were put back; restoring "
            ),
            "{message}"
        );
        assert!(message.contains("mise.lock failed:"), "{message}");
    }

    #[test]
    fn mise_that_cannot_be_started_puts_everything_back_too() {
        let dir = repo("0.1.0");
        let before = snapshot(dir.path());
        let f = Fake {
            cannot_start: Some("mise install".to_owned()),
            ..fake()
        };
        let message = run(dir.path(), &f, "0.2.0").unwrap_err().to_string();
        assert_eq!(
            message,
            "mise install could not be started (No such file or directory), the pins and the lock file were put back"
        );
        assert_eq!(snapshot(dir.path()), before);
    }

    #[test]
    fn a_render_that_fails_afterwards_leaves_the_new_pins_in_place_and_reports_its_exit_code() {
        let dir = repo("0.1.0");
        let f = Fake {
            exits: HashMap::from([(render_line(dir.path(), "pantheon.pkl"), 3)]),
            ..fake()
        };
        let r = run(dir.path(), &f, "0.2.0").unwrap();
        assert_eq!(r.render_exit, Some(3));
        assert_eq!(get(dir.path(), "mise.toml"), mise("0.2.0"));
        assert_eq!(get(dir.path(), "pantheon.pkl"), amends("0.2.0", &h('c')));
    }

    #[test]
    fn a_render_that_cannot_be_started_is_an_error_that_says_the_pins_are_updated() {
        let dir = repo("0.1.0");
        let f = Fake {
            cannot_start: Some(render_line(dir.path(), "pantheon.pkl")),
            ..fake()
        };
        let message = run(dir.path(), &f, "0.2.0").unwrap_err().to_string();
        assert_eq!(
            message,
            "the pins are updated, but the render could not be started (No such file or directory)"
        );
        assert_eq!(get(dir.path(), "mise.toml"), mise("0.2.0"));
    }

    #[test]
    fn no_render_stops_after_mise() {
        let dir = repo("0.1.0");
        let f = fake();
        let r = run_with(
            dir.path(),
            &f,
            "0.2.0",
            UpgradeOptions {
                no_render: true,
                ..opts()
            },
        )
        .unwrap();
        let execs: Vec<_> = calls(&f)
            .into_iter()
            .filter(|c| c.starts_with("exec"))
            .collect();
        assert_eq!(execs, ["exec mise install", "exec mise lock"]);
        assert_eq!(r.render_exit, None);
    }

    #[test]
    fn dry_run_downloads_and_verifies_lists_what_would_change_and_writes_and_runs_nothing() {
        let dir = repo("0.1.0");
        let before = snapshot(dir.path());
        let f = fake();
        let r = run_with(
            dir.path(),
            &f,
            "0.2.0",
            UpgradeOptions {
                dry_run: true,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(snapshot(dir.path()), before);
        let kinds: Vec<_> = calls(&f)
            .iter()
            .map(|c| c.split(' ').next().unwrap().to_owned())
            .collect();
        assert_eq!(kinds, ["fetch", "verify"]);
        assert!(r.dry_run);
        assert!(r.changes.join("\n").contains("0.1.0 -> 0.2.0"));
        assert_eq!(r.render_exit, None);
        assert!(!has(dir.path(), ".pkl-gen"), "a dry run takes no lock");
    }

    #[test]
    fn the_same_version_is_refused_and_an_older_one_needs_force() {
        let dir = repo("0.2.0");
        assert_eq!(
            run(dir.path(), &fake(), "0.2.0").unwrap_err().to_string(),
            "already at 0.2.0"
        );
        let older = repo("0.3.0");
        assert_eq!(
            run(older.path(), &fake(), "0.2.0").unwrap_err().to_string(),
            "0.2.0 is older than 0.3.0, which is pinned now (use --force to go back)"
        );
        let forced = run_with(
            older.path(),
            &fake(),
            "0.2.0",
            UpgradeOptions {
                force: true,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(forced.to, "0.2.0");
    }

    #[test]
    fn a_pre_release_is_an_exact_pin_like_any_version_and_a_leading_v_is_accepted() {
        let dir = repo("0.1.0");
        let f = with_text(&format!("{}  pantheon-scaffold@0.2.0-rc.1\n", h('c')));
        let r = run(dir.path(), &f, "v0.2.0-rc.1").unwrap();
        assert_eq!(r.to, "0.2.0-rc.1");
        assert!(get(dir.path(), "pantheon.pkl").contains("pantheon-scaffold@0.2.0-rc.1::sha256:"));
        assert!(calls(&f)[0].ends_with("/v0.2.0-rc.1/checksums.txt"));
    }

    #[test]
    fn a_version_that_is_not_a_version_is_refused_before_anything_is_fetched() {
        let f = fake();
        let message = run(repo("0.1.0").path(), &f, "latest")
            .unwrap_err()
            .to_string();
        assert_eq!(message, "\"latest\" is not a version");
        assert!(calls(&f).is_empty());
    }

    #[test]
    fn a_repository_whose_pins_cannot_be_found_is_an_error_before_anything_is_fetched() {
        let dir = repo("0.1.0");
        put(dir.path(), "mise.toml", "[tools]\nbun = \"1.4.2\"\n");
        let f = fake();
        assert!(
            run(dir.path(), &f, "0.2.0")
                .unwrap_err()
                .to_string()
                .contains(RUNNER_TOOL)
        );
        assert!(calls(&f).is_empty());
    }

    #[test]
    fn a_pinned_version_that_is_not_a_version_is_an_error_not_a_crash() {
        let dir = repo("0.1.0");
        put(dir.path(), "pantheon.pkl", &amends("weird", &h('a')));
        let message = run(dir.path(), &fake(), "0.2.0").unwrap_err().to_string();
        assert_eq!(message, "\"weird\" is not a version");
    }

    #[test]
    fn a_custom_config_file_name_is_used_for_the_pins_and_the_render() {
        let dir = repo("0.1.0");
        put(dir.path(), "other.pkl", &get(dir.path(), "pantheon.pkl"));
        let f = fake();
        run_with(
            dir.path(),
            &f,
            "0.2.0",
            UpgradeOptions {
                config: Some("other.pkl".to_owned()),
                ..opts()
            },
        )
        .unwrap();
        assert!(get(dir.path(), "other.pkl").contains("v0.2.0"));
        assert!(calls(&f).last().unwrap().contains("render other.pkl"));
    }

    fn hold_the_lock(root: &Path) {
        let started = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        put(
            root,
            ".pkl-gen/lock",
            &format!(
                "{{\"pid\":{},\"started\":\"{started}\"}}",
                std::process::id()
            ),
        );
    }

    #[test]
    fn an_upgrade_refuses_while_another_run_holds_the_lock() {
        let dir = repo("0.1.0");
        hold_the_lock(dir.path());
        let f = fake();
        let message = run(dir.path(), &f, "0.2.0").unwrap_err().to_string();
        assert!(message.contains("is held by process"), "{message}");
        assert!(!calls(&f).iter().any(|c| c.starts_with("exec")));
        assert_eq!(get(dir.path(), "mise.toml"), mise("0.1.0"));
    }

    #[test]
    fn the_lock_is_released_before_the_new_runner_renders_so_its_own_render_can_take_it() {
        let dir = repo("0.1.0");
        let locked = std::rc::Rc::new(RefCell::new(None));
        let (seen, root) = (locked.clone(), dir.path().to_owned());
        let f = Fake {
            on_exec: Some(Box::new(move |c| {
                if c.contains("render") {
                    *seen.borrow_mut() = Some(root.join(".pkl-gen/lock").exists());
                }
            })),
            ..fake()
        };
        run(dir.path(), &f, "0.2.0").unwrap();
        assert_eq!(*locked.borrow(), Some(false));
        assert!(!has(dir.path(), ".pkl-gen/lock"));
    }

    #[test]
    fn the_lock_is_released_after_a_failure_too() {
        let dir = repo("0.1.0");
        assert!(run(dir.path(), &failing("mise install"), "0.2.0").is_err());
        assert!(!has(dir.path(), ".pkl-gen/lock"));
    }

    // What mise lock records for the runner is checked after it runs. Both locks are real output for the published v0.1.1.
    const ALL: &str = include_str!("../tests/data/mise-lock-all-platforms.lock");
    const THREE: &str = include_str!("../tests/data/mise-lock-three-platforms.lock");

    fn lock_writes(root: &Path, text: &'static str) -> Option<OnExec> {
        let root = root.to_owned();
        Some(Box::new(move |c| {
            if c == "mise lock" {
                put(&root, "mise.lock", text);
            }
        }))
    }

    #[test]
    fn a_lock_that_names_platforms_the_runner_is_not_built_for_fails_the_upgrade_and_puts_everything_back()
     {
        let dir = repo("0.1.0");
        let before = snapshot(dir.path());
        let f = Fake {
            on_exec: lock_writes(dir.path(), ALL),
            ..fake()
        };
        let message = run(dir.path(), &f, "0.2.0").unwrap_err().to_string();
        assert!(message.starts_with("mise lock recorded the wrong runner for some platforms, so the pins and the lock file were put back:\n  "), "{message}");
        for needle in ["windows-x64", "macos-x64", "lockfile_platforms"] {
            assert!(message.contains(needle), "{needle}: {message}");
        }
        assert!(
            message.ends_with(
                "lockfile_platforms = [\"linux-x64\", \"linux-arm64\", \"macos-arm64\"]"
            ),
            "{message}"
        );
        assert_eq!(snapshot(dir.path()), before);
        assert!(!calls(&f).iter().any(|c| c.contains("render")));
    }

    #[test]
    fn a_lock_limited_to_the_supported_platforms_passes_and_is_kept() {
        let dir = repo("0.1.0");
        let f = Fake {
            on_exec: lock_writes(dir.path(), THREE),
            ..fake()
        };
        let r = run(dir.path(), &f, "0.2.0").unwrap();
        assert_eq!(r.render_exit, Some(0));
        assert_eq!(get(dir.path(), "mise.lock"), THREE);
    }

    #[test]
    fn a_lock_with_no_entry_for_the_runner_is_not_an_error() {
        let dir = repo("0.1.0");
        assert_eq!(
            run(dir.path(), &fake(), "0.2.0").unwrap().render_exit,
            Some(0)
        );
    }

    #[test]
    fn a_repository_with_no_lock_file_after_mise_is_not_an_error() {
        let dir = repo("0.1.0");
        std::fs::remove_file(dir.path().join("mise.lock")).unwrap();
        assert_eq!(
            run(dir.path(), &fake(), "0.2.0").unwrap().render_exit,
            Some(0)
        );
    }

    fn import_line(v: &str, sum: &str) -> String {
        format!(
            "import \"package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{v}/pantheon-scaffold@{v}::sha256:{sum}#/Builtins.pkl\""
        )
    }

    #[test]
    fn upgrade_moves_every_import_of_the_package_with_the_amends_line_and_says_so() {
        let dir = repo("0.1.0");
        let first_line = amends("0.1.0", &h('a')).lines().next().unwrap().to_owned();
        put(
            dir.path(),
            "pantheon.pkl",
            &format!(
                "{first_line}\n{}\n\noptions {{ toolchain = \"1.96.1\" }}\n",
                import_line("0.1.0", &h('a'))
            ),
        );
        let r = run(dir.path(), &fake(), "0.2.0").unwrap();
        assert!(get(dir.path(), "pantheon.pkl").contains(&import_line("0.2.0", &h('c'))));
        assert!(!get(dir.path(), "pantheon.pkl").contains("0.1.0"));
        assert!(
            r.changes[0].ends_with(", 1 import of the package"),
            "{}",
            r.changes[0]
        );
    }

    #[test]
    fn two_imports_are_counted_in_the_plural() {
        let dir = repo("0.1.0");
        let first_line = amends("0.1.0", &h('a')).lines().next().unwrap().to_owned();
        let two = format!(
            "{first_line}\n{}\n{}\n",
            import_line("0.1.0", &h('a')),
            import_line("0.1.0", &h('a')).replace("Builtins", "Tools")
        );
        put(dir.path(), "pantheon.pkl", &two);
        let r = run_with(
            dir.path(),
            &fake(),
            "0.2.0",
            UpgradeOptions {
                dry_run: true,
                ..opts()
            },
        )
        .unwrap();
        assert!(
            r.changes[0].ends_with(", 2 imports of the package"),
            "{}",
            r.changes[0]
        );
    }

    #[test]
    fn a_dry_run_lists_the_imports_it_would_move_and_changes_nothing() {
        let dir = repo("0.1.0");
        let first_line = amends("0.1.0", &h('a')).lines().next().unwrap().to_owned();
        let text = format!("{first_line}\n{}\n", import_line("0.1.0", &h('a')));
        put(dir.path(), "pantheon.pkl", &text);
        let r = run_with(
            dir.path(),
            &fake(),
            "0.2.0",
            UpgradeOptions {
                dry_run: true,
                ..opts()
            },
        )
        .unwrap();
        assert!(r.changes[0].contains("1 import"));
        assert_eq!(get(dir.path(), "pantheon.pkl"), text);
    }

    fn manifest_of(root: &Path, files: &[(&str, FileClass, String)]) {
        let mut manifest = Manifest {
            package_version: Some("0.1.0".into()),
            ..Manifest::empty()
        };
        for (path, class, hash) in files {
            manifest.files.insert(
                (*path).to_owned(),
                ManifestEntry {
                    class: *class,
                    hash: Some(hash.clone()),
                    remove_on_detach: false,
                },
            );
        }
        put(
            root,
            ".pkl-gen/manifest.json",
            &serialise_manifest(&manifest),
        );
    }

    fn read_hash(root: &Path, path: &str) -> Option<String> {
        crate::manifest::read_manifest(root)
            .unwrap()
            .files
            .get(path)
            .and_then(|e| e.hash.clone())
    }

    #[test]
    fn the_runner_pin_upgrade_writes_into_an_owned_mise_toml_is_recorded_in_the_manifest_so_the_render_that_follows_does_not_read_it_as_a_hand_edit()
     {
        let dir = repo("0.1.0");
        manifest_of(
            dir.path(),
            &[
                (
                    "mise.toml",
                    FileClass::Owned,
                    sha256(&get(dir.path(), "mise.toml")),
                ),
                ("other.txt", FileClass::Owned, h('e')),
            ],
        );
        run(dir.path(), &fake(), "0.2.0").unwrap();
        assert_eq!(
            read_hash(dir.path(), "mise.toml"),
            Some(sha256(&get(dir.path(), "mise.toml")))
        );
        assert!(get(dir.path(), "mise.toml").contains(&format!("\"{RUNNER_TOOL}\" = \"0.2.0\"")));
        assert_eq!(read_hash(dir.path(), "other.txt"), Some(h('e')));
        assert_eq!(
            crate::manifest::read_manifest(dir.path())
                .unwrap()
                .package_version
                .as_deref(),
            Some("0.1.0")
        );
    }

    #[test]
    fn a_mise_toml_that_was_edited_by_hand_keeps_its_old_hash_so_the_render_still_refuses_it() {
        let dir = repo("0.1.0");
        manifest_of(
            dir.path(),
            &[(
                "mise.toml",
                FileClass::Owned,
                sha256(&get(dir.path(), "mise.toml")),
            )],
        );
        let old = read_hash(dir.path(), "mise.toml");
        put(
            dir.path(),
            "mise.toml",
            &format!("{}# a hand edit\n", get(dir.path(), "mise.toml")),
        );
        run(dir.path(), &fake(), "0.2.0").unwrap();
        assert_eq!(read_hash(dir.path(), "mise.toml"), old);
    }

    #[test]
    fn a_repository_with_no_manifest_or_a_mise_toml_the_package_does_not_own_gets_no_manifest_written()
     {
        let none = repo("0.1.0");
        run(none.path(), &fake(), "0.2.0").unwrap();
        assert!(!has(none.path(), ".pkl-gen/manifest.json"));
        let seeded = repo("0.1.0");
        manifest_of(
            seeded.path(),
            &[(
                "mise.toml",
                FileClass::Seed,
                sha256(&get(seeded.path(), "mise.toml")),
            )],
        );
        let before = get(seeded.path(), ".pkl-gen/manifest.json");
        run(seeded.path(), &fake(), "0.2.0").unwrap();
        assert_eq!(get(seeded.path(), ".pkl-gen/manifest.json"), before);
    }

    #[test]
    fn when_mise_lock_fails_the_manifest_is_put_back_with_the_other_files() {
        let dir = repo("0.1.0");
        manifest_of(
            dir.path(),
            &[(
                "mise.toml",
                FileClass::Owned,
                sha256(&get(dir.path(), "mise.toml")),
            )],
        );
        let manifest = get(dir.path(), ".pkl-gen/manifest.json");
        assert!(run(dir.path(), &failing("mise lock"), "0.2.0").is_err());
        assert_eq!(get(dir.path(), ".pkl-gen/manifest.json"), manifest);
    }

    #[test]
    fn a_manifest_that_cannot_be_read_stops_the_upgrade_before_anything_is_written() {
        let dir = repo("0.1.0");
        put(dir.path(), ".pkl-gen/manifest.json", "<<<<<<< x\n");
        let before = snapshot(dir.path());
        let f = fake();
        assert!(
            run(dir.path(), &f, "0.2.0")
                .unwrap_err()
                .to_string()
                .contains("--repair")
        );
        assert_eq!(snapshot(dir.path()), before);
        assert!(!calls(&f).iter().any(|c| c.starts_with("exec")));
    }

    // The real dependencies, with `gh` and `mise` replaced by small scripts that record what they were asked.
    fn script(dir: &Path, name: &str, body: &str) -> OsString {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.into_os_string()
    }

    #[test]
    fn the_system_attestation_check_runs_gh_with_the_file_and_the_repository() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("log");
        let deps = SystemDeps {
            gh: script(
                dir.path(),
                "gh",
                &format!("echo \"$@\" > '{}'", log.display()),
            ),
            ..SystemDeps::default()
        };
        assert_eq!(
            deps.verify_attestation(Path::new("/x/checksums.txt"), "o/r"),
            Ok(())
        );
        assert_eq!(
            std::fs::read_to_string(log).unwrap().trim(),
            "attestation verify /x/checksums.txt --repo o/r"
        );
    }

    #[test]
    fn a_failed_attestation_check_gives_what_gh_said() {
        let dir = tempfile::tempdir().unwrap();
        let deps = SystemDeps {
            gh: script(dir.path(), "gh", "echo 'no valid attestation' >&2; exit 1"),
            ..SystemDeps::default()
        };
        assert_eq!(
            deps.verify_attestation(Path::new("f"), "o/r"),
            Err("no valid attestation".to_owned())
        );
        let silent = SystemDeps {
            gh: script(dir.path(), "gh2", "exit 4"),
            ..SystemDeps::default()
        };
        assert_eq!(
            silent.verify_attestation(Path::new("f"), "o/r"),
            Err("gh attestation verify exited 4".to_owned())
        );
        let missing = SystemDeps {
            gh: dir.path().join("nope").into_os_string(),
            ..SystemDeps::default()
        };
        assert!(
            missing
                .verify_attestation(Path::new("f"), "o/r")
                .unwrap_err()
                .contains("could not run")
        );
    }

    #[test]
    fn the_system_exec_runs_the_program_in_the_folder_and_returns_its_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("log");
        let mise = script(
            dir.path(),
            "mise",
            &format!(
                "echo \"$PWD|$@\" >> '{0}'; [ \"$1\" = lock ] && exit 7; exit 0",
                log.display()
            ),
        );
        let deps = SystemDeps {
            mise,
            ..SystemDeps::default()
        };
        let work = tempfile::tempdir().unwrap();
        assert_eq!(deps.exec(&["mise", "install"], work.path()), Ok(0));
        assert_eq!(deps.exec(&["mise", "lock"], work.path()), Ok(7));
        let lines: Vec<String> = std::fs::read_to_string(log)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        let here = work.path().canonicalize().unwrap().display().to_string();
        assert_eq!(lines, [format!("{here}|install"), format!("{here}|lock")]);
    }

    #[test]
    fn a_program_that_cannot_be_started_is_an_error_and_a_program_killed_by_a_signal_is_minus_one()
    {
        let dir = tempfile::tempdir().unwrap();
        let missing = SystemDeps {
            mise: dir.path().join("nope").into_os_string(),
            ..SystemDeps::default()
        };
        assert!(missing.exec(&["mise", "x"], dir.path()).is_err());
        let killed = SystemDeps {
            mise: script(dir.path(), "mise", "kill -9 $$"),
            ..SystemDeps::default()
        };
        assert_eq!(killed.exec(&["mise", "x"], dir.path()), Ok(-1));
    }

    #[test]
    fn the_system_fetch_reads_a_url_and_reports_a_status() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for (status, body) in [(200, "hello"), (404, "gone")] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let text = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(text.as_bytes());
            }
        });
        let deps = SystemDeps::default();
        assert_eq!(
            deps.fetch_text(&format!("http://{address}/a")),
            Ok("hello".to_owned())
        );
        assert_eq!(
            deps.fetch_text(&format!("http://{address}/b")),
            Err("HTTP 404".to_owned())
        );
    }
}
