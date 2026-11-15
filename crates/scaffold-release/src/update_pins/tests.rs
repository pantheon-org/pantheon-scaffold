use super::*;
use scaffold_core::freshness::parse_freshness;
use std::collections::BTreeMap;
use std::io::Read;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use time::macros::datetime;

fn sha(c: char) -> String {
    c.to_string().repeat(40)
}

fn pins_text() -> String {
    format!(
        "/// Action pins.\nmodule x\n\nclass Pin {{ action: String }}\n\ncheckout: Pin = new {{ action = \"actions/checkout\"; sha = \"{}\"; version = \"v1.0.0\" }}\nsetup: Pin = new {{ action = \"actions/setup-node\"; sha = \"{}\"; version = \"v2.1.0\" }}\nnested: Pin = new {{ action = \"owner/repo/.github/workflows/x.yml\"; sha = \"{}\"; version = \"v3.0.0\" }}\n",
        sha('a'),
        sha('b'),
        sha('c')
    )
}

/// A fake of the four GitHub answers the update needs. Latest: repo to tag. Commits: "repo@tag" to sha.
#[derive(Default)]
struct Fake {
    latest: BTreeMap<String, Option<String>>,
    commits: BTreeMap<String, String>,
    releases: Option<Vec<String>>,
    missing: Vec<String>,
    throws: Vec<String>,
}

impl PinApi for Fake {
    fn latest_release(&self, repo: &str) -> Result<Option<String>, String> {
        if self.throws.iter().any(|r| r == repo) {
            return Err("HTTP 504".to_owned());
        }
        Ok(self.latest.get(repo).cloned().flatten())
    }
    fn tag_commit(&self, repo: &str, tag: &str) -> Result<String, String> {
        self.commits
            .get(&format!("{repo}@{tag}"))
            .cloned()
            .ok_or_else(|| format!("no such tag {tag}"))
    }
    fn is_release(&self, repo: &str, tag: &str) -> Result<bool, String> {
        let key = format!("{repo}@{tag}");
        Ok(match &self.releases {
            Some(list) => list.contains(&key),
            None => self.commits.contains_key(&key),
        })
    }
    fn commit_exists(&self, repo: &str, sha: &str) -> Result<bool, String> {
        Ok(!self.missing.contains(&format!("{repo}@{sha}")))
    }
}

fn fake(latest: &[(&str, &str)], commits: &[(&str, String)]) -> Fake {
    Fake {
        latest: latest
            .iter()
            .map(|(r, t)| ((*r).to_owned(), Some((*t).to_owned())))
            .collect(),
        commits: commits
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
        ..Fake::default()
    }
}

fn current() -> Fake {
    fake(
        &[
            ("actions/checkout", "v1.0.0"),
            ("actions/setup-node", "v2.1.0"),
            ("owner/repo", "v3.0.0"),
        ],
        &[
            ("actions/checkout@v1.0.0", sha('a')),
            ("actions/setup-node@v2.1.0", sha('b')),
            ("owner/repo@v3.0.0", sha('c')),
        ],
    )
}

fn names(problems: &[PinProblem]) -> Vec<&str> {
    problems.iter().map(|p| p.name.as_str()).collect()
}

#[test]
fn every_pin_line_is_read_with_the_repository_part_of_a_workflow_path() {
    let pins = parse_pins(&pins_text()).unwrap();
    let shown: Vec<(&str, &str, &str)> = pins
        .iter()
        .map(|p| (p.name.as_str(), p.action.as_str(), p.version.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            ("checkout", "actions/checkout", "v1.0.0"),
            ("setup", "actions/setup-node", "v2.1.0"),
            ("nested", "owner/repo/.github/workflows/x.yml", "v3.0.0"),
        ]
    );
    assert_eq!(repo_of("owner/repo/.github/workflows/x.yml"), "owner/repo");
    assert_eq!(repo_of("actions/checkout"), "actions/checkout");
}

#[test]
fn a_pin_line_that_cannot_be_read_is_an_error_never_skipped() {
    let bad = r#"bad: Pin = new { action = "a/b"; sha = "short"; version = "v1" }"#;
    assert_eq!(
        parse_pins(bad).unwrap_err(),
        format!("cannot read the pin: {bad}")
    );
}

