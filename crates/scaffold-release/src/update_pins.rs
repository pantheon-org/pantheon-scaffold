//! Pin updates. Ported from `runner/src/pinupdate.ts` and `runner/scripts/update-pins.ts` (Phase 3 of
//! `docs/ts-tooling-rust-port.md`).
//!
//! For each action pinned in `pkl/Pins.pkl`: find its latest published release, resolve the tag to a commit, check that the commit
//! belongs to the upstream repository and that the tag is a release, and move the pin only to a newer release. A pin whose commit is
//! not the commit of its recorded tag is a problem too. Nothing here writes to GitHub, and a GitHub error is a problem, never
//! "unchanged". The messages, the order of the requests and the exit codes follow the TypeScript, so the two can be compared
//! (`runner/scripts/update-pins-diff.sh`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use scaffold_core::freshness::render_freshness;
use scaffold_core::http::get_with_status;
use scaffold_core::semver::{compare_semver, parse_semver};
use serde_json::Value;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinLine {
    pub name: String,
    pub action: String,
    pub sha: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinChange {
    pub name: String,
    pub action: String,
    pub from: String,
    pub to: String,
    pub to_sha: String,
    pub major: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinProblem {
    pub name: String,
    pub message: String,
}

/// Every `Name: Pin = new { action = "..."; sha = "..."; version = "..." }` line of `Pins.pkl`. A line that looks like a pin and
/// cannot be read is an error, never skipped.
pub fn parse_pins(text: &str) -> Result<Vec<PinLine>, String> {
    static LINE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r#"^(?P<name>[A-Za-z][A-Za-z0-9]*): Pin = new \{ action = "(?P<action>[^"]+)"; sha = "(?P<sha>[0-9a-f]{40})"; version = "(?P<version>[^"]+)" \}$"#,
        )
        .expect("valid")
    });
    let mut pins = Vec::new();
    for line in text.split('\n') {
        if !line.contains(": Pin = new {") {
            continue;
        }
        let Some(m) = LINE.captures(line) else {
            return Err(format!("cannot read the pin: {line}"));
        };
        pins.push(PinLine {
            name: m["name"].to_owned(),
            action: m["action"].to_owned(),
            sha: m["sha"].to_owned(),
            version: m["version"].to_owned(),
        });
    }
    Ok(pins)
}

/// `owner/repo` of an action, which may carry a path (`owner/repo/.github/workflows/x.yml`).
pub fn repo_of(action: &str) -> String {
    action.split('/').take(2).collect::<Vec<_>>().join("/")
}

/// The four answers the update needs from GitHub. The error is the message to show.
pub trait PinApi {
    /// The tag of the latest published release (not a draft or a pre-release), or `None` when the repository has none.
    fn latest_release(&self, repo: &str) -> Result<Option<String>, String>;
    /// The commit a tag points to, with an annotated tag followed to its commit.
    fn tag_commit(&self, repo: &str, tag: &str) -> Result<String, String>;
    /// Whether the tag has a published release.
    fn is_release(&self, repo: &str, tag: &str) -> Result<bool, String>;
    /// Whether the commit exists in the repository.
    fn commit_exists(&self, repo: &str, sha: &str) -> Result<bool, String>;
}

/// The changes to make and the problems found, for every pin in `pins`, in the order of the file.
pub fn update_pins(pins: &[PinLine], api: &dyn PinApi) -> (Vec<PinChange>, Vec<PinProblem>) {
    let mut changes = Vec::new();
    let mut problems = Vec::new();
    for pin in pins {
        match check_pin(pin, api) {
            Ok(Some(change)) => changes.push(change),
            Ok(None) => {}
            Err(message) => problems.push(PinProblem {
                name: pin.name.clone(),
                message,
            }),
        }
    }
    (changes, problems)
}

