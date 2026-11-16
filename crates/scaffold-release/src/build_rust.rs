//! `scaffold-release build-rust`: builds the runner for one release target, on a machine of that target, and puts it where the release
//! expects it: one file named `pantheon-scaffold-<target>`. Ported from `runner/scripts/build-rust.ts` (Phase 5 of
//! `docs/ts-tooling-rust-port.md`). The release workflow and the pull request checks both run it, so a pull request builds the release
//! binary exactly as a tag will.
//!
//! The Linux builds are static musl binaries, which run on any Linux distribution whatever its glibc; the macOS build is a normal one.
//! `ring`, which rustls uses, has C code. `zig cc` compiles it against musl, so the build needs neither musl-gcc nor the Ubuntu archive.
//! zig comes from mise (`mise.rust-build.toml`, `MISE_ENV=rust-build`), pinned and checksummed in `mise.rust-build.lock`. After the
//! build the tool checks what it made: the binary reports the version it was built for, and a Linux binary is static.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use scaffold_core::semver::parse_semver;
use serde_json::{Map, Value, json};

use crate::json::{quote, resolve, stringify_indent_1};
use crate::{TARGETS, binary_name};

pub const USAGE: &str = "usage: scaffold-release build-rust --target <linux-x64|linux-arm64|darwin-arm64> [--version <x.y.z>] [--out <dir>] [--print-plan]";

/// The Rust target triple of each release target, in the order of `TARGETS`.
pub const TRIPLES: [(&str, &str); 3] = [
    ("linux-x64", "x86_64-unknown-linux-musl"),
    ("linux-arm64", "aarch64-unknown-linux-musl"),
    ("darwin-arm64", "aarch64-apple-darwin"),
];

/// zig's name for a musl triple.
fn zig_target(triple: &str) -> Option<&'static str> {
    match triple {
        "x86_64-unknown-linux-musl" => Some("x86_64-linux-musl"),
        "aarch64-unknown-linux-musl" => Some("aarch64-linux-musl"),
        _ => None,
    }
}

/// A script around `zig cc`, which is the C compiler of a musl build.
#[derive(Debug, PartialEq, Eq)]
pub struct Wrapper {
    pub path: PathBuf,
    pub content: String,
}

/// What to run for a target. Nothing is run to make one, so it can be checked without a toolchain.
#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    pub triple: &'static str,
    pub cargo: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Where cargo leaves the binary.
    pub built: PathBuf,
    /// Where the release expects it.
    pub asset: PathBuf,
    pub must_be_static: bool,
    /// The C compiler the build runs; none when the target needs none (macOS).
    pub wrapper: Option<Wrapper>,
}

/// `zig cc` takes its own `-target`, so the `--target` that the cc crate adds is dropped and every other argument is passed on as it
/// came.
fn wrapper_script(zig_target: &str) -> String {
    format!(
        "#!/bin/sh\nfor a in \"$@\"; do shift; case \"$a\" in --target=*) ;; *) set -- \"$@\" \"$a\" ;; esac; done\nexec zig cc -target {zig_target} \"$@\"\n"
    )
}

pub fn plan(target: &str, version: &str, out: &Path, root: &Path) -> Result<Plan, String> {
    let Some((_, triple)) = TRIPLES.iter().find(|(t, _)| *t == target) else {
        return Err(format!(
            "unknown target {target}, expected one of {}",
            TARGETS.join(", ")
        ));
    };
    let bare = version.strip_prefix('v').unwrap_or(version);
    if parse_semver(bare).is_none() {
        return Err(format!("{} is not a version", quote(version)));
    }
    let wrapper = zig_target(triple).map(|zig| Wrapper {
        path: root
            .join("target")
            .join("zig-cc")
            .join(format!("cc-{triple}")),
        content: wrapper_script(zig),
    });
    let mut env = vec![("SCAFFOLD_VERSION".to_owned(), bare.to_owned())];
    if let Some(w) = &wrapper {
        env.push((
            format!("CC_{}", triple.replace('-', "_")),
            w.path.to_string_lossy().into_owned(),
        ));
    }
    Ok(Plan {
        triple,
        cargo: [
            "build",
            "--release",
            "--locked",
            "-p",
            "scaffold-cli",
            "--target",
            triple,
        ]
        .map(str::to_owned)
        .to_vec(),
        env,
        built: root
            .join("target")
            .join(triple)
            .join("release")
            .join("pantheon-scaffold"),
        asset: out.join(binary_name(target)),
        must_be_static: wrapper.is_some(),
        wrapper,
    })
}

impl Plan {
    /// The plan as JSON, in the shape `JSON.stringify` gives the TypeScript one, so the two can be compared byte for byte.
    pub fn to_json(&self) -> Value {
        let env: Map<String, Value> = self
            .env
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        json!({
            "triple": self.triple,
            "cargo": self.cargo,
            "env": env,
            "built": self.built.to_string_lossy(),
            "asset": self.asset.to_string_lossy(),
            "mustBeStatic": self.must_be_static,
            "wrapper": self.wrapper.as_ref().map(|w| json!({
                "path": w.path.to_string_lossy(),
                "content": w.content,
            })),
        })
    }
}

/// `ldd` on a static binary says so, in one of two words depending on how it was linked.
pub fn is_static(ldd: &str) -> bool {
    ldd.contains("not a dynamic executable") || ldd.contains("statically linked")
}