#[test]
fn the_real_pins_pkl_is_read_completely() {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../pkl/Pins.pkl"),
    )
    .unwrap();
    let pins = parse_pins(&text).unwrap();
    assert_eq!(
        pins.len(),
        text.lines().filter(|l| l.contains(": Pin = new {")).count()
    );
    assert!(pins.len() >= 5, "{}", pins.len());
    for p in &pins {
        assert_eq!(p.sha.len(), 40, "{}", p.name);
    }
}

#[test]
fn pins_already_at_the_latest_release_and_matching_their_tag_change_nothing() {
    let (changes, problems) = update_pins(&parse_pins(&pins_text()).unwrap(), &current());
    assert_eq!((changes, problems), (vec![], vec![]));
}

#[test]
fn a_pin_behind_its_latest_release_moves_to_that_commit_and_a_major_bump_is_flagged() {
    let api = fake(
        &[
            ("actions/checkout", "v1.2.0"),
            ("actions/setup-node", "v3.0.0"),
            ("owner/repo", "v3.0.0"),
        ],
        &[
            ("actions/checkout@v1.2.0", sha('d')),
            ("actions/setup-node@v3.0.0", sha('e')),
            ("owner/repo@v3.0.0", sha('c')),
        ],
    );
    let (changes, problems) = update_pins(&parse_pins(&pins_text()).unwrap(), &api);
    assert_eq!(problems, []);
    assert_eq!(
        changes,
        [
            PinChange {
                name: "checkout".to_owned(),
                action: "actions/checkout".to_owned(),
                from: "v1.0.0".to_owned(),
                to: "v1.2.0".to_owned(),
                to_sha: sha('d'),
                major: false
            },
            PinChange {
                name: "setup".to_owned(),
                action: "actions/setup-node".to_owned(),
                from: "v2.1.0".to_owned(),
                to: "v3.0.0".to_owned(),
                to_sha: sha('e'),
                major: true
            },
        ]
    );
}

#[test]
fn a_pin_is_never_moved_to_an_older_release() {
    let api = fake(
        &[
            ("actions/checkout", "v0.9.0"),
            ("actions/setup-node", "v2.1.0"),
            ("owner/repo", "v3.0.0"),
        ],
        &[
            ("actions/checkout@v1.0.0", sha('a')),
            ("actions/setup-node@v2.1.0", sha('b')),
            ("owner/repo@v3.0.0", sha('c')),
        ],
    );
    assert_eq!(
        update_pins(&parse_pins(&pins_text()).unwrap(), &api),
        (vec![], vec![])
    );
}

#[test]
fn a_pin_whose_commit_is_not_the_commit_of_its_recorded_tag_is_a_problem_even_when_up_to_date() {
    let mut api = current();
    api.commits
        .insert("actions/checkout@v1.0.0".to_owned(), sha('f'));
    let (changes, problems) = update_pins(&parse_pins(&pins_text()).unwrap(), &api);
    assert_eq!(changes, []);
    assert_eq!(
        problems,
        [PinProblem {
            name: "checkout".to_owned(),
            message: format!(
                "actions/checkout v1.0.0 resolves to {}, not the pinned {}",
                sha('f'),
                sha('a')
            )
        }]
    );
}

#[test]
fn a_latest_tag_that_is_not_a_published_release_is_a_problem_and_nothing_changes_for_it() {
    let mut api = fake(
        &[
            ("actions/checkout", "v1.2.0"),
            ("actions/setup-node", "v2.1.0"),
            ("owner/repo", "v3.0.0"),
        ],
        &[
            ("actions/checkout@v1.2.0", sha('d')),
            ("actions/setup-node@v2.1.0", sha('b')),
            ("owner/repo@v3.0.0", sha('c')),
        ],
    );
    api.releases = Some(vec![
        "actions/setup-node@v2.1.0".to_owned(),
        "owner/repo@v3.0.0".to_owned(),
    ]);
    let (changes, problems) = update_pins(&parse_pins(&pins_text()).unwrap(), &api);
    assert_eq!(changes, []);
    assert_eq!(names(&problems), ["checkout"]);
    assert_eq!(
        problems[0].message,
        "actions/checkout v1.2.0 is a tag but not a published release"
    );
}

