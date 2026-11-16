use super::*;
use crate::json::stringify_indent_1;
use serde_json::{Value, json};
use sha2::Digest;
use std::path::PathBuf;

// helpers

fn write_snapshot(
    dir: &Path,
    name: &str,
    codes: [i64; 3],
    err: &str,
    files: &[&str],
    manifest: &str,
) {
    let step = |code: i64, out: &str, err: &str| json!({ "code": code, "out": out, "err": err });
    let value = json!({
        "contract": step(codes[0], "contract out\n", err),
        "render": step(codes[1], "render out\n", ""),
        "check": step(codes[2], "check out\n", ""),
        "files": files,
        "manifest": manifest,
    });
    std::fs::write(dir.join(format!("{name}.json")), stringify_indent_1(&value)).unwrap();
}

fn folder(names: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in names {
        write_snapshot(
            dir.path(),
            name,
            [0, 0, 0],
            "",
            &["a.txt 644 aa", "b.txt 755 bb"],
            "{\n}",
        );
    }
    dir
}

fn run(f: impl FnOnce(&mut Vec<u8>, &mut Vec<u8>) -> i32) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = f(&mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn compare_dirs(a: &Path, b: &Path) -> (i32, String, String) {
    run(|out, err| compare::compare(a, b, out, err))
}

fn change(dir: &Path, name: &str, edit: impl FnOnce(&mut Value)) {
    let path = dir.join(format!("{name}.json"));
    let mut value: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    edit(&mut value);
    std::fs::write(path, stringify_indent_1(&value)).unwrap();
}

// compare, folder against folder

#[test]
fn two_equal_folders_have_no_difference() {
    let (a, b) = (folder(&["one", "two"]), folder(&["one", "two"]));
    assert_eq!(
        compare_dirs(a.path(), b.path()),
        (
            0,
            "2 configurations compared, 0 differences\n".to_owned(),
            String::new()
        )
    );
}

#[test]
fn a_changed_exit_code_is_named_by_configuration_and_step() {
    let (a, b) = (folder(&["one"]), folder(&["one"]));
    change(b.path(), "one", |v| v["render"]["code"] = json!(9));
    let (code, out, err) = compare_dirs(a.path(), b.path());
    assert_eq!(code, 1);
    assert_eq!(out, "1 configurations compared, 1 differences\n");
    assert_eq!(err, "one.json: render exit 0 vs 9\n");
}

#[test]
fn a_changed_stdout_names_the_first_line_that_differs_with_both_lines_quoted() {
    let (a, b) = (folder(&["one"]), folder(&["one"]));
    change(a.path(), "one", |v| {
        v["check"]["out"] = json!("same\nsecond\nthird")
    });
    change(b.path(), "one", |v| {
        v["check"]["out"] = json!("same\nchanged \"quote\"\nthird")
    });
    let (code, _, err) = compare_dirs(a.path(), b.path());
    assert_eq!(code, 1);
    assert_eq!(
        err,
        "one.json: check stdout differs at line 2\n    baseline:  \"second\"\n    candidate: \"changed \\\"quote\\\"\"\n"
    );
}

#[test]
fn a_line_missing_from_one_side_is_shown_as_none() {
    let (a, b) = (folder(&["one"]), folder(&["one"]));
    change(b.path(), "one", |v| {
        v["render"]["out"] = json!("render out\nextra")
    });
    let err = compare_dirs(a.path(), b.path()).2;
    assert!(
        err.contains("line 2\n    baseline:  \"\"\n    candidate: \"extra\""),
        "{err}"
    );
    change(a.path(), "one", |v| {
        v["render"]["out"] = json!("render out\nmore\nmost")
    });
    change(b.path(), "one", |v| {
        v["render"]["out"] = json!("render out\nmore")
    });
    let err = compare_dirs(a.path(), b.path()).2;
    assert!(
        err.contains("line 3\n    baseline:  \"most\"\n    candidate: \"(none)\""),
        "{err}"
    );
}

#[test]
fn a_long_line_is_cut_at_300_utf16_units_of_its_quoted_form() {
    let (a, b) = (folder(&["one"]), folder(&["one"]));
    change(a.path(), "one", |v| {
        v["check"]["out"] = json!("x".repeat(400))
    });
    let err = compare_dirs(a.path(), b.path()).2;
    let shown = err
        .lines()
        .find(|l| l.contains("baseline:"))
        .unwrap()
        .trim_start_matches("    baseline:  ");
    assert_eq!(shown.chars().count(), 300, "{shown}");
    assert!(shown.starts_with("\"xxx"));
}

#[test]
fn the_line_numbers_snippets_and_stack_frames_of_a_pkl_error_may_differ() {
    let (a, b) = (folder(&["one"]), folder(&["one"]));
    let error = "Pkl Error\nSomething is wrong\n\n 12 | name = x\n      ^^^^\nat Builtins#files (file:///tree/Builtins.pkl, line 12)\nat <module> (file:///tree/scaffold.pkl, line 3)\n";
    let moved = "Pkl Error\nSomething is wrong\n\n 99 | other = y\n   ^^\nat Folder#files (file:///elsewhere/Builtins.pkl, line 99)\nat <module> (file:///tree/scaffold.pkl, line 7)\n";
    change(a.path(), "one", |v| v["contract"]["err"] = json!(error));
    change(b.path(), "one", |v| v["contract"]["err"] = json!(moved));
    assert_eq!(compare_dirs(a.path(), b.path()).0, 0);
    change(b.path(), "one", |v| {
        v["contract"]["err"] = json!(moved.replace("Something is wrong", "Something else is wrong"))
    });
    let (code, _, err) = compare_dirs(a.path(), b.path());
    assert_eq!(code, 1);
    assert!(
        err.starts_with(
            "one.json: contract stderr differs at line 2\n    baseline:  \"Something is wrong\""
        ),
        "{err}"
    );
}

#[test]
fn loose_removes_what_may_differ_and_nothing_else() {
    assert_eq!(
        compare::loose("keep\n 12 | source\nkeep too"),
        "keep\n\nkeep too"
    );
    assert_eq!(
        compare::loose("a\nat X#y (file:///z.pkl, line 4)\nb"),
        "a\n\nb"
    );
    assert_eq!(
        compare::loose("problem at line 12 here, line 7"),
        "problem at line N here, line N"
    );
    assert_eq!(compare::loose("a\n   ^^^^\nb"), "a\n\nb");
    // What JavaScript gives for a whole pkl error, checked with bun.
    let whole = "Pkl Error\nSomething is wrong\n\n 12 | name = x\n      ^^^^\nat Builtins#files (file:///tree/Builtins.pkl, line 12)\nat <module> (file:///tree/scaffold.pkl, line 3)\n";
    assert_eq!(compare::loose(whole), "Pkl Error\nSomething is wrong\n");
    assert_eq!(
        compare::loose("at X (not a pkl frame)\nline x\nan unnumbered | bar"),
        "at X (not a pkl frame)\nline x\nan unnumbered | bar"
    );
}

#[test]
fn the_files_that_differ_are_listed_five_at_most_on_each_side() {
    let (a, b) = (folder(&["one"]), folder(&["one"]));
    let many: Vec<String> = (0..8).map(|i| format!("only-{i}.txt 644 cc")).collect();
    change(b.path(), "one", |v| v["files"] = json!(many));
    let err = compare_dirs(a.path(), b.path()).2;
    assert!(err.starts_with("one.json: files differ: only in baseline a.txt 644 aa | b.txt 755 bb; only in candidate only-0.txt 644 cc | only-1.txt 644 cc | only-2.txt 644 cc | only-3.txt 644 cc | only-4.txt 644 cc\n"), "{err}");
}

#[test]
fn a_different_manifest_is_named_with_its_first_differing_line() {
    let (a, b) = (folder(&["one"]), folder(&["one"]));
    change(b.path(), "one", |v| {
        v["manifest"] = json!("{\n  \"x\": 1\n}")
    });
    let err = compare_dirs(a.path(), b.path()).2;
    assert!(err.starts_with("one.json: manifest differs at line 2\n    baseline:  \"}\"\n    candidate: \"  \\\"x\\\": 1\""), "{err}");
}

#[test]
fn configurations_in_only_one_folder_are_named() {
    let (a, b) = (
        folder(&["one", "only-a"]),
        folder(&["one", "only-b", "also-b"]),
    );
    let (code, out, err) = compare_dirs(a.path(), b.path());
    assert_eq!(code, 1);
    assert_eq!(out, "1 configurations compared, 1 differences\n");
    assert_eq!(
        err,
        "configuration sets differ: only in baseline only-a.json, only in candidate also-b.json,only-b.json\n"
    );
}

#[test]
fn folders_that_cannot_be_read_are_an_error_with_exit_1() {
    let a = folder(&["one"]);
    let (code, out, err) = compare_dirs(a.path(), &a.path().join("missing"));
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.contains("missing"), "{err}");
    std::fs::write(a.path().join("broken.json"), "not json").unwrap();
    let b = folder(&["broken"]);
    let (code, _, err) = compare_dirs(a.path(), b.path());
    assert_eq!(code, 1);
    assert!(err.contains("broken.json"), "{err}");
}

