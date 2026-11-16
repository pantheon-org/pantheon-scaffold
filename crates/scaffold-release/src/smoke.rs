//! Smoke suite for a compiled runner binary. Ported from `runner/scripts/smoke.ts` (Phase 4 of `docs/ts-tooling-rust-port.md`): the checks,
//! their names and their order, and the lines it prints, were the TypeScript's, so the two could be compared on three platforms before Phase 6 deleted the TypeScript.
//!
//! It runs the binary from a directory unrelated to the source tree, so anything the binary needs (the schema, the version) has to be inside
//! it. It needs `pkl` and `git` on the PATH and a checkout of this repository for the pkl package and the fixtures. Run it on every target:
//! the filesystem it runs on is part of the test (case-sensitive on Linux, case-insensitive on a default macOS volume).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::json::{octal_mode, resolve};

pub const USAGE: &str =
    "usage: scaffold-release smoke <binary> [--version <x.y.z>] [--repo <checkout>]";

#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    pub binary: PathBuf,
    pub version: String,
    pub repo: PathBuf,
}

/// The arguments, or the message for a usage error. `repo` is the checkout to read the fixtures from, the current directory by default.
pub fn parse_args(args: &[&str], cwd: &Path) -> Result<Args, String> {
    let flag = |name: &str, fallback: Option<&str>| -> Result<String, String> {
        let value = match args.iter().position(|a| *a == name) {
            None => fallback,
            Some(i) => args.get(i + 1).copied(),
        };
        value.map(str::to_owned).ok_or_else(|| USAGE.to_owned())
    };
    let binary = match args.first() {
        Some(first) => (*first).to_owned(),
        None => flag("--binary", None)?,
    };
    Ok(Args {
        binary: resolve(&binary, cwd),
        version: flag("--version", Some("0.0.0"))?,
        repo: resolve(&flag("--repo", Some("."))?, cwd),
    })
}

struct Res {
    code: i32,
    out: String,
    err: String,
}

/// How a result shows in a failure.
fn show(r: &Res) -> String {
    format!("exit {}\nstdout: {}\nstderr: {}", r.code, r.out, r.err)
}

/// The platform as Bun names it: `linux` or `darwin`, then `x64` or `arm64`.
fn platform() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{os}-{arch}")
}

/// `new Date().toISOString()`: UTC, with milliseconds.
fn iso_now() -> String {
    time::OffsetDateTime::now_utc()
        .format(time::macros::format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        ))
        .expect("a time formats")
}

struct Smoke<'a> {
    binary: &'a Path,
    repo: &'a Path,
    base: PathBuf,
    failures: u32,
    out: &'a mut dyn Write,
}

impl Smoke<'_> {
    fn fixture(&self, name: &str) -> String {
        self.repo
            .join("crates/scaffold-package-tests/fixtures")
            .join(name)
            .join("scaffold.pkl")
            .to_string_lossy()
            .into_owned()
    }

    fn package(&self) -> String {
        self.repo.join("pkl").to_string_lossy().into_owned()
    }

    /// An empty folder of that name under the scratch folder.
    fn fresh(&self, name: &str) -> Result<PathBuf, String> {
        let dir = self.base.join(name);
        std::fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(dir)
    }

    /// Runs the binary, with the environment of this process less every `GIT_` variable, and trims what it prints.
    fn bin(&self, args: &[&str], cwd: &Path) -> Result<Res, String> {
        let output = Command::new(self.binary)
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .envs(clean_env())
            .output()
            .map_err(|e| format!("{}: {e}", self.binary.display()))?;
        Ok(Res {
            code: output.status.code().unwrap_or(-1),
            out: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            err: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }

    fn run(&self, args: &[&str]) -> Result<Res, String> {
        self.bin(args, &self.base.clone())
    }

    fn render(&self, config: &str, repo: &Path, extra: &[&str]) -> Result<Res, String> {
        let (fixture, root, package) = (
            self.fixture(config),
            repo.to_string_lossy().into_owned(),
            self.package(),
        );
        let mut args = vec![
            "render",
            &fixture,
            "--root",
            &root,
            "--allow-local-package",
            &package,
        ];
        args.extend(extra);
        self.run(&args)
    }

    fn check(&self, config: &str, repo: &Path) -> Result<Res, String> {
        let (fixture, root, package) = (
            self.fixture(config),
            repo.to_string_lossy().into_owned(),
            self.package(),
        );
        self.run(&[
            "check",
            &fixture,
            "--root",
            &root,
            "--allow-local-package",
            &package,
        ])
    }

    /// Records one check: `PASS  name`, or `FAIL  name` with its detail indented.
    fn ok(&mut self, name: &str, pass: bool, detail: &str) {
        let tail = if !pass && !detail.is_empty() {
            format!("\n      {}", detail.replace('\n', "\n      "))
        } else {
            String::new()
        };
        let _ = writeln!(
            self.out,
            "{}  {name}{tail}",
            if pass { "PASS" } else { "FAIL" }
        );
        if !pass {
            self.failures += 1;
        }
    }

    /// The setup of a scenario failed: one failed check named after it, and the other scenarios still run.
    fn failed_setup(&mut self, name: &str, message: &str) {
        self.ok(name, false, message);
    }

    /// Renders a fixture and fails, with the output, when the render did not succeed.
    fn must_render(&self, config: &str, repo: &Path) -> Result<(), String> {
        let r = self.render(config, repo, &[])?;
        if r.code != 0 {
            return Err(format!("setup render of {config} failed: {}", show(&r)));
        }
        Ok(())
    }
}

fn clean_env() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(k, _)| !k.starts_with("GIT_"))
        .collect()
}

