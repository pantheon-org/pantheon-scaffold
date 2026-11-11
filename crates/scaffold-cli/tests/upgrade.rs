//! `upgrade` through the real binary, with a local server in place of the release and fake `mise` and `gh` programs on the
//! PATH that record what they were asked. Ported from `runner/test/cli-upgrade.test.ts`.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

const RUNNER_TOOL: &str = "github:pantheon-org/pantheon-scaffold";

fn h(c: char) -> String {
    c.to_string().repeat(64)
}

fn amends(v: &str, sum: &str) -> String {
    format!(
        "amends \"package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{v}/pantheon-scaffold@{v}::sha256:{sum}#/Config.pkl\"\n"
    )
}

fn mise_toml(v: &str) -> String {
    format!("[tools]\n\"{RUNNER_TOOL}\" = \"{v}\"\n")
}

struct Setup {
    bin: tempfile::TempDir,
    repo: tempfile::TempDir,
    base: String,
}

struct Run {
    code: i32,
    out: String,
    err: String,
}

/// A local server for the release, and fake mise and gh. `checksums` is what the server answers for v0.2.0, `None` for a
/// release that is not there.
fn setup(checksums: Option<String>) -> Setup {
    let bin = tempfile::tempdir().unwrap();
    std::fs::write(bin.path().join("calls"), "").unwrap();
    // A fake pkl: it records the file it was asked about and what that file said, and prints an empty contract, or fails with
    // PKL_FAIL as its message.
    let pkl = format!(
        "#!/bin/sh\nfor last; do :; done\necho \"pkl $*\" >> \"{calls}\"\ncat \"$last\" >> \"{calls}.pkl\"\nif [ -n \"$PKL_FAIL\" ]; then echo \"$PKL_FAIL\" >&2; exit 1; fi\necho '{{}}'\n",
        calls = bin.path().join("calls").display()
    );
    let pkl_path = bin.path().join("pkl");
    std::fs::write(&pkl_path, pkl).unwrap();
    std::fs::set_permissions(&pkl_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    for name in ["mise", "gh"] {
        let script = format!(
            "#!/bin/sh\necho \"{name} $*\" >> \"{calls}\"\nif [ \"{name}\" = gh ] && [ -n \"$GH_FAIL\" ]; then echo \"no valid attestation\" >&2; exit 1; fi\nif [ \"{name}\" = mise ] && [ \"$1\" = x ] && [ -n \"$MISE_RENDER_EXIT\" ]; then exit \"$MISE_RENDER_EXIT\"; fi\nexit 0\n",
            calls = bin.path().join("calls").display()
        );
        let path = bin.path().join(name);
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
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
            let wanted = String::from_utf8_lossy(&request).contains("GET /v0.2.0/checksums.txt ");
            let (status, body) = match (&checksums, wanted) {
                (Some(text), true) => (200, text.clone()),
                _ => (404, "gone".to_owned()),
            };
            let answer = format!(
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(answer.as_bytes());
        }
    });
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("pantheon.pkl"), amends("0.1.0", &h('a'))).unwrap();
    std::fs::write(repo.path().join("mise.toml"), mise_toml("0.1.0")).unwrap();
    Setup { bin, repo, base }
}

impl Setup {
    fn run(&self, args: &[&str], env: &[(&str, &str)]) -> Run {
        let path = format!(
            "{}:{}",
            self.bin.path().display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut command = Command::new(env!("CARGO_BIN_EXE_pantheon-scaffold"));
        command
            .args(args)
            .current_dir(self.repo.path())
            .env("PATH", path)
            .env("PANTHEON_SCAFFOLD_RELEASE_BASE", &self.base)
            .env_remove("GITHUB_TOKEN")
            .env_remove("GH_TOKEN")
            .env_remove("CI");
        for (name, value) in env {
            command.env(name, value);
        }
        let output = command.output().unwrap();
        Run {
            code: output.status.code().unwrap_or(-1),
            out: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            err: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }
    }

    fn root(&self) -> String {
        self.repo.path().display().to_string()
    }

    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.bin.path().join("calls"))
            .unwrap()
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// What the fake pkl was shown: the text of the config it was asked to evaluate.
    fn pkl_saw(&self) -> String {
        std::fs::read_to_string(self.bin.path().join("calls.pkl")).unwrap()
    }

    fn get(&self, file: &str) -> String {
        std::fs::read_to_string(self.repo.path().join(file)).unwrap()
    }
}

fn preset_text(v: &str, sum: &str) -> String {
    format!(
        "import \"package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{v}/pantheon-scaffold@{v}::sha256:{sum}#/Config.pkl\"\n\nfunction apply(w) = w\n"
    )
}

fn default_checksums() -> Option<String> {
    Some(format!("{}  pantheon-scaffold@0.2.0\n", h('c')))
}