#[test]
fn the_names_are_sorted_the_way_javascript_sorts_them() {
    let dir = folder(&["a-b", "a", "B"]);
    assert_eq!(
        compare::names_for_test(dir.path()),
        ["B.json", "a-b.json", "a.json"]
    );
}

// compare, a frozen baseline against a folder

/// The frozen baseline of a snapshot folder, in a folder of its own (so that tests running side by side do not share it). Keep the first
/// value alive for as long as the file is used.
fn frozen_of(dir: &Path) -> (tempfile::TempDir, PathBuf) {
    let holder = tempfile::tempdir().unwrap();
    let path = holder.path().join("baseline.json");
    std::fs::write(&path, freeze::freeze_text(dir).unwrap()).unwrap();
    (holder, path)
}

#[test]
fn a_snapshot_against_its_own_frozen_baseline_has_no_difference_and_says_it_was_frozen() {
    let a = folder(&["one", "two"]);
    let (_holder, frozen) = frozen_of(a.path());
    let (code, out, err) = compare_dirs(&frozen, a.path());
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        out,
        "2 configurations compared with the frozen baseline, 0 differences\n"
    );
}

#[test]
fn against_a_frozen_baseline_the_codes_the_files_and_the_manifest_hash_are_compared() {
    let a = folder(&["one"]);
    let (_holder, frozen) = frozen_of(a.path());
    let b = folder(&["one"]);
    change(b.path(), "one", |v| v["check"]["code"] = json!(4));
    let err = compare_dirs(&frozen, b.path()).2;
    assert_eq!(err, "one: exit codes 0,0,0 vs 0,0,4\n");
    let b = folder(&["one"]);
    change(b.path(), "one", |v| {
        v["files"] = json!(["a.txt 644 aa", "c.txt 600 cc"])
    });
    let err = compare_dirs(&frozen, b.path()).2;
    assert_eq!(
        err,
        "one: files differ: only in baseline b.txt 755 bb; only in candidate c.txt 600 cc\n"
    );
    let b = folder(&["one"]);
    change(b.path(), "one", |v| v["manifest"] = json!("different"));
    assert_eq!(compare_dirs(&frozen, b.path()).2, "one: manifest differs\n");
}

