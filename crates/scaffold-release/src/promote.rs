//! Promotion of a pre-release to a stable release. Ported from `runner/src/promote.ts` and `runner/scripts/promote.ts`
//! (Phase 2 of `docs/ts-tooling-rust-port.md`).
//!
//! Every guard runs before anything is edited, and each is recorded. The edit is the flag, the title and the notes: the assets are
//! never touched. After it the same checks run again, and the checksums file must be byte for byte the one that was attested before
//! the edit. The guards, their order, the calls made to `gh` and the report lines follow the TypeScript, so the two can be compared
//! (`runner/scripts/promote-diff.sh`).

use std::cell::Cell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use scaffold_core::checksums::{find_checksum, parse_checksums};
use scaffold_core::knownbad::parse_known_bad;
use scaffold_core::preflight::KnownBad;
use serde_json::Value;

use crate::{asset_names, json, sha256, tag_version};

/// What the promotion needs to know about a release, as `gh release view` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub is_prerelease: bool,
    pub is_draft: bool,
    pub assets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunJob {
    pub name: String,
    pub conclusion: String,
}

/// The conclusion of the release workflow run for the tag push, and of each of its jobs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseRun {
    pub conclusion: String,
    pub jobs: Vec<RunJob>,
}

/// Everything the promotion reaches outside itself for. Each can be replaced, which is how the tests run without a release.
pub trait PromoteDeps {
    fn view_release(&self, tag: &str) -> Result<ReleaseInfo, String>;
    fn download_asset(&self, tag: &str, name: &str) -> Result<Vec<u8>, String>;
    /// Fails unless the bytes have a valid build provenance attestation from the repository.
    fn verify_attestation(&self, name: &str, bytes: &[u8]) -> Result<(), String>;
    fn known_bad(&self) -> Result<KnownBad, String>;
    /// The release workflow run for the tag push. `None` when no finished run is found.
    fn release_run(&self, tag: &str) -> Result<Option<ReleaseRun>, String>;
    fn latest_tag(&self) -> Option<String>;
    fn edit(&self, tag: &str, title: &str, notes: &str) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub guard: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteReport {
    pub tag: String,
    pub dry_run: bool,
    pub steps: Vec<Step>,
    pub promoted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// A guard refused. Its step is the last of the report.
    Refused {
        message: String,
        report: PromoteReport,
    },
    /// Something other than a guard went wrong (a `gh` call failed, a checksums line could not be read).
    Other(String),
}

impl Failure {
    pub fn message(&self) -> &str {
        match self {
            Failure::Refused { message, .. } | Failure::Other(message) => message,
        }
    }
}

/// The title and notes of a stable release: the pre-release wording of the release workflow, without the word.
pub fn stable_wording(info: &ReleaseInfo) -> (String, String) {
    static LABEL: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"\s*\(pre-release\)\s*$").expect("valid"));
    let title = LABEL.replace(&info.name, "").into_owned();
    let notes = match info.body.strip_prefix("Pre-release of") {
        Some(rest) => format!("Release of{rest}"),
        None => info.body.clone(),
    };
    (title, notes)
}

struct Guards<'a> {
    report: PromoteReport,
    deps: &'a dyn PromoteDeps,
}

impl Guards<'_> {
    fn pass(&mut self, guard: &str, detail: impl Into<String>) {
        self.report.steps.push(Step {
            guard: guard.to_owned(),
            ok: true,
            detail: detail.into(),
        });
    }

    fn fail(&mut self, guard: &str, detail: impl Into<String>) -> Failure {
        let detail = detail.into();
        self.report.steps.push(Step {
            guard: guard.to_owned(),
            ok: false,
            detail: detail.clone(),
        });
        Failure::Refused {
            message: format!("{guard}: {detail}"),
            report: self.report.clone(),
        }
    }

    /// The checks that run before the edit and again after it. Returns the bytes of the attested `checksums.txt`.
    fn checks(&mut self, tag: &str, version: &str) -> Result<Vec<u8>, Failure> {
        let info = self.deps.view_release(tag).map_err(Failure::Other)?;
        if info.is_draft {
            return Err(self.fail("release state", format!("{tag} is a draft")));
        }
        let expected = asset_names(version);
        let mut got = info.assets.clone();
        got.sort();
        if got != expected {
            return Err(self.fail(
                "assets",
                format!("expected {}, found {}", expected.join(", "), got.join(", ")),
            ));
        }
        let checksums = self
            .deps
            .download_asset(tag, "checksums.txt")
            .map_err(Failure::Other)?;
        if let Err(e) = self.deps.verify_attestation("checksums.txt", &checksums) {
            return Err(self.fail("attestation", format!("checksums.txt: {e}")));
        }
        let text = String::from_utf8_lossy(&checksums).into_owned();
        for entry in parse_checksums(&text).map_err(Failure::Other)? {
            if !info.assets.contains(&entry.name) {
                return Err(self.fail(
                    "digests",
                    format!(
                        "checksums.txt lists {}, which the release does not carry",
                        entry.name
                    ),
                ));
            }
            let bytes = self
                .deps
                .download_asset(tag, &entry.name)
                .map_err(Failure::Other)?;
            let listed = find_checksum(&text, &entry.name).map_err(Failure::Other)?;
            if sha256(&bytes) != listed {
                return Err(self.fail(
                    "digests",
                    format!("{} does not match checksums.txt", entry.name),
                ));
            }
        }
        Ok(checksums)
    }
}