#[test]
fn upgrade_verifies_the_release_rewrites_the_pins_refreshes_mise_and_re_renders() {
    let s = setup(default_checksums());
    let r = s.run(&["upgrade", "0.2.0", "--root", &s.root()], &[]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("0.1.0 -> 0.2.0"), "{}", r.out);
    assert!(r.out.ends_with("upgraded 0.1.0 -> 0.2.0"), "{}", r.out);
    assert_eq!(s.get("pantheon.pkl"), amends("0.2.0", &h('c')));
    assert_eq!(s.get("mise.toml"), mise_toml("0.2.0"));
    let calls = s.calls();
    assert!(
        calls[0].starts_with("gh attestation verify ")
            && calls[0].contains("checksums.txt --repo pantheon-org/pantheon-scaffold"),
        "{}",
        calls[0]
    );
    // The moved config is evaluated before the first thing mise does.
    assert!(
        calls[1].starts_with("pkl ") && calls[1].ends_with("/pantheon.pkl"),
        "{}",
        calls[1]
    );
    assert_eq!(
        calls[2..],
        [
            "mise install",
            "mise lock",
            &format!(
                "mise x -- pantheon-scaffold render pantheon.pkl --root {}",
                s.root()
            )
        ]
    );
    assert_eq!(s.pkl_saw(), amends("0.2.0", &h('c')));
}

#[test]
fn a_config_that_the_new_package_cannot_evaluate_stops_the_upgrade_before_anything_is_changed() {
    let s = setup(default_checksums());
    let message = "preset: job \"build\" does not exist in workflow \"Rust CI\". Jobs: build-test";
    let r = s.run(
        &["upgrade", "0.2.0", "--root", &s.root()],
        &[("PKL_FAIL", message)],
    );
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(
        r.err
            .starts_with("pantheon.pkl does not evaluate with 0.2.0, so nothing was changed:\n"),
        "{}",
        r.err
    );
    assert!(r.err.contains(message), "{}", r.err);
    assert_eq!(s.get("pantheon.pkl"), amends("0.1.0", &h('a')));
    assert_eq!(s.get("mise.toml"), mise_toml("0.1.0"));
    assert!(
        s.calls()
            .iter()
            .all(|c| c.starts_with("gh") || c.starts_with("pkl")),
        "mise must not run: {:?}",
        s.calls()
    );
    assert!(!s.repo.path().join(".pkl-gen/lock").exists());
}

#[test]
fn the_evaluation_gets_the_cache_folder_and_the_local_package_folders_of_the_command_line() {
    let s = setup(default_checksums());
    let extra = tempfile::tempdir().unwrap();
    let cache = extra.path().join("cache").display().to_string();
    let local = extra.path().join("pkgs").display().to_string();
    let r = s.run(
        &[
            "upgrade",
            "0.2.0",
            "--root",
            &s.root(),
            "--no-render",
            "--cache-dir",
            &cache,
            "--allow-local-package",
            &local,
            "--attempts",
            "1",
        ],
        &[],
    );
    assert_eq!(r.code, 0, "{}", r.err);
    let call = s
        .calls()
        .into_iter()
        .find(|c| c.starts_with("pkl "))
        .unwrap();
    assert!(call.contains(&format!("--cache-dir {cache}")), "{call}");
    // pkl is given the folder as an escaped pattern, so only its last part is looked for.
    assert!(call.contains("pkgs/"), "{call}");
}