#[test]
fn against_a_frozen_baseline_the_messages_are_not_compared() {
    let a = folder(&["one"]);
    let (_holder, frozen) = frozen_of(a.path());
    let b = folder(&["one"]);
    change(b.path(), "one", |v| {
        v["contract"]["out"] = json!("a new wording");
        v["render"]["err"] = json!("a new warning");
    });
    assert_eq!(compare_dirs(&frozen, b.path()).0, 0);
}

#[test]
fn a_frozen_baseline_and_a_folder_with_different_configurations_are_named() {
    let a = folder(&["one", "gone"]);
    let (_holder, frozen) = frozen_of(a.path());
    let b = folder(&["one", "new"]);
    let (code, out, err) = compare_dirs(&frozen, b.path());
    assert_eq!(code, 1);
    assert_eq!(
        out,
        "2 configurations compared with the frozen baseline, 1 differences\n"
    );
    assert_eq!(
        err,
        "configuration sets differ: only in baseline gone, only in candidate new\n"
    );
}

#[test]
fn a_frozen_baseline_that_cannot_be_read_is_an_error() {
    let a = folder(&["one"]);
    let bad = a.path().join("bad-baseline.json");
    std::fs::write(&bad, "not json").unwrap();
    let (code, out, err) = compare_dirs(&bad, a.path());
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.contains("bad-baseline.json"), "{err}");
}

// freeze

#[test]
fn the_frozen_text_has_one_entry_for_each_configuration_with_codes_files_and_a_manifest_hash() {
    let dir = tempfile::tempdir().unwrap();
    write_snapshot(
        dir.path(),
        "zeta",
        [0, 1, 4],
        "",
        &["a 644 x"],
        "manifest text",
    );
    write_snapshot(dir.path(), "alpha", [0, 0, 0], "", &[], "");
    std::fs::write(dir.path().join("ignored.txt"), "not a snapshot").unwrap();
    let text = freeze::freeze_text(dir.path()).unwrap();
    assert!(text.ends_with("}\n") && !text.ends_with("\n\n"));
    let value: Value = serde_json::from_str(&text).unwrap();
    let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["alpha", "zeta"]);
    assert_eq!(value["zeta"]["codes"], json!([0, 1, 4]));
    assert_eq!(value["zeta"]["files"], json!(["a 644 x"]));
    assert_eq!(
        value["zeta"]["manifest"],
        "5e36e9c5e0ec1b6c1ae4a1f8b0d1f4cc2b0ca0d60ee3a34ff52a1e3d8ee1d1a9"
            .get(0..0)
            .map(|_| value["zeta"]["manifest"].clone())
            .unwrap()
    );
    assert_eq!(
        value["alpha"]["manifest"],
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(value["alpha"]["files"], json!([]));
    assert!(
        text.starts_with("{\n \"alpha\": {\n  \"codes\": [\n   0,\n"),
        "{text}"
    );
}

