use super::*;
use std::path::PathBuf;

fn p(target: &str) -> Plan {
    plan(target, "0.2.0", Path::new("/o"), Path::new("/r")).unwrap()
}

#[test]
fn every_release_target_has_a_rust_target_and_the_linux_ones_are_static_musl_builds() {
    let names: Vec<&str> = TRIPLES.iter().map(|(t, _)| *t).collect();
    assert_eq!(names, TARGETS);
    assert_eq!(
        TRIPLES,
        [
            ("linux-x64", "x86_64-unknown-linux-musl"),
            ("linux-arm64", "aarch64-unknown-linux-musl"),
            ("darwin-arm64", "aarch64-apple-darwin"),
        ]
    );
}

#[test]
fn the_plan_builds_the_command_line_crate_release_locked_for_the_target_with_the_version_set() {
    let plan = p("linux-arm64");
    assert_eq!(plan.triple, "aarch64-unknown-linux-musl");
    assert_eq!(
        plan.cargo,
        [
            "build",
            "--release",
            "--locked",
            "-p",
            "scaffold-cli",
            "--target",
            "aarch64-unknown-linux-musl"
        ]
    );
    assert_eq!(
        plan.env[0],
        ("SCAFFOLD_VERSION".to_owned(), "0.2.0".to_owned())
    );
    assert_eq!(
        plan.built,
        PathBuf::from("/r/target/aarch64-unknown-linux-musl/release/pantheon-scaffold")
    );
    assert_eq!(
        plan.asset,
        PathBuf::from("/o/pantheon-scaffold-linux-arm64")
    );
}

#[test]
fn the_asset_names_are_the_ones_the_release_has_always_used_so_mise_picks_the_same_files() {
    for target in TARGETS {
        assert_eq!(
            p(target).asset,
            PathBuf::from(format!("/o/pantheon-scaffold-{target}"))
        );
    }
}

#[test]
fn a_linux_build_compiles_its_c_code_with_a_zig_cc_wrapper_for_its_own_target_and_a_macos_build_is_left_alone()
 {
    let x64 = p("linux-x64");
    assert_eq!(
        x64.env[1],
        (
            "CC_x86_64_unknown_linux_musl".to_owned(),
            "/r/target/zig-cc/cc-x86_64-unknown-linux-musl".to_owned()
        )
    );
    let wrapper = x64.wrapper.unwrap();
    assert_eq!(
        wrapper.path,
        PathBuf::from("/r/target/zig-cc/cc-x86_64-unknown-linux-musl")
    );
    assert!(
        wrapper
            .content
            .contains("exec zig cc -target x86_64-linux-musl ")
    );
    let arm = p("linux-arm64");
    assert_eq!(arm.env[1].0, "CC_aarch64_unknown_linux_musl");
    assert!(
        arm.wrapper
            .unwrap()
            .content
            .contains("exec zig cc -target aarch64-linux-musl ")
    );
    let mac = p("darwin-arm64");
    assert_eq!(mac.env.len(), 1);
    assert_eq!(mac.wrapper, None);
}

#[test]
fn nothing_in_a_plan_needs_musl_gcc_or_the_ubuntu_archive() {
    for target in TARGETS {
        assert!(!format!("{:?}", p(target)).contains("musl-gcc"));
    }
}

#[test]
fn the_wrapper_hands_zig_the_other_arguments_untouched_with_spaces_kept_and_drops_the_target_the_cc_crate_adds()
 {
    let dir = tempfile::tempdir().unwrap();
    let plan = plan("linux-x64", "0.2.0", Path::new("/o"), dir.path()).unwrap();
    let wrapper = plan.wrapper.unwrap();
    std::fs::create_dir_all(wrapper.path.parent().unwrap()).unwrap();
    std::fs::write(&wrapper.path, &wrapper.content).unwrap();
    crate::testutil::settle(&wrapper.path);
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    // A stand-in for zig that prints each argument on its own line.
    let zig = bin.join("zig");
    std::fs::write(
        &zig,
        "#!/bin/sh\nfor a in \"$@\"; do printf \"%s\\n\" \"$a\"; done\n",
    )
    .unwrap();
    crate::testutil::settle(&zig);
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let output = Command::new(&wrapper.path)
        .args([
            "-O3",
            "--target=x86_64-unknown-linux-musl",
            "-I",
            "a dir/with space",
            "-c",
            "x.c",
        ])
        .env("PATH", path)
        .output()
        .unwrap();
    let lines: Vec<String> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        lines,
        [
            "cc",
            "-target",
            "x86_64-linux-musl",
            "-O3",
            "-I",
            "a dir/with space",
            "-c",
            "x.c"
        ]
    );
}

#[test]
fn only_the_musl_builds_must_be_static() {
    assert!(p("linux-x64").must_be_static);
    assert!(p("linux-arm64").must_be_static);
    assert!(!p("darwin-arm64").must_be_static);
}

#[test]
fn a_leading_v_is_dropped_from_the_version_and_a_pre_release_is_a_version() {
    let plan = plan("linux-x64", "v0.2.0-rc.1", Path::new("/o"), Path::new("/r")).unwrap();
    assert_eq!(plan.env[0].1, "0.2.0-rc.1");
}

#[test]
fn an_unknown_target_or_a_version_that_is_not_one_is_refused() {
    let o = Path::new("/o");
    let r = Path::new("/r");
    assert_eq!(
        plan("windows-x64", "0.2.0", o, r).unwrap_err(),
        "unknown target windows-x64, expected one of linux-x64, linux-arm64, darwin-arm64"
    );
    assert_eq!(
        plan("linux-x64", "latest", o, r).unwrap_err(),
        "\"latest\" is not a version"
    );
}

