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
        out.starts_with("ok    run from main: refs/heads/main\nok    tag shape: version 0.1.5\n")
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
fn a_gh_that_is_killed_has_no_exit_code_and_is_reported_as_exit_minus_1() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("gh");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(&script, "#!/bin/sh\nkill -9 $$\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let gh = Gh::new(&script, "o/r", dir.path()).unwrap();
    assert_eq!(
        gh.view_release(TAG).unwrap_err(),
        "gh release view failed (exit -1): "
    );
}

#[test]
fn a_gh_that_cannot_be_started_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let gh = Gh::new(dir.path().join("no-such-gh"), "o/r", dir.path()).unwrap();
    let error = gh.view_release(TAG).unwrap_err();
    assert!(error.starts_with("gh release: "), "{error}");
}

#[test]
fn a_run_with_no_finished_release_run_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("gh");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(
            &script,
            "#!/bin/sh\necho '[{\"databaseId\":1,\"status\":\"queued\",\"conclusion\":\"\"}]'\n",
        )
        .unwrap();
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