#[test]
fn a_commit_that_does_not_exist_in_the_upstream_repository_is_a_problem() {
    let mut api = fake(
        &[
            ("actions/checkout", "v1.2.0"),
            ("actions/setup-node", "v2.1.0"),
            ("owner/repo", "v3.0.0"),
        ],
        &[
            ("actions/checkout@v1.2.0", sha('d')),
            ("actions/setup-node@v2.1.0", sha('b')),
            ("owner/repo@v3.0.0", sha('c')),
        ],
    );
    api.missing.push(format!("actions/checkout@{}", sha('d')));
    let (_, problems) = update_pins(&parse_pins(&pins_text()).unwrap(), &api);
    assert_eq!(
        problems[0].message,
        format!(
            "actions/checkout v1.2.0 resolves to {}, which does not belong to actions/checkout",
            sha('d')
        )
    );
}

#[test]
fn a_repository_with_no_release_and_a_github_error_are_problems_never_unchanged() {
    let mut api = fake(
        &[("actions/setup-node", "v2.1.0"), ("owner/repo", "v3.0.0")],
        &[
            ("actions/setup-node@v2.1.0", sha('b')),
            ("owner/repo@v3.0.0", sha('c')),
        ],
    );
    api.throws.push("actions/checkout".to_owned());
    let (_, problems) = update_pins(&parse_pins(&pins_text()).unwrap(), &api);
    assert_eq!(names(&problems), ["checkout"]);
    assert_eq!(problems[0].message, "actions/checkout: HTTP 504");

    let mut none = fake(
        &[("actions/setup-node", "v2.1.0"), ("owner/repo", "v3.0.0")],
        &[
            ("actions/setup-node@v2.1.0", sha('b')),
            ("owner/repo@v3.0.0", sha('c')),
        ],
    );
    none.latest.insert("actions/checkout".to_owned(), None);
    let (_, problems) = update_pins(&parse_pins(&pins_text()).unwrap(), &none);
    assert_eq!(problems[0].message, "no release found on actions/checkout");
}

#[test]
fn a_version_that_is_not_a_version_is_a_problem_and_the_update_does_not_guess() {
    let text = pins_text().replace("version = \"v1.0.0\"", "version = \"main\"");
    let (_, problems) = update_pins(&parse_pins(&text).unwrap(), &current());
    assert_eq!(
        problems[0].message,
        "actions/checkout: main is not a version, the update will not guess"
    );
    let mut odd = current();
    odd.latest
        .insert("actions/checkout".to_owned(), Some("nightly".to_owned()));
    let (_, problems) = update_pins(&parse_pins(&pins_text()).unwrap(), &odd);
    assert_eq!(
        problems[0].message,
        "actions/checkout: nightly is not a version, the update will not guess"
    );
}

#[test]
fn applying_changes_rewrites_only_the_pin_lines_that_changed_and_twice_changes_nothing_more() {
    let changes = [PinChange {
        name: "checkout".to_owned(),
        action: "actions/checkout".to_owned(),
        from: "v1.0.0".to_owned(),
        to: "v1.2.0".to_owned(),
        to_sha: sha('d'),
        major: false,
    }];
    let once = apply_changes(&pins_text(), &changes).unwrap();
    assert_eq!(
        once,
        pins_text().replace(
            &format!("sha = \"{}\"; version = \"v1.0.0\"", sha('a')),
            &format!("sha = \"{}\"; version = \"v1.2.0\"", sha('d'))
        )
    );
    assert_eq!(apply_changes(&once, &changes).unwrap(), once);
}

#[test]
fn applying_a_change_to_a_pin_that_is_not_in_the_file_is_an_error() {
    let ghost = [PinChange {
        name: "ghost".to_owned(),
        action: "a/b".to_owned(),
        from: "v1".to_owned(),
        to: "v2".to_owned(),
        to_sha: sha('d'),
        major: false,
    }];
    assert_eq!(
        apply_changes(&pins_text(), &ghost).unwrap_err(),
        "no pin named ghost in the file"
    );
}