/// `git -C <dir> ...` with a fixed author, and no signing. Fails with the output when git does.
fn git(dir: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=smoke",
            "-c",
            "user.email=smoke@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env_clear()
        .envs(clean_env())
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {}: {}{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        )
        .trim()
        .to_owned());
    }
    Ok(())
}

fn commit_all(dir: &Path) -> Result<(), String> {
    git(dir, &["add", "-A"])?;
    git(dir, &["commit", "-q", "-m", "snapshot"])
}

fn mode(path: &Path) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(octal_mode(meta.permissions().mode()))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// Runs the suite. Exit 0 when every check passed, 1 when one failed.
pub fn smoke(args: &Args, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let scratch = match tempfile::Builder::new().prefix("scaffold-smoke-").tempdir() {
        Ok(dir) => dir,
        Err(e) => {
            let _ = writeln!(err, "{e}");
            return 1;
        }
    };
    let mut s = Smoke {
        binary: &args.binary,
        repo: &args.repo,
        base: scratch.path().to_path_buf(),
        failures: 0,
        out,
    };
    match suite(&mut s, &args.version) {
        Ok(()) => {}
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 1;
        }
    }
    let _ = if s.failures > 0 {
        writeln!(s.out, "\n{} failing", s.failures)
    } else {
        writeln!(s.out, "\nall passed")
    };
    i32::from(s.failures > 0)
}

/// A scenario whose setup can fail: a failure is one failed check named after it, and the other scenarios still run.
fn scenario(s: &mut Smoke, name: &str, body: fn(&mut Smoke) -> Result<(), String>) {
    if let Err(message) = body(s) {
        s.failed_setup(name, &message);
    }
}

fn suite(s: &mut Smoke, expected_version: &str) -> Result<(), String> {
    // Case-sensitive or not: write one name and look for it in another case.
    let probe = s.fresh("probe")?;
    std::fs::write(probe.join("probe-file"), "x").map_err(|e| e.to_string())?;
    let case_insensitive = probe.join("PROBE-FILE").exists();
    let _ = writeln!(
        s.out,
        "binary {}\nfilesystem: {}, platform {}\n",
        s.binary.display(),
        if case_insensitive {
            "case-insensitive"
        } else {
            "case-sensitive"
        },
        platform()
    );
    basics(s, expected_version)?;
    render_flow(s)?;
    lock_flow(s)?;
    case_flow(s)?;
    scenario(s, "orphan deletion in a clean git repository", orphan_clean);
    scenario(s, "a dirty tracked tree refuses the deletion", orphan_dirty);
    Ok(())
}

fn basics(s: &mut Smoke, expected_version: &str) -> Result<(), String> {
    let r = s.run(&["--version"])?;
    s.ok(
        "--version prints the version the binary was built for",
        r.code == 0 && r.out == expected_version,
        &show(&r),
    );
    let u = s.run(&["nope"])?;
    s.ok(
        "an unknown command is a usage error (exit 2)",
        u.code == 2 && u.err.contains("usage"),
        &show(&u),
    );
    let (minimal, package) = (s.fixture("minimal"), s.package());
    let c = s.run(&["contract", &minimal, "--allow-local-package", &package])?;
    s.ok(
        "contract works from an unrelated directory, so the schema is inside the binary",
        c.code == 0 && c.out.starts_with("contract ok: 4 files"),
        &show(&c),
    );
    Ok(())
}