/// Promotion of a pre-release. `git_ref` is the ref the workflow ran on, and a promotion runs only from `refs/heads/main`.
pub fn promote(
    tag: &str,
    git_ref: &str,
    deps: &dyn PromoteDeps,
    dry_run: bool,
) -> Result<PromoteReport, Failure> {
    let mut g = Guards {
        report: PromoteReport {
            tag: tag.to_owned(),
            dry_run,
            steps: Vec::new(),
            promoted: false,
        },
        deps,
    };

    if git_ref != "refs/heads/main" {
        return Err(g.fail(
            "run from main",
            format!(
                "the workflow ran on {}, a promotion runs only from refs/heads/main",
                json(git_ref)
            ),
        ));
    }
    g.pass("run from main", git_ref);

    let Some(version) = tag_version(tag) else {
        return Err(g.fail(
            "tag shape",
            format!(
                "{} is not a release tag (expected v<major>.<minor>.<patch>[-pre-release])",
                json(tag)
            ),
        ));
    };
    g.pass("tag shape", format!("version {version}"));

    let before = deps.view_release(tag).map_err(Failure::Other)?;
    if !before.is_prerelease {
        return Err(g.fail(
            "release state",
            format!(
                "{tag} is already a stable release: nothing to promote, and a published version is never re-published"
            ),
        ));
    }
    g.pass("release state", format!("{tag} is a pre-release"));
    let attested = g.checks(tag, &version)?;
    g.pass(
        "assets, attestation and digests",
        format!(
            "{} assets, checksums.txt attested, every listed digest matches",
            before.assets.len()
        ),
    );

    let known_bad = deps.known_bad().map_err(Failure::Other)?;
    if let Some(bad) = known_bad.versions.iter().find(|v| v.version == version) {
        return Err(g.fail(
            "known bad",
            format!("{version} is on the known-bad list: {}", bad.reason),
        ));
    }
    g.pass("known bad", format!("{version} is not on the list"));

    let Some(run) = deps.release_run(tag).map_err(Failure::Other)? else {
        return Err(g.fail(
            "release run",
            format!("no run of the release workflow found for {tag}"),
        ));
    };
    let unsuccessful: Vec<String> = std::iter::once(("run", run.conclusion.as_str()))
        .chain(
            run.jobs
                .iter()
                .map(|j| (j.name.as_str(), j.conclusion.as_str())),
        )
        .filter(|(_, conclusion)| *conclusion != "success")
        .map(|(name, conclusion)| format!("{name} ({conclusion})"))
        .collect();
    if !unsuccessful.is_empty() {
        return Err(g.fail(
            "release run",
            format!("not successful: {}", unsuccessful.join(", ")),
        ));
    }
    if !run
        .jobs
        .iter()
        .any(|j| j.name.starts_with("Verify the published release"))
    {
        return Err(g.fail("release run", "the run has no verify-release job"));
    }
    g.pass(
        "release run",
        format!(
            "the release workflow succeeded for {tag}, {} jobs",
            run.jobs.len()
        ),
    );

    if dry_run {
        return Ok(g.report);
    }

    let (title, notes) = stable_wording(&before);
    deps.edit(tag, &title, &notes).map_err(Failure::Other)?;
    let after = deps.view_release(tag).map_err(Failure::Other)?;
    if after.is_prerelease {
        return Err(g.fail("after the edit", format!("{tag} is still a pre-release")));
    }
    let after_checksums = g.checks(tag, &version)?;
    if after_checksums != attested {
        return Err(g.fail(
            "after the edit",
            "checksums.txt is not byte for byte the one that was attested before the edit",
        ));
    }
    let latest = deps.latest_tag();
    if latest.as_deref() != Some(tag) {
        let shown = latest
            .as_deref()
            .map_or_else(|| "undefined".to_owned(), json);
        return Err(g.fail(
            "after the edit",
            format!("the latest release is {shown}, expected {tag}"),
        ));
    }
    g.pass(
        "after the edit",
        "stable and latest, the assets and checksums.txt are unchanged and still attested",
    );
    g.report.promoted = true;
    Ok(g.report)
}

/// The real calls: the `gh` command line, with the arguments the TypeScript passed.
pub struct Gh {
    program: PathBuf,
    repo: String,
    root: PathBuf,
    work: PathBuf,
    counter: Cell<u32>,
}

