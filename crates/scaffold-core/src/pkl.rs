//! Evaluates the consumer's `pantheon.pkl` through the pinned `pkl` binary, ported from `runner/src/pkl.ts`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::contract::ContractError;

/// Flags that restrict what the consumer's pantheon.pkl may load or read.
pub const ALLOWED_MODULES: &str = "pkl:,file:,package:,projectpackage:";
// package: lets a package module read its own assets, https: lets pkl download the package.
// There is no file: here, so a pantheon.pkl cannot read arbitrary files.
pub const ALLOWED_RESOURCES: &str = "package:,projectpackage:,https:";

pub struct EvalOptions {
    /// Absolute folders whose files a local (file:) package may read, for working on a checkout of the package.
    pub allow_files_under: Vec<PathBuf>,
    /// Attempts for a failure that looks like a network or download problem. Other failures are not retried.
    pub attempts: u32,
    /// Time to wait between attempts.
    pub delay: Duration,
    /// Where pkl keeps downloaded packages. A warm cache lets an offline run render.
    pub cache_dir: Option<PathBuf>,
    /// Extra environment for the pkl process, for tests.
    pub env: Vec<(String, String)>,
    /// How long one pkl run may take before it is stopped. The TypeScript runner has no limit.
    pub timeout: Duration,
    /// The binary to run. `pkl` unless a test points elsewhere.
    pub binary: OsString,
}

impl Default for EvalOptions {
    fn default() -> Self {
        Self {
            allow_files_under: Vec::new(),
            attempts: 3,
            delay: Duration::from_secs(2),
            cache_dir: None,
            env: Vec::new(),
            timeout: Duration::from_secs(120),
            binary: OsString::from("pkl"),
        }
    }
}

pub fn allowed_resources(opts: &EvalOptions) -> String {
    let local = opts.allow_files_under.iter().map(|dir| {
        let mut text = dir.to_string_lossy().into_owned();
        if !text.ends_with('/') {
            text.push('/');
        }
        format!("^file://{}", escape_regex(&text))
    });
    std::iter::once(ALLOWED_RESOURCES.to_owned())
        .chain(local)
        .collect::<Vec<_>>()
        .join(",")
}

fn escape_regex(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if ".*+?^${}()|[]\\".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A failure worth retrying: the package or a pin could not be fetched. A newly created release can answer 404 for a
/// while.
pub fn looks_like_fetch_failure(stderr: &str) -> bool {
    static PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r"(?i)Exception when making request|Cannot download package|Received unexpected status code\s*`?(404|429|5\d\d)|Unable to (resolve|connect)|timed out|Connection (refused|reset)",
        )
        .expect("the pattern is valid")
    });
    PATTERN.is_match(stderr)
}

struct Run {
    out: String,
    err: String,
    code: i32,
}

/// Starts the process. A binary that is open for writing is refused by Linux ("text file busy"), which happens for a
/// moment while an installer replaces it, so that one error is retried for up to a second.
fn spawn(command: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    retry_while_busy(50, Duration::from_millis(20), || command.spawn())
}

/// Runs `attempt`, and runs it again after `delay` for as long as it fails with "executable file busy", at most
/// `retries` more times. Any other result is returned at once.
fn retry_while_busy<T>(
    retries: u32,
    delay: Duration,
    mut attempt: impl FnMut() -> std::io::Result<T>,
) -> std::io::Result<T> {
    for _ in 0..retries {
        match attempt() {
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(delay)
            }
            other => return other,
        }
    }
    attempt()
}

fn once(config: &Path, cwd: &Path, opts: &EvalOptions) -> Result<Run, ContractError> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut command = Command::new(&opts.binary);
    command.args(["eval", "-f", "json"]);
    if let Some(cache) = &opts.cache_dir {
        command.arg("--cache-dir").arg(cache);
    }
    command
        .args(["--allowed-modules", ALLOWED_MODULES, "--allowed-resources"])
        .arg(allowed_resources(opts))
        .arg(config)
        .current_dir(cwd)
        .envs(opts.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let binary = opts.binary.to_string_lossy();
    let mut child = spawn(&mut command).map_err(|e| {
        ContractError(if e.kind() == std::io::ErrorKind::NotFound {
            format!("{binary} was not found: install the pinned pkl with mise")
        } else {
            format!("could not start {binary}: {e}")
        })
    })?;
    let read = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            String::from_utf8_lossy(&bytes).into_owned()
        })
    };
    let out = read(Box::new(child.stdout.take().expect("stdout is piped")));
    let err = read(Box::new(child.stderr.take().expect("stderr is piped")));
    let deadline = std::time::Instant::now() + opts.timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() >= deadline => {
                // The reader threads are left to end on their own: a grandchild can hold the pipes open.
                let _ = child.kill();
                let _ = child.wait();
                return Err(ContractError(format!(
                    "pkl eval timed out after {} seconds",
                    opts.timeout.as_secs_f64()
                )));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(ContractError(format!("could not wait for {binary}: {e}"))),
        }
    };
    Ok(Run {
        out: out.join().unwrap_or_default(),
        err: err.join().unwrap_or_default(),
        code: status.code().unwrap_or(-1),
    })
}