#[test]
fn ldd_output_says_whether_a_binary_is_static() {
    assert!(is_static("\tnot a dynamic executable\n"));
    assert!(is_static("\tstatically linked\n"));
    assert!(!is_static(
        "\tlinux-vdso.so.1 (0x00007ffd)\n\tlibc.so.6 => /lib/libc.so.6\n"
    ));
    assert!(!is_static(""));
}

#[test]
fn the_host_decides_which_target_can_be_built_here() {
    assert_eq!(host_target("linux", "x86_64"), Some("linux-x64"));
    assert_eq!(host_target("linux", "aarch64"), Some("linux-arm64"));
    assert_eq!(host_target("macos", "aarch64"), Some("darwin-arm64"));
    assert_eq!(host_target("macos", "x86_64"), None);
    assert_eq!(host_target("windows", "x86_64"), None);
    assert_eq!(host_target("linux", "riscv64"), None);
}

#[test]
fn the_arguments_are_target_version_and_out_and_the_target_is_required() {
    let cwd = Path::new("/work");
    let a = parse_args(
        &[
            "--target",
            "linux-x64",
            "--version",
            "1.2.3",
            "--out",
            "out",
        ],
        cwd,
    )
    .unwrap();
    assert_eq!(
        a,
        Args {
            target: "linux-x64".to_owned(),
            version: "1.2.3".to_owned(),
            out: PathBuf::from("/work/out"),
            print_plan: false,
        }
    );
    let defaults = parse_args(&["--target", "linux-x64"], cwd).unwrap();
    assert_eq!(
        (defaults.version.as_str(), defaults.out),
        ("0.0.0", PathBuf::from("/work/dist"))
    );
    assert!(
        parse_args(&["--target", "x", "--print-plan"], cwd)
            .unwrap()
            .print_plan
    );
    assert_eq!(parse_args(&[], cwd).unwrap_err(), "--target needs a value");
    assert_eq!(
        parse_args(&["--target"], cwd).unwrap_err(),
        "--target needs a value"
    );
    assert_eq!(
        parse_args(&["--target", "linux-x64", "--version"], cwd).unwrap_err(),
        "--version needs a value"
    );
    assert_eq!(
        parse_args(&["--target", "linux-x64", "--out"], cwd).unwrap_err(),
        "--out needs a value"
    );
}

#[test]
fn the_plan_is_printed_as_json_in_the_shape_javascript_gives_it() {
    let mac = stringify_indent_1(&p("darwin-arm64").to_json());
    assert_eq!(
        mac,
        "{\n \"triple\": \"aarch64-apple-darwin\",\n \"cargo\": [\n  \"build\",\n  \"--release\",\n  \"--locked\",\n  \"-p\",\n  \"scaffold-cli\",\n  \"--target\",\n  \"aarch64-apple-darwin\"\n ],\n \"env\": {\n  \"SCAFFOLD_VERSION\": \"0.2.0\"\n },\n \"built\": \"/r/target/aarch64-apple-darwin/release/pantheon-scaffold\",\n \"asset\": \"/o/pantheon-scaffold-darwin-arm64\",\n \"mustBeStatic\": false,\n \"wrapper\": null\n}"
    );
    let linux: Value =
        serde_json::from_str(&stringify_indent_1(&p("linux-x64").to_json())).unwrap();
    assert_eq!(linux["mustBeStatic"], true);
    assert_eq!(
        linux["env"]["CC_x86_64_unknown_linux_musl"],
        "/r/target/zig-cc/cc-x86_64-unknown-linux-musl"
    );
    assert_eq!(
        linux["wrapper"]["path"],
        "/r/target/zig-cc/cc-x86_64-unknown-linux-musl"
    );
}

fn run_command(args: &[&str]) -> (i32, String, String) {
    let root = tempfile::tempdir().unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = command(args, root.path(), &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn print_plan_prints_the_plan_and_runs_nothing_whatever_the_host() {
    for target in TARGETS {
        let (code, out, err) =
            run_command(&["--target", target, "--version", "1.0.0", "--print-plan"]);
        assert_eq!((code, err.as_str()), (0, ""), "{target}");
        assert!(out.starts_with("{\n \"triple\": "), "{out}");
        assert!(out.ends_with("}\n"), "{out}");
    }
}

#[test]
fn a_target_this_machine_cannot_build_is_refused_and_so_are_bad_arguments() {
    let here = host_target(std::env::consts::OS, std::env::consts::ARCH);
    let other = TARGETS.iter().find(|t| Some(**t) != here).unwrap();
    let (code, out, err) = run_command(&["--target", other]);
    assert_eq!(
        (code, out.as_str(), err.as_str()),
        (
            1,
            "",
            format!(
                "this machine builds {}, not {other}: build each target on a machine of its own\n",
                here.unwrap_or("no release target")
            )
            .as_str()
        )
    );
    assert_eq!(run_command(&[]).2, "--target needs a value\n");
    assert_eq!(
        run_command(&["--target", "nope"]).2,
        "unknown target nope, expected one of linux-x64, linux-arm64, darwin-arm64\n"
    );
    assert_eq!(
        run_command(&["--target", "linux-x64", "--version", "x"]).2,
        "\"x\" is not a version\n"
    );
}
