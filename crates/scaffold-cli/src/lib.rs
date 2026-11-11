//! The pantheon-scaffold command line, ported from `runner/src/cli.ts`.
//!
//! This build handles every command of the TypeScript runner: `--version`, `contract`, `render`, `check`, `detach`,
//! `freshness`, `latest` and `upgrade`.

use std::io::Write;
use std::path::{Path, PathBuf};

/// The version the binary was built for: `SCAFFOLD_VERSION` at build time, or 0.0.0 when it is not set.
pub const VERSION: &str = match option_env!("SCAFFOLD_VERSION") {
    Some(version) => version,
    None => "0.0.0",
};

/// Exit codes: 0 ok, 1 error, 2 usage, 3 refused (a hand edit, an existing file, the delete guard, an older package or
/// `--strict`), 4 drift (`check` found a difference).
pub const EXIT_OK: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_REFUSED: i32 = 3;
pub const EXIT_DRIFT: i32 = 4;

pub const USAGE: &str = "usage (exit codes: 0 ok, 1 error, 2 usage, 3 refused, 4 drift; --dry-run exits 3 when the render would be refused, otherwise 0):
  pantheon-scaffold --version
  pantheon-scaffold contract <pantheon.pkl>
  pantheon-scaffold render <pantheon.pkl> [--root <dir>] [--dry-run] [--force] [--repair] [--strict] [--cache-dir <dir>] [--allow-local-package <dir>]... [--attempts <n>]
  pantheon-scaffold check <pantheon.pkl> [--root <dir>] [--strict] [--cache-dir <dir>] [--allow-local-package <dir>]... [--attempts <n>]
  pantheon-scaffold detach [--root <dir>] [--dry-run] [--force]
  pantheon-scaffold latest [--include-prereleases] [--url <url>] [--root <dir>] [--attempts <n>]
  pantheon-scaffold freshness [--url <url>] [--max-age-days <n>] [--attempts <n>]
  pantheon-scaffold upgrade <version> [--root <dir>] [--config <file>] [--dry-run] [--force] [--no-render]";

/// The default age limit of `freshness`, in days.
pub const MAX_AGE_DAYS: u64 = 30;

#[derive(Debug, PartialEq, Eq)]
pub struct Flags {
    pub include_prereleases: bool,
    pub url: Option<String>,
    pub max_age_days: u64,
    pub root: PathBuf,
    pub dry_run: bool,
    pub force: bool,
    pub repair: bool,
    pub strict: bool,
    pub no_render: bool,
    pub config: Option<String>,
    pub cache_dir: Option<PathBuf>,
    pub local: Vec<PathBuf>,
    pub attempts: u32,
}

/// An absolute path with `.` and `..` folded away, like Node's `path.resolve`. Nothing is read from the disk.
fn resolve(cwd: &Path, value: &str) -> PathBuf {
    use std::path::Component;
    let joined = cwd.join(value);
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// A whole number of at least 1, read the way JavaScript's `Number()` reads it.
fn whole_number(text: Option<&String>) -> Option<u64> {
    let n = text?.trim().parse::<f64>().ok()?;
    (n.is_finite() && n.fract() == 0.0 && (1.0..9.0e15).contains(&n)).then_some(n as u64)
}

/// Reads the options that follow the command. An error is the message to print before the usage text.
pub fn parse_flags(args: &[String], cwd: &Path) -> Result<Flags, String> {
    let mut f = Flags {
        include_prereleases: false,
        url: None,
        max_age_days: MAX_AGE_DAYS,
        root: cwd.to_owned(),
        dry_run: false,
        force: false,
        repair: false,
        strict: false,
        no_render: false,
        config: None,
        cache_dir: None,
        local: Vec::new(),
        attempts: 3,
    };
    let mut rest = args.iter();
    while let Some(a) = rest.next().map(String::as_str) {
        match a {
            "--dry-run" => f.dry_run = true,
            "--force" => f.force = true,
            "--repair" => f.repair = true,
            "--strict" => f.strict = true,
            "--no-render" => f.no_render = true,
            "--include-prereleases" => f.include_prereleases = true,
            "--max-age-days" | "--attempts" => {
                let n = whole_number(rest.next())
                    .ok_or_else(|| format!("{a} needs a whole number of at least 1"))?;
                if a == "--attempts" {
                    f.attempts = u32::try_from(n).unwrap_or(u32::MAX);
                } else {
                    f.max_age_days = n;
                }
            }
            "--url" | "--root" | "--allow-local-package" | "--cache-dir" | "--config" => {
                let value = rest
                    .next()
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| format!("{a} needs a value"))?;
                match a {
                    "--url" => f.url = Some(value.clone()),
                    "--root" => f.root = resolve(cwd, value),
                    "--cache-dir" => f.cache_dir = Some(resolve(cwd, value)),
                    "--config" => f.config = Some(value.clone()),
                    _ => f.local.push(resolve(cwd, value)),
                }
            }
            _ => return Err(format!("unknown option {a}")),
        }
    }
    Ok(f)
}

struct Io<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
}

impl Io<'_> {
    fn say(&mut self, line: &str) {
        let _ = writeln!(self.out, "{line}");
    }

    fn complain(&mut self, text: &str) {
        let _ = writeln!(self.err, "{text}");
    }
}

/// What the command line reads from the environment, read once so a test can give its own.
#[derive(Debug, Default, Clone)]
pub struct Env {
    /// `PANTHEON_SCAFFOLD_FRESHNESS_URL`: where the freshness record is fetched from.
    pub freshness_url: Option<String>,
    /// `PANTHEON_SCAFFOLD_KNOWN_BAD_URL`: where the known-bad list is fetched from.
    pub known_bad_url: Option<String>,
    /// `GITHUB_TOKEN`, or `GH_TOKEN` when that is not set: sent to the GitHub API by `latest`.
    pub github_token: Option<String>,
    /// `PANTHEON_SCAFFOLD_RELEASE_BASE`: where `upgrade` downloads a release's checksums from.
    pub release_base: Option<String>,
}

impl Env {
    pub fn from_process() -> Self {
        let var = |name: &str| std::env::var(name).ok();
        Self {
            freshness_url: var("PANTHEON_SCAFFOLD_FRESHNESS_URL"),
            known_bad_url: var("PANTHEON_SCAFFOLD_KNOWN_BAD_URL"),
            github_token: github_token(var("GITHUB_TOKEN"), var("GH_TOKEN")),
            release_base: var("PANTHEON_SCAFFOLD_RELEASE_BASE"),
        }
    }
}

/// `GITHUB_TOKEN`, or `GH_TOKEN` when the first is not set at all. A variable that is set to nothing still counts as set,
/// as it does for the TypeScript runner's `??`.
fn github_token(github: Option<String>, gh: Option<String>) -> Option<String> {
    github.or(gh)
}

/// Runs the command line and returns the exit code. `cwd` is where relative paths start.
pub fn run(argv: &[String], cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    run_with_env(argv, cwd, &Env::from_process(), out, err)
}

/// Like `run`, with the environment given instead of read.
pub fn run_with_env(
    argv: &[String],
    cwd: &Path,
    env: &Env,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let mut io = Io { out, err };
    let Some((cmd, rest)) = argv.split_first() else {
        io.complain(USAGE);
        return EXIT_USAGE;
    };
    if cmd == "--version" {
        io.say(VERSION);
        return EXIT_OK;
    }
    let no_config = matches!(cmd.as_str(), "detach" | "freshness" | "latest");
    let needs_config = matches!(cmd.as_str(), "contract" | "render" | "check" | "upgrade");
    let config = if no_config { None } else { rest.first() };
    if !(no_config || needs_config && config.is_some()) {
        io.complain(USAGE);
        return EXIT_USAGE;
    }
    let flag_args = if no_config { rest } else { &rest[1..] };
    let flags = match parse_flags(flag_args, cwd) {
        Ok(flags) => flags,
        Err(message) => {
            io.complain(&format!("{message}\n{USAGE}"));
            return EXIT_USAGE;
        }
    };
    match cmd.as_str() {
        "contract" | "render" | "check" => {
            let config = config.expect("checked above");
            command(cmd, config, cwd, &flags, env, &mut io)
        }
        "detach" => finish(run_detach(&flags, &mut io), &mut io),
        "freshness" => finish(run_freshness(&flags, env, &mut io), &mut io),
        "latest" => finish(run_latest(&flags, env, &mut io), &mut io),
        "upgrade" => {
            let version = config.expect("checked above");
            finish(run_upgrade(version, &flags, env, &mut io), &mut io)
        }
        _ => {
            io.complain(&format!(
                "{cmd} is not available in the Rust runner yet, use the TypeScript runner"
            ));
            EXIT_ERROR
        }
    }
}

