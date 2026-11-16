//! Promotion of a pre-release to a stable release. Ported from `runner/src/promote.ts` and `runner/scripts/promote.ts`
//! (Phase 2 of `docs/ts-tooling-rust-port.md`).
//!
//! Every guard runs before anything is edited, and each is recorded. The edit is the flag, the title and the notes: the assets are
//! never touched. After it the same checks run again, and the checksums file must be byte for byte the one that was attested before
//! the edit. The guards, their order, the calls made to `gh` and the report lines follow the TypeScript, so the two can be compared
//! (which Phase 6 deleted, with the differential script that compared it with this).

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
mod tests;