impl Gh {
    pub fn new(program: impl Into<PathBuf>, repo: &str, root: &Path) -> Result<Gh, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let work =
            std::env::temp_dir().join(format!("promote-{:x}{:x}", std::process::id(), stamp));
        std::fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
        Ok(Gh {
            program: program.into(),
            repo: repo.to_owned(),
            root: root.to_owned(),
            work,
            counter: Cell::new(0),
        })
    }

    fn subdir(&self, kind: &str) -> Result<PathBuf, String> {
        self.counter.set(self.counter.get() + 1);
        let dir = self.work.join(format!("{kind}-{:x}", self.counter.get()));
        std::fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(dir)
    }

    /// Runs `gh`, returning its exit code and its output without the trailing line break.
    fn gh(&self, args: &[&str]) -> Result<(i32, String, String), String> {
        let output = Command::new(&self.program)
            .args(args)
            .output()
            .map_err(|e| format!("gh {}: {e}", args.first().unwrap_or(&"")))?;
        Ok((
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }

    fn need(&self, args: &[&str]) -> Result<String, String> {
        let (code, out, err) = self.gh(args)?;
        if code != 0 {
            return Err(format!(
                "gh {} {} failed (exit {code}): {err}",
                args[0],
                args.get(1).unwrap_or(&"")
            ));
        }
        Ok(out)
    }
}

impl Drop for Gh {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.work);
    }
}

fn text(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("gh returned no {key}"))
}

fn flag(value: &Value, key: &str) -> Result<bool, String> {
    value
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("gh returned no {key}"))
}