/// Fails closed: after the attempts are used up a fetch problem is an error, never a pass.
pub fn eval_contract_json(
    config: &Path,
    cwd: &Path,
    opts: &EvalOptions,
) -> Result<serde_json::Value, ContractError> {
    let attempts = opts.attempts.max(1);
    for attempt in 1..=attempts {
        let run = once(config, cwd, opts)?;
        if run.code == 0 {
            return serde_json::from_str(&run.out)
                .map_err(|_| ContractError("pkl eval did not print JSON".to_owned()));
        }
        let fetch = looks_like_fetch_failure(&run.err);
        if !fetch || attempt == attempts {
            let tries = if attempt > 1 {
                format!(" after {attempt} attempts")
            } else {
                String::new()
            };
            let closed = if fetch { ", failing closed" } else { "" };
            return Err(ContractError(format!(
                "pkl eval failed (exit {}){tries}{closed}:\n{}",
                run.code,
                run.err.trim()
            )));
        }
        std::thread::sleep(opts.delay);
    }
    unreachable!("the last attempt always returns")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    fn fast() -> EvalOptions {
        EvalOptions {
            delay: Duration::from_millis(1),
            ..EvalOptions::default()
        }
    }

    /// A stand-in for pkl: a shell script with the given body.
    fn fake(dir: &Path, body: &str) -> OsString {
        let path = dir.join("fake-pkl");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.into_os_string()
    }

    fn err(result: Result<serde_json::Value, ContractError>) -> String {
        result.unwrap_err().to_string()
    }

    #[test]
    fn allowed_resources_lists_the_package_and_https_schemes_only_by_default() {
        assert_eq!(
            allowed_resources(&EvalOptions::default()),
            "package:,projectpackage:,https:"
        );
    }

    #[test]
    fn a_local_package_folder_is_allowed_with_its_regex_characters_escaped() {
        let opts = EvalOptions {
            allow_files_under: vec![PathBuf::from("/work/a.b+c"), PathBuf::from("/other/")],
            ..EvalOptions::default()
        };
        assert_eq!(
            allowed_resources(&opts),
            r"package:,projectpackage:,https:,^file:///work/a\.b\+c/,^file:///other/"
        );
    }

    #[test]
    fn download_and_network_problems_look_like_fetch_failures() {
        for text in [
            "Exception when making request to https://x",
            "Cannot download package",
            "Received unexpected status code `404`",
            "Received unexpected status code 503",
            "Unable to resolve host",
            "request timed out",
            "Connection refused",
        ] {
            assert!(looks_like_fetch_failure(text), "{text}");
        }
        assert!(!looks_like_fetch_failure("Type mismatch: expected String"));
        assert!(!looks_like_fetch_failure(
            "Received unexpected status code 403"
        ));
    }

    #[test]
    fn the_minimal_fixture_evaluates_to_a_contract() {
        let fixture = root().join("runner/test/fixtures/minimal/pantheon.pkl");
        let json = eval_contract_json(&fixture, &root(), &EvalOptions::default()).unwrap();
        assert_eq!(json["schemaVersion"], 1);
        assert!(json["files"].as_array().unwrap().len() >= 4);
    }

    #[test]
    fn a_path_with_non_ascii_characters_evaluates() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("café ünï");
        std::fs::create_dir(&folder).unwrap();
        let config = folder.join("conf.pkl");
        // An explicit renderer, because the restricted resource list refuses the lookup of `pkl.outputFormat`.
        std::fs::write(
            &config,
            "output { renderer = new JsonRenderer {} }\na = 1\n",
        )
        .unwrap();
        let json = eval_contract_json(&config, &folder, &EvalOptions::default()).unwrap();
        assert_eq!(json["a"], 1);
    }

    #[test]
    fn a_missing_binary_is_an_error_that_names_it() {
        let opts = EvalOptions {
            binary: OsString::from("pkl-that-does-not-exist"),
            ..fast()
        };
        let message = err(eval_contract_json(
            Path::new("x.pkl"),
            Path::new("."),
            &opts,
        ));
        assert_eq!(
            message,
            "pkl-that-does-not-exist was not found: install the pinned pkl with mise"
        );
    }

    #[test]
    fn a_run_that_takes_too_long_is_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let opts = EvalOptions {
            binary: fake(dir.path(), "exec sleep 8"),
            timeout: Duration::from_millis(300),
            ..fast()
        };
        let started = std::time::Instant::now();
        let message = err(eval_contract_json(Path::new("x.pkl"), dir.path(), &opts));
        assert_eq!(message, "pkl eval timed out after 0.3 seconds");
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn an_ordinary_failure_is_reported_once_with_its_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let opts = EvalOptions {
            binary: fake(dir.path(), "echo 'Type mismatch' >&2; exit 1"),
            ..fast()
        };
        assert_eq!(
            err(eval_contract_json(Path::new("x.pkl"), dir.path(), &opts)),
            "pkl eval failed (exit 1):\nType mismatch"
        );
    }

    #[test]
    fn a_fetch_failure_is_retried_then_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let count = dir.path().join("count");
        let body = format!(
            "echo x >> '{}'; echo 'Cannot download package' >&2; exit 1",
            count.display()
        );
        let opts = EvalOptions {
            binary: fake(dir.path(), &body),
            attempts: 3,
            ..fast()
        };
        let message = err(eval_contract_json(Path::new("x.pkl"), dir.path(), &opts));
        assert_eq!(
            message,
            "pkl eval failed (exit 1) after 3 attempts, failing closed:\nCannot download package"
        );
        assert_eq!(std::fs::read_to_string(count).unwrap().lines().count(), 3);
    }

    #[test]
    fn a_run_killed_by_a_signal_reports_exit_minus_one() {
        let dir = tempfile::tempdir().unwrap();
        let opts = EvalOptions {
            binary: fake(dir.path(), "kill -9 $$"),
            ..fast()
        };
        assert_eq!(
            err(eval_contract_json(Path::new("x.pkl"), dir.path(), &opts)),
            "pkl eval failed (exit -1):\n"
        );
    }

    #[test]
    fn a_fetch_failure_that_clears_on_a_later_attempt_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("seen");
        let body = format!(
            "if [ -f '{0}' ]; then echo '{{\"ok\":true}}'; else touch '{0}'; echo 'Cannot download package' >&2; exit 1; fi",
            marker.display()
        );
        let opts = EvalOptions {
            binary: fake(dir.path(), &body),
            attempts: 2,
            ..fast()
        };
        assert_eq!(
            eval_contract_json(Path::new("x.pkl"), dir.path(), &opts).unwrap()["ok"],
            true
        );
    }

    fn busy() -> std::io::Error {
        std::io::Error::from(std::io::ErrorKind::ExecutableFileBusy)
    }

    #[test]
    fn a_busy_start_is_retried_until_it_works() {
        let mut calls = 0;
        let result = retry_while_busy(5, Duration::ZERO, || {
            calls += 1;
            if calls <= 5 { Err(busy()) } else { Ok(calls) }
        });
        assert_eq!(
            result.unwrap(),
            6,
            "five busy answers use all five retries, the sixth call is the last"
        );
    }

    #[test]
    fn a_start_that_stays_busy_gives_up_after_the_retries() {
        let mut calls = 0;
        let result: std::io::Result<()> = retry_while_busy(5, Duration::ZERO, || {
            calls += 1;
            Err(busy())
        });
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::ExecutableFileBusy
        );
        assert_eq!(calls, 6);
    }

    #[test]
    fn any_other_error_is_returned_at_once() {
        let mut calls = 0;
        let result: std::io::Result<()> = retry_while_busy(5, Duration::ZERO, || {
            calls += 1;
            Err(std::io::Error::from(std::io::ErrorKind::NotFound))
        });
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
        assert_eq!(calls, 1);
    }

    #[test]
    fn a_success_is_returned_at_once() {
        let mut calls = 0;
        let result = retry_while_busy(5, Duration::from_secs(60), || {
            calls += 1;
            Ok::<_, std::io::Error>(7)
        });
        assert_eq!((result.unwrap(), calls), (7, 1));
    }

    #[test]
    fn a_binary_that_is_briefly_busy_is_started_once_it_is_free() {
        // Linux refuses to run a file that is open for writing ("text file busy"). That happens for a moment when an
        // installer is still replacing the binary, and in tests when another thread forks while a script is written.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fake-pkl");
        std::fs::write(&path, "#!/bin/sh\necho '{\"ok\":true}'\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let writer = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            drop(writer);
        });
        let opts = EvalOptions {
            binary: path.into_os_string(),
            ..fast()
        };
        assert_eq!(
            eval_contract_json(Path::new("x.pkl"), dir.path(), &opts).unwrap()["ok"],
            true
        );
        release.join().unwrap();
    }

    #[test]
    fn output_that_is_not_json_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let opts = EvalOptions {
            binary: fake(dir.path(), "echo not json"),
            ..fast()
        };
        assert_eq!(
            err(eval_contract_json(Path::new("x.pkl"), dir.path(), &opts)),
            "pkl eval did not print JSON"
        );
    }

    #[test]
    fn the_flags_restrict_modules_and_resources() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("args");
        let body = format!("echo \"$@\" > '{}'; echo '{{}}'", log.display());
        let opts = EvalOptions {
            binary: fake(dir.path(), &body),
            cache_dir: Some(PathBuf::from("/cache")),
            ..fast()
        };
        eval_contract_json(Path::new("conf.pkl"), dir.path(), &opts).unwrap();
        assert_eq!(
            std::fs::read_to_string(log).unwrap().trim(),
            "eval -f json --cache-dir /cache --allowed-modules pkl:,file:,package:,projectpackage: --allowed-resources package:,projectpackage:,https: conf.pkl"
        );
    }
}