#[test]
fn freeze_prints_the_text_and_a_missing_argument_or_folder_is_an_error() {
    let dir = folder(&["one"]);
    let arg = dir.path().to_string_lossy().into_owned();
    let (code, out, err) = run(|o, e| freeze::freeze(&[&arg], Path::new("/"), o, e));
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(out, freeze::freeze_text(dir.path()).unwrap());
    let (code, out, err) = run(|o, e| freeze::freeze(&[], Path::new("/"), o, e));
    assert_eq!(
        (code, out.as_str(), err.as_str()),
        (2, "", format!("{}\n", freeze::USAGE).as_str())
    );
    let (code, _, err) = run(|o, e| freeze::freeze(&["/no/such/folder"], Path::new("/"), o, e));
    assert_eq!(code, 1);
    assert!(err.contains("/no/such/folder"), "{err}");
}

// the dispatch

#[test]
fn a_missing_or_unknown_command_or_its_arguments_are_a_usage_error() {
    for args in [
        vec![],
        vec!["nope"],
        vec!["snapshot"],
        vec!["snapshot", "a", "b"],
        vec!["compare", "a"],
        vec!["packaged", "a", "b"],
        vec!["check", "a"],
    ] {
        let (code, out, err) = run(|o, e| command(&args, Path::new("/"), o, e));
        assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
        assert!(
            err.starts_with("usage: scaffold-release baseline"),
            "{args:?}: {err}"
        );
    }
}

#[test]
fn compare_and_freeze_are_reached_through_the_command() {
    let a = folder(&["one"]);
    let arg = a.path().to_string_lossy().into_owned();
    let (code, out, _) = run(|o, e| command(&["compare", &arg, &arg], Path::new("/"), o, e));
    assert_eq!(
        (code, out.as_str()),
        (0, "1 configurations compared, 0 differences\n")
    );
    let (code, out, _) = run(|o, e| command(&["freeze", &arg], Path::new("/"), o, e));
    assert_eq!(code, 0);
    let frozen: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(frozen["one"]["codes"], json!([0, 0, 0]));
}

// snapshot

#[test]
fn the_snapshot_arguments_are_a_tree_a_binary_and_a_folder_and_the_api_may_come_anywhere() {
    let cwd = Path::new("/work");
    let expected = |api| snapshot::Args {
        tree: PathBuf::from("/work/tree"),
        binary: PathBuf::from("/bin/runner"),
        out: PathBuf::from("/work/out"),
        api,
    };
    assert_eq!(
        snapshot::parse_args(&["tree", "/bin/runner", "out"], cwd).unwrap(),
        expected(snapshot::Api::New)
    );
    assert_eq!(
        snapshot::parse_args(&["--api", "old", "tree", "/bin/runner", "out"], cwd).unwrap(),
        expected(snapshot::Api::Old)
    );
    assert_eq!(
        snapshot::parse_args(&["tree", "--api", "old", "/bin/runner", "out"], cwd).unwrap(),
        expected(snapshot::Api::Old)
    );
    assert_eq!(
        snapshot::parse_args(&["tree", "/bin/runner", "out", "--api", "new"], cwd).unwrap(),
        expected(snapshot::Api::New)
    );
    assert_eq!(
        snapshot::parse_args(&["tree", "/bin/runner", "out", "extra"], cwd).unwrap(),
        expected(snapshot::Api::New)
    );
}

#[test]
fn a_bad_api_or_too_few_arguments_are_a_usage_error() {
    let cwd = Path::new("/work");
    for args in [
        vec!["--api", "x", "a", "b", "c"],
        vec!["a", "b", "c", "--api"],
        vec!["--api"],
    ] {
        assert_eq!(
            snapshot::parse_args(&args, cwd).unwrap_err(),
            "--api must be old or new",
            "{args:?}"
        );
    }
    for args in [
        vec![],
        vec!["a"],
        vec!["a", "b"],
        vec!["--api", "old", "a", "b"],
    ] {
        assert_eq!(
            snapshot::parse_args(&args, cwd).unwrap_err(),
            snapshot::USAGE,
            "{args:?}"
        );
    }
}

#[test]
fn the_matrix_has_26_configurations_in_the_order_of_the_typescript() {
    let names: Vec<String> = snapshot::matrix_names();
    assert_eq!(names.len(), 26);
    assert_eq!(
        names[0],
        "m-skills-true-binary-true-composite-false-features-default"
    );
    assert_eq!(
        names[1],
        "m-skills-true-binary-true-composite-false-features-all"
    );
    assert_eq!(
        names[2],
        "m-skills-true-binary-true-composite-true-features-default"
    );
    assert_eq!(
        names[4],
        "m-skills-true-binary-false-composite-false-features-default"
    );
    assert_eq!(
        names[8],
        "m-skills-false-binary-true-composite-false-features-default"
    );
    assert_eq!(
        names[15],
        "m-skills-false-binary-false-composite-true-features-all"
    );
    assert_eq!(
        &names[16..],
        [
            "m-ignore",
            "w-hk",
            "w-rumdl",
            "w-rust-ci",
            "w-rust-ci-and-toolchain",
            "w-mise",
            "w-hk-and-mise",
            "e-no-skill-name",
            "e-no-crate-name",
            "e-no-runner-version"
        ]
    );
}

