//! Black-box features for the runner. The same files run against any build of the runner: the binary is named by the
//! `SCAFFOLD_BIN` environment variable, and nothing here knows what language it was written in.
//!
//! Needs `pkl` and `git` on the PATH, and a checkout of this repository for the pkl package and the fixtures.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use cucumber::{World, given, then, when};
use tempfile::TempDir;

/// What the stub server answers, by path: a status and a body.
type Routes = Arc<Mutex<HashMap<String, (u16, String)>>>;

#[derive(Debug)]
struct Stub {
    port: u16,
    routes: Routes,
}

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
    /// Variables a scenario sets for the binary, on top of the clean environment.
    env: Vec<(String, String)>,
    /// A local HTTP server standing in for the release list, the freshness record and the known-bad list. Started by the first step that needs it.
    stub: Option<Stub>,
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
            env: Vec::new(),
            stub: None,
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
        let word = if word.contains("<stub>") {
            word.replace("<stub>", &self.stub_url())
        } else {
            word.to_string()
        };
        word.replace("<repo>", &self.repo().display().to_string())
            .replace(
                "<package>",
                &self.checkout.join("pkl").display().to_string(),
            )
    }

    /// The address of the stub server, started on first use.
    fn stub_url(&self) -> String {
        let stub = self
            .stub
            .as_ref()
            .expect("a step started the stub server before this one");
        format!("http://127.0.0.1:{}", stub.port)
    }

    fn stub(&mut self) -> &Stub {
        self.stub.get_or_insert_with(start_stub)
    }

    fn last(&self) -> &Run {
        self.last
            .as_ref()
            .expect("a command was run before this step")
    }
}

/// The environment the binary sees: the current one, without anything git reads from it and without `CI`, which changes how
/// the runner judges a stale lock. A scenario that needs it sets it with a step.
fn clean_env() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(k, _)| !k.starts_with("GIT_") && k != "CI")
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