impl PromoteDeps for Gh {
    fn view_release(&self, tag: &str) -> Result<ReleaseInfo, String> {
        let out = self.need(&[
            "release",
            "view",
            tag,
            "--repo",
            &self.repo,
            "--json",
            "tagName,name,body,isPrerelease,isDraft,assets",
        ])?;
        let v: Value = serde_json::from_str(&out).map_err(|e| format!("gh release view: {e}"))?;
        let assets = v
            .get("assets")
            .and_then(Value::as_array)
            .ok_or("gh returned no assets")?
            .iter()
            .map(|a| text(a, "name"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ReleaseInfo {
            tag_name: text(&v, "tagName")?,
            name: text(&v, "name")?,
            body: text(&v, "body")?,
            is_prerelease: flag(&v, "isPrerelease")?,
            is_draft: flag(&v, "isDraft")?,
            assets,
        })
    }

    fn download_asset(&self, tag: &str, name: &str) -> Result<Vec<u8>, String> {
        let dir = self.subdir("dl")?;
        let dir_text = dir.to_string_lossy().into_owned();
        self.need(&[
            "release",
            "download",
            tag,
            "--repo",
            &self.repo,
            "--pattern",
            name,
            "--dir",
            &dir_text,
        ])?;
        let file = dir.join(name);
        std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))
    }

    fn verify_attestation(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let dir = self.subdir("at")?;
        let file = dir.join(name);
        std::fs::write(&file, bytes).map_err(|e| format!("{}: {e}", file.display()))?;
        let path = file.to_string_lossy().into_owned();
        self.need(&["attestation", "verify", &path, "--repo", &self.repo])
            .map(|_| ())
    }

    fn known_bad(&self) -> Result<KnownBad, String> {
        let file = self.root.join("known-bad.json");
        let text =
            std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        // The list is read from the checkout and an invalid one stops the promotion. The consumer's loader, which falls back to a
        // bundled copy, is not used here: a stale list must never let a known-bad version through.
        parse_known_bad(&text)
            .ok_or_else(|| "known-bad.json is not a valid known-bad list".to_owned())
    }

    fn release_run(&self, tag: &str) -> Result<Option<ReleaseRun>, String> {
        let listed = self.need(&[
            "run",
            "list",
            "--repo",
            &self.repo,
            "--workflow",
            "release.yml",
            "--event",
            "push",
            "--branch",
            tag,
            "--limit",
            "10",
            "--json",
            "databaseId,status,conclusion",
        ])?;
        let runs: Value = serde_json::from_str(&listed).map_err(|e| format!("gh run list: {e}"))?;
        let Some(done) = runs
            .as_array()
            .ok_or("gh run list did not return a list")?
            .iter()
            .find(|r| r.get("status").and_then(Value::as_str) == Some("completed"))
        else {
            return Ok(None);
        };
        let id = done
            .get("databaseId")
            .and_then(Value::as_u64)
            .ok_or("gh returned no databaseId")?
            .to_string();
        let viewed = self.need(&["run", "view", &id, "--repo", &self.repo, "--json", "jobs"])?;
        let view: Value = serde_json::from_str(&viewed).map_err(|e| format!("gh run view: {e}"))?;
        let jobs = view
            .get("jobs")
            .and_then(Value::as_array)
            .ok_or("gh returned no jobs")?
            .iter()
            .map(|j| {
                Ok(RunJob {
                    name: text(j, "name")?,
                    conclusion: j
                        .get("conclusion")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Some(ReleaseRun {
            conclusion: done
                .get("conclusion")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            jobs,
        }))
    }

    fn latest_tag(&self) -> Option<String> {
        let path = format!("repos/{}/releases/latest", self.repo);
        match self.gh(&["api", &path, "--jq", ".tag_name"]) {
            Ok((0, out, _)) => Some(out),
            _ => None,
        }
    }

    fn edit(&self, tag: &str, title: &str, notes: &str) -> Result<(), String> {
        self.need(&[
            "release",
            "edit",
            tag,
            "--repo",
            &self.repo,
            "--prerelease=false",
            "--latest",
            "--title",
            title,
            "--notes",
            notes,
        ])
        .map(|_| ())
    }
}

pub const USAGE: &str = "usage: scaffold-release promote check <tag> | promote <tag>";

/// Runs the guards and prints the report the way the TypeScript did. Exit 0 on success, 1 for a refusal or a failure.
pub fn execute(
    tag: &str,
    git_ref: &str,
    dry_run: bool,
    deps: &dyn PromoteDeps,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match promote(tag, git_ref, deps, dry_run) {
        Ok(report) => {
            for s in &report.steps {
                let _ = writeln!(out, "ok    {}: {}", s.guard, s.detail);
            }
            let _ = if report.promoted {
                writeln!(out, "{tag} is now a stable release.")
            } else {
                writeln!(
                    out,
                    "dry run: every guard passed for {tag}, nothing was edited."
                )
            };
            0
        }
        Err(Failure::Refused { message, report }) => {
            for s in &report.steps {
                let _ = writeln!(
                    out,
                    "{} {}: {}",
                    if s.ok { "ok   " } else { "FAIL " },
                    s.guard,
                    s.detail
                );
            }
            let _ = writeln!(err, "refused: {message}");
            1
        }
        Err(Failure::Other(message)) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}

/// `promote check <tag>` or `promote promote <tag>`, with the real `gh`. Reads `GH_REPO` (default
/// `pantheon-org/pantheon-scaffold`) and `GITHUB_REF`, and `known-bad.json` under `root`.
pub fn command(args: &[&str], root: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (dry_run, tag) = match args {
        ["check", tag] => (true, *tag),
        ["promote", tag] => (false, *tag),
        _ => {
            let _ = writeln!(err, "{USAGE}");
            return 2;
        }
    };
    let repo =
        std::env::var("GH_REPO").unwrap_or_else(|_| "pantheon-org/pantheon-scaffold".to_owned());
    let git_ref = std::env::var("GITHUB_REF").unwrap_or_default();
    let gh = match Gh::new("gh", &repo, root) {
        Ok(gh) => gh,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 1;
        }
    };
    execute(tag, &git_ref, dry_run, &gh, out, err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scaffold_core::checksums::{Checksum, format_checksums};
    use scaffold_core::preflight::KnownBadVersion;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    const TAG: &str = "v0.1.5";
    const VERSION: &str = "0.1.5";
    const MAIN: &str = "refs/heads/main";

    struct World {
        release: ReleaseInfo,
        files: BTreeMap<String, Vec<u8>>,
        bad_attestation: Option<String>,
        known_bad: Vec<(String, String)>,
        run: Option<ReleaseRun>,
        latest: Option<String>,
        /// What the edit does to the world, so a test can make it go wrong.
        on_edit: Option<fn(&mut World)>,
        edits: Vec<(String, String)>,
    }

    struct Fake(RefCell<World>);

    fn job(name: &str, conclusion: &str) -> RunJob {
        RunJob {
            name: name.to_owned(),
            conclusion: conclusion.to_owned(),
        }
    }

    /// A release as the release workflow publishes it: the files, a checksums.txt that matches them, and the pre-release wording.
    fn world() -> World {
        let names = asset_names(VERSION);
        let mut files = BTreeMap::new();
        for n in names.iter().filter(|n| *n != "checksums.txt") {
            files.insert(n.clone(), format!("bytes of {n}").into_bytes());
        }
        let listed: Vec<Checksum> = names
            .iter()
            .filter(|n| *n != "checksums.txt" && !n.ends_with(".sha256"))
            .map(|n| Checksum {
                name: n.clone(),
                sha256: sha256(&files[n]),
            })
            .collect();
        files.insert(
            "checksums.txt".to_owned(),
            format_checksums(&listed).unwrap().into_bytes(),
        );
        World {
            release: ReleaseInfo {
                tag_name: TAG.to_owned(),
                name: format!("{TAG} (pre-release)"),
                body: "Pre-release of the package and the runner, released together.".to_owned(),
                is_prerelease: true,
                is_draft: false,
                assets: names,
            },
            files,
            bad_attestation: None,
            known_bad: Vec::new(),
            run: Some(ReleaseRun {
                conclusion: "success".to_owned(),
                jobs: vec![
                    job("Attest and publish", "success"),
                    job("Verify the published release on ubuntu-latest", "success"),
                ],
            }),
            latest: Some("v0.1.4".to_owned()),
            on_edit: None,
            edits: Vec::new(),
        }
    }

    impl PromoteDeps for Fake {
        fn view_release(&self, _: &str) -> Result<ReleaseInfo, String> {
            Ok(self.0.borrow().release.clone())
        }
        fn download_asset(&self, _: &str, name: &str) -> Result<Vec<u8>, String> {
            self.0
                .borrow()
                .files
                .get(name)
                .cloned()
                .ok_or_else(|| format!("no asset {name}"))
        }
        fn verify_attestation(&self, name: &str, _: &[u8]) -> Result<(), String> {
            if self.0.borrow().bad_attestation.as_deref() == Some(name) {
                return Err("no valid attestation".to_owned());
            }
            Ok(())
        }
        fn known_bad(&self) -> Result<KnownBad, String> {
            Ok(KnownBad {
                versions: self
                    .0
                    .borrow()
                    .known_bad
                    .iter()
                    .map(|(version, reason)| KnownBadVersion {
                        version: version.clone(),
                        reason: reason.clone(),
                    })
                    .collect(),
            })
        }
        fn release_run(&self, _: &str) -> Result<Option<ReleaseRun>, String> {
            Ok(self.0.borrow().run.clone())
        }
        fn latest_tag(&self) -> Option<String> {
            self.0.borrow().latest.clone()
        }
        fn edit(&self, _: &str, title: &str, notes: &str) -> Result<(), String> {
            let mut w = self.0.borrow_mut();
            w.edits.push((title.to_owned(), notes.to_owned()));
            w.release.is_prerelease = false;
            w.release.name = title.to_owned();
            w.release.body = notes.to_owned();
            w.latest = Some(TAG.to_owned());
            if let Some(f) = w.on_edit {
                f(&mut w);
            }
            Ok(())
        }
    }

    fn run(
        w: World,
        dry_run: bool,
        git_ref: &str,
        tag: &str,
    ) -> (Fake, Result<PromoteReport, Failure>) {
        let fake = Fake(RefCell::new(w));
        let result = promote(tag, git_ref, &fake, dry_run);
        (fake, result)
    }

    /// The guard that refused, with the failure.
    fn refused(w: World, dry_run: bool, git_ref: &str, tag: &str) -> (Fake, String, PromoteReport) {
        let (fake, result) = run(w, dry_run, git_ref, tag);
        match result {
            Err(Failure::Refused { message, report }) => (fake, message, report),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    fn refuses(w: World, guard: &str) -> (Fake, String, PromoteReport) {
        let (fake, message, report) = refused(w, false, MAIN, TAG);
        assert!(message.starts_with(&format!("{guard}:")), "{message}");
        (fake, message, report)
    }

    fn edits(fake: &Fake) -> usize {
        fake.0.borrow().edits.len()
    }

    #[test]
    fn a_dry_run_passes_every_guard_and_edits_nothing() {
        let (fake, result) = run(world(), true, MAIN, TAG);
        let report = result.unwrap();
        assert!(!report.promoted);
        assert!(report.steps.iter().all(|s| s.ok));
        let guards: Vec<&str> = report.steps.iter().map(|s| s.guard.as_str()).collect();
        assert_eq!(
            guards,
            [
                "run from main",
                "tag shape",
                "release state",
                "assets, attestation and digests",
                "known bad",
                "release run"
            ]
        );
        assert_eq!(edits(&fake), 0);
        assert!(fake.0.borrow().release.is_prerelease);
    }

    #[test]
    fn a_promotion_edits_the_flag_the_title_and_the_notes_once_and_checks_the_result() {
        let (fake, result) = run(world(), false, MAIN, TAG);
        let report = result.unwrap();
        assert!(report.promoted);
        assert_eq!(
            fake.0.borrow().edits,
            [(
                TAG.to_owned(),
                "Release of the package and the runner, released together.".to_owned()
            )]
        );
        let last = report.steps.last().unwrap();
        assert_eq!((last.guard.as_str(), last.ok), ("after the edit", true));
        assert!(!fake.0.borrow().release.is_prerelease);
    }

    #[test]
    fn it_runs_only_from_main() {
        for git_ref in ["refs/heads/feat/x", "refs/tags/v0.1.5", ""] {
            let (fake, message, _) = refused(world(), false, git_ref, TAG);
            assert!(message.starts_with("run from main:"), "{message}");
            assert_eq!(edits(&fake), 0);
        }
        let (_, message, _) = refused(world(), false, "refs/heads/feat/x", TAG);
        assert_eq!(
            message,
            "run from main: the workflow ran on \"refs/heads/feat/x\", a promotion runs only from refs/heads/main"
        );
    }

    #[test]
    fn a_tag_that_is_not_a_release_tag_is_refused() {
        for tag in ["0.1.5", "v0.1", "main", "v0.1.5; rm -rf"] {
            let (_, message, _) = refused(world(), false, MAIN, tag);
            assert!(message.starts_with("tag shape:"), "{tag}: {message}");
        }
    }

    #[test]
    fn a_release_that_is_already_stable_or_a_draft_is_not_promoted() {
        let mut stable = world();
        stable.release.is_prerelease = false;
        let (fake, message, _) = refuses(stable, "release state");
        assert!(message.contains("already a stable release"));
        assert_eq!(edits(&fake), 0);
        let mut draft = world();
        draft.release.is_draft = true;
        let (fake, message, _) = refuses(draft, "release state");
        assert!(message.contains("is a draft"));
        assert_eq!(edits(&fake), 0);
    }

    #[test]
    fn the_release_must_carry_exactly_the_files_a_release_carries() {
        let mut missing = world();
        missing
            .release
            .assets
            .retain(|n| n != "pantheon-scaffold-linux-x64");
        let (fake, _, _) = refuses(missing, "assets");
        assert_eq!(edits(&fake), 0);
        let mut extra = world();
        extra.release.assets.push("surprise.bin".to_owned());
        let (fake, message, _) = refuses(extra, "assets");
        assert!(message.contains("surprise.bin"));
        assert_eq!(edits(&fake), 0);
    }

    #[test]
    fn a_checksums_file_without_a_valid_attestation_stops_the_promotion() {
        let mut w = world();
        w.bad_attestation = Some("checksums.txt".to_owned());
        let (fake, message, report) = refuses(w, "attestation");
        assert_eq!(message, "attestation: checksums.txt: no valid attestation");
        assert!(!report.steps.last().unwrap().ok);
        assert_eq!(edits(&fake), 0);
    }

    #[test]
    fn an_asset_whose_bytes_do_not_match_checksums_txt_stops_the_promotion() {
        let mut w = world();
        w.files.insert(
            "pantheon-scaffold-darwin-arm64".to_owned(),
            b"tampered".to_vec(),
        );
        let (fake, message, _) = refuses(w, "digests");
        assert_eq!(
            message,
            "digests: pantheon-scaffold-darwin-arm64 does not match checksums.txt"
        );
        assert_eq!(edits(&fake), 0);
    }

    #[test]
    fn a_file_that_checksums_txt_lists_but_the_release_does_not_carry_stops_the_promotion() {
        let mut w = world();
        let extra = format!("{}  ghost-file\n", "0".repeat(64));
        let text = String::from_utf8(w.files["checksums.txt"].clone()).unwrap() + &extra;
        w.files
            .insert("checksums.txt".to_owned(), text.into_bytes());
        let (_, message, _) = refuses(w, "digests");
        assert_eq!(
            message,
            "digests: checksums.txt lists ghost-file, which the release does not carry"
        );
    }

    #[test]
    fn a_version_on_the_known_bad_list_is_refused_with_its_reason() {
        let mut w = world();
        w.known_bad = vec![(VERSION.to_owned(), "it breaks upgrade".to_owned())];
        let (fake, message, _) = refuses(w, "known bad");
        assert!(message.contains("it breaks upgrade"));
        assert_eq!(edits(&fake), 0);
        let mut other = world();
        other.known_bad = vec![("0.1.4".to_owned(), "x".to_owned())];
        assert!(run(other, true, MAIN, TAG).1.is_ok());
    }

    #[test]
    fn the_release_workflow_must_have_succeeded_for_the_tag_verify_job_included() {
        let verify = |conclusion: &str| job("Verify the published release on x", conclusion);
        let mut none = world();
        none.run = None;
        refuses(none, "release run");
        let mut failed_run = world();
        failed_run.run = Some(ReleaseRun {
            conclusion: "failure".to_owned(),
            jobs: vec![verify("success")],
        });
        let (_, message, _) = refuses(failed_run, "release run");
        assert_eq!(message, "release run: not successful: run (failure)");
        let mut failed_job = world();
        failed_job.run = Some(ReleaseRun {
            conclusion: "success".to_owned(),
            jobs: vec![verify("failure")],
        });
        let (_, message, _) = refuses(failed_job, "release run");
        assert_eq!(
            message,
            "release run: not successful: Verify the published release on x (failure)"
        );
        let mut no_verify = world();
        no_verify.run = Some(ReleaseRun {
            conclusion: "success".to_owned(),
            jobs: vec![job("Attest and publish", "success")],
        });
        let (fake, message, _) = refuses(no_verify, "release run");
        assert!(message.contains("no verify-release job"));
        assert_eq!(edits(&fake), 0);
    }

    #[test]
    fn after_the_edit_a_release_that_is_still_a_pre_release_not_latest_or_with_changed_checksums_is_a_failure()
     {
        let mut still = world();
        still.on_edit = Some(|w| w.release.is_prerelease = true);
        let (_, message, _) = refuses(still, "after the edit");
        assert_eq!(message, "after the edit: v0.1.5 is still a pre-release");
        let mut not_latest = world();
        not_latest.on_edit = Some(|w| w.latest = Some("v0.9.9".to_owned()));
        let (_, message, _) = refuses(not_latest, "after the edit");
        assert_eq!(
            message,
            "after the edit: the latest release is \"v0.9.9\", expected v0.1.5"
        );
        let mut no_latest = world();
        no_latest.on_edit = Some(|w| w.latest = None);
        let (_, message, _) = refuses(no_latest, "after the edit");
        assert_eq!(
            message,
            "after the edit: the latest release is undefined, expected v0.1.5"
        );
        let mut changed = world();
        changed.on_edit = Some(|w| {
            w.files.get_mut("checksums.txt").unwrap().push(b'\n');
        });
        let (_, message, _) = refuses(changed, "after the edit");
        assert!(message.contains("not byte for byte"), "{message}");
    }

    #[test]
    fn the_stable_wording_drops_the_pre_release_label_and_nothing_else() {
        let info = ReleaseInfo {
            tag_name: TAG.to_owned(),
            name: "v0.1.5 (pre-release)".to_owned(),
            body: "Pre-release of the package. checksums.txt lists every file.".to_owned(),
            is_prerelease: true,
            is_draft: false,
            assets: Vec::new(),
        };
        assert_eq!(
            stable_wording(&info),
            (
                "v0.1.5".to_owned(),
                "Release of the package. checksums.txt lists every file.".to_owned()
            )
        );
        let plain = ReleaseInfo {
            name: "v0.1.5".to_owned(),
            body: "Notes.".to_owned(),
            ..info
        };
        assert_eq!(
            stable_wording(&plain),
            ("v0.1.5".to_owned(), "Notes.".to_owned())
        );
    }

    /// A call to `gh` that fails is not a guard: it has no step, and its message is shown as it is.
    struct Down;
    impl PromoteDeps for Down {
        fn view_release(&self, _: &str) -> Result<ReleaseInfo, String> {
            Err("gh release view failed (exit 1): HTTP 502".to_owned())
        }
        fn download_asset(&self, _: &str, _: &str) -> Result<Vec<u8>, String> {
            unreachable!()
        }
        fn verify_attestation(&self, _: &str, _: &[u8]) -> Result<(), String> {
            unreachable!()
        }
        fn known_bad(&self) -> Result<KnownBad, String> {
            unreachable!()
        }
        fn release_run(&self, _: &str) -> Result<Option<ReleaseRun>, String> {
            unreachable!()
        }
        fn latest_tag(&self) -> Option<String> {
            unreachable!()
        }
        fn edit(&self, _: &str, _: &str, _: &str) -> Result<(), String> {
            unreachable!()
        }
    }

    #[test]
    fn a_failing_gh_call_is_not_a_guard_and_is_reported_as_it_is() {
        let result = promote(TAG, MAIN, &Down, false);
        assert_eq!(
            result,
            Err(Failure::Other(
                "gh release view failed (exit 1): HTTP 502".to_owned()
            ))
        );
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(execute(TAG, MAIN, false, &Down, &mut out, &mut err), 1);
        assert_eq!(String::from_utf8(out).unwrap(), "");
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "gh release view failed (exit 1): HTTP 502\n"
        );
    }

    #[test]
    fn the_report_is_printed_the_way_the_typescript_printed_it() {
        let fake = Fake(RefCell::new(world()));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(execute(TAG, MAIN, true, &fake, &mut out, &mut err), 0);
        let out = String::from_utf8(out).unwrap();
        assert!(
            out.starts_with(
                "ok    run from main: refs/heads/main\nok    tag shape: version 0.1.5\n"
            )
        );
        assert!(out.ends_with("dry run: every guard passed for v0.1.5, nothing was edited.\n"));
        assert_eq!(String::from_utf8(err).unwrap(), "");

        let fake = Fake(RefCell::new(world()));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(execute(TAG, MAIN, false, &fake, &mut out, &mut err), 0);
        assert!(
            String::from_utf8(out)
                .unwrap()
                .ends_with("v0.1.5 is now a stable release.\n")
        );

        let mut w = world();
        w.release.is_prerelease = false;
        let fake = Fake(RefCell::new(w));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(execute(TAG, MAIN, true, &fake, &mut out, &mut err), 1);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "ok    run from main: refs/heads/main\nok    tag shape: version 0.1.5\nFAIL  release state: v0.1.5 is already a stable release: nothing to promote, and a published version is never re-published\n"
        );
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "refused: release state: v0.1.5 is already a stable release: nothing to promote, and a published version is never re-published\n"
        );
    }

    // --- the calls to gh ---------------------------------------------------------------------------------------------------------

    /// A stand-in `gh` that records every call, one line per call with the arguments joined by U+001F, and answers like the real one.
    fn fake_gh(dir: &Path) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let log = dir.join("gh.log");
        let script = dir.join("gh");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
printf '%s\037' "$@" >> '{log}'
printf '\n' >> '{log}'
case "$1 $2" in
  "release view") echo '{{"tagName":"v0.1.5","name":"v0.1.5 (pre-release)","body":"Pre-release of x","isPrerelease":true,"isDraft":false,"assets":[{{"name":"a.zip"}},{{"name":"b.bin"}}]}}' ;;
  "release download")
    while [ $# -gt 0 ]; do case "$1" in --dir) dir="$2";; --pattern) pattern="$2";; esac; shift; done
    printf 'content of %s' "$pattern" > "$dir/$pattern" ;;
  "run list") echo '[{{"databaseId":1,"status":"in_progress","conclusion":""}},{{"databaseId":7,"status":"completed","conclusion":"success"}}]' ;;
  "run view") echo '{{"jobs":[{{"name":"Attest and publish","conclusion":"success"}}]}}' ;;
  "api repos/o/r/releases/latest") echo 'v0.1.5' ;;