/// One pin: a change to make, nothing to do, or the reason it cannot be moved. A GitHub error is prefixed with the action, as the
/// TypeScript did when it caught it.
fn check_pin(pin: &PinLine, api: &dyn PinApi) -> Result<Option<PinChange>, String> {
    let repo = repo_of(&pin.action);
    let action = &pin.action;
    let asked = |e: String| format!("{action}: {e}");
    let current = parse_semver(&pin.version);
    let Some(latest) = api.latest_release(&repo).map_err(asked)? else {
        return Err(format!("no release found on {repo}"));
    };
    let next = parse_semver(&latest);
    let (Some(current), Some(next)) = (current, next) else {
        let shown = if parse_semver(&pin.version).is_none() {
            &pin.version
        } else {
            &latest
        };
        return Err(format!(
            "{action}: {shown} is not a version, the update will not guess"
        ));
    };
    let newer = compare_semver(&latest, &pin.version).map_err(asked)?;
    if newer.is_le() {
        // Up to date (or the latest release is older): the pin must still be the commit of its recorded tag.
        let commit = api.tag_commit(&repo, &pin.version).map_err(asked)?;
        if commit != pin.sha {
            return Err(format!(
                "{action} {} resolves to {commit}, not the pinned {}",
                pin.version, pin.sha
            ));
        }
        return Ok(None);
    }
    if !api.is_release(&repo, &latest).map_err(asked)? {
        return Err(format!(
            "{action} {latest} is a tag but not a published release"
        ));
    }
    let to_sha = api.tag_commit(&repo, &latest).map_err(asked)?;
    if !api.commit_exists(&repo, &to_sha).map_err(asked)? {
        return Err(format!(
            "{action} {latest} resolves to {to_sha}, which does not belong to {repo}"
        ));
    }
    Ok(Some(PinChange {
        name: pin.name.clone(),
        action: pin.action.clone(),
        from: pin.version.clone(),
        to: latest,
        to_sha,
        major: next.major != current.major,
    }))
}

/// Rewrites the sha and version of the named pin lines. Everything else in the file is left as it is, and applying a change twice
/// is harmless.
pub fn apply_changes(text: &str, changes: &[PinChange]) -> Result<String, String> {
    static VALUES: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"sha = "[0-9a-f]{40}"; version = "[^"]+""#).expect("valid")
    });
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    for c in changes {
        let start = format!("{}: Pin = new {{", c.name);
        let Some(line) = lines.iter_mut().find(|l| l.starts_with(&start)) else {
            return Err(format!("no pin named {} in the file", c.name));
        };
        let new = format!(r#"sha = "{}"; version = "{}""#, c.to_sha, c.to);
        *line = VALUES.replace(line, regex::NoExpand(&new)).into_owned();
    }
    Ok(lines.join("\n"))
}

/// The text of the pull request body: one line for each pin that moved.
pub fn summary(changes: &[PinChange]) -> String {
    let rows: Vec<String> = changes
        .iter()
        .map(|c| {
            let major = if c.major {
                " (**major**, read its release notes)"
            } else {
                ""
            };
            format!("- `{}` {} to {}{major}", c.action, c.from, c.to)
        })
        .collect();
    format!(
        "Moves the pinned actions to their latest published release. Each commit was checked to belong to the upstream repository and the tag to be a release.\n\n{}\n",
        rows.join("\n")
    )
}