#[test]
fn a_version_with_a_dollar_sign_is_written_as_it_is() {
    let changes = [PinChange {
        name: "checkout".to_owned(),
        action: "actions/checkout".to_owned(),
        from: "v1.0.0".to_owned(),
        to: "v1.2.0-$1".to_owned(),
        to_sha: sha('d'),
        major: false,
    }];
    let text = apply_changes(&pins_text(), &changes).unwrap();
    assert!(text.contains("version = \"v1.2.0-$1\""), "{text}");
}

// The commands of runner/test/update-pins-script.test.ts, through `execute` with a fake in place of GitHub.

fn temp_pins(text: &str) -> (tempfile::TempDir, Options) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Pins.pkl"), text).unwrap();
    let options = Options {
        pins: dir.path().join("Pins.pkl"),
        no_write: false,
        freshness_out: Some(dir.path().join("freshness.json")),
        summary: Some(dir.path().join("summary.md")),
    };
    (dir, options)
}

fn script_pins() -> String {
    format!(
        "module x\nclass Pin {{ action: String }}\ncheckout: Pin = new {{ action = \"actions/checkout\"; sha = \"{}\"; version = \"v1.0.0\" }}\nsetup: Pin = new {{ action = \"actions/setup-node\"; sha = \"{}\"; version = \"v2.0.0\" }}\n",
        sha('a'),
        sha('b')
    )
}

fn behind() -> Fake {
    fake(
        &[
            ("actions/checkout", "v1.1.0"),
            ("actions/setup-node", "v2.0.0"),
        ],
        &[
            ("actions/checkout@v1.1.0", sha('c')),
            ("actions/setup-node@v2.0.0", sha('b')),
        ],
    )
}

const NOW: OffsetDateTime = datetime!(2026-11-08 12:00:00 UTC);

fn exec(options: &Options, api: &Fake) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = execute(options, api, NOW, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn a_no_write_run_lists_the_changes_and_writes_nothing() {
    let (dir, mut options) = temp_pins(&script_pins());
    options.no_write = true;
    let (code, out, err) = exec(&options, &behind());
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        out,
        "checked 2 pin(s)\n  actions/checkout v1.0.0 -> v1.1.0\nno-write run: nothing written\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("Pins.pkl")).unwrap(),
        script_pins()
    );
    assert!(!dir.path().join("freshness.json").exists());
    assert!(!dir.path().join("summary.md").exists());
}

#[test]
fn a_real_run_rewrites_the_changed_pin_and_writes_the_freshness_record_and_a_summary() {
    let (dir, options) = temp_pins(&script_pins());
    let (code, out, err) = exec(&options, &behind());
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        out,
        "checked 2 pin(s)\n  actions/checkout v1.0.0 -> v1.1.0\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("Pins.pkl")).unwrap(),
        script_pins().replace(
            &format!("sha = \"{}\"; version = \"v1.0.0\"", sha('a')),
            &format!("sha = \"{}\"; version = \"v1.1.0\"", sha('c'))
        )
    );
    let record = std::fs::read_to_string(dir.path().join("freshness.json")).unwrap();
    assert_eq!(parse_freshness(&record).unwrap(), NOW);
    let summary = std::fs::read_to_string(dir.path().join("summary.md")).unwrap();
    assert!(
        summary.contains("- `actions/checkout` v1.0.0 to v1.1.0\n"),
        "{summary}"
    );
    assert!(!summary.contains("setup-node"));
    assert!(summary.starts_with("Moves the pinned actions to their latest published release."));
}

#[test]
fn when_nothing_changed_the_freshness_record_is_still_written_and_no_summary_is() {
    let (dir, options) = temp_pins(&script_pins());
    let api = fake(
        &[
            ("actions/checkout", "v1.0.0"),
            ("actions/setup-node", "v2.0.0"),
        ],
        &[
            ("actions/checkout@v1.0.0", sha('a')),
            ("actions/setup-node@v2.0.0", sha('b')),
        ],
    );
    let (code, out, _) = exec(&options, &api);
    assert_eq!(code, 0);
    assert_eq!(out, "checked 2 pin(s)\nno pin changed\n");
    assert!(dir.path().join("freshness.json").exists());
    assert!(!dir.path().join("summary.md").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("Pins.pkl")).unwrap(),
        script_pins()
    );
}

