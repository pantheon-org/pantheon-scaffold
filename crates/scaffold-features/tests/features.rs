//! Black-box features for the runner. The same files run against any build of the runner: the binary is named by the
//! `SCAFFOLD_BIN` environment variable, and nothing here knows what language it was written in.
//!
//! Needs `pkl` and `git` on the PATH, and a checkout of this repository for the pkl package and the fixtures.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use cucumber::{World, given, then, when};
use tempfile::TempDir;

/// What one run of the binary printed and returned.
#[derive(Debug, Clone)]
struct Run {
    code: i32,
    out: String,
    err: String,
}

#[derive(Debug, World)]
#[world(init = Self::new)]
struct ScaffoldWorld {
    /// Holds the repository under test, and is the directory the binary runs from, so it needs nothing from the source tree.
    base: TempDir,
    binary: PathBuf,
    checkout: PathBuf,
    last: Option<Run>,
}

impl ScaffoldWorld {
    fn new() -> Self {
        let binary = std::env::var("SCAFFOLD_BIN").unwrap_or_else(|_| {
            panic!("SCAFFOLD_BIN must name the runner binary to test, for example SCAFFOLD_BIN=dist/pantheon-scaffold-linux-x64")
        });
        let binary =
            fs::canonicalize(&binary).unwrap_or_else(|e| panic!("SCAFFOLD_BIN {binary}: {e}"));
        let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the checkout");
        let base = TempDir::new().expect("a temporary directory");
        fs::create_dir(base.path().join("repo")).expect("the repository directory");
        Self {
            base,
            binary,
            checkout,
            last: None,
        }
    }

    fn repo(&self) -> PathBuf {
        self.base.path().join("repo")
    }

    /// Words of a command line, with the placeholders replaced: `<repo>`, `<package>` and `<fixture:NAME>`.
    fn expand(&self, word: &str) -> String {
        if let Some(name) = word
            .strip_prefix("<fixture:")
            .and_then(|w| w.strip_suffix('>'))
        {
            return self
                .checkout
                .join("runner/test/fixtures")
                .join(name)
                .join("pantheon.pkl")
                .display()
                .to_string();
        }
        word.replace("<repo>", &self.repo().display().to_string())
            .replace(
                "<package>",
                &self.checkout.join("pkl").display().to_string(),
            )
    }

    fn last(&self) -> &Run {
        self.last
            .as_ref()
            .expect("a command was run before this step")
    }
}

/// The environment the binary sees: the current one, without anything git reads from it.
fn clean_env() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(k, _)| !k.starts_with("GIT_"))
        .collect()
}

fn run_git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-C",
            &dir.display().to_string(),
            "-c",
            "user.name=features",
            "-c",
            "user.email=features@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env_clear()
        .envs(clean_env())
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {}: {}{}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
}

/// `\n` in a feature file means a newline, so file contents can be written on one line.
fn unescape(text: &str) -> String {
    text.replace("\\n", "\n")
}

#[given("a new empty repository")]
fn empty_repository(_world: &mut ScaffoldWorld) {}

#[given("a new git repository")]
fn git_repository(world: &mut ScaffoldWorld) {
    run_git(&world.repo(), &["init", "-q", "-b", "main"]);
}

#[when(expr = "I run {string}")]
fn run_binary(world: &mut ScaffoldWorld, command: String) {
    let args: Vec<String> = command
        .split_whitespace()
        .map(|w| world.expand(w))
        .collect();
    let out = Command::new(&world.binary)
        .args(&args)
        .current_dir(world.base.path())
        .env_clear()
        .envs(clean_env())
        .output()
        .expect("the binary runs");
    world.last = Some(Run {
        code: out.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&out.stdout).trim().to_string(),
        err: String::from_utf8_lossy(&out.stderr).trim().to_string(),
    });
}

/// The same step as a precondition, so a scenario can read `Given ... And I run ...`.
#[given(expr = "I run {string}")]
fn given_run_binary(world: &mut ScaffoldWorld, command: String) {
    run_binary(world, command);
}