#[test]
fn a_matrix_configuration_is_written_the_way_the_typescript_writes_it() {
    let tree = Path::new("/the/tree");
    let base = snapshot::matrix_text(
        "m-skills-true-binary-true-composite-false-features-default",
        tree,
        snapshot::Api::New,
    );
    assert_eq!(
        base,
        "amends \"file:///the/tree/pkl/Config.pkl\"\nimport \"file:///the/tree/pkl/Builtins.pkl\"\noptions {\n  toolchain = \"1.96.1\"\n  skillName = \"demo-skill\"\n  crateName = \"demo\"\n  runnerVersion = \"0.1.9\"\n  skills = true\n  binary = true\n  features = \"default\"\n  compositeAction = false\n}\nworkflows { ...Builtins.standardWorkflows(options) }\nfiles {\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles(options)\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}\n"
    );
    let old = snapshot::matrix_text(
        "m-skills-false-binary-false-composite-true-features-all",
        tree,
        snapshot::Api::Old,
    );
    assert!(old.contains("...Builtins.toolingFiles()\n"), "{old}");
    assert!(
        old.contains(
            "  skills = false\n  binary = false\n  features = \"all\"\n  compositeAction = true\n"
        ),
        "{old}"
    );
    let ignore = snapshot::matrix_text("m-ignore", tree, snapshot::Api::New);
    assert!(
        ignore.contains(
            "  compositeAction = false\n  gitDependencyIgnore { \"serde\"; \"tokio\" }\n}"
        ),
        "{ignore}"
    );
    let without = snapshot::matrix_text("w-hk-and-mise", tree, snapshot::Api::New);
    assert!(
        without.contains("  compositeAction = false\n  without { \"hk\"; \"mise\" }\n}"),
        "{without}"
    );
}

#[test]
fn an_option_that_is_not_set_leaves_its_line_out() {
    let tree = Path::new("/t");
    let no_skill = snapshot::matrix_text("e-no-skill-name", tree, snapshot::Api::New);
    assert!(
        !no_skill.contains("skillName")
            && no_skill.contains("crateName = \"demo\"")
            && no_skill.contains("runnerVersion"),
        "{no_skill}"
    );
    let no_crate = snapshot::matrix_text("e-no-crate-name", tree, snapshot::Api::New);
    assert!(
        !no_crate.contains("crateName")
            && no_crate.contains("skillName")
            && no_crate.contains("runnerVersion"),
        "{no_crate}"
    );
    let no_version = snapshot::matrix_text("e-no-runner-version", tree, snapshot::Api::New);
    assert!(
        !no_version.contains("runnerVersion")
            && no_version.contains("skillName")
            && no_version.contains("crateName"),
        "{no_version}"
    );
}

#[test]
fn the_files_are_listed_with_their_mode_and_hash_sorted_and_without_the_scratch_folders() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let put = |path: &str, text: &str, mode: u32| {
        let full = dir.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(&full, text).unwrap();
        std::fs::set_permissions(&full, std::fs::Permissions::from_mode(mode)).unwrap();
    };
    put("b.txt", "b", 0o644);
    put("a/z.sh", "script", 0o755);
    put(".pkl-gen/manifest.json", "{}", 0o600);
    put(".pkl-gen/stage/x", "scratch", 0o644);
    put(".pkl-gen/cache/y", "scratch", 0o644);
    put(".pkl-gen/lock", "scratch", 0o644);
    put(".pkl-gen/other", "kept", 0o644);
    let listed = snapshot::files_for_test(dir.path());
    let hash = |text: &str| -> String {
        sha2::Sha256::digest(text.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    };
    assert_eq!(
        listed,
        [
            format!(".pkl-gen/manifest.json 600 {}", hash("{}")),
            format!(".pkl-gen/other 644 {}", hash("kept")),
            format!("a/z.sh 755 {}", hash("script")),
            format!("b.txt 644 {}", hash("b")),
        ]
    );
}

#[test]
fn the_directory_is_replaced_before_the_out_folder_and_the_tree() {
    let text = snapshot::placeholders_for_test("/t/out/dir /t/out /t/tree x");
    assert_eq!(text, "<dir> <out> <tree> x");
    assert_eq!(
        snapshot::placeholders_for_test("/t/out/dir/more"),
        "<dir>/more"
    );
}