fn render_flow(s: &mut Smoke) -> Result<(), String> {
    let repo = s.fresh("render")?;
    let r = s.render("minimal", &repo, &[])?;
    s.ok(
        "render writes the files",
        r.code == 0
            && repo.join("rust-toolchain.toml").exists()
            && repo.join(".pkl-gen/manifest.json").exists(),
        &show(&r),
    );
    let hello = repo.join(".github/scripts/hello.sh");
    let hello_mode = if hello.exists() {
        mode(&hello)?
    } else {
        "file missing".to_owned()
    };
    s.ok(
        "the executable bit is set from the contract",
        hello.exists() && hello_mode == "755",
        &hello_mode,
    );
    let again = s.render("minimal", &repo, &[])?;
    s.ok(
        "a second render writes and deletes nothing",
        again.code == 0 && again.out.contains("written 0 file(s), deleted 0"),
        &show(&again),
    );
    let c = s.check("minimal", &repo)?;
    s.ok("check finds no drift", c.code == 0, &show(&c));
    std::fs::write(repo.join("rust-toolchain.toml"), "# mine\n").map_err(|e| e.to_string())?;
    let drift = s.check("minimal", &repo)?;
    s.ok(
        "check reports a hand edit as drift (exit 4)",
        drift.code == 4,
        &show(&drift),
    );
    let refused = s.render("minimal", &repo, &[])?;
    s.ok(
        "render refuses to overwrite a hand edit (exit 3) and leaves it",
        refused.code == 3 && read(&repo.join("rust-toolchain.toml")) == "# mine\n",
        &show(&refused),
    );
    let forced = s.render("minimal", &repo, &["--force"])?;
    s.ok(
        "--force overwrites it",
        forced.code == 0 && read(&repo.join("rust-toolchain.toml")) != "# mine\n",
        &show(&forced),
    );
    let root = repo.to_string_lossy().into_owned();
    let d = s.run(&["detach", "--root", &root])?;
    s.ok(
        "detach removes .pkl-gen and leaves the files",
        d.code == 0 && !repo.join(".pkl-gen").exists() && repo.join("rust-toolchain.toml").exists(),
        &show(&d),
    );
    Ok(())
}

fn lock_flow(s: &mut Smoke) -> Result<(), String> {
    let repo = s.fresh("locked")?;
    std::fs::create_dir(repo.join(".pkl-gen")).map_err(|e| e.to_string())?;
    let lock = format!(
        "{{\"pid\":{},\"started\":\"{}\"}}",
        std::process::id(),
        iso_now()
    );
    std::fs::write(repo.join(".pkl-gen/lock"), lock).map_err(|e| e.to_string())?;
    let r = s.render("minimal", &repo, &[])?;
    s.ok(
        "a render while the lock is held fails (exit 1) and writes nothing",
        r.code == 1 && r.err.contains("is held") && !repo.join("rust-toolchain.toml").exists(),
        &show(&r),
    );
    Ok(())
}

fn case_flow(s: &mut Smoke) -> Result<(), String> {
    let collision = s.fresh("collision")?;
    let r = s.render("smoke-case-collision", &collision, &[])?;
    s.ok(
        "two paths that differ only by case are refused on any filesystem (exit 1)",
        r.code == 1 && r.err.contains("differ only by case"),
        &show(&r),
    );
    let repo = s.fresh("rename")?;
    s.render("smoke-case-a", &repo, &[])?;
    let rn = s.render("smoke-case-b", &repo, &[])?;
    s.ok(
        "a case-only rename is refused (exit 3) and the old file is kept",
        rn.code == 3
            && rn.err.contains("differ only by case")
            && read(&repo.join("readme.md")) == "one\n",
        &show(&rn),
    );
    Ok(())
}

fn orphan_clean(s: &mut Smoke) -> Result<(), String> {
    let repo = s.fresh("orphan")?;
    git(&repo, &["init", "-q", "-b", "main"])?;
    s.must_render("smoke-orphan-a", &repo)?;
    commit_all(&repo)?;
    let r = s.render("smoke-orphan-b", &repo, &[])?;
    s.ok(
        "an orphan is deleted in a clean git repository, so git works inside the binary",
        r.code == 0
            && r.out.contains("deleted 1")
            && !repo.join("b.txt").exists()
            && repo.join("a.txt").exists(),
        &show(&r),
    );
    Ok(())
}

fn orphan_dirty(s: &mut Smoke) -> Result<(), String> {
    let dirty = s.fresh("orphan-dirty")?;
    git(&dirty, &["init", "-q", "-b", "main"])?;
    s.must_render("smoke-orphan-a", &dirty)?;
    std::fs::write(dirty.join("notes.md"), "n\n").map_err(|e| e.to_string())?;
    commit_all(&dirty)?;
    std::fs::write(dirty.join("notes.md"), "changed\n").map_err(|e| e.to_string())?;
    let d = s.render("smoke-orphan-b", &dirty, &[])?;
    s.ok(
        "a dirty tracked tree refuses the deletion (exit 3) and keeps the file",
        d.code == 3 && d.err.contains("not clean") && dirty.join("b.txt").exists(),
        &show(&d),
    );
    Ok(())
}