#[test]
fn a_problem_fails_the_run_and_writes_nothing_not_even_the_freshness_record() {
    let (dir, options) = temp_pins(&script_pins());
    let mut api = behind();
    api.releases = Some(vec!["actions/setup-node@v2.0.0".to_owned()]);
    let (code, out, err) = exec(&options, &api);
    assert_eq!(code, 1);
    assert_eq!(out, "checked 2 pin(s)\nno pin changed\n");
    assert_eq!(
        err,
        "problem with checkout: actions/checkout v1.1.0 is a tag but not a published release\n1 problem(s): nothing was written, the freshness record included\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("Pins.pkl")).unwrap(),
        script_pins()
    );
    assert!(!dir.path().join("freshness.json").exists());
}

#[test]
fn a_major_bump_is_marked_in_the_summary_and_in_the_listing() {
    let (dir, options) = temp_pins(&script_pins());
    let api = fake(
        &[
            ("actions/checkout", "v2.0.0"),
            ("actions/setup-node", "v2.0.0"),
        ],
        &[
            ("actions/checkout@v2.0.0", sha('c')),
            ("actions/setup-node@v2.0.0", sha('b')),
        ],
    );
    let (code, out, _) = exec(&options, &api);
    assert_eq!(code, 0);
    assert!(
        out.contains("actions/checkout v1.0.0 -> v2.0.0 (major)\n"),
        "{out}"
    );
    let summary = std::fs::read_to_string(dir.path().join("summary.md")).unwrap();
    assert!(
        summary.contains("(**major**, read its release notes)"),
        "{summary}"
    );
}

#[test]
fn a_run_with_no_freshness_or_summary_option_writes_only_the_pins() {
    let (dir, mut options) = temp_pins(&script_pins());
    options.freshness_out = None;
    options.summary = None;
    assert_eq!(exec(&options, &behind()).0, 0);
    assert!(!dir.path().join("freshness.json").exists());
    assert!(!dir.path().join("summary.md").exists());
    assert!(
        std::fs::read_to_string(dir.path().join("Pins.pkl"))
            .unwrap()
            .contains("v1.1.0")
    );
}

#[test]
fn a_pins_file_that_is_missing_or_has_a_line_that_cannot_be_read_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    let options = Options {
        pins: dir.path().join("none.pkl"),
        no_write: false,
        freshness_out: None,
        summary: None,
    };
    let (code, out, err) = exec(&options, &behind());
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.contains("none.pkl"), "{err}");

    let bad = r#"bad: Pin = new { action = "a/b"; sha = "short"; version = "v1" }"#;
    let (_dir, options) = temp_pins(bad);
    let (code, _, err) = exec(&options, &behind());
    assert_eq!(code, 1);
    assert_eq!(err, format!("cannot read the pin: {bad}\n"));
}

#[test]
fn the_options_are_read_and_an_unknown_one_or_a_missing_value_is_a_usage_error() {
    let root = Path::new("/checkout");
    assert_eq!(
        parse_options(&[], root).unwrap(),
        Options {
            pins: PathBuf::from("/checkout/pkl/Pins.pkl"),
            no_write: false,
            freshness_out: None,
            summary: None
        }
    );
    assert_eq!(
        parse_options(
            &[
                "--no-write",
                "--pins",
                "p.pkl",
                "--freshness-out",
                "f.json",
                "--summary",
                "s.md"
            ],
            root
        )
        .unwrap(),
        Options {
            pins: PathBuf::from("p.pkl"),
            no_write: true,
            freshness_out: Some(PathBuf::from("f.json")),
            summary: Some(PathBuf::from("s.md"))
        }
    );
    assert_eq!(
        parse_options(&["--nope"], root).unwrap_err(),
        format!("unknown option --nope\n{USAGE}")
    );
    for flag in ["--pins", "--freshness-out", "--summary"] {
        assert_eq!(
            parse_options(&[flag], root).unwrap_err(),
            format!("{flag} needs a value")
        );
    }
}