#[test]
fn a_dry_run_does_not_run_pkl_and_lists_the_presets_it_would_move() {
    let s = setup(default_checksums());
    let preset = preset_text("0.1.0", &h('a'));
    std::fs::create_dir_all(s.repo.path().join("presets")).unwrap();
    std::fs::write(s.repo.path().join("presets/ci.pkl"), &preset).unwrap();
    let r = s.run(&["upgrade", "0.2.0", "--root", &s.root(), "--dry-run"], &[]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(
        r.out
            .contains("would change  presets/ci.pkl: 1 import of the package"),
        "{}",
        r.out
    );
    assert!(
        s.calls().iter().all(|c| !c.starts_with("pkl")),
        "{:?}",
        s.calls()
    );
    assert_eq!(s.get("presets/ci.pkl"), preset);
}

#[test]
fn the_imports_of_the_package_in_a_preset_move_with_the_config_and_are_evaluated_before_mise() {
    let s = setup(default_checksums());
    std::fs::create_dir_all(s.repo.path().join("presets")).unwrap();
    std::fs::write(
        s.repo.path().join("presets/ci.pkl"),
        preset_text("0.1.0", &h('a')),
    )
    .unwrap();
    let r = s.run(&["upgrade", "0.2.0", "--root", &s.root()], &[]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(
        r.out
            .contains("changed  presets/ci.pkl: 1 import of the package"),
        "{}",
        r.out
    );
    assert_eq!(s.get("presets/ci.pkl"), preset_text("0.2.0", &h('c')));
}

#[test]
fn a_preset_that_no_longer_fits_is_put_back_with_the_config_and_nothing_is_changed() {
    let s = setup(default_checksums());
    std::fs::create_dir_all(s.repo.path().join("presets")).unwrap();
    let preset = preset_text("0.1.0", &h('a'));
    std::fs::write(s.repo.path().join("presets/ci.pkl"), &preset).unwrap();
    let message = "preset: job \"build\" does not exist in workflow \"Rust CI\". Jobs: build-test";
    let r = s.run(
        &["upgrade", "0.2.0", "--root", &s.root()],
        &[("PKL_FAIL", message)],
    );
    assert_eq!(r.code, 1);
    assert!(r.err.contains(message), "{}", r.err);
    assert_eq!(s.get("presets/ci.pkl"), preset);
    assert_eq!(s.get("pantheon.pkl"), amends("0.1.0", &h('a')));
    assert_eq!(s.get("mise.toml"), mise_toml("0.1.0"));
    assert!(
        s.calls()
            .iter()
            .all(|c| c.starts_with("gh") || c.starts_with("pkl")),
        "{:?}",
        s.calls()
    );
}

#[test]
fn a_failed_attestation_stops_with_exit_1_a_clean_message_and_no_file_touched_and_mise_never_run() {
    let s = setup(default_checksums());
    let before = s.get("pantheon.pkl");
    let r = s.run(
        &["upgrade", "0.2.0", "--root", &s.root()],
        &[("GH_FAIL", "1")],
    );
    assert_eq!(r.code, 1);
    assert!(r.err.contains("attestation check failed"), "{}", r.err);
    assert!(r.err.contains("no valid attestation"), "{}", r.err);
    assert!(!r.err.contains("panicked"), "{}", r.err);
    assert_eq!(s.get("pantheon.pkl"), before);
    assert!(s.calls().iter().all(|c| !c.starts_with("mise")));
}

#[test]
fn the_exit_code_of_the_re_render_is_the_exit_code_of_upgrade() {
    let s = setup(default_checksums());
    let r = s.run(
        &["upgrade", "0.2.0", "--root", &s.root()],
        &[("MISE_RENDER_EXIT", "3")],
    );
    assert_eq!(r.code, 3);
    assert_eq!(
        r.err,
        "upgrade: the pins are updated, but the render exited 3. Fix what it reports and run render again."
    );
    assert_eq!(s.get("mise.toml"), mise_toml("0.2.0"));
}

#[test]
fn dry_run_verifies_and_lists_the_change_and_writes_and_runs_nothing_else() {
    let s = setup(default_checksums());
    let r = s.run(&["upgrade", "0.2.0", "--root", &s.root(), "--dry-run"], &[]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(
        r.out.contains("would change  pantheon.pkl: 0.1.0 -> 0.2.0"),
        "{}",
        r.out
    );
    assert!(
        r.out
            .ends_with("dry run: the release was verified, nothing was changed"),
        "{}",
        r.out
    );
    assert_eq!(s.get("mise.toml"), mise_toml("0.1.0"));
    assert!(s.calls().iter().all(|c| c.starts_with("gh")));
}

#[test]
fn no_render_stops_after_mise() {
    let s = setup(default_checksums());
    let r = s.run(
        &["upgrade", "0.2.0", "--root", &s.root(), "--no-render"],
        &[],
    );
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(
        r.out
            .ends_with("upgraded 0.1.0 -> 0.2.0, run render with the new runner"),
        "{}",
        r.out
    );
    assert!(s.calls().iter().all(|c| !c.contains("render")));
}

#[test]
fn a_release_that_cannot_be_fetched_fails_closed_with_exit_1() {
    let s = setup(None);
    let r = s.run(&["upgrade", "0.2.0", "--root", &s.root()], &[]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("could not fetch"), "{}", r.err);
    assert_eq!(s.get("mise.toml"), mise_toml("0.1.0"));
}

#[test]
fn a_missing_version_an_unknown_option_and_a_version_that_is_not_one() {
    let s = setup(default_checksums());
    assert_eq!(s.run(&["upgrade", "--root", &s.root()], &[]).code, 2);
    assert_eq!(s.run(&["upgrade", "0.2.0", "--nope"], &[]).code, 2);
    let r = s.run(&["upgrade", "latest", "--root", &s.root()], &[]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err, "\"latest\" is not a version");
}

#[test]
fn a_config_named_with_config_is_used_and_force_allows_going_back() {
    let s = setup(default_checksums());
    std::fs::write(s.repo.path().join("other.pkl"), amends("0.3.0", &h('a'))).unwrap();
    std::fs::write(s.repo.path().join("mise.toml"), mise_toml("0.3.0")).unwrap();
    let refused = s.run(
        &[
            "upgrade",
            "0.2.0",
            "--root",
            &s.root(),
            "--config",
            "other.pkl",
        ],
        &[],
    );
    assert_eq!(refused.code, 1);
    assert_eq!(
        refused.err,
        "0.2.0 is older than 0.3.0, which is pinned now (use --force to go back)"
    );
    let forced = s.run(
        &[
            "upgrade",
            "0.2.0",
            "--root",
            &s.root(),
            "--config",
            "other.pkl",
            "--force",
        ],
        &[],
    );
    assert_eq!(forced.code, 0, "{}", forced.err);
    assert!(s.get("other.pkl").contains("v0.2.0"));
    assert!(
        s.calls().last().unwrap().contains("render other.pkl"),
        "{:?}",
        s.calls()
    );
}