/// The target this machine can build, if any. Only native builds are made: nothing here cross-compiles.
pub fn host_target(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Some("linux-x64"),
        ("linux", "aarch64") => Some("linux-arm64"),
        ("macos", "aarch64") => Some("darwin-arm64"),
        _ => None,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    pub target: String,
    pub version: String,
    pub out: PathBuf,
    pub print_plan: bool,
}

/// The arguments, or the message for an error. `--target` is required, `--version` is `0.0.0` and `--out` is `dist` by default.
pub fn parse_args(args: &[&str], cwd: &Path) -> Result<Args, String> {
    let get = |flag: &str, fallback: Option<&str>| -> Result<String, String> {
        let value = match args.iter().position(|a| *a == flag) {
            None => fallback,
            Some(i) => args.get(i + 1).copied(),
        };
        value
            .map(str::to_owned)
            .ok_or_else(|| format!("{flag} needs a value"))
    };
    Ok(Args {
        target: get("--target", None)?,
        version: get("--version", Some("0.0.0"))?,
        out: resolve(&get("--out", Some("dist"))?, cwd),
        print_plan: args.contains(&"--print-plan"),
    })
}

/// Runs a program with its output going where ours goes, and fails on any exit other than 0.
fn run(program: &str, args: &[String], env: &[(String, String)]) -> Result<(), String> {
    let shown = || format!("{program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .envs(env.iter().map(|(k, v)| (k, v)))
        .status()
        .map_err(|e| format!("{}: {e}", shown()))?;
    if status.success() {
        return Ok(());
    }
    Err(format!(
        "{} failed (exit {})",
        shown(),
        status.code().unwrap_or(-1)
    ))
}

/// Writes the C compiler wrapper of a musl build, after checking that zig is there to be run by it.
fn make_wrapper(wrapper: &Wrapper) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let zig = Command::new("zig").arg("version").output();
    if !zig.is_ok_and(|o| o.status.success()) {
        return Err("zig is not installed: run with MISE_ENV=rust-build so mise installs it (mise.rust-build.toml)".to_owned());
    }
    let io = |e: std::io::Error| format!("{}: {e}", wrapper.path.display());
    if let Some(folder) = wrapper.path.parent() {
        std::fs::create_dir_all(folder).map_err(io)?;
    }
    std::fs::write(&wrapper.path, &wrapper.content).map_err(io)?;
    std::fs::set_permissions(&wrapper.path, std::fs::Permissions::from_mode(0o755)).map_err(io)
}

/// Builds the binary for the plan, copies it to its asset path and checks it. Returns the version the binary reports.
fn build(plan: &Plan, version: &str, out: &Path) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(wrapper) = &plan.wrapper {
        make_wrapper(wrapper)?;
    }
    run(
        "rustup",
        &["target".into(), "add".into(), plan.triple.into()],
        &[],
    )?;
    run("cargo", &plan.cargo, &plan.env)?;
    let io = |path: &Path| {
        let path = path.to_owned();
        move |e: std::io::Error| format!("{}: {e}", path.display())
    };
    std::fs::create_dir_all(out).map_err(io(out))?;
    std::fs::copy(&plan.built, &plan.asset).map_err(io(&plan.built))?;
    std::fs::set_permissions(&plan.asset, std::fs::Permissions::from_mode(0o755))
        .map_err(io(&plan.asset))?;

    let expected = version.strip_prefix('v').unwrap_or(version);
    let output = Command::new(&plan.asset)
        .arg("--version")
        .output()
        .map_err(io(&plan.asset))?;
    let reported = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if reported != expected {
        return Err(format!(
            "the binary reports version {}, expected {expected}",
            quote(&reported)
        ));
    }
    // Only a musl build is static, and a musl target is only built on Linux (the host check of `command`), where `ldd` is.
    if plan.must_be_static {
        let ldd = Command::new("ldd").arg(&plan.asset).output();
        let said = ldd.map_or_else(
            |e| e.to_string(),
            |o| {
                format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                )
            },
        );
        if !is_static(&said) {
            return Err(format!(
                "{} is not a static binary:\n{}",
                plan.asset.display(),
                said.trim()
            ));
        }
    }
    Ok(reported)
}

/// `build-rust --target <t> [--version <v>] [--out <dir>] [--print-plan]`, run from the root of the checkout. With `--print-plan` it
/// prints what it would run and exits, which is what the differential script compares with the TypeScript plan.
pub fn command(args: &[&str], root: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let result = parse_args(args, root).and_then(|a| {
        let p = plan(&a.target, &a.version, &a.out, root)?;
        if a.print_plan {
            return Ok(stringify_indent_1(&p.to_json()));
        }
        let here = host_target(std::env::consts::OS, std::env::consts::ARCH);
        if here != Some(a.target.as_str()) {
            return Err(format!(
                "this machine builds {}, not {}: build each target on a machine of its own",
                here.unwrap_or("no release target"),
                a.target
            ));
        }
        let reported = build(&p, &a.version, &a.out)?;
        Ok(format!(
            "built {} ({}, version {reported})",
            p.asset.display(),
            p.triple
        ))
    });
    match result {
        Ok(line) => {
            let _ = writeln!(out, "{line}");
            0
        }
        Err(message) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}

#[cfg(test)]
mod tests;