#[test]
fn the_command_reports_a_usage_error_with_exit_2() {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(command(&["--nope"], Path::new("/x"), &mut out, &mut err), 2);
    assert_eq!(String::from_utf8(out).unwrap(), "");
    assert!(
        String::from_utf8(err)
            .unwrap()
            .starts_with("unknown option --nope")
    );
}

// The real client against a local server standing in for GitHub.

type Seen = Arc<Mutex<Vec<String>>>;

/// Serves routes (a path and the answers for it, one per request, the last repeating). Records each request as its request line
/// and its lowercased headers. Any other path answers 500.
fn github(routes: Vec<(&str, Vec<(u16, &str)>)>) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let seen: Seen = Arc::default();
    let log = Arc::clone(&seen);
    let mut routes: BTreeMap<String, Vec<(u16, String)>> = routes
        .into_iter()
        .map(|(p, answers)| {
            (
                p.to_owned(),
                answers
                    .into_iter()
                    .map(|(s, b)| (s, b.to_owned()))
                    .collect(),
            )
        })
        .collect();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => request.extend_from_slice(&buffer[..n]),
                }
            }
            let request = String::from_utf8_lossy(&request).to_string();
            let path = request
                .lines()
                .next()
                .and_then(|l| l.split(' ').nth(1))
                .unwrap_or("")
                .to_owned();
            log.lock().unwrap().push(request.to_lowercase());
            let (status, body) = match routes.get_mut(&path) {
                Some(answers) if answers.len() > 1 => answers.remove(0),
                Some(answers) => answers[0].clone(),
                None => (500, "unexpected".to_owned()),
            };
            let text = format!(
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(text.as_bytes());
        }
    });
    (format!("http://{address}"), seen)
}

fn client(base: &str) -> GithubApi {
    GithubApi::new(base, Some("t0ken".to_owned()), Duration::ZERO)
}

fn paths(seen: &Seen) -> Vec<String> {
    seen.lock()
        .unwrap()
        .iter()
        .map(|r| r.lines().next().unwrap().to_owned())
        .collect()
}

