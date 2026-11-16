//! `scaffold-release build-rust` run as the workflows run it, against stand-ins for `rustup`, `cargo`, `zig` and `ldd` that record what they
//! were asked and leave a script where cargo would leave the binary. Only the target of this machine can be built, so that is the one
//! tested; the parts that are specific to the other targets are covered by the unit tests of the plan.

mod common;

use common::settle;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// A file that was written and is then run must not be open in a process forked by another test thread. The tool runs scripts that
/// were written a moment before, so the tests of this file take turns.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

struct Setup {
    root: tempfile::TempDir,
    bin: PathBuf,
    log: PathBuf,
}

const TARGET_AND_TRIPLE: (&str, &str) = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    ("linux-x64", "x86_64-unknown-linux-musl")
} else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
    ("linux-arm64", "aarch64-unknown-linux-musl")
} else {
    ("darwin-arm64", "aarch64-apple-darwin")
};

const LINUX: bool = cfg!(target_os = "linux");

fn script(path: &Path, body: &str) {
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    settle(path);
}

fn setup(with_zig: bool) -> Setup {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("fakebin");
    std::fs::create_dir(&bin).unwrap();
    let log = root.path().join("calls.log");
    script(
        &bin.join("rustup"),
        "echo \"rustup $*\" >> \"$LOG\"\n[ -z \"$FAKE_RUSTUP_EXIT\" ] || exit \"$FAKE_RUSTUP_EXIT\"",
    );
    script(
        &bin.join("cargo"),
        "echo \"cargo $*\" >> \"$LOG\"\necho \"SCAFFOLD_VERSION=$SCAFFOLD_VERSION\" >> \"$LOG\"\nenv | grep '^CC_' | sort | sed 's/^/env /' >> \"$LOG\"\n[ -z \"$FAKE_CARGO_EXIT\" ] || exit \"$FAKE_CARGO_EXIT\"\nfor a in \"$@\"; do t=$a; done\nd=target/$t/release\nmkdir -p \"$d\"\nprintf '#!/bin/sh\\necho \"%s\"\\n' \"${FAKE_REPORTS:-$SCAFFOLD_VERSION}\" > \"$d/pantheon-scaffold\"\nchmod 755 \"$d/pantheon-scaffold\"",
    );
    script(
        &bin.join("ldd"),
        "printf '%s\\n' \"${FAKE_LDD:-	statically linked}\"",
    );
    if with_zig {
        script(&bin.join("zig"), "echo 0.14.0");
    }
    Setup { root, bin, log }
}

struct Outcome {
    code: i32,
    out: String,
    err: String,
}

fn build(s: &Setup, extra: &[&str], env: &[(&str, &str)]) -> Outcome {
    let _turn = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // The system folders stay on the path for `env`, `grep` and `sh`; the stand-ins come first.
    let path = format!("{}:/usr/bin:/bin", s.bin.display());
    let mut command = Command::new(env!("CARGO_BIN_EXE_scaffold-release"));
    command
        .current_dir(s.root.path())
        .env("PATH", path)
        .env("LOG", &s.log)
        .args([
            "build-rust",
            "--target",
            TARGET_AND_TRIPLE.0,
            "--version",
            "1.2.3",
        ])
        .args(extra);
    for (k, v) in env {
        command.env(k, v);
    }
    let output = command.output().unwrap();
    Outcome {
        code: output.status.code().unwrap_or(-1),
        out: String::from_utf8(output.stdout).unwrap(),
        err: String::from_utf8(output.stderr).unwrap(),
    }
}

fn calls(s: &Setup) -> String {
    std::fs::read_to_string(&s.log).unwrap_or_default()
}

#[test]
fn a_build_runs_rustup_and_cargo_for_the_target_and_puts_the_binary_where_the_release_expects_it() {
    let s = setup(true);
    let result = build(&s, &["--out", "dist"], &[]);
    let (target, triple) = TARGET_AND_TRIPLE;
    let asset = s
        .root
        .path()
        .join("dist")
        .join(format!("pantheon-scaffold-{target}"));
    assert_eq!(result.err, "");
    assert_eq!(result.code, 0);
    assert_eq!(
        result.out,
        format!("built {} ({triple}, version 1.2.3)\n", asset.display())
    );
    let log = calls(&s);
    assert!(
        log.contains(&format!("rustup target add {triple}\n")),
        "{log}"
    );
    assert!(
        log.contains(&format!(
            "cargo build --release --locked -p scaffold-cli --target {triple}\n"
        )),
        "{log}"
    );
    assert!(log.contains("SCAFFOLD_VERSION=1.2.3\n"), "{log}");
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&asset).unwrap().permissions().mode() & 0o777,
        0o755
    );
    // The wrapper is made, and cargo is told to use it, on the targets that need one.
    let wrapper = s
        .root
        .path()
        .join("target/zig-cc")
        .join(format!("cc-{triple}"));
    assert_eq!(wrapper.is_file(), LINUX);
    let var = format!("env CC_{}={}", triple.replace('-', "_"), wrapper.display());
    assert_eq!(log.contains(&var), LINUX, "{log}");
}