/// The known-bad list: fetched, or the cached copy, or the snapshot built into the runner.
fn load_known_list(flags: &Flags, env: &Env) -> scaffold_core::knownbad::KnownBadResult {
    use scaffold_core::knownbad::{
        KNOWN_BAD_URL, KnownBadOptions, bundled_snapshot, load_known_bad,
    };
    let fetcher = |url: &str| scaffold_core::http::get_text(url, std::time::Duration::from_secs(5));
    let bundled = bundled_snapshot();
    let options = KnownBadOptions {
        url: env.known_bad_url.as_deref().unwrap_or(KNOWN_BAD_URL),
        fetcher: &fetcher,
        bundled: &bundled,
    };
    load_known_bad(&flags.root, &options)
}

fn run_latest(flags: &Flags, env: &Env, io: &mut Io) -> Result<i32, String> {
    use scaffold_core::latest::{FetchOptions, RELEASES_API, fetch_releases, latest_version};
    use std::time::Duration;
    let fetcher = |url: &str, headers: &[(&str, &str)]| {
        scaffold_core::http::get_text_with(url, Duration::from_secs(15), headers)
    };
    let options = FetchOptions {
        fetcher: &fetcher,
        url: flags.url.as_deref().unwrap_or(RELEASES_API),
        token: env.github_token.as_deref(),
        attempts: flags.attempts,
        delay: Duration::from_secs(2),
    };
    let releases = fetch_releases(&options).map_err(|e| e.to_string())?;
    let known_bad = load_known_list(flags, env).list;
    match latest_version(&releases, flags.include_prereleases, Some(&known_bad)) {
        Some(version) => {
            io.say(&version);
            Ok(EXIT_OK)
        }
        None => {
            let hint = if flags.include_prereleases {
                ""
            } else {
                ". Use --include-prereleases to offer pre-releases"
            };
            io.complain(&format!(
                "no release found that qualifies (stable, not a draft, not known bad){hint}"
            ));
            Ok(EXIT_ERROR)
        }
    }
}

fn run_upgrade(version: &str, flags: &Flags, env: &Env, io: &mut Io) -> Result<i32, String> {
    use scaffold_core::upgrade::{SystemDeps, UpgradeOptions, upgrade};
    let options = UpgradeOptions {
        config: flags.config.clone(),
        dry_run: flags.dry_run,
        force: flags.force,
        no_render: flags.no_render,
        base_url: env.release_base.clone(),
        ..UpgradeOptions::default()
    };
    let report = upgrade(&flags.root, version, &options, &SystemDeps::default())
        .map_err(|e| e.to_string())?;
    let verb = if report.dry_run {
        "would change"
    } else {
        "changed"
    };
    for change in &report.changes {
        io.say(&format!("{verb}  {change}"));
    }
    if report.dry_run {
        io.say("dry run: the release was verified, nothing was changed");
        return Ok(EXIT_OK);
    }
    if let Some(code) = report.render_exit.filter(|code| *code != 0) {
        io.complain(&format!("upgrade: the pins are updated, but the render exited {code}. Fix what it reports and run render again."));
        return Ok(code);
    }
    let hint = if report.render_exit.is_none() {
        ", run render with the new runner"
    } else {
        ""
    };
    io.say(&format!("upgraded {} -> {}{hint}", report.from, report.to));
    Ok(EXIT_OK)
}

/// An error stops the command with exit 1 and its message.
fn finish(result: Result<i32, String>, io: &mut Io) -> i32 {
    result.unwrap_or_else(|message| {
        io.complain(&message);
        EXIT_ERROR
    })
}

/// The record's address: the flag, then `PANTHEON_SCAFFOLD_FRESHNESS_URL`, then the package repository's own.
fn freshness_url(flags: &Flags, env_value: Option<String>) -> String {
    let from_env = env_value.filter(|value| !value.is_empty());
    flags
        .url
        .clone()
        .or(from_env)
        .unwrap_or_else(|| scaffold_core::freshness::FRESHNESS_URL.to_owned())
}

fn run_freshness(flags: &Flags, env: &Env, io: &mut Io) -> Result<i32, String> {
    use scaffold_core::freshness::{CheckOptions, State, check_freshness};
    use std::time::Duration;
    let url = freshness_url(flags, env.freshness_url.clone());
    let fetcher = |url: &str| scaffold_core::http::get_text(url, Duration::from_secs(15));
    let options = CheckOptions {
        fetcher: &fetcher,
        url: &url,
        now: time::OffsetDateTime::now_utc(),
        max_age_days: flags.max_age_days,
        attempts: flags.attempts,
        delay: Duration::from_secs(2),
    };
    let result = check_freshness(&options).map_err(|e| e.to_string())?;
    let when = format!("{} day(s) ago ({})", result.age_days, result.checked_at);
    if result.state == State::Stale {
        io.complain(&format!(
            "stale: the last successful pin check was {when}, more than {}. The pin-update workflow has stopped running or failing.",
            flags.max_age_days
        ));
        return Ok(EXIT_REFUSED);
    }
    io.say(&format!("fresh: the last successful pin check was {when}"));
    Ok(EXIT_OK)
}

fn run_detach(flags: &Flags, io: &mut Io) -> Result<i32, String> {
    use scaffold_core::detach::{DetachKind, DetachOptions, detach};
    let options = DetachOptions {
        dry_run: flags.dry_run,
        force: flags.force,
        ..DetachOptions::default()
    };
    let report = detach(&flags.root, &options).map_err(|e| e.to_string())?;
    for action in &report.actions {
        io.say(&format!("{:<9} {}", action.kind_name(), action.path));
    }
    if !report.refusals.is_empty() {
        let lines: Vec<String> = report.refusals.iter().map(|r| format!("  {r}")).collect();
        io.complain(&format!(
            "refused, nothing was changed:\n{}",
            lines.join("\n")
        ));
        return Ok(EXIT_REFUSED);
    }
    let deleted = report
        .actions
        .iter()
        .filter(|a| a.kind == DetachKind::Delete)
        .count();
    io.say(&if report.dry_run {
        format!("dry run: would delete {deleted} file(s) and remove the manifest")
    } else {
        format!("detached: deleted {deleted} file(s) and removed the manifest, every other file is a plain file now")
    });
    Ok(EXIT_OK)
}

fn command(cmd: &str, config: &str, cwd: &Path, flags: &Flags, env: &Env, io: &mut Io) -> i32 {
    match command_inner(cmd, config, cwd, flags, env, io) {
        Ok(code) => code,
        Err(message) => {
            io.complain(&message);
            EXIT_ERROR
        }
    }
}

fn command_inner(
    cmd: &str,
    config: &str,
    cwd: &Path,
    flags: &Flags,
    env: &Env,
    io: &mut Io,
) -> Result<i32, String> {
    let contract = load_contract(config, cwd, flags)?;
    if cmd == "contract" {
        let hatch = &contract.escape_hatch;
        io.say(&format!(
            "contract ok: {} files, {} pins, escape hatch {} of {} constructs",
            contract.files.len(),
            contract.pins.len(),
            hatch.uses,
            hatch.constructs
        ));
        return Ok(EXIT_OK);
    }
    let mode = mode_for(cmd);
    let package_version = match gate(&contract, flags, mode, env, io)? {
        Gate::Stop(code) => return Ok(code),
        Gate::Go(version) => version,
    };
    if cmd == "check" {
        run_check(&contract, flags, io)
    } else {
        run_render(&contract, flags, package_version, io)
    }
}

/// `check` writes nothing, so an older package only warns there; `render` refuses it.
fn mode_for(cmd: &str) -> scaffold_core::preflight::Mode {
    if cmd == "check" {
        scaffold_core::preflight::Mode::Check
    } else {
        scaffold_core::preflight::Mode::Render
    }
}