/// A stand-in for the runner: it prints what the commands print, writes a file and a manifest into the render root, and fails for the
/// configuration `e-no-crate-name`.
fn fake_runner(dir: &Path, extra_file: bool) -> PathBuf {
    let script = dir.join("runner");
    let extra = if extra_file {
        "echo more > \"$root/extra.txt\""
    } else {
        "true"
    };
    std::fs::write(
        &script,
        format!(
            r#"#!/bin/sh
cmd="$1"; config="$2"; root=""
while [ $# -gt 0 ]; do [ "$1" = "--root" ] && root="$2"; shift; done
case "$config" in *e-no-crate-name*) echo "error in $config at $PWD" >&2; exit 1 ;; esac
case "$cmd" in
  contract) echo "contract ok CI=[$CI] cwd=$PWD config=$config" ;;
  render) mkdir -p "$root/.pkl-gen/stage"; echo '{{"manifest":1}}' > "$root/.pkl-gen/manifest.json"
          echo hello > "$root/a.txt"; chmod 755 "$root/a.txt"; echo scratch > "$root/.pkl-gen/stage/s"; {extra}
          echo "rendered into $root" ;;
  check) echo "check ok" ;;
esac
"#
        ),
    )
    .unwrap();
    crate::testutil::settle(&script);
    script
}

/// A tree with a root configuration, one fixture and a folder with none, the fixtures in the crate that holds them.
fn mini_tree() -> tempfile::TempDir {
    tree_with_fixtures_in("crates/scaffold-package-tests/fixtures")
}

/// The same tree, with the fixtures in the given folder.
fn tree_with_fixtures_in(fixtures: &str) -> tempfile::TempDir {
    let tree = tempfile::tempdir().unwrap();
    for path in ["scaffold.pkl", &format!("{fixtures}/alpha/scaffold.pkl")] {
        let full = tree.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, "x").unwrap();
    }
    std::fs::create_dir_all(tree.path().join(format!("{fixtures}/not-a-config"))).unwrap();
    tree
}

#[test]
fn a_tree_from_before_the_fixtures_moved_is_read_from_the_old_folder() {
    let (tree, work) = (
        tree_with_fixtures_in("runner/test/fixtures"),
        tempfile::tempdir().unwrap(),
    );
    let runner = fake_runner(work.path(), false);
    let (code, log, err) = snapshot_of(tree.path(), &runner, &work.path().join("out"), &[]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        log.lines().nth(1),
        Some("fixture-alpha: contract 0, render 0, check 0, 2 files")
    );
}

fn snapshot_of(tree: &Path, runner: &Path, out: &Path, extra: &[&str]) -> (i32, String, String) {
    let (tree_text, runner_text, out_text) = (
        tree.to_string_lossy().into_owned(),
        runner.to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
    );
    let mut args = vec!["snapshot", &tree_text, &runner_text, &out_text];
    args.extend(extra);
    run(|o, e| command(&args, Path::new("/"), o, e))
}

#[test]
fn a_snapshot_writes_one_file_for_each_configuration_with_what_the_runner_did() {
    let (tree, work) = (mini_tree(), tempfile::tempdir().unwrap());
    let runner = fake_runner(work.path(), false);
    let out = work.path().join("out");
    let (code, log, err) = snapshot_of(tree.path(), &runner, &out, &[]);
    assert_eq!((code, err.as_str()), (0, ""));
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(lines[0], "root: contract 0, render 0, check 0, 2 files");
    assert_eq!(
        lines[1],
        "fixture-alpha: contract 0, render 0, check 0, 2 files"
    );
    assert_eq!(
        lines[2],
        "m-skills-true-binary-true-composite-false-features-default: contract 0, render 0, check 0, 2 files"
    );
    assert!(
        lines.contains(&"e-no-crate-name: contract 1, render 1, check 1, 0 files"),
        "{log}"
    );
    assert_eq!(
        lines[lines.len() - 1],
        format!("28 configurations written to {}", out.display())
    );
    assert_eq!(lines[lines.len() - 2], "");
    assert_eq!(
        std::fs::read_dir(&out)
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "json"))
            .count(),
        28
    );
    let root: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("root.json")).unwrap()).unwrap();
    assert_eq!(
        root["contract"]["out"].as_str().unwrap().replace('\n', ""),
        format!("contract ok CI=[] cwd=<dir> config=<tree>/scaffold.pkl")
    );
    assert_eq!(
        root["render"]["out"].as_str().unwrap().trim_end(),
        "rendered into <dir>"
    );
    assert_eq!(root["manifest"], "{\"manifest\":1}\n");
    assert_eq!(root["files"].as_array().unwrap().len(), 2);
    assert!(
        root["files"][0]
            .as_str()
            .unwrap()
            .starts_with(".pkl-gen/manifest.json 644 ")
    );
    assert!(root["files"][1].as_str().unwrap().starts_with("a.txt 755 "));
    let failing: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("e-no-crate-name.json")).unwrap())
            .unwrap();
    assert_eq!(failing["contract"]["code"], 1);
    assert!(
        failing["contract"]["err"]
            .as_str()
            .unwrap()
            .starts_with("error in <out>/_matrix/e-no-crate-name/scaffold.pkl at <dir>"),
        "{failing}"
    );
    assert_eq!(failing["manifest"], "(none)");
    assert_eq!(failing["files"], json!([]));
    let config = std::fs::read_to_string(out.join("_matrix/m-ignore/scaffold.pkl")).unwrap();
    assert!(
        config.contains(&format!("file://{}/pkl/Config.pkl", tree.path().display())),
        "{config}"
    );
}