esac
"#,
                log = log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        (script, log)
    }

    /// The calls recorded in the log, with the private folder the tool downloads into replaced by `<work>`.
    fn calls(log: &Path) -> Vec<Vec<String>> {
        std::fs::read_to_string(log)
            .unwrap()
            .lines()
            .map(|line| {
                line.split('\u{1f}')
                    .filter(|a| !a.is_empty())
                    .map(|a| match a.find("/promote-") {
                        Some(i) => format!(
                            "<work>/{}",
                            a[i + 1..].split_once('/').map_or("", |(_, rest)| rest)
                        ),
                        None => a.to_owned(),
                    })
                    .collect()
            })
            .collect()
    }

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_gh_calls_carry_the_arguments_the_typescript_passed() {
        let dir = tempfile::tempdir().unwrap();
        let (script, log) = fake_gh(dir.path());
        let gh = Gh::new(&script, "o/r", dir.path()).unwrap();

        let info = gh.view_release(TAG).unwrap();
        assert_eq!(info.assets, ["a.zip", "b.bin"]);
        assert!(info.is_prerelease && !info.is_draft);
        assert_eq!(
            gh.download_asset(TAG, "b.bin").unwrap(),
            b"content of b.bin"
        );
        gh.verify_attestation("checksums.txt", b"x").unwrap();
        let run = gh.release_run(TAG).unwrap().unwrap();
        assert_eq!(run.conclusion, "success");
        assert_eq!(run.jobs, [job("Attest and publish", "success")]);
        assert_eq!(gh.latest_tag(), Some("v0.1.5".to_owned()));
        gh.edit(TAG, "v0.1.5", "Release of x, with spaces").unwrap();

        assert_eq!(
            calls(&log),
            [
                args(&[
                    "release",
                    "view",
                    TAG,
                    "--repo",
                    "o/r",
                    "--json",
                    "tagName,name,body,isPrerelease,isDraft,assets"
                ]),
                args(&[
                    "release",
                    "download",
                    TAG,
                    "--repo",
                    "o/r",
                    "--pattern",
                    "b.bin",
                    "--dir",
                    "<work>/dl-1"
                ]),
                args(&[
                    "attestation",
                    "verify",
                    "<work>/at-2/checksums.txt",
                    "--repo",
                    "o/r"
                ]),
                args(&[
                    "run",
                    "list",
                    "--repo",
                    "o/r",
                    "--workflow",
                    "release.yml",
                    "--event",
                    "push",
                    "--branch",
                    TAG,
                    "--limit",
                    "10",
                    "--json",
                    "databaseId,status,conclusion"
                ]),
                args(&["run", "view", "7", "--repo", "o/r", "--json", "jobs"]),
                args(&["api", "repos/o/r/releases/latest", "--jq", ".tag_name"]),
                args(&[
                    "release",
                    "edit",
                    TAG,
                    "--repo",
                    "o/r",
                    "--prerelease=false",
                    "--latest",
                    "--title",
                    "v0.1.5",
                    "--notes",
                    "Release of x, with spaces"
                ]),
            ]
        );
    }

    #[test]
    fn a_gh_call_that_fails_says_which_call_and_why() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("gh");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(
                &script,
                "#!/bin/sh\necho 'HTTP 404: Not Found' >&2\nexit 1\n",
            )
            .unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let gh = Gh::new(&script, "o/r", dir.path()).unwrap();
        assert_eq!(
            gh.view_release(TAG).unwrap_err(),
            "gh release view failed (exit 1): HTTP 404: Not Found"
        );
        assert_eq!(
            gh.verify_attestation("checksums.txt", b"x").unwrap_err(),
            "gh attestation verify failed (exit 1): HTTP 404: Not Found"
        );
        assert_eq!(gh.latest_tag(), None);
        assert_eq!(
            gh.release_run(TAG).unwrap_err(),
            "gh run list failed (exit 1): HTTP 404: Not Found"
        );
    }

    #[test]
    fn a_run_with_no_finished_release_run_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("gh");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(&script, "#!/bin/sh\necho '[{\"databaseId\":1,\"status\":\"queued\",\"conclusion\":\"\"}]'\n").unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let gh = Gh::new(&script, "o/r", dir.path()).unwrap();
        assert_eq!(gh.release_run(TAG).unwrap(), None);
    }

    #[test]
    fn gh_leaves_no_download_folder_behind() {
        let dir = tempfile::tempdir().unwrap();
        let (script, _) = fake_gh(dir.path());
        let gh = Gh::new(&script, "o/r", dir.path()).unwrap();
        gh.download_asset(TAG, "b.bin").unwrap();
        let work = gh.work.clone();
        assert!(work.exists());
        drop(gh);
        assert!(!work.exists());
    }

    #[test]
    fn the_known_bad_list_is_read_from_the_checkout_and_an_invalid_one_stops_the_promotion() {
        let dir = tempfile::tempdir().unwrap();
        let gh = Gh::new("gh", "o/r", dir.path()).unwrap();
        assert!(gh.known_bad().unwrap_err().contains("known-bad.json"));
        std::fs::write(dir.path().join("known-bad.json"), "not json").unwrap();
        assert_eq!(
            gh.known_bad().unwrap_err(),
            "known-bad.json is not a valid known-bad list"
        );
        std::fs::write(
            dir.path().join("known-bad.json"),
            r#"{"versions":[{"version":"0.1.0","reason":"bad"}]}"#,
        )
        .unwrap();
        assert_eq!(gh.known_bad().unwrap().versions[0].version, "0.1.0");
    }
}