#[when(expr = "I write {string} to the file {string}")]
fn write_file(world: &mut ScaffoldWorld, content: String, path: String) {
    let target = world.repo().join(path);
    fs::create_dir_all(target.parent().expect("a parent directory")).expect("the parent directory");
    fs::write(target, unescape(&content)).expect("the file is written");
}

#[given(expr = "I write {string} to the file {string}")]
fn given_write_file(world: &mut ScaffoldWorld, content: String, path: String) {
    write_file(world, content, path);
}

#[when("I commit everything")]
fn commit_everything(world: &mut ScaffoldWorld) {
    run_git(&world.repo(), &["add", "-A"]);
    run_git(&world.repo(), &["commit", "-q", "-m", "snapshot"]);
}

#[given("I commit everything")]
fn given_commit_everything(world: &mut ScaffoldWorld) {
    commit_everything(world);
}

#[then(expr = "the exit code is {int}")]
fn exit_code(world: &mut ScaffoldWorld, code: i32) {
    let last = world.last();
    assert_eq!(
        last.code, code,
        "exit {}\nstdout: {}\nstderr: {}",
        last.code, last.out, last.err
    );
}

#[then(expr = "the output contains {string}")]
fn output_contains(world: &mut ScaffoldWorld, text: String) {
    let last = world.last();
    assert!(
        last.out.contains(&text),
        "stdout does not contain {text:?}\nstdout: {}\nstderr: {}",
        last.out,
        last.err
    );
}

#[then(expr = "the output is {string}")]
fn output_is(world: &mut ScaffoldWorld, text: String) {
    let last = world.last();
    assert_eq!(last.out, text, "stderr: {}", last.err);
}

#[then(expr = "the error contains {string}")]
fn error_contains(world: &mut ScaffoldWorld, text: String) {
    let last = world.last();
    assert!(
        last.err.contains(&text),
        "stderr does not contain {text:?}\nstdout: {}\nstderr: {}",
        last.out,
        last.err
    );
}

#[then(expr = "the file {string} exists")]
fn file_exists(world: &mut ScaffoldWorld, path: String) {
    assert!(
        world.repo().join(&path).exists(),
        "{path} does not exist\n{:?}",
        world.last
    );
}

#[then(expr = "the file {string} does not exist")]
fn file_missing(world: &mut ScaffoldWorld, path: String) {
    assert!(!world.repo().join(&path).exists(), "{path} exists");
}

#[then(expr = "the file {string} has the mode {string}")]
fn file_mode(world: &mut ScaffoldWorld, path: String, mode: String) {
    let actual = fs::metadata(world.repo().join(&path))
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(format!("{actual:o}"), mode, "mode of {path}");
}

#[then(expr = "the file {string} has the content {string}")]
fn file_content(world: &mut ScaffoldWorld, path: String, content: String) {
    let actual =
        fs::read_to_string(world.repo().join(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!(actual, unescape(&content), "content of {path}");
}

#[then(expr = "the file {string} does not have the content {string}")]
fn file_content_differs(world: &mut ScaffoldWorld, path: String, content: String) {
    let actual =
        fs::read_to_string(world.repo().join(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_ne!(
        actual,
        unescape(&content),
        "content of {path} is the one it must not have"
    );
}

/// The version the binary under test was built for: `SCAFFOLD_VERSION`, or 0.0.0 as the build script defaults to.
#[then("the output is the expected version")]
fn output_is_expected_version(world: &mut ScaffoldWorld) {
    let expected = std::env::var("SCAFFOLD_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
    let last = world.last();
    assert_eq!(last.out, expected, "stderr: {}", last.err);
}

/// UTC time as an ISO 8601 string with milliseconds, which is how the runner writes the time in its lock file.
fn iso_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970");
    let (secs, millis) = (now.as_secs() as i64, now.subsec_millis());
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[given("the write lock is held by a running process")]
fn lock_held(world: &mut ScaffoldWorld) {
    let dir = world.repo().join(".pkl-gen");
    fs::create_dir_all(&dir).expect("the tool directory");
    let held = format!(
        "{{\"pid\":{},\"started\":\"{}\"}}",
        std::process::id(),
        iso_now()
    );
    fs::write(dir.join("lock"), held).expect("the lock file");
}

#[tokio::main]
async fn main() {
    ScaffoldWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../features"))
        .await;
}