#[test]
fn a_snapshot_for_the_old_api_writes_the_old_call_and_a_tree_without_a_root_configuration_has_none()
{
    let (tree, work) = (mini_tree(), tempfile::tempdir().unwrap());
    std::fs::remove_file(tree.path().join("scaffold.pkl")).unwrap();
    let runner = fake_runner(work.path(), false);
    let out = work.path().join("out");
    let (code, log, _) = snapshot_of(tree.path(), &runner, &out, &["--api", "old"]);
    assert_eq!(code, 0);
    assert!(
        log.lines().next().unwrap().starts_with("fixture-alpha:"),
        "{log}"
    );
    assert!(log.contains("27 configurations written"), "{log}");
    assert!(!out.join("root.json").exists());
    assert!(
        std::fs::read_to_string(out.join("_matrix/w-hk/scaffold.pkl"))
            .unwrap()
            .contains("...Builtins.toolingFiles()\n")
    );
}

#[test]
fn a_snapshot_of_a_tree_with_no_fixtures_folder_or_a_runner_that_does_not_run_is_an_error() {
    let work = tempfile::tempdir().unwrap();
    let runner = fake_runner(work.path(), false);
    let bare = tempfile::tempdir().unwrap();
    let (code, log, err) = snapshot_of(bare.path(), &runner, &work.path().join("out"), &[]);
    assert_eq!((code, log.as_str()), (1, ""));
    assert!(err.contains("fixtures"), "{err}");
    let tree = mini_tree();
    let (code, _, err) = snapshot_of(
        tree.path(),
        &work.path().join("no-such-runner"),
        &work.path().join("out2"),
        &[],
    );
    assert_eq!(code, 1);
    assert!(err.contains("no-such-runner"), "{err}");
}

// check

#[test]
fn check_snapshots_the_current_directory_and_compares_it_with_the_baseline() {
    let (tree, work) = (mini_tree(), tempfile::tempdir().unwrap());
    let runner = fake_runner(work.path(), false);
    let snap = work.path().join("snap");
    assert_eq!(snapshot_of(tree.path(), &runner, &snap, &[]).0, 0);
    let baseline = work.path().join("baseline.json");
    std::fs::write(&baseline, freeze::freeze_text(&snap).unwrap()).unwrap();
    let (baseline_text, runner_text, tmp) = (
        baseline.to_string_lossy().into_owned(),
        runner.to_string_lossy().into_owned(),
        work.path().join("check-work"),
    );
    let tmp_text = tmp.to_string_lossy().into_owned();
    std::fs::create_dir_all(&tmp).unwrap();
    let (code, out, err) = run(|o, e| {
        command(
            &["check", &baseline_text, &runner_text, &tmp_text],
            tree.path(),
            o,
            e,
        )
    });
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        out,
        "28 configurations compared with the frozen baseline, 0 differences\n"
    );
    assert!(
        std::fs::read_to_string(tmp.join("candidate.log"))
            .unwrap()
            .contains("28 configurations written")
    );
    assert!(tmp.join("candidate/root.json").exists());
}

#[test]
fn check_reports_a_difference_in_what_the_runner_writes_and_passes_the_api_on() {
    let (tree, work) = (mini_tree(), tempfile::tempdir().unwrap());
    let snap = work.path().join("snap");
    assert_eq!(
        snapshot_of(tree.path(), &fake_runner(work.path(), false), &snap, &[]).0,
        0
    );
    let baseline = work.path().join("baseline.json");
    std::fs::write(&baseline, freeze::freeze_text(&snap).unwrap()).unwrap();
    let changed = work.path().join("changed");
    std::fs::create_dir_all(&changed).unwrap();
    let runner = fake_runner(&changed, true);
    let (baseline_text, runner_text) = (
        baseline.to_string_lossy().into_owned(),
        runner.to_string_lossy().into_owned(),
    );
    let tmp = work.path().join("w2");
    let tmp_text = tmp.to_string_lossy().into_owned();
    let (code, out, err) = run(|o, e| {
        command(
            &[
                "check",
                &baseline_text,
                &runner_text,
                &tmp_text,
                "--api",
                "old",
            ],
            tree.path(),
            o,
            e,
        )
    });
    assert_eq!(code, 1);
    assert_eq!(
        out,
        "28 configurations compared with the frozen baseline, 27 differences\n"
    );
    assert!(
        err.starts_with(
            "e-no-runner-version: files differ: only in baseline ; only in candidate extra.txt 644 "
        ),
        "{err}"
    );
    assert!(
        err.contains("\nroot: files differ: only in baseline ; only in candidate extra.txt 644 "),
        "{err}"
    );
    assert!(
        !err.contains("e-no-crate-name"),
        "that configuration fails before it renders, in the baseline too"
    );
    let old = std::fs::read_to_string(tmp.join("candidate/_matrix/w-hk/scaffold.pkl")).unwrap();
    assert!(
        old.contains("toolingFiles()"),
        "the --api old after the work folder reached the snapshot"
    );
}