/// `\n` and `\r` in a feature file mean a newline and a carriage return, so file contents can be written on one line.
fn unescape(text: &str) -> String {
    text.replace("\\n", "\n").replace("\\r", "\r")
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
        .envs(world.env.iter().cloned())
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

#[then(expr = "the output does not contain {string}")]
fn output_lacks(world: &mut ScaffoldWorld, text: String) {
    let run = world.last();
    assert!(
        !run.out.contains(&unescape(&text)),
        "stdout contains {text:?}, which it must not\nstdout: {}\nstderr: {}",
        run.out,
        run.err
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

#[then(expr = "the file {string} contains {string}")]
fn file_contains(world: &mut ScaffoldWorld, path: String, part: String) {
    let actual =
        fs::read_to_string(world.repo().join(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert!(
        actual.contains(&unescape(&part)),
        "{path} does not contain {part:?}:\n{actual}"
    );
}

#[then(expr = "the file {string} does not contain {string}")]
fn file_does_not_contain(world: &mut ScaffoldWorld, path: String, part: String) {
    let actual =
        fs::read_to_string(world.repo().join(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert!(
        !actual.contains(&unescape(&part)),
        "{path} contains {part:?}, which it must not:\n{actual}"
    );
}

/// Copies a directory tree, replacing `@PACKAGE@` in every `.pkl` file with the path of the package under test.
fn copy_project(from: &Path, to: &Path, package: &str) {
    fs::create_dir_all(to).expect("the project directory");
    for entry in fs::read_dir(from).unwrap_or_else(|e| panic!("{}: {e}", from.display())) {
        let entry = entry.expect("a directory entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_project(&entry.path(), &target, package);
        } else if entry.path().extension().is_some_and(|e| e == "pkl") {
            let text = fs::read_to_string(entry.path()).expect("a readable pkl file");
            fs::write(&target, text.replace("@PACKAGE@", package))
                .expect("the pkl file is written");
        } else {
            fs::copy(entry.path(), &target).expect("the file is copied");
        }
    }
}

/// A small repository from `features/projects/NAME`: a `pantheon.pkl` and the `presets/` it uses.
#[given(expr = "the project {string}")]
fn the_project(world: &mut ScaffoldWorld, name: String) {
    let package = world.checkout.join("pkl").display().to_string();
    copy_project(
        &world.checkout.join("features/projects").join(&name),
        &world.repo(),
        &package,
    );
}

/// The version the binary under test was built for: `SCAFFOLD_VERSION`, or 0.0.0 as the build script defaults to.
#[then("the output is the expected version")]
fn output_is_expected_version(world: &mut ScaffoldWorld) {
    let expected = std::env::var("SCAFFOLD_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
    let last = world.last();
    assert_eq!(last.out, expected, "stderr: {}", last.err);
}

/// UTC time `age_secs` ago as an ISO 8601 string with milliseconds, which is how the runner writes the time in its lock file.
fn iso_ago(age_secs: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970");
    let (secs, millis) = (now.as_secs() as i64 - age_secs, now.subsec_millis());
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

/// Writes the runner's lock file as a process with `pid` took it `age_secs` ago.
fn write_lock(world: &ScaffoldWorld, pid: u32, age_secs: i64) {
    let dir = world.repo().join(".pkl-gen");
    fs::create_dir_all(&dir).expect("the tool directory");
    let held = format!("{{\"pid\":{pid},\"started\":\"{}\"}}", iso_ago(age_secs));
    fs::write(dir.join("lock"), held).expect("the lock file");
}

/// The id of a process that has already finished, so nothing is running under it.
fn dead_pid() -> u32 {
    let mut child = Command::new("true").spawn().expect("true runs");
    let pid = child.id();
    child.wait().expect("true finishes");
    pid
}

#[given("the write lock is held by a running process")]
fn lock_held(world: &mut ScaffoldWorld) {
    write_lock(world, std::process::id(), 0);
}

#[given("the write lock is held by a process that no longer exists")]
fn lock_held_by_dead_process(world: &mut ScaffoldWorld) {
    write_lock(world, dead_pid(), 0);
}

#[given(expr = "the write lock is held by a running process and is {int} minutes old")]
fn lock_held_and_old(world: &mut ScaffoldWorld, minutes: i64) {
    write_lock(world, std::process::id(), minutes * 60);
}

#[given(expr = "the environment variable {string} is {string}")]
fn set_env(world: &mut ScaffoldWorld, name: String, value: String) {
    let value = world.expand(&value);
    world.env.push((name, value));
}

#[given(expr = "the file {string} is a symbolic link to a file outside the repository")]
fn link_outside(world: &mut ScaffoldWorld, path: String) {
    let outside = world.base.path().join("outside");
    fs::create_dir_all(&outside).expect("the outside directory");
    let link = world.repo().join(path);
    fs::create_dir_all(link.parent().expect("a parent directory")).expect("the parent directory");
    std::os::unix::fs::symlink(outside.join("target.txt"), link).expect("the symbolic link");
}

#[then(expr = "the file {string} does not exist outside the repository")]
fn outside_missing(world: &mut ScaffoldWorld, name: String) {
    assert!(
        !world.base.path().join("outside").join(&name).exists(),
        "{name} was written outside the repository"
    );
}

/// Starts the stub server on a free port. It answers every request from its route table, and 404 for a path it does not know.
fn start_stub() -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("the address").port();
    let routes: Routes = Arc::default();
    let shared = Arc::clone(&routes);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            answer(stream, &shared);
        }
    });
    Stub { port, routes }
}

fn answer(mut stream: TcpStream, routes: &Routes) {
    let Ok(clone) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(clone);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
            break;
        }
    }
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();
    let (status, body) = routes
        .lock()
        .expect("the route table")
        .get(&path)
        .cloned()
        .unwrap_or((404, "not found".to_string()));
    let reply = format!(
        "HTTP/1.1 {status} stub\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(reply.as_bytes());
}

fn set_route(world: &mut ScaffoldWorld, path: String, status: u16, body: String) {
    world
        .stub()
        .routes
        .lock()
        .expect("the route table")
        .insert(path, (status, body));
}

#[given(expr = "the stub server answers {string} with {string}")]
fn stub_answers(world: &mut ScaffoldWorld, path: String, body: String) {
    set_route(world, path, 200, body);
}

#[given(expr = "the stub server answers {string} with status {int}")]
fn stub_answers_status(world: &mut ScaffoldWorld, path: String, status: u16) {
    set_route(world, path, status, "stub failure".to_string());
}

/// A freshness record whose last successful check was `days` days ago, or ahead when `days` is negative.
fn freshness_record(days: i64) -> String {
    let when = iso_ago(days * 86_400);
    format!(
        "{{\"schemaVersion\":1,\"lastSuccessfulPinCheck\":\"{}Z\"}}",
        &when[..19]
    )
}

#[given(expr = "the stub server answers {string} with a freshness record from {int} days ago")]
fn stub_freshness_ago(world: &mut ScaffoldWorld, path: String, days: i64) {
    set_route(world, path, 200, freshness_record(days));
}

#[given(expr = "the stub server answers {string} with a freshness record dated {int} days ahead")]
fn stub_freshness_ahead(world: &mut ScaffoldWorld, path: String, days: i64) {
    set_route(world, path, 200, freshness_record(-days));
}

/// The runner asks for its known-bad list on every `render` and `latest`; point it at the stub so no test reaches the network.
#[given(expr = "the stub server serves the known-bad list {string}")]
fn stub_known_bad(world: &mut ScaffoldWorld, list: String) {
    set_route(world, "/known-bad.json".to_string(), 200, list);
    let url = format!("{}/known-bad.json", world.stub_url());
    world
        .env
        .push(("PANTHEON_SCAFFOLD_KNOWN_BAD_URL".to_string(), url));
}

/// Scenarios tagged with any of these (comma-separated in `SCAFFOLD_SKIP_TAGS`, without the `@`) are not run. The Rust
/// runner uses it for the commands it has not ported yet; the TypeScript runner runs everything. The skipped tags are
/// printed, so a skip never goes unnoticed.
fn skipped_tags() -> Vec<String> {
    std::env::var("SCAFFOLD_SKIP_TAGS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}

#[tokio::main]
async fn main() {
    let skip = skipped_tags();
    if !skip.is_empty() {
        eprintln!(
            "SCAFFOLD_SKIP_TAGS: not running scenarios tagged {}",
            skip.join(", ")
        );
    }
    ScaffoldWorld::cucumber()
        .fail_on_skipped()
        .filter_run_and_exit(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../features"),
            move |feature, _, scenario| {
                !feature
                    .tags
                    .iter()
                    .chain(&scenario.tags)
                    .any(|tag| skip.contains(tag))
            },
        )
        .await;
}