/// JavaScript's `encodeURIComponent`: everything but letters, digits and `-_.!~*'()` is written as `%XX` for each byte of its UTF-8.
fn encode_component(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The real answers, from the GitHub REST API. A 5xx is retried, a 404 is an answer.
pub struct GithubApi {
    base: String,
    token: Option<String>,
    delay: Duration,
}

/// Attempts at one request, the first one included.
const ATTEMPTS: u32 = 3;
const TIMEOUT: Duration = Duration::from_secs(20);

impl GithubApi {
    pub fn new(base: &str, token: Option<String>, delay: Duration) -> GithubApi {
        GithubApi {
            base: base.to_owned(),
            token,
            delay,
        }
    }

    /// The status of the answer and its body as JSON (`None` for a 404, and for a body that is not JSON).
    fn get(&self, path: &str) -> Result<(u16, Option<Value>), String> {
        let url = format!("{}{path}", self.base);
        let authorization = self.token.as_ref().map(|t| format!("Bearer {t}"));
        let mut headers = vec![("Accept", "application/vnd.github+json")];
        if let Some(value) = &authorization {
            headers.push(("Authorization", value));
        }
        for attempt in 1..=ATTEMPTS {
            let response = get_with_status(&url, TIMEOUT, &headers)?;
            if response.status >= 500 && attempt < ATTEMPTS {
                std::thread::sleep(self.delay);
                continue;
            }
            let json = if response.status == 404 {
                None
            } else {
                serde_json::from_str(&response.body).ok()
            };
            return Ok((response.status, json));
        }
        unreachable!("the loop returns on the last attempt")
    }

    fn must(&self, path: &str) -> Result<Value, String> {
        let (status, json) = self.get(path)?;
        match (status, json) {
            (200, Some(json)) => Ok(json),
            (200, None) => Err(format!("GET {path} answered a body that is not JSON")),
            (status, _) => Err(format!("GET {path} answered HTTP {status}")),
        }
    }
}

impl PinApi for GithubApi {
    fn latest_release(&self, repo: &str) -> Result<Option<String>, String> {
        let path = format!("/repos/{repo}/releases/latest");
        let (status, json) = self.get(&path)?;
        if status == 404 {
            return Ok(None);
        }
        if status != 200 {
            return Err(format!("GET {path} answered HTTP {status}"));
        }
        json.as_ref()
            .and_then(|j| j.get("tag_name"))
            .and_then(Value::as_str)
            .map(|tag| Some(tag.to_owned()))
            .ok_or_else(|| format!("GET {path} answered no tag_name"))
    }

    fn tag_commit(&self, repo: &str, tag: &str) -> Result<String, String> {
        let mut object = self
            .must(&format!(
                "/repos/{repo}/git/ref/tags/{}",
                encode_component(tag)
            ))?
            .get("object")
            .cloned()
            .unwrap_or(Value::Null);
        while object.get("type").and_then(Value::as_str) == Some("tag") {
            let sha = object
                .get("sha")
                .and_then(Value::as_str)
                .unwrap_or_default();
            object = self
                .must(&format!("/repos/{repo}/git/tags/{sha}"))?
                .get("object")
                .cloned()
                .unwrap_or(Value::Null);
        }
        let kind = object.get("type").and_then(Value::as_str);
        if kind != Some("commit") {
            return Err(format!(
                "{repo} tag {tag} points to a {}, not a commit",
                kind.unwrap_or("undefined")
            ));
        }
        object
            .get("sha")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("{repo} tag {tag} has no commit sha"))
    }

    fn is_release(&self, repo: &str, tag: &str) -> Result<bool, String> {
        let (status, json) = self.get(&format!(
            "/repos/{repo}/releases/tags/{}",
            encode_component(tag)
        ))?;
        Ok(status == 200
            && json
                .as_ref()
                .and_then(|j| j.get("draft"))
                .and_then(Value::as_bool)
                == Some(false))
    }

    fn commit_exists(&self, repo: &str, sha: &str) -> Result<bool, String> {
        Ok(self.get(&format!("/repos/{repo}/commits/{sha}"))?.0 == 200)
    }
}

pub const USAGE: &str = "usage: scaffold-release update-pins [--pins <file>] [--no-write] [--freshness-out <file>] [--summary <file>]";

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub pins: PathBuf,
    pub no_write: bool,
    pub freshness_out: Option<PathBuf>,
    pub summary: Option<PathBuf>,
}