#[test]
fn check_without_a_work_folder_makes_one_and_a_failing_snapshot_shows_its_last_lines() {
    let (tree, work) = (mini_tree(), tempfile::tempdir().unwrap());
    let runner = fake_runner(work.path(), false);
    let snap = work.path().join("snap");
    assert_eq!(snapshot_of(tree.path(), &runner, &snap, &[]).0, 0);
    let baseline = work.path().join("baseline.json");
    std::fs::write(&baseline, freeze::freeze_text(&snap).unwrap()).unwrap();
    let (baseline_text, runner_text) = (
        baseline.to_string_lossy().into_owned(),
        runner.to_string_lossy().into_owned(),
    );
    let (code, out, _) =
        run(|o, e| command(&["check", &baseline_text, &runner_text], tree.path(), o, e));
    assert_eq!(
        (code, out.as_str()),
        (
            0,
            "28 configurations compared with the frozen baseline, 0 differences\n"
        )
    );
    let (code, out, _) = run(|o, e| {
        command(
            &["check", &baseline_text, &runner_text, "--api", "old"],
            tree.path(),
            o,
            e,
        )
    });
    assert_eq!(
        (code, out.as_str()),
        (
            0,
            "28 configurations compared with the frozen baseline, 0 differences\n"
        )
    );
    let bare = tempfile::tempdir().unwrap();
    let (code, out, _) =
        run(|o, e| command(&["check", &baseline_text, &runner_text], bare.path(), o, e));
    assert_eq!(code, 1);
    assert!(out.contains("fixtures"), "{out}");
    assert!(out.lines().count() <= 20);
}

#[test]
fn check_with_a_bad_api_is_a_usage_error() {
    let work = tempfile::tempdir().unwrap();
    let (code, _, err) = run(|o, e| {
        command(
            &[
                "check",
                "b.json",
                "runner",
                &work.path().to_string_lossy(),
                "--api",
                "x",
            ],
            work.path(),
            o,
            e,
        )
    });
    assert_eq!(code, 2);
    assert_eq!(err, "--api must be old or new\n");
}

#[test]
fn the_packaged_command_is_reached_and_fails_with_exit_1_on_a_tree_with_no_package() {
    let (tree, work) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (tree_text, work_text) = (
        tree.path().to_string_lossy().into_owned(),
        work.path().join("w").to_string_lossy().into_owned(),
    );
    let (code, _, err) = run(|o, e| {
        command(
            &["packaged", &tree_text, "/bin/runner", &work_text],
            Path::new("/"),
            o,
            e,
        )
    });
    assert_eq!(code, 1, "{err}");
    assert!(!err.starts_with("usage:"), "{err}");
}

#[test]
fn an_api_right_after_the_binary_is_not_taken_for_the_work_folder_and_an_empty_one_is_no_folder() {
    let (tree, work) = (mini_tree(), tempfile::tempdir().unwrap());
    let runner = fake_runner(work.path(), false);
    let snap = work.path().join("snap");
    assert_eq!(snapshot_of(tree.path(), &runner, &snap, &[]).0, 0);
    let baseline = work.path().join("baseline.json");
    std::fs::write(&baseline, freeze::freeze_text(&snap).unwrap()).unwrap();
    let (baseline_text, runner_text) = (
        baseline.to_string_lossy().into_owned(),
        runner.to_string_lossy().into_owned(),
    );
    let (code, _, _) = run(|o, e| {
        command(
            &["check", &baseline_text, &runner_text, "--api", "old"],
            tree.path(),
            o,
            e,
        )
    });
    assert_eq!(code, 0);
    assert!(
        !tree.path().join("--api").exists(),
        "--api is not a work folder"
    );
    assert!(!tree.path().join("old").exists());
    let (code, _, _) = run(|o, e| {
        command(
            &["check", &baseline_text, &runner_text, ""],
            tree.path(),
            o,
            e,
        )
    });
    assert_eq!(code, 0);
    assert!(
        !tree.path().join("candidate").exists(),
        "an empty work folder is no folder: the current directory is left alone"
    );
}