#[test]
fn the_out_folder_is_made_and_dist_is_the_default() {
    let s = setup(true);
    let result = build(&s, &[], &[]);
    assert_eq!(result.code, 0, "{}", result.err);
    let asset = s
        .root
        .path()
        .join("dist")
        .join(format!("pantheon-scaffold-{}", TARGET_AND_TRIPLE.0));
    assert!(asset.is_file());
    let nested = build(&s, &["--out", "deep/er/dist"], &[]);
    assert_eq!(nested.code, 0, "{}", nested.err);
    assert!(s.root.path().join("deep/er/dist").is_dir());
}

#[test]
fn a_binary_that_reports_another_version_is_an_error() {
    let s = setup(true);
    let result = build(&s, &[], &[("FAKE_REPORTS", "9.9.9")]);
    assert_eq!(
        (result.code, result.out.as_str(), result.err.as_str()),
        (
            1,
            "",
            "the binary reports version \"9.9.9\", expected 1.2.3\n"
        )
    );
}

#[test]
fn a_failing_cargo_or_rustup_is_an_error_that_names_the_command_and_its_exit_code() {
    let s = setup(true);
    let triple = TARGET_AND_TRIPLE.1;
    let cargo = build(&s, &[], &[("FAKE_CARGO_EXIT", "3")]);
    assert_eq!(cargo.code, 1);
    assert_eq!(
        cargo.err,
        format!(
            "cargo build --release --locked -p scaffold-cli --target {triple} failed (exit 3)\n"
        )
    );
    assert!(!s.root.path().join("dist").exists());
    let rustup = build(&s, &[], &[("FAKE_RUSTUP_EXIT", "7")]);
    assert_eq!(rustup.code, 1);
    assert_eq!(
        rustup.err,
        format!("rustup target add {triple} failed (exit 7)\n")
    );
}

#[test]
fn a_linux_build_without_zig_says_how_to_get_it() {
    if !LINUX {
        return;
    }
    let s = setup(false);
    let result = build(&s, &[], &[]);
    assert_eq!(result.code, 1);
    assert_eq!(
        result.err,
        "zig is not installed: run with MISE_ENV=rust-build so mise installs it (mise.rust-build.toml)\n"
    );
    assert_eq!(calls(&s), "");
}

#[test]
fn a_linux_binary_that_is_not_static_is_an_error_that_shows_what_ldd_said() {
    if !LINUX {
        return;
    }
    let s = setup(true);
    let dynamic = "\tlinux-vdso.so.1 (0x00007ffd)\n\tlibc.so.6 => /lib/libc.so.6";
    let result = build(&s, &[], &[("FAKE_LDD", dynamic)]);
    let asset = s
        .root
        .path()
        .join("dist")
        .join(format!("pantheon-scaffold-{}", TARGET_AND_TRIPLE.0));
    assert_eq!(result.code, 1);
    assert_eq!(
        result.err,
        format!(
            "{} is not a static binary:\n{}\n",
            asset.display(),
            dynamic.trim()
        )
    );
    let said_static = build(&s, &[], &[("FAKE_LDD", "\tnot a dynamic executable")]);
    assert_eq!(said_static.code, 0, "{}", said_static.err);
}

#[test]
fn the_macos_build_needs_no_zig_and_is_not_checked_for_being_static() {
    if LINUX {
        return;
    }
    let s = setup(false);
    let result = build(&s, &[], &[("FAKE_LDD", "\tlibSystem.B.dylib")]);
    assert_eq!(result.code, 0, "{}", result.err);
    assert!(!s.root.path().join("target/zig-cc").exists());
}

#[test]
fn another_target_than_this_machines_is_refused_before_anything_is_run() {
    let s = setup(true);
    let other = if TARGET_AND_TRIPLE.0 == "linux-x64" {
        "darwin-arm64"
    } else {
        "linux-x64"
    };
    let _turn = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let output = Command::new(env!("CARGO_BIN_EXE_scaffold-release"))
        .current_dir(s.root.path())
        .env("PATH", format!("{}:/usr/bin:/bin", s.bin.display()))
        .env("LOG", &s.log)
        .args(["build-rust", "--target", other])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).ends_with(&format!(
        ", not {other}: build each target on a machine of its own\n"
    )));
    assert_eq!(calls(&s), "");
}