/// `smoke <binary> [--version <x.y.z>] [--repo <checkout>]`. Exit 2 for a usage error.
pub fn command(args: &[&str], cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match parse_args(args, cwd) {
        Ok(parsed) => smoke(&parsed, out, err),
        Err(message) => {
            let _ = writeln!(err, "{message}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arguments_are_the_binary_a_version_and_a_checkout() {
        let cwd = Path::new("/work");
        assert_eq!(
            parse_args(&["dist/bin"], cwd).unwrap(),
            Args {
                binary: PathBuf::from("/work/dist/bin"),
                version: "0.0.0".to_owned(),
                repo: PathBuf::from("/work")
            }
        );
        assert_eq!(
            parse_args(&["/b", "--version", "1.2.3", "--repo", "../r"], cwd).unwrap(),
            Args {
                binary: PathBuf::from("/b"),
                version: "1.2.3".to_owned(),
                repo: PathBuf::from("/r")
            }
        );
    }

    #[test]
    fn no_binary_and_a_flag_with_no_value_are_usage_errors() {
        let cwd = Path::new("/work");
        assert_eq!(parse_args(&[], cwd).unwrap_err(), USAGE);
        assert_eq!(parse_args(&["/b", "--version"], cwd).unwrap_err(), USAGE);
        assert_eq!(parse_args(&["/b", "--repo"], cwd).unwrap_err(), USAGE);
    }

    #[test]
    fn the_platform_is_named_the_way_bun_names_it() {
        let name = platform();
        assert!(
            ["linux-x64", "linux-arm64", "darwin-arm64", "darwin-x64"].contains(&name.as_str()),
            "{name}"
        );
    }

    #[test]
    fn the_time_is_in_utc_with_milliseconds_like_to_iso_string() {
        let now = iso_now();
        assert_eq!(now.len(), 24, "{now}");
        assert!(
            now.ends_with('Z') && now.as_bytes()[10] == b'T' && now.as_bytes()[19] == b'.',
            "{now}"
        );
    }

    #[test]
    fn a_failed_check_shows_its_detail_indented_and_a_passed_one_does_not() {
        let mut printed = Vec::new();
        let mut smoke = Smoke {
            binary: Path::new("b"),
            repo: Path::new("r"),
            base: PathBuf::new(),
            failures: 0,
            out: &mut printed,
        };
        smoke.ok("fine", true, "ignored");
        smoke.ok("broken", false, "line one\nline two");
        smoke.ok("broken without detail", false, "");
        assert_eq!(smoke.failures, 2);
        assert_eq!(
            String::from_utf8(printed).unwrap(),
            "PASS  fine\nFAIL  broken\n      line one\n      line two\nFAIL  broken without detail\n"
        );
    }

    #[test]
    fn a_result_is_shown_with_its_exit_code_and_both_streams() {
        let r = Res {
            code: 3,
            out: "o".to_owned(),
            err: "e".to_owned(),
        };
        assert_eq!(show(&r), "exit 3\nstdout: o\nstderr: e");
    }
}

#[cfg(test)]
mod more_tests {
    use super::*;

    fn smoke_of(repo: &str) -> Smoke<'_> {
        Smoke {
            binary: Path::new("b"),
            repo: Path::new(repo),
            base: PathBuf::new(),
            failures: 0,
            out: Box::leak(Box::new(Vec::new())),
        }
    }

    #[test]
    fn the_package_and_a_fixture_are_under_the_checkout() {
        let s = smoke_of("/r");
        assert_eq!(s.package(), "/r/pkl");
        assert_eq!(
            s.fixture("minimal"),
            "/r/crates/scaffold-package-tests/fixtures/minimal/scaffold.pkl"
        );
    }

    #[test]
    fn a_runner_that_is_killed_has_no_exit_code_and_is_reported_as_exit_minus_1() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("runner");
        std::fs::write(&script, "#!/bin/sh\nkill -9 $$\n").unwrap();
        crate::testutil::settle(&script);
        let mut out = Vec::new();
        let s = Smoke {
            binary: &script,
            repo: Path::new("/r"),
            base: dir.path().to_path_buf(),
            failures: 0,
            out: &mut out,
        };
        let result = s.run(&["--version"]).unwrap();
        assert_eq!(show(&result), "exit -1\nstdout: \nstderr: ");
    }

    #[test]
    fn the_runner_gets_the_environment_of_this_process_without_the_git_variables() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("runner");
        std::fs::write(
            &script,
            "#!/bin/sh\nenv | grep -c '^GIT_' || true\nenv | grep -c '^PATH=' || true\n",
        )
        .unwrap();
        crate::testutil::settle(&script);
        let mut out = Vec::new();
        let s = Smoke {
            binary: &script,
            repo: Path::new("/r"),
            base: dir.path().to_path_buf(),
            failures: 0,
            out: &mut out,
        };
        let result = s.run(&[]).unwrap();
        assert_eq!(result.out, "0\n1");
    }
}