/// The options, or the message for a usage error (an empty message means print the usage line).
pub fn parse_options(args: &[&str], root: &Path) -> Result<Options, String> {
    let mut options = Options {
        pins: root.join("pkl/Pins.pkl"),
        no_write: false,
        freshness_out: None,
        summary: None,
    };
    let mut iter = args.iter();
    while let Some(&arg) = iter.next() {
        let mut value = || {
            iter.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        match arg {
            "--no-write" => options.no_write = true,
            "--pins" => options.pins = value()?,
            "--freshness-out" => options.freshness_out = Some(value()?),
            "--summary" => options.summary = Some(value()?),
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    Ok(options)
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Checks every pin, and writes what the options ask for. Exit 0: every pin was checked (changes may have been written). Exit 1: a
/// pin failed a check, and nothing is written, the freshness record included, so a failing check never looks like a successful one.
pub fn execute(
    options: &Options,
    api: &dyn PinApi,
    now: OffsetDateTime,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let mut run = || -> Result<i32, String> {
        let text = std::fs::read_to_string(&options.pins)
            .map_err(|e| format!("{}: {e}", options.pins.display()))?;
        let pins = parse_pins(&text)?;
        let (changes, problems) = update_pins(&pins, api);

        let say =
            |out: &mut dyn Write, line: String| writeln!(out, "{line}").map_err(|e| e.to_string());
        say(out, format!("checked {} pin(s)", pins.len()))?;
        for c in &changes {
            let major = if c.major { " (major)" } else { "" };
            say(out, format!("  {} {} -> {}{major}", c.action, c.from, c.to))?;
        }
        if changes.is_empty() {
            say(out, "no pin changed".to_owned())?;
        }
        if !problems.is_empty() {
            for p in &problems {
                say(err, format!("problem with {}: {}", p.name, p.message))?;
            }
            say(
                err,
                format!(
                    "{} problem(s): nothing was written, the freshness record included",
                    problems.len()
                ),
            )?;
            return Ok(1);
        }
        if options.no_write {
            say(out, "no-write run: nothing written".to_owned())?;
            return Ok(0);
        }
        if !changes.is_empty() {
            write(&options.pins, &apply_changes(&text, &changes)?)?;
        }
        if let Some(path) = &options.freshness_out {
            write(path, &render_freshness(now))?;
        }
        if let (Some(path), false) = (&options.summary, changes.is_empty()) {
            write(path, &summary(&changes))?;
        }
        Ok(0)
    };
    match run() {
        Ok(code) => code,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}

/// The token to send: `GITHUB_TOKEN` if it is set, otherwise `GH_TOKEN`, and none when what is chosen is empty. A `GITHUB_TOKEN` that is
/// set but empty is chosen and so ends the search, like `??` in the TypeScript, which does not fall through on an empty string.
pub fn token_from(github_token: Option<String>, gh_token: Option<String>) -> Option<String> {
    github_token.or(gh_token).filter(|token| !token.is_empty())
}

/// `update-pins [options]`, with the real GitHub. Reads `GITHUB_TOKEN` (then `GH_TOKEN`) and `PANTHEON_SCAFFOLD_GITHUB_API`, which
/// replaces the API address for tests, and `pkl/Pins.pkl` under `root` unless `--pins` names another file.
pub fn command(args: &[&str], root: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let options = match parse_options(args, root) {
        Ok(options) => options,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 2;
        }
    };
    let token = token_from(
        std::env::var("GITHUB_TOKEN").ok(),
        std::env::var("GH_TOKEN").ok(),
    );
    let base = std::env::var("PANTHEON_SCAFFOLD_GITHUB_API")
        .unwrap_or_else(|_| "https://api.github.com".to_owned());
    let api = GithubApi::new(&base, token, Duration::from_secs(2));
    execute(&options, &api, OffsetDateTime::now_utc(), out, err)
}

#[cfg(test)]
mod tests;