fn eval_options(flags: &Flags) -> scaffold_core::pkl::EvalOptions {
    let cache_dir = flags
        .cache_dir
        .clone()
        .unwrap_or_else(|| flags.root.join(".pkl-gen/cache"));
    scaffold_core::pkl::EvalOptions {
        allow_files_under: flags.local.clone(),
        attempts: flags.attempts,
        cache_dir: Some(cache_dir),
        ..scaffold_core::pkl::EvalOptions::default()
    }
}

fn render_options(flags: &Flags, package_version: Option<String>) -> scaffold_core::apply::Options {
    scaffold_core::apply::Options {
        force: flags.force,
        dry_run: flags.dry_run,
        repair: flags.repair,
        package_version,
        ..scaffold_core::apply::Options::default()
    }
}

fn load_contract(
    config: &str,
    cwd: &Path,
    flags: &Flags,
) -> Result<scaffold_core::contract::Contract, String> {
    let options = eval_options(flags);
    let json = scaffold_core::pkl::eval_contract_json(&resolve(cwd, config), &flags.root, &options)
        .map_err(|e| e.to_string())?;
    scaffold_core::contract::parse_contract(&json).map_err(|e| e.to_string())
}

enum Gate {
    Stop(i32),
    Go(Option<String>),
}

/// The rules checked before anything is rendered or compared.
fn gate(
    contract: &scaffold_core::contract::Contract,
    flags: &Flags,
    mode: scaffold_core::preflight::Mode,
    env: &Env,
    io: &mut Io,
) -> Result<Gate, String> {
    use scaffold_core::preflight::{PreflightOptions, package_version_of, preflight};
    let known_bad = match package_version_of(contract.package_uri.as_deref()) {
        Err(message) => {
            io.complain(&message);
            return Ok(Gate::Stop(EXIT_ERROR));
        }
        // Only a package loaded by version can be on the list; a local package never is.
        Ok(Some(_)) => {
            let loaded = load_known_list(flags, env);
            if let Some(note) = &loaded.note {
                io.complain(&format!("warning: {note}"));
            }
            Some(loaded.list)
        }
        Ok(None) => None,
    };
    // --repair exists to fix a manifest that cannot be read, so it must not read it here.
    let manifest = if flags.repair {
        scaffold_core::manifest::Manifest::empty()
    } else {
        scaffold_core::manifest::read_manifest(&flags.root).map_err(|e| e.to_string())?
    };
    let options = PreflightOptions {
        mode,
        runner_version: VERSION,
        force: flags.force,
        strict: flags.strict,
        known_bad: known_bad.as_ref(),
    };
    let pre = preflight(contract, &manifest, &options);
    for warning in &pre.warnings {
        io.complain(&format!("warning: {warning}"));
    }
    if !pre.errors.is_empty() {
        io.complain(&pre.errors.join("\n"));
        return Ok(Gate::Stop(EXIT_ERROR));
    }
    if !pre.refusals.is_empty() {
        let lines: Vec<String> = pre.refusals.iter().map(|r| format!("  {r}")).collect();
        io.complain(&format!(
            "refused, nothing was written:\n{}",
            lines.join("\n")
        ));
        return Ok(Gate::Stop(EXIT_REFUSED));
    }
    Ok(Gate::Go(pre.package_version))
}

fn run_check(
    contract: &scaffold_core::contract::Contract,
    flags: &Flags,
    io: &mut Io,
) -> Result<i32, String> {
    use scaffold_core::check::{Reason, check};
    let result = check(contract, &flags.root).map_err(|e| e.to_string())?;
    if result.drift.is_empty() {
        io.say(&format!(
            "check ok: {} owned file(s) match the render",
            result.checked
        ));
        return Ok(EXIT_OK);
    }
    let lines: Vec<String> = result
        .drift
        .iter()
        .map(|d| {
            let reason = match d.reason {
                Reason::Missing => "missing",
                Reason::Different => "different",
                Reason::Mode => "mode",
            };
            let diff = if d.diff.is_empty() {
                String::new()
            } else {
                format!("\n{}", d.diff)
            };
            format!("  {}: {reason}{diff}", d.path)
        })
        .collect();
    io.complain(&format!(
        "drift in {} of {} owned file(s):\n{}",
        result.drift.len(),
        result.checked,
        lines.join("\n")
    ));
    Ok(EXIT_DRIFT)
}

