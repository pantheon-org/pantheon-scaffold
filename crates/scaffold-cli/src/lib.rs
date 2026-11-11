//! The pantheon-scaffold command line, ported from `runner/src/cli.ts`.
//!
//! This build handles `--version`, `contract`, `render`, `check`, `detach` and `freshness`. `latest` and `upgrade` parse
//! their options like the TypeScript runner but stop with an error until they are ported.

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

/// Runs the command line and returns the exit code. `cwd` is where relative paths start.
pub fn run(argv: &[String], cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
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
            command(cmd, config, cwd, &flags, &mut io)
        }
        "detach" => finish(run_detach(&flags, &mut io), &mut io),
        "freshness" => finish(run_freshness(&flags, &mut io), &mut io),
        _ => {
            io.complain(&format!(
                "{cmd} is not available in the Rust runner yet, use the TypeScript runner"
            ));
            EXIT_ERROR
        }
    }
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

fn run_freshness(flags: &Flags, io: &mut Io) -> Result<i32, String> {
    use scaffold_core::freshness::{CheckOptions, State, check_freshness};
    use std::time::Duration;
    let url = freshness_url(flags, std::env::var("PANTHEON_SCAFFOLD_FRESHNESS_URL").ok());
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

fn command(cmd: &str, config: &str, cwd: &Path, flags: &Flags, io: &mut Io) -> i32 {
    match command_inner(cmd, config, cwd, flags, io) {
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
    let package_version = match gate(&contract, flags, mode, io)? {
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
    io: &mut Io,
) -> Result<Gate, String> {
    use scaffold_core::preflight::{PreflightOptions, package_version_of, preflight};
    match package_version_of(contract.package_uri.as_deref()) {
        Err(message) => {
            io.complain(&message);
            return Ok(Gate::Stop(EXIT_ERROR));
        }
        Ok(Some(_)) => io.complain(
            "warning: this build does not check the known-bad list of package versions yet",
        ),
        Ok(None) => {}
    }
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
        known_bad: None,
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

    #[test]
    fn commands_that_are_not_ported_stop_with_an_error() {
        for items in [&["latest"][..], &["upgrade", "1.0.0"]] {
            let (code, out, err) = go(items, &here());
            assert_eq!((code, out.as_str()), (EXIT_ERROR, ""), "{items:?}");
            assert_eq!(
                err,
                format!(
                    "{} is not available in the Rust runner yet, use the TypeScript runner\n",
                    items[0]
                )
            );
        }
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

    fn gate_with(
        contract: &Contract,
        flags: &Flags,
        mode: scaffold_core::preflight::Mode,
    ) -> (i32, Option<String>, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut io = Io {
            out: &mut out,
            err: &mut err,
        };
        let result = gate(contract, flags, mode, &mut io).unwrap();
        let text = String::from_utf8(err).unwrap();
        match result {
            Gate::Stop(code) => (code, None, text),
            Gate::Go(version) => (EXIT_OK, version, text),
        }
    }

    #[test]
    fn the_gate_passes_the_package_version_on_and_says_the_known_bad_list_is_not_checked() {
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
        assert_eq!(
            err,
            "warning: this build does not check the known-bad list of package versions yet\n"
        );
        let (_, version, err) = gate_with(
            &contract_with(None),
            &f,
            scaffold_core::preflight::Mode::Render,
        );
        assert_eq!((version, err.as_str()), (None, ""));
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