#[test]
fn the_latest_release_is_its_tag_and_a_repository_with_none_is_none() {
    let (base, seen) = github(vec![
        (
            "/repos/o/r/releases/latest",
            vec![(200, r#"{"tag_name":"v1.2.3"}"#)],
        ),
        ("/repos/o/none/releases/latest", vec![(404, "{}")]),
        ("/repos/o/odd/releases/latest", vec![(200, "{}")]),
        ("/repos/o/down/releases/latest", vec![(403, "{}")]),
    ]);
    let api = client(&base);
    assert_eq!(api.latest_release("o/r"), Ok(Some("v1.2.3".to_owned())));
    assert_eq!(api.latest_release("o/none"), Ok(None));
    assert_eq!(
        api.latest_release("o/odd").unwrap_err(),
        "GET /repos/o/odd/releases/latest answered no tag_name"
    );
    assert_eq!(
        api.latest_release("o/down").unwrap_err(),
        "GET /repos/o/down/releases/latest answered HTTP 403"
    );
    assert_eq!(paths(&seen).len(), 4);
}

#[test]
fn a_request_carries_the_accept_header_and_the_token_when_there_is_one() {
    let (base, seen) = github(vec![(
        "/repos/o/r/releases/latest",
        vec![(200, r#"{"tag_name":"v1"}"#)],
    )]);
    client(&base).latest_release("o/r").unwrap();
    GithubApi::new(&base, None, Duration::ZERO)
        .latest_release("o/r")
        .unwrap();
    let requests = seen.lock().unwrap().clone();
    assert!(
        requests[0].contains("accept: application/vnd.github+json\r\n"),
        "{}",
        requests[0]
    );
    assert!(
        requests[0].contains("authorization: bearer t0ken\r\n"),
        "{}",
        requests[0]
    );
    assert!(requests[1].contains("accept: application/vnd.github+json\r\n"));
    assert!(!requests[1].contains("authorization:"), "{}", requests[1]);
}

#[test]
fn a_5xx_is_retried_up_to_three_attempts_and_a_404_is_an_answer_that_is_not() {
    let (base, seen) = github(vec![
        (
            "/repos/o/flaky/releases/latest",
            vec![(503, "{}"), (502, "{}"), (200, r#"{"tag_name":"v9"}"#)],
        ),
        ("/repos/o/down/releases/latest", vec![(500, "{}")]),
        ("/repos/o/gone/releases/latest", vec![(404, "{}")]),
    ]);
    let api = client(&base);
    assert_eq!(api.latest_release("o/flaky"), Ok(Some("v9".to_owned())));
    assert_eq!(paths(&seen).len(), 3);
    assert_eq!(
        api.latest_release("o/down").unwrap_err(),
        "GET /repos/o/down/releases/latest answered HTTP 500"
    );
    assert_eq!(paths(&seen).len(), 6, "three more attempts, no more");
    assert_eq!(api.latest_release("o/gone"), Ok(None));
    assert_eq!(paths(&seen).len(), 7, "a 404 is not retried");
}

#[test]
fn a_tag_resolves_to_its_commit_and_an_annotated_tag_is_followed_to_it() {
    let (base, seen) = github(vec![
        (
            "/repos/o/r/git/ref/tags/v1.0.0",
            vec![(200, r#"{"object":{"type":"commit","sha":"c0ffee"}}"#)],
        ),
        (
            "/repos/o/r/git/ref/tags/v2.0.0",
            vec![(200, r#"{"object":{"type":"tag","sha":"t1"}}"#)],
        ),
        (
            "/repos/o/r/git/tags/t1",
            vec![(200, r#"{"object":{"type":"tag","sha":"t2"}}"#)],
        ),
        (
            "/repos/o/r/git/tags/t2",
            vec![(200, r#"{"object":{"type":"commit","sha":"abcdef"}}"#)],
        ),
        (
            "/repos/o/r/git/ref/tags/v3.0.0",
            vec![(200, r#"{"object":{"type":"tree","sha":"x"}}"#)],
        ),
    ]);
    let api = client(&base);
    assert_eq!(api.tag_commit("o/r", "v1.0.0"), Ok("c0ffee".to_owned()));
    assert_eq!(api.tag_commit("o/r", "v2.0.0"), Ok("abcdef".to_owned()));
    assert_eq!(
        api.tag_commit("o/r", "v3.0.0").unwrap_err(),
        "o/r tag v3.0.0 points to a tree, not a commit"
    );
    assert_eq!(
        api.tag_commit("o/r", "v9.9.9").unwrap_err(),
        "GET /repos/o/r/git/ref/tags/v9.9.9 answered HTTP 500"
    );
    let seen = paths(&seen);
    assert!(
        seen.contains(&"get /repos/o/r/git/tags/t2 http/1.1".to_owned()),
        "{seen:?}"
    );
}

#[test]
fn a_tag_in_a_path_is_percent_encoded_like_encode_uri_component() {
    assert_eq!(encode_component("v1.0.0-rc.1"), "v1.0.0-rc.1");
    assert_eq!(encode_component("a b/é"), "a%20b%2F%C3%A9");
    assert_eq!(encode_component("!~*'()-_."), "!~*'()-_.");
    assert_eq!(encode_component("+#?&="), "%2B%23%3F%26%3D");
    let (base, seen) = github(vec![(
        "/repos/o/r/git/ref/tags/a%20b%2F%C3%A9",
        vec![(200, r#"{"object":{"type":"commit","sha":"c0ffee"}}"#)],
    )]);
    assert_eq!(
        client(&base).tag_commit("o/r", "a b/é"),
        Ok("c0ffee".to_owned())
    );
    assert_eq!(
        paths(&seen),
        ["get /repos/o/r/git/ref/tags/a%20b%2f%c3%a9 http/1.1"]
    );
}

#[test]
fn a_release_is_published_only_when_it_exists_and_is_not_a_draft() {
    let (base, _) = github(vec![
        (
            "/repos/o/r/releases/tags/v1",
            vec![(200, r#"{"draft":false}"#)],
        ),
        (
            "/repos/o/r/releases/tags/v2",
            vec![(200, r#"{"draft":true}"#)],
        ),
        ("/repos/o/r/releases/tags/v3", vec![(404, "{}")]),
        ("/repos/o/r/releases/tags/v4", vec![(200, "{}")]),
    ]);
    let api = client(&base);
    assert_eq!(api.is_release("o/r", "v1"), Ok(true));
    assert_eq!(api.is_release("o/r", "v2"), Ok(false));
    assert_eq!(api.is_release("o/r", "v3"), Ok(false));
    assert_eq!(api.is_release("o/r", "v4"), Ok(false));
}

#[test]
fn a_commit_exists_when_the_repository_answers_200_for_it() {
    let sha = "a".repeat(40);
    let (base, _) = github(vec![(
        &format!("/repos/o/r/commits/{sha}"),
        vec![(200, "{}")],
    )]);
    let api = client(&base);
    assert_eq!(api.commit_exists("o/r", &sha), Ok(true));
    assert_eq!(api.commit_exists("o/r", &"b".repeat(40)), Ok(false));
}

#[test]
fn a_server_that_cannot_be_reached_is_an_error_not_an_answer() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let api = client(&format!("http://127.0.0.1:{port}"));
    assert!(api.latest_release("o/r").is_err());
    assert!(api.is_release("o/r", "v1").is_err());
}

#[test]
fn the_whole_update_runs_against_a_server_standing_in_for_github() {
    let sha_c = sha('c');
    let (base, seen) = github(vec![
        (
            "/repos/actions/checkout/releases/latest",
            vec![(200, r#"{"tag_name":"v1.1.0"}"#)],
        ),
        (
            "/repos/actions/checkout/releases/tags/v1.1.0",
            vec![(200, r#"{"draft":false}"#)],
        ),
        (
            "/repos/actions/checkout/git/ref/tags/v1.1.0",
            vec![(
                200,
                &format!(r#"{{"object":{{"type":"commit","sha":"{sha_c}"}}}}"#),
            )],
        ),
        (
            &format!("/repos/actions/checkout/commits/{sha_c}"),
            vec![(200, "{}")],
        ),
        (
            "/repos/actions/setup-node/releases/latest",
            vec![(200, r#"{"tag_name":"v2.0.0"}"#)],
        ),
        (
            "/repos/actions/setup-node/git/ref/tags/v2.0.0",
            vec![(
                200,
                &format!(r#"{{"object":{{"type":"commit","sha":"{}"}}}}"#, sha('b')),
            )],
        ),
    ]);
    let (dir, options) = temp_pins(&script_pins());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = execute(&options, &client(&base), NOW, &mut out, &mut err);
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert!(
        std::fs::read_to_string(dir.path().join("Pins.pkl"))
            .unwrap()
            .contains("v1.1.0")
    );
    assert_eq!(
        paths(&seen),
        vec![
            "get /repos/actions/checkout/releases/latest http/1.1".to_owned(),
            "get /repos/actions/checkout/releases/tags/v1.1.0 http/1.1".to_owned(),
            "get /repos/actions/checkout/git/ref/tags/v1.1.0 http/1.1".to_owned(),
            format!("get /repos/actions/checkout/commits/{sha_c} http/1.1"),
            "get /repos/actions/setup-node/releases/latest http/1.1".to_owned(),
            "get /repos/actions/setup-node/git/ref/tags/v2.0.0 http/1.1".to_owned(),
        ]
    );
}

#[test]
fn the_token_is_github_token_then_gh_token_and_an_empty_one_is_no_token() {
    let some = |t: &str| Some(t.to_owned());
    assert_eq!(token_from(some("a"), some("b")), some("a"));
    assert_eq!(token_from(None, some("b")), some("b"));
    assert_eq!(token_from(None, None), None);
    // Empty but set: chosen, so it ends the search (like ?? in the TypeScript), and then it is no token.
    assert_eq!(token_from(some(""), some("b")), None);
    assert_eq!(token_from(None, some("")), None);
    assert_eq!(token_from(some("a"), some("")), some("a"));
}