fn run_render(
    contract: &scaffold_core::contract::Contract,
    flags: &Flags,
    package_version: Option<String>,
    io: &mut Io,
) -> Result<i32, String> {
    use scaffold_core::apply::{Kind, apply};
    let options = render_options(flags, package_version);
    let report = apply(contract, &flags.root, &options).map_err(|e| e.to_string())?;
    for action in &report.actions {
        io.say(&format!("{:<9} {}", action.kind.name(), action.path));
    }
    if !report.refusals.is_empty() {
        let lines: Vec<String> = report.refusals.iter().map(|r| format!("  {r}")).collect();
        io.complain(&format!(
            "refused {} file(s), nothing was written:\n{}",
            report.refusals.len(),
            lines.join("\n")
        ));
        return Ok(if flags.repair { EXIT_OK } else { EXIT_REFUSED });
    }
    let count = |kinds: &[Kind]| {
        report
            .actions
            .iter()
            .filter(|a| kinds.contains(&a.kind))
            .count()
    };
    io.say(&if report.dry_run {
        format!(
            "dry run: would write {} file(s), delete {}",
            count(&[Kind::Create, Kind::Update, Kind::Seed]),
            count(&[Kind::Delete])
        )
    } else {
        format!(
            "written {} file(s), deleted {}",
            report.written, report.deleted
        )
    });
    Ok(EXIT_OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scaffold_core::contract::{Contract, EscapeHatch};

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn flags(items: &[&str]) -> Flags {
        parse_flags(&args(items), Path::new("/work")).unwrap()
    }

    fn flag_error(items: &[&str]) -> String {
        parse_flags(&args(items), Path::new("/work")).unwrap_err()
    }

    /// Runs the command line and returns (exit code, stdout, stderr).
    fn go(items: &[&str], cwd: &Path) -> (i32, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&args(items), cwd, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn here() -> PathBuf {
        PathBuf::from("/work")
    }

    #[test]
    fn the_defaults() {
        let f = flags(&[]);
        assert_eq!(f.root, PathBuf::from("/work"));
        assert_eq!((f.attempts, f.max_age_days), (3, 30));
        assert!(
            !(f.dry_run || f.force || f.repair || f.strict || f.no_render || f.include_prereleases)
        );
        assert_eq!(
            (f.url, f.config, f.cache_dir, f.local),
            (None, None, None, vec![])
        );
    }

    #[test]
    fn each_switch_turns_on_only_itself() {
        assert!(flags(&["--dry-run"]).dry_run);
        assert!(flags(&["--force"]).force);
        assert!(flags(&["--repair"]).repair);
        assert!(flags(&["--strict"]).strict);
        assert!(flags(&["--no-render"]).no_render);
        assert!(flags(&["--include-prereleases"]).include_prereleases);
        let all = flags(&[
            "--dry-run",
            "--force",
            "--repair",
            "--strict",
            "--no-render",
            "--include-prereleases",
        ]);
        assert!(
            all.dry_run
                && all.force
                && all.repair
                && all.strict
                && all.no_render
                && all.include_prereleases
        );
        let only_force = flags(&["--force"]);
        assert!(
            !(only_force.dry_run
                || only_force.repair
                || only_force.strict
                || only_force.no_render
                || only_force.include_prereleases)
        );
    }

    #[test]
    fn options_with_a_value() {
        let f = flags(&[
            "--url",
            "http://x/y",
            "--config",
            "c.pkl",
            "--attempts",
            "5",
            "--max-age-days",
            "7",
        ]);
        assert_eq!(
            (
                f.url.as_deref(),
                f.config.as_deref(),
                f.attempts,
                f.max_age_days
            ),
            (Some("http://x/y"), Some("c.pkl"), 5, 7)
        );
    }

    #[test]
    fn paths_are_made_absolute_from_the_working_directory() {
        let f = flags(&[
            "--root",
            "sub/../x",
            "--cache-dir",
            "/abs/./c",
            "--allow-local-package",
            "p",
            "--allow-local-package",
            "../q",
        ]);
        assert_eq!(f.root, PathBuf::from("/work/x"));
        assert_eq!(f.cache_dir, Some(PathBuf::from("/abs/c")));
        assert_eq!(f.local, [PathBuf::from("/work/p"), PathBuf::from("/q")]);
    }

    #[test]
    fn a_number_option_needs_a_whole_number_of_at_least_one() {
        for flag in ["--attempts", "--max-age-days"] {
            let message = format!("{flag} needs a whole number of at least 1");
            for bad in ["0", "-2", "1.5", "abc", "", "NaN", "Infinity"] {
                assert_eq!(flag_error(&[flag, bad]), message, "{flag} {bad:?}");
            }
            assert_eq!(flag_error(&[flag]), message);
        }
        assert_eq!(flags(&["--attempts", " 4 "]).attempts, 4);
        assert_eq!(flags(&["--attempts", "2.0"]).attempts, 2);
        assert_eq!(flags(&["--attempts", "1e1"]).attempts, 10);
    }

    #[test]
    fn a_value_option_needs_a_value() {
        for flag in [
            "--url",
            "--root",
            "--allow-local-package",
            "--cache-dir",
            "--config",
        ] {
            assert_eq!(flag_error(&[flag]), format!("{flag} needs a value"));
            assert_eq!(flag_error(&[flag, ""]), format!("{flag} needs a value"));
        }
    }

    #[test]
    fn an_unknown_option_is_named() {
        assert_eq!(
            flag_error(&["--dry-run", "--nope"]),
            "unknown option --nope"
        );
        assert_eq!(flag_error(&["stray"]), "unknown option stray");
    }

    #[test]
    fn version_prints_the_build_version() {
        assert_eq!(
            go(&["--version"], &here()),
            (EXIT_OK, format!("{VERSION}\n"), String::new())
        );
    }

    #[test]
    fn no_command_or_an_unknown_one_or_a_missing_config_is_a_usage_error() {
        for items in [
            &[][..],
            &["nope"],
            &["render"],
            &["contract"],
            &["check"],
            &["upgrade"],
        ] {
            assert_eq!(
                go(items, &here()),
                (EXIT_USAGE, String::new(), format!("{USAGE}\n")),
                "{items:?}"
            );
        }
    }

    #[test]
    fn a_bad_option_prints_the_message_and_the_usage() {
        assert_eq!(
            go(&["detach", "--nope"], &here()),
            (
                EXIT_USAGE,
                String::new(),
                format!("unknown option --nope\n{USAGE}\n")
            )
        );
        assert_eq!(go(&["render", "x.pkl", "--url"], &here()).0, EXIT_USAGE);
        assert_eq!(go(&["latest", "--url"], &here()).0, EXIT_USAGE);
    }

    /// A repository pinned to 0.1.0, for the refusals `upgrade` makes before it reaches for the network.
    fn pinned_repo() -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        std::fs::write(
            repo.path().join("pantheon.pkl"),
            "amends \"package://github.com/pantheon-org/pantheon-scaffold/releases/download/v0.1.0/pantheon-scaffold@0.1.0#/Config.pkl\"\n",
        )
        .unwrap();
        std::fs::write(
            repo.path().join("mise.toml"),
            "[tools]\n\"github:pantheon-org/pantheon-scaffold\" = \"0.1.0\"\n",
        )
        .unwrap();
        repo
    }

    fn upgrade_in(repo: &Path, version: &str, extra: &[&str]) -> (i32, String, String) {
        let root = repo.display().to_string();
        let mut items = vec!["upgrade", version, "--root", root.as_str()];
        items.extend_from_slice(extra);
        // Nothing listens at the release address, so a run that got past the refusals would fail on the download.
        let env = Env {
            release_base: Some("http://127.0.0.1:1".to_owned()),
            ..Env::default()
        };
        go_env(&items, &env, &here())
    }

    #[test]
    fn upgrade_refuses_a_version_that_is_not_a_version() {
        let repo = pinned_repo();
        let (code, out, err) = upgrade_in(repo.path(), "banana", &[]);
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (EXIT_ERROR, "", "\"banana\" is not a version\n")
        );
    }

    #[test]
    fn upgrade_to_the_version_already_pinned_is_an_error() {
        let repo = pinned_repo();
        let (code, _, err) = upgrade_in(repo.path(), "0.1.0", &[]);
        assert_eq!((code, err.as_str()), (EXIT_ERROR, "already at 0.1.0\n"));
        assert_eq!(
            upgrade_in(repo.path(), "v0.1.0", &[]).2,
            "already at 0.1.0\n"
        );
    }

    #[test]
    fn upgrade_to_an_older_version_needs_force() {
        let repo = pinned_repo();
        let (code, _, err) = upgrade_in(repo.path(), "0.0.9", &[]);
        assert_eq!(code, EXIT_ERROR);
        assert_eq!(
            err,
            "0.0.9 is older than 0.1.0, which is pinned now (use --force to go back)\n"
        );
        // With --force it gets past the refusal and fails on the download instead.
        let (code, _, err) = upgrade_in(repo.path(), "0.0.9", &["--force"]);
        assert_eq!(code, EXIT_ERROR);
        assert!(
            err.starts_with("could not fetch http://127.0.0.1:1/v0.0.9/checksums.txt: "),
            "{err}"
        );
    }

    #[test]
    fn upgrade_needs_a_version_and_knows_its_options() {
        assert_eq!(
            go(&["upgrade"], &here()),
            (EXIT_USAGE, String::new(), format!("{USAGE}\n"))
        );
        let (code, _, err) = go(&["upgrade", "0.2.0", "--nope"], &here());
        assert_eq!(
            (code, err.as_str()),
            (
                EXIT_USAGE,
                format!("unknown option --nope\n{USAGE}\n").as_str()
            )
        );
    }

    #[test]
    fn check_is_the_only_command_that_runs_the_gate_in_check_mode() {
        use scaffold_core::preflight::Mode;
        assert!(mode_for("check") == Mode::Check);
        assert!(mode_for("render") == Mode::Render);
        assert!(mode_for("contract") == Mode::Render);
    }

    #[test]
    fn the_pkl_options_come_from_the_flags() {
        let given = flags(&[
            "--allow-local-package",
            "p",
            "--attempts",
            "7",
            "--cache-dir",
            "/c",
            "--root",
            "/r",
        ]);
        let o = eval_options(&given);
        assert_eq!(
            (o.allow_files_under, o.attempts, o.cache_dir),
            (vec![PathBuf::from("/work/p")], 7, Some(PathBuf::from("/c")))
        );
        let o = eval_options(&flags(&["--root", "/r"]));
        assert_eq!(
            (o.attempts, o.cache_dir),
            (3, Some(PathBuf::from("/r/.pkl-gen/cache")))
        );
    }

    #[test]
    fn the_render_options_come_from_the_flags() {
        let o = render_options(
            &flags(&["--force", "--dry-run", "--repair"]),
            Some("1.2.3".into()),
        );
        assert!(o.force && o.dry_run && o.repair);
        assert_eq!(o.package_version.as_deref(), Some("1.2.3"));
        let o = render_options(&flags(&[]), None);
        assert!(!(o.force || o.dry_run || o.repair));
        assert_eq!(o.package_version, None);
    }

    // The commands below run the real pkl against the fixtures of the TypeScript runner.
    fn checkout() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    fn minimal() -> String {
        checkout()
            .join("runner/test/fixtures/minimal/pantheon.pkl")
            .display()
            .to_string()
    }

    fn package() -> String {
        checkout().join("pkl").display().to_string()
    }

    fn with_repo(command: &str, extra: &[&str], repo: &Path) -> (i32, String, String) {
        let (config, package, root) = (minimal(), package(), repo.display().to_string());
        let mut items = vec![
            command,
            config.as_str(),
            "--root",
            root.as_str(),
            "--allow-local-package",
            package.as_str(),
        ];
        items.extend_from_slice(extra);
        go(&items, &here())
    }

    #[test]
    fn contract_reports_what_the_config_holds() {
        let (config, package) = (minimal(), package());
        let (code, out, err) = go(
            &["contract", &config, "--allow-local-package", &package],
            &checkout(),
        );
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (
                EXIT_OK,
                "contract ok: 4 files, 1 pins, escape hatch 0 of 4 constructs\n",
                ""
            )
        );
    }

    #[test]
    fn a_config_that_does_not_exist_is_an_error() {
        let (code, out, err) = go(&["contract", "/no/such/pantheon.pkl"], &checkout());
        assert_eq!((code, out.as_str()), (EXIT_ERROR, ""));
        assert!(err.starts_with("pkl eval failed (exit 1)"), "{err}");
    }

    #[test]
    fn render_then_render_again_then_check() {
        let repo = tempfile::tempdir().unwrap();
        let (code, out, err) = with_repo("render", &[], repo.path());
        assert_eq!((code, err.as_str()), (EXIT_OK, ""));
        assert_eq!(
            out,
            "create    .github/scripts/hello.sh\ncreate    .github/workflows/hello.yml\nexternal  mise.lock\ncreate    rust-toolchain.toml\nwritten 3 file(s), deleted 0\n"
        );
        let (code, out, _) = with_repo("render", &[], repo.path());
        assert_eq!(code, EXIT_OK);
        assert!(
            out.starts_with("unchanged .github/scripts/hello.sh\n"),
            "{out}"
        );
        assert!(out.ends_with("written 0 file(s), deleted 0\n"), "{out}");
        assert_eq!(
            with_repo("check", &[], repo.path()),
            (
                EXIT_OK,
                "check ok: 3 owned file(s) match the render\n".to_owned(),
                String::new()
            )
        );
    }

    #[test]
    fn check_reports_a_hand_edit_as_drift_and_render_refuses_it() {
        let repo = tempfile::tempdir().unwrap();
        with_repo("render", &[], repo.path());
        std::fs::write(repo.path().join("rust-toolchain.toml"), "edited\n").unwrap();
        let (code, out, err) = with_repo("check", &[], repo.path());
        assert_eq!((code, out.as_str()), (EXIT_DRIFT, ""));
        assert!(err.starts_with("drift in 1 of 3 owned file(s):\n  rust-toolchain.toml: different\n    line 1 on disk:  edited\n"), "{err}");
        std::fs::remove_file(repo.path().join(".github/scripts/hello.sh")).unwrap();
        assert!(with_repo("check", &[], repo.path()).2.contains("drift in 2 of 3 owned file(s):\n  .github/scripts/hello.sh: missing\n  rust-toolchain.toml: different"));
        let (code, _, err) = with_repo("render", &[], repo.path());
        assert_eq!(code, EXIT_REFUSED);
        assert!(
            err.starts_with(
                "refused 1 file(s), nothing was written:\n  rust-toolchain.toml: was edited by hand"
            ),
            "{err}"
        );
    }

    #[test]
    fn a_mode_difference_is_drift_too() {
        use std::os::unix::fs::PermissionsExt;
        let repo = tempfile::tempdir().unwrap();
        with_repo("render", &[], repo.path());
        std::fs::set_permissions(
            repo.path().join(".github/scripts/hello.sh"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        let err = with_repo("check", &[], repo.path()).2;
        assert!(
            err.contains(
                "  .github/scripts/hello.sh: mode\n    expected executable, found not executable"
            ),
            "{err}"
        );
    }

    #[test]
    fn a_dry_run_says_what_it_would_do_and_writes_nothing() {
        let repo = tempfile::tempdir().unwrap();
        let (code, out, _) = with_repo("render", &["--dry-run"], repo.path());
        assert_eq!(code, EXIT_OK);
        assert!(
            out.ends_with("dry run: would write 3 file(s), delete 0\n"),
            "{out}"
        );
        assert!(!repo.path().join(".pkl-gen").exists());
    }

    #[test]
    fn repair_lists_what_it_left_out_and_still_exits_ok() {
        let repo = tempfile::tempdir().unwrap();
        with_repo("render", &[], repo.path());
        std::fs::write(repo.path().join("rust-toolchain.toml"), "edited\n").unwrap();
        let (code, _, err) = with_repo("render", &["--repair"], repo.path());
        assert_eq!(code, EXIT_OK);
        assert!(
            err.contains(
                "rust-toolchain.toml: differs from the render, left out of the repaired manifest"
            ),
            "{err}"
        );
    }

    #[test]
    fn a_broken_manifest_is_an_error_that_points_at_repair() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".pkl-gen")).unwrap();
        std::fs::write(repo.path().join(".pkl-gen/manifest.json"), "{").unwrap();
        let (code, _, err) = with_repo("render", &[], repo.path());
        assert_eq!(code, EXIT_ERROR);
        assert!(err.contains("--repair"), "{err}");
    }

    fn git(root: &Path, items: &[&str]) {
        let mut command = std::process::Command::new("git");
        command
            .arg("-C")
            .arg(root)
            .args(["-c", "user.name=t", "-c", "user.email=t@e.invalid"])
            .args(items);
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        assert!(command.output().unwrap().status.success(), "git {items:?}");
    }

    /// A git repository holding the render of the `smoke-detach` fixture, committed.
    fn detachable() -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(
            repo.path().join(".gitignore"),
            ".pkl-gen/lock\n.pkl-gen/stage/\n.pkl-gen/cache/\n",
        )
        .unwrap();
        let config = checkout()
            .join("runner/test/fixtures/smoke-detach/pantheon.pkl")
            .display()
            .to_string();
        let (root, package) = (repo.path().display().to_string(), package());
        let (code, _, err) = go(
            &[
                "render",
                &config,
                "--root",
                &root,
                "--allow-local-package",
                &package,
            ],
            &here(),
        );
        assert_eq!((code, err.as_str()), (EXIT_OK, ""));
        git(repo.path(), &["add", "-A"]);
        git(repo.path(), &["commit", "-qm", "render"]);
        repo
    }

    fn detach_in(repo: &Path, extra: &[&str]) -> (i32, String, String) {
        let root = repo.display().to_string();
        let mut items = vec!["detach", "--root", root.as_str()];
        items.extend_from_slice(extra);
        go(&items, &here())
    }

    #[test]
    fn detach_without_a_manifest_has_nothing_to_do() {
        let repo = tempfile::tempdir().unwrap();
        let (code, out, err) = detach_in(repo.path(), &[]);
        assert_eq!((code, out.as_str()), (EXIT_ERROR, ""));
        assert_eq!(
            err,
            ".pkl-gen/manifest.json not found, there is nothing to detach\n"
        );
    }

    #[test]
    fn a_dry_run_of_detach_lists_the_plan_and_changes_nothing() {
        let repo = detachable();
        let (code, out, err) = detach_in(repo.path(), &["--dry-run"]);
        assert_eq!((code, err.as_str()), (EXIT_OK, ""));
        assert!(out.starts_with("delete    "), "{out}");
        assert!(out.contains("manifest  .pkl-gen/manifest.json\n"), "{out}");
        assert!(
            out.ends_with("dry run: would delete 1 file(s) and remove the manifest\n"),
            "{out}"
        );
        assert!(repo.path().join(".pkl-gen/manifest.json").exists());
    }

    #[test]
    fn detach_deletes_the_marked_files_and_removes_the_manifest() {
        let repo = detachable();
        let (code, out, err) = detach_in(repo.path(), &[]);
        assert_eq!((code, err.as_str()), (EXIT_OK, ""));
        assert!(out.ends_with("detached: deleted 1 file(s) and removed the manifest, every other file is a plain file now\n"), "{out}");
        assert!(!repo.path().join(".pkl-gen").exists());
    }

    #[test]
    fn detach_counts_only_the_files_it_deletes() {
        let repo = detachable();
        std::fs::remove_file(repo.path().join("tool.txt")).unwrap();
        git(repo.path(), &["commit", "-qam", "removed by hand"]);
        let (code, out, _) = detach_in(repo.path(), &["--dry-run"]);
        assert_eq!(code, EXIT_OK);
        assert_eq!(
            out,
            "forget    tool.txt\nmanifest  .pkl-gen/manifest.json\ndry run: would delete 0 file(s) and remove the manifest\n"
        );
        let (_, out, _) = detach_in(repo.path(), &[]);
        assert!(out.ends_with("detached: deleted 0 file(s) and removed the manifest, every other file is a plain file now\n"), "{out}");
    }

    #[test]
    fn detach_refuses_a_marked_file_that_was_edited_by_hand() {
        let repo = detachable();
        let marked = detach_in(repo.path(), &["--dry-run"])
            .1
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .to_owned();
        std::fs::write(repo.path().join(&marked), "edited\n").unwrap();
        git(repo.path(), &["commit", "-qam", "edit"]);
        let (code, out, err) = detach_in(repo.path(), &[]);
        assert_eq!(code, EXIT_REFUSED);
        // A refused file is not an action, so only the manifest line is listed.
        assert_eq!(out, "manifest  .pkl-gen/manifest.json\n");
        assert!(
            err.starts_with("refused, nothing was changed:\n  "),
            "{err}"
        );
        assert!(
            err.contains(
                "edited by hand since the last render, so detach will not delete it (use --force)"
            ),
            "{err}"
        );
        assert_eq!(detach_in(repo.path(), &["--force"]).0, EXIT_OK);
    }

    /// Serves one canned answer per connection, in order, and returns the address.
    fn serve(answers: Vec<(u16, String)>) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for (status, body) in answers {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut seen = Vec::new();
                let mut buffer = [0u8; 1024];
                while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => seen.extend_from_slice(&buffer[..n]),
                    }
                }
                let text = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(text.as_bytes());
            }
        });
        format!("http://{address}/freshness.json")
    }

    /// A freshness record for a check made `days` days ago (negative: ahead).
    fn record_from(days: i64) -> String {
        let when = time::OffsetDateTime::now_utc() - time::Duration::days(days);
        let text = when
            .format(&time::macros::format_description!(
                "[year]-[month]-[day]T[hour]:[minute]:[second]Z"
            ))
            .unwrap();
        format!("{{\"schemaVersion\":1,\"lastSuccessfulPinCheck\":\"{text}\"}}")
    }

    fn freshness_against(answer: (u16, String), extra: &[&str]) -> (i32, String, String) {
        let url = serve(vec![answer]);
        let mut items = vec!["freshness", "--url", url.as_str()];
        items.extend_from_slice(extra);
        go(&items, &here())
    }

    #[test]
    fn a_recent_record_is_fresh() {
        let (code, out, err) = freshness_against((200, record_from(3)), &[]);
        assert_eq!((code, err.as_str()), (EXIT_OK, ""));
        assert!(
            out.starts_with("fresh: the last successful pin check was 3 day(s) ago ("),
            "{out}"
        );
        assert!(out.ends_with("Z)\n"), "{out}");
    }

    #[test]
    fn a_record_older_than_thirty_days_is_stale() {
        let (code, out, err) = freshness_against((200, record_from(40)), &[]);
        assert_eq!((code, out.as_str()), (EXIT_REFUSED, ""));
        assert!(
            err.starts_with("stale: the last successful pin check was 40 day(s) ago ("),
            "{err}"
        );
        assert!(
            err.ends_with(
                "Z), more than 30. The pin-update workflow has stopped running or failing.\n"
            ),
            "{err}"
        );
    }

    #[test]
    fn the_age_limit_can_be_changed() {
        let (code, _, err) = freshness_against((200, record_from(10)), &["--max-age-days", "5"]);
        assert_eq!(code, EXIT_REFUSED);
        assert!(err.contains("more than 5."), "{err}");
        assert_eq!(
            freshness_against((200, record_from(10)), &["--max-age-days", "11"]).0,
            EXIT_OK
        );
    }

    #[test]
    fn a_record_that_cannot_be_used_is_an_error_with_its_reason() {
        let cases = [
            (200, "not json".to_owned(), "is not valid JSON"),
            (
                200,
                "{\"schemaVersion\":2,\"lastSuccessfulPinCheck\":\"2026-11-08T12:00:00Z\"}"
                    .to_owned(),
                "this runner reads 1",
            ),
            (
                200,
                "{\"schemaVersion\":1,\"lastSuccessfulPinCheck\":\"2026-11-08T12:00:00+01:00\"}"
                    .to_owned(),
                "must be a UTC time",
            ),
            (200, record_from(-3), "dated in the future"),
            (500, "boom".to_owned(), "failing closed: HTTP 500"),
        ];
        for (status, body, expected) in cases {
            let (code, out, err) = freshness_against((status, body), &["--attempts", "1"]);
            assert_eq!((code, out.as_str()), (EXIT_ERROR, ""), "{expected}");
            assert!(err.contains(expected), "{expected}: {err}");
        }
    }

    #[test]
    fn freshness_reads_the_address_from_the_given_environment() {
        let server = stub(vec![("/freshness.json", 200, record_from(2))]);
        let env = Env {
            freshness_url: Some(format!("{}/freshness.json", server.base)),
            ..Env::default()
        };
        let (code, out, _) = go_env(&["freshness"], &env, &here());
        assert_eq!(code, EXIT_OK);
        assert!(
            out.starts_with("fresh: the last successful pin check was 2 day(s) ago"),
            "{out}"
        );
    }

    #[test]
    fn the_url_comes_from_the_flag_then_the_environment_then_the_default() {
        use scaffold_core::freshness::FRESHNESS_URL;
        let with_flag = flags(&["--url", "http://flag"]);
        assert_eq!(
            freshness_url(&with_flag, Some("http://env".into())),
            "http://flag"
        );
        assert_eq!(
            freshness_url(&flags(&[]), Some("http://env".into())),
            "http://env"
        );
        assert_eq!(freshness_url(&flags(&[]), None), FRESHNESS_URL);
        assert_eq!(
            freshness_url(&flags(&[]), Some(String::new())),
            FRESHNESS_URL,
            "an empty variable is not an address"
        );
    }

    /// Answers each path with its canned status and body, for as long as the test runs. The text of every request is
    /// kept, lower-cased, so a test can look at the headers that were sent.
    struct Stub {
        base: String,
        requests: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Stub {
        /// The text of the requests that were made for `path`, in order.
        fn requests_for(&self, path: &str) -> Vec<String> {
            let prefix = format!("get {path} ");
            self.requests
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r.starts_with(&prefix))
                .cloned()
                .collect()
        }
    }

    fn stub(routes: Vec<(&'static str, u16, String)>) -> Stub {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = requests.clone();
        std::thread::spawn(move || {
            while let Ok((mut stream, _)) = listener.accept() {
                let mut request = Vec::new();
                let mut buffer = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => request.extend_from_slice(&buffer[..n]),
                    }
                }
                let text = String::from_utf8_lossy(&request).to_lowercase();
                let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
                seen.lock().unwrap().push(text.clone());
                let (status, body) = routes
                    .iter()
                    .find(|(route, _, _)| *route == path)
                    .map_or((404, "no route".to_owned()), |(_, status, body)| {
                        (*status, body.clone())
                    });
                let answer = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(answer.as_bytes());
            }
        });
        Stub { base, requests }
    }

    fn go_env(items: &[&str], env: &Env, cwd: &Path) -> (i32, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with_env(&args(items), cwd, env, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    const KNOWN_BAD_PATH: &str = "/known-bad.json";

    /// `latest` against a stub that serves `releases` and, optionally, a known-bad list.
    fn latest_against(
        releases: &str,
        known_bad: Option<&str>,
        extra: &[&str],
    ) -> (i32, String, String) {
        let mut routes = vec![("/releases", 200, releases.to_owned())];
        if let Some(list) = known_bad {
            routes.push((KNOWN_BAD_PATH, 200, list.to_owned()));
        }
        let server = stub(routes);
        let repo = tempfile::tempdir().unwrap();
        let env = Env {
            known_bad_url: Some(format!("{}{KNOWN_BAD_PATH}", server.base)),
            ..Env::default()
        };
        let (url, root) = (
            format!("{}/releases", server.base),
            repo.path().display().to_string(),
        );
        let mut items = vec!["latest", "--url", url.as_str(), "--root", root.as_str()];
        items.extend_from_slice(extra);
        go_env(&items, &env, &here())
    }

    const RELEASES: &str = r#"[{"tag_name":"v0.1.4"},{"tag_name":"v0.1.5"},{"tag_name":"v0.2.0","prerelease":true},{"tag_name":"v0.3.0","draft":true}]"#;

    #[test]
    fn latest_names_the_newest_stable_release() {
        assert_eq!(
            latest_against(RELEASES, None, &[]),
            (EXIT_OK, "0.1.5\n".to_owned(), String::new())
        );
    }

    #[test]
    fn latest_offers_a_pre_release_only_when_asked() {
        let (code, out, _) = latest_against(RELEASES, None, &["--include-prereleases"]);
        assert_eq!((code, out.as_str()), (EXIT_OK, "0.2.0\n"));
    }

    #[test]
    fn latest_skips_a_version_on_the_known_bad_list() {
        let list = r#"{"versions":[{"version":"0.1.5","reason":"broken"}]}"#;
        let (code, out, err) = latest_against(RELEASES, Some(list), &[]);
        assert_eq!((code, out.as_str(), err.as_str()), (EXIT_OK, "0.1.4\n", ""));
    }

    #[test]
    fn latest_ignores_a_tag_that_is_not_a_version() {
        let (code, out, _) = latest_against(
            r#"[{"tag_name":"nightly"},{"tag_name":"v0.1.5"}]"#,
            None,
            &[],
        );
        assert_eq!((code, out.as_str()), (EXIT_OK, "0.1.5\n"));
    }

    #[test]
    fn latest_with_nothing_that_qualifies_is_an_error_with_a_hint() {
        let only_pre = r#"[{"tag_name":"v0.2.0","prerelease":true}]"#;
        let (code, out, err) = latest_against(only_pre, None, &[]);
        assert_eq!((code, out.as_str()), (EXIT_ERROR, ""));
        assert_eq!(
            err,
            "no release found that qualifies (stable, not a draft, not known bad). Use --include-prereleases to offer pre-releases\n"
        );
        let (code, _, err) = latest_against("[]", None, &["--include-prereleases"]);
        assert_eq!(code, EXIT_ERROR);
        assert_eq!(
            err,
            "no release found that qualifies (stable, not a draft, not known bad)\n"
        );
    }

    #[test]
    fn latest_fails_closed_when_the_release_list_cannot_be_fetched() {
        let server = stub(vec![("/releases", 500, "boom".to_owned())]);
        let (url, repo) = (
            format!("{}/releases", server.base),
            tempfile::tempdir().unwrap(),
        );
        let root = repo.path().display().to_string();
        let (code, out, err) = go_env(
            &["latest", "--url", &url, "--root", &root, "--attempts", "1"],
            &Env::default(),
            &here(),
        );
        assert_eq!((code, out.as_str()), (EXIT_ERROR, ""));
        assert!(
            err.contains("after 1 attempts, failing closed: HTTP 500"),
            "{err}"
        );
    }

    #[test]
    fn latest_fails_when_the_answer_is_not_a_list_of_releases() {
        let (code, _, err) = latest_against(r#"{"message":"rate limited"}"#, None, &[]);
        assert_eq!(code, EXIT_ERROR);
        assert!(
            err.ends_with("/releases did not return a list of releases\n"),
            "{err}"
        );
    }

    #[test]
    fn latest_sends_the_github_token_and_the_accept_header_when_there_is_a_token() {
        let server = stub(vec![("/releases", 200, "[]".to_owned())]);
        let repo = tempfile::tempdir().unwrap();
        let env = Env {
            github_token: Some("t0ken".to_owned()),
            known_bad_url: Some(format!("{}{KNOWN_BAD_PATH}", server.base)),
            ..Env::default()
        };
        let (url, root) = (
            format!("{}/releases", server.base),
            repo.path().display().to_string(),
        );
        let items = [
            "latest",
            "--url",
            url.as_str(),
            "--root",
            root.as_str(),
            "--attempts",
            "1",
        ];
        go_env(&items, &env, &here());
        let sent = server.requests_for("/releases");
        assert_eq!(sent.len(), 1, "{sent:?}");
        assert!(
            sent[0].contains("accept: application/vnd.github+json\r\n"),
            "{}",
            sent[0]
        );
        assert!(
            sent[0].contains("authorization: bearer t0ken\r\n"),
            "{}",
            sent[0]
        );
        let known_bad = server.requests_for(KNOWN_BAD_PATH);
        assert!(
            !known_bad.is_empty() && known_bad.iter().all(|r| !r.contains("authorization")),
            "the token goes to GitHub's API only"
        );
        let without = Env {
            github_token: None,
            ..env
        };
        go_env(&items, &without, &here());
        let sent = server.requests_for("/releases");
        assert_eq!(sent.len(), 2);
        assert!(!sent[1].contains("authorization"), "{}", sent[1]);
    }

    #[test]
    fn the_environment_is_read_from_the_process() {
        // A child copy of this test binary gets the variables, so no other test sees them.
        let run_child = |vars: &[(&str, &str)]| {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "tests::child_reads_the_environment",
                    "--exact",
                    "--include-ignored",
                    "--nocapture",
                ])
                .env_remove("GITHUB_TOKEN")
                .env_remove("GH_TOKEN")
                .env_remove("PANTHEON_SCAFFOLD_FRESHNESS_URL")
                .env_remove("PANTHEON_SCAFFOLD_KNOWN_BAD_URL")
                .env_remove("PANTHEON_SCAFFOLD_RELEASE_BASE");
            for (name, value) in vars {
                command.env(name, value);
            }
            let output = command.output().unwrap();
            String::from_utf8_lossy(&output.stdout).into_owned()
        };
        let all = run_child(&[
            ("PANTHEON_SCAFFOLD_FRESHNESS_URL", "http://fresh"),
            ("PANTHEON_SCAFFOLD_KNOWN_BAD_URL", "http://bad"),
            ("GITHUB_TOKEN", "from-github"),
            ("GH_TOKEN", "from-gh"),
            ("PANTHEON_SCAFFOLD_RELEASE_BASE", "http://release"),
        ]);
        assert!(
            all.contains("ENV freshness=Some(\"http://fresh\") known_bad=Some(\"http://bad\") token=Some(\"from-github\") release=Some(\"http://release\")"),
            "{all}"
        );
        let gh_only = run_child(&[("GH_TOKEN", "from-gh")]);
        assert!(
            gh_only
                .contains("ENV freshness=None known_bad=None token=Some(\"from-gh\") release=None"),
            "{gh_only}"
        );
        assert!(
            run_child(&[]).contains("ENV freshness=None known_bad=None token=None release=None")
        );
    }

    #[test]
    #[ignore = "run only by the test above, in a child process"]
    fn child_reads_the_environment() {
        let env = Env::from_process();
        println!(
            "ENV freshness={:?} known_bad={:?} token={:?} release={:?}",
            env.freshness_url, env.known_bad_url, env.github_token, env.release_base
        );
    }

    #[test]
    fn the_token_comes_from_github_token_then_gh_token() {
        let some = |v: &str| Some(v.to_owned());
        assert_eq!(github_token(some("a"), some("b")).as_deref(), Some("a"));
        assert_eq!(github_token(None, some("b")).as_deref(), Some("b"));
        assert_eq!(github_token(some(""), some("b")).as_deref(), Some(""));
        assert_eq!(github_token(None, None), None);
    }

    #[test]
    fn an_empty_token_sends_no_authorization_header() {
        let server = stub(vec![("/releases", 200, "[]".to_owned())]);
        let repo = tempfile::tempdir().unwrap();
        let env = Env {
            github_token: Some(String::new()),
            known_bad_url: Some(format!("{}{KNOWN_BAD_PATH}", server.base)),
            ..Env::default()
        };
        let (url, root) = (
            format!("{}/releases", server.base),
            repo.path().display().to_string(),
        );
        go_env(
            &["latest", "--url", &url, "--root", &root, "--attempts", "1"],
            &env,
            &here(),
        );
        assert!(!server.requests_for("/releases")[0].contains("authorization"));
    }

    fn known_bad_env(server: &Stub) -> Env {
        Env {
            known_bad_url: Some(format!("{}{KNOWN_BAD_PATH}", server.base)),
            ..Env::default()
        }
    }

    #[test]
    fn a_known_bad_package_warns_and_strict_refuses_it() {
        use scaffold_core::preflight::Mode;
        let server = stub(vec![(
            KNOWN_BAD_PATH,
            200,
            r#"{"versions":[{"version":"0.2.0","reason":"broken"}]}"#.to_owned(),
        )]);
        let repo = tempfile::tempdir().unwrap();
        let f = Flags {
            root: repo.path().to_owned(),
            ..flags(&[])
        };
        let c = contract_with(Some(&uri("0.2.0")));
        let (code, version, err) = gate_env(&c, &f, Mode::Render, &known_bad_env(&server));
        assert_eq!((code, version.as_deref()), (EXIT_OK, Some("0.2.0")));
        assert_eq!(
            err,
            "warning: package 0.2.0 is on the known-bad list: broken\n"
        );
        assert!(
            repo.path().join(".pkl-gen/cache/known-bad.json").exists(),
            "the fetched list is cached"
        );
        let strict = Flags { strict: true, ..f };
        let (code, _, err) = gate_env(&c, &strict, Mode::Render, &known_bad_env(&server));
        assert_eq!(code, EXIT_REFUSED);
        assert_eq!(
            err,
            "refused, nothing was written:\n  package 0.2.0 is on the known-bad list: broken (--strict)\n"
        );
    }

    #[test]
    fn a_package_that_is_not_on_the_list_passes_with_no_warning() {
        use scaffold_core::preflight::Mode;
        let server = stub(vec![(
            KNOWN_BAD_PATH,
            200,
            r#"{"versions":[{"version":"0.1.0","reason":"old"}]}"#.to_owned(),
        )]);
        let repo = tempfile::tempdir().unwrap();
        let f = Flags {
            root: repo.path().to_owned(),
            strict: true,
            ..flags(&[])
        };
        let (code, version, err) = gate_env(
            &contract_with(Some(&uri("0.2.0"))),
            &f,
            Mode::Render,
            &known_bad_env(&server),
        );
        assert_eq!(
            (code, version.as_deref(), err.as_str()),
            (EXIT_OK, Some("0.2.0"), "")
        );
    }

    #[test]
    fn when_the_list_cannot_be_fetched_the_cached_copy_is_used_and_the_run_goes_on() {
        use scaffold_core::preflight::Mode;
        let repo = tempfile::tempdir().unwrap();
        let cache = repo.path().join(".pkl-gen/cache");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(
            cache.join("known-bad.json"),
            r#"{"versions":[{"version":"0.2.0","reason":"from the cache"}]}"#,
        )
        .unwrap();
        let server = stub(vec![(KNOWN_BAD_PATH, 500, "boom".to_owned())]);
        let f = Flags {
            root: repo.path().to_owned(),
            ..flags(&[])
        };
        let (code, _, err) = gate_env(
            &contract_with(Some(&uri("0.2.0"))),
            &f,
            Mode::Render,
            &known_bad_env(&server),
        );
        assert_eq!(code, EXIT_OK);
        assert_eq!(
            err,
            "warning: known-bad list: the fetch failed (HTTP 500), using the cached list\nwarning: package 0.2.0 is on the known-bad list: from the cache\n"
        );
    }

    fn contract_with(uri: Option<&str>) -> Contract {
        Contract {
            schema_version: 1,
            files: vec![],
            pins: vec![],
            escape_hatch: EscapeHatch {
                uses: 0,
                constructs: 0,
            },
            package_uri: uri.map(str::to_owned),
            requires: None,
            security_builtins: None,
        }
    }

    fn uri(version: &str) -> String {
        format!(
            "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{version}/pantheon-scaffold@{version}#/Config.pkl"
        )
    }

    /// An address nothing listens on, so a test that reaches for the known-bad list fails fast and offline.
    const NOWHERE: &str = "http://127.0.0.1:1/known-bad.json";

    fn gate_env(
        contract: &Contract,
        flags: &Flags,
        mode: scaffold_core::preflight::Mode,
        env: &Env,
    ) -> (i32, Option<String>, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut io = Io {
            out: &mut out,
            err: &mut err,
        };
        let result = gate(contract, flags, mode, env, &mut io).unwrap();
        let text = String::from_utf8(err).unwrap();
        match result {
            Gate::Stop(code) => (code, None, text),
            Gate::Go(version) => (EXIT_OK, version, text),
        }
    }

    fn gate_with(
        contract: &Contract,
        flags: &Flags,
        mode: scaffold_core::preflight::Mode,
    ) -> (i32, Option<String>, String) {
        let env = Env {
            known_bad_url: Some(NOWHERE.to_owned()),
            ..Env::default()
        };
        gate_env(contract, flags, mode, &env)
    }

    #[test]
    fn the_gate_passes_the_package_version_on_and_falls_back_when_the_known_bad_list_cannot_be_fetched()
     {
        let repo = tempfile::tempdir().unwrap();
        let f = Flags {
            root: repo.path().to_owned(),
            ..flags(&[])
        };
        let (code, version, err) = gate_with(
            &contract_with(Some(&uri("0.2.0"))),
            &f,
            scaffold_core::preflight::Mode::Render,
        );
        assert_eq!((code, version.as_deref()), (EXIT_OK, Some("0.2.0")));
        assert!(
            err.starts_with("warning: known-bad list: the fetch failed (")
                && err.ends_with("), using the bundled snapshot\n"),
            "{err}"
        );
        let (_, version, err) = gate_with(
            &contract_with(None),
            &f,
            scaffold_core::preflight::Mode::Render,
        );
        assert_eq!(
            (version, err.as_str()),
            (None, ""),
            "a local package never looks the list up"
        );
        assert!(
            !repo.path().join(".pkl-gen/cache").exists(),
            "nothing was fetched, so nothing was cached"
        );
    }

    #[test]
    fn the_gate_refuses_an_older_package_than_the_last_render_and_check_only_warns() {
        let repo = tempfile::tempdir().unwrap();
        let mut manifest = scaffold_core::manifest::Manifest::empty();
        manifest.package_version = Some("0.3.0".into());
        std::fs::create_dir_all(repo.path().join(".pkl-gen")).unwrap();
        std::fs::write(
            repo.path().join(".pkl-gen/manifest.json"),
            scaffold_core::manifest::serialise_manifest(&manifest),
        )
        .unwrap();
        let f = Flags {
            root: repo.path().to_owned(),
            ..flags(&[])
        };
        let c = contract_with(Some(&uri("0.2.0")));
        let (code, _, err) = gate_with(&c, &f, scaffold_core::preflight::Mode::Render);
        assert_eq!(code, EXIT_REFUSED);
        assert!(
            err.contains("refused, nothing was written:\n  package 0.2.0 is older than 0.3.0"),
            "{err}"
        );
        let forced = Flags { force: true, ..f };
        let (code, _, err) = gate_with(&c, &forced, scaffold_core::preflight::Mode::Render);
        assert_eq!(code, EXIT_OK);
        assert!(
            err.contains("warning: package 0.2.0 is older than 0.3.0"),
            "{err}"
        );
        let (code, _, _) = gate_with(&c, &forced, scaffold_core::preflight::Mode::Check);
        assert_eq!(code, EXIT_OK);
    }

    #[test]
    fn the_gate_stops_with_an_error_for_a_runner_that_is_too_old_or_a_bad_package_uri() {
        let repo = tempfile::tempdir().unwrap();
        let f = Flags {
            root: repo.path().to_owned(),
            ..flags(&[])
        };
        let mut c = contract_with(None);
        c.requires = Some(scaffold_core::contract::Requires {
            runner: "99.0.0".into(),
        });
        let (code, _, err) = gate_with(&c, &f, scaffold_core::preflight::Mode::Render);
        assert_eq!(code, EXIT_ERROR);
        assert_eq!(
            err,
            format!(
                "runner {VERSION} is older than the 99.0.0 this package requires, upgrade the runner\n"
            )
        );
        let bad = contract_with(Some("package://github.com/x/y#/Config.pkl"));
        let (code, _, err) = gate_with(&bad, &f, scaffold_core::preflight::Mode::Render);
        assert_eq!(code, EXIT_ERROR);
        assert_eq!(
            err,
            "package://github.com/x/y#/Config.pkl: the package URI has no readable version\n"
        );
    }

    #[test]
    fn a_disabled_security_builtin_is_a_warning_and_repair_does_not_read_the_manifest() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".pkl-gen")).unwrap();
        std::fs::write(repo.path().join(".pkl-gen/manifest.json"), "{").unwrap();
        let f = Flags {
            root: repo.path().to_owned(),
            repair: true,
            ..flags(&[])
        };
        let mut c = contract_with(None);
        c.security_builtins = Some(vec![scaffold_core::contract::SecurityBuiltin {
            name: "codeql".into(),
            enabled: false,
        }]);
        let (code, _, err) = gate_with(&c, &f, scaffold_core::preflight::Mode::Render);
        assert_eq!(code, EXIT_OK);
        assert_eq!(
            err,
            "warning: required Security builtin codeql is disabled\n"
        );
    }
}
