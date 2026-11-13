//! `migrate` through the real binary, against a release: a local server in place of GitHub, and fake `gh` and `pkl` programs on
//! the PATH. The fake pkl prints a contract written by the test, with the address of the package the config amends, so these
//! tests are about what `migrate` reports and writes for a given contract, not about what the package renders; that is the
//! scenarios of features/migrate.feature, which use the real package.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

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

fn script(path: &std::path::Path, body: &str) {
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// The contract the fake pkl prints. `files` is `(path, class, content)`; an external file has no content.
fn contract_json(files: &[(&str, &str, &str)]) -> String {
    let files: Vec<serde_json::Value> = files
        .iter()
        .map(|(path, class, content)| {
            let mut file = serde_json::json!({"path": path, "class": class, "mode": "0644"});
            if *class != "external" {
                file["content"] = serde_json::json!(content);
            }
            file
        })
        .collect();
    serde_json::json!({
        "schemaVersion": 1, "files": files, "pins": [],
        "escapeHatch": {"uses": 0, "constructs": 0}, "packageUri": "@URI@#/Config.pkl"
    })
    .to_string()
}

/// A server that knows release 0.2.0 (its checksums and an empty known-bad list), and fake `gh` and `pkl`.
fn setup(files: &[(&str, &str, &str)]) -> Setup {
    setup_with_presets(files, files)
}

/// The fake pkl prints `files` for a config that applies no Presets, and `with_presets` for one that lists some, as the real
/// package would once the Presets keep the files that drifted.
fn setup_with_presets(files: &[(&str, &str, &str)], with_presets: &[(&str, &str, &str)]) -> Setup {
    let bin = tempfile::tempdir().unwrap();
    script(&bin.path().join("gh"), "#!/bin/sh\nexit 0\n");
    std::fs::write(bin.path().join("contract.json"), contract_json(files)).unwrap();
    std::fs::write(
        bin.path().join("contract-presets.json"),
        contract_json(with_presets),
    )
    .unwrap();
    script(
        &bin.path().join("pkl"),
        &format!(
            "#!/bin/sh\nfor last; do :; done\nuri=$(grep -o 'package://[^\"#]*' \"$last\" | head -1)\ncontract=\"{dir}/contract.json\"\nif grep -q '^presets {{' \"$last\"; then contract=\"{dir}/contract-presets.json\"; fi\nsed \"s|@URI@|$uri|\" \"$contract\"\n",
            dir = bin.path().display()
        ),
    );
    let routes: HashMap<String, String> = HashMap::from([
        ("/known-bad.json".to_owned(), "{\"versions\":[]}".to_owned()),
        (
            "/v0.2.0/checksums.txt".to_owned(),
            format!("{}  pantheon-scaffold@0.2.0\n", "c".repeat(64)),
        ),
    ]);
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
            let text = String::from_utf8_lossy(&request);
            let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let (status, body) = match routes.get(&path) {
                Some(body) => (200, body.clone()),
                None => (404, "gone".to_owned()),
            };
            let answer = format!(
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(answer.as_bytes());
        }
    });
    Setup {
        bin,
        repo: tempfile::tempdir().unwrap(),
        base,
    }
}

impl Setup {
    fn put(&self, file: &str, text: &str) {
        let path = self.repo.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn run(&self, args: &[&str]) -> Run {
        let path = format!(
            "{}:{}",
            self.bin.path().display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let root = self.repo.path().display().to_string();
        let output = Command::new(env!("CARGO_BIN_EXE_pantheon-scaffold"))
            .arg("migrate")
            .args(["--root", &root, "--package-version", "0.2.0"])
            .args(args)
            .current_dir(self.repo.path())
            .env("PATH", path)
            .env("PANTHEON_SCAFFOLD_RELEASE_BASE", &self.base)
            .env(
                "PANTHEON_SCAFFOLD_KNOWN_BAD_URL",
                format!("{}/known-bad.json", self.base),
            )
            .env_remove("GITHUB_TOKEN")
            .env_remove("GH_TOKEN")
            .env_remove("CI")
            .output()
            .unwrap();
        Run {
            code: output.status.code().unwrap_or(-1),
            out: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            err: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }
    }

    fn get(&self, file: &str) -> String {
        std::fs::read_to_string(self.repo.path().join(file)).unwrap()
    }

    fn exists(&self, file: &str) -> bool {
        self.repo.path().join(file).exists()
    }
}

const WORKFLOW: &str = "name: CI\n'on': push\njobs:\n  test:\n    timeout-minutes: 10\n";

/// A repository with one identical file, one YAML file equal to the render in other text, and one file of its own.
fn clean() -> Setup {
    let s = setup(&[
        ("a.toml", "owned", "x = 1\n"),
        (".github/workflows/ci.yml", "owned", WORKFLOW),
        ("new.toml", "owned", "n = 1\n"),
    ]);
    s.put("a.toml", "x = 1\n");
    s.put(
        ".github/workflows/ci.yml",
        "# mine\nname: \"CI\"\n\"on\": push\njobs: {test: {timeout-minutes: 10}}\n",
    );
    s.put("README.md", "# mine\n");
    s
}

#[test]
fn a_dry_run_lists_every_file_with_its_class_and_writes_nothing() {
    let s = clean();
    let r = s.run(&["--toolchain", "1.96.1"]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    for line in [
        "identical  a.toml",
        "identical  .github/workflows/ci.yml",
        "equal as YAML, different text: --write replaces it with the render's text",
        "new        new.toml",
        "per-repo   README.md",
        "migrate: 2 identical, 0 drifted, 1 new, 1 per-repo. Nothing was written. Run again with --write",
    ] {
        assert!(r.out.contains(line), "{line}\n{}", r.out);
    }
    assert!(!s.exists("pantheon.pkl"));
    assert!(!s.exists("new.toml"));
    assert!(!s.exists(".pkl-gen/manifest.json"));
    assert!(s.get(".github/workflows/ci.yml").starts_with("# mine"));
}

#[test]
fn a_dry_run_with_a_drifted_file_shows_the_difference_and_says_write_is_refused() {
    let s = clean();
    s.put("a.toml", "x = 2\n");
    let r = s.run(&["--toolchain", "1.96.1"]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert!(r.out.contains("drifted    a.toml"), "{}", r.out);
    assert!(r.out.contains("line 1 on disk:  x = 2"), "{}", r.out);
    assert!(
        r.out.contains(
            "migrate: 1 identical, 1 drifted, 1 new, 1 per-repo. Nothing was written. --write is refused while a file has drifted; --as-presets keeps each one as a Preset and --accept-builtins replaces each with the Builtins' file."
        ),
        "{}",
        r.out
    );
    assert!(!s.exists("pantheon.pkl") && !s.exists("new.toml"));
}

#[test]
fn write_is_refused_while_a_file_has_drifted_and_nothing_is_written() {
    let s = clean();
    s.put("a.toml", "x = 2\n");
    let r = s.run(&["--toolchain", "1.96.1", "--write"]);
    assert_eq!(r.code, 3, "{} {}", r.out, r.err);
    assert!(
        r.err.contains("1 file(s) drifted") && r.err.contains("  a.toml"),
        "{}",
        r.err
    );
    assert_eq!(s.get("a.toml"), "x = 2\n");
    assert!(
        !s.exists("pantheon.pkl") && !s.exists("new.toml") && !s.exists(".pkl-gen/manifest.json")
    );
}

#[test]
fn write_creates_the_new_files_and_adopts_the_yaml_text_and_leaves_the_repositorys_own_files() {
    let s = clean();
    let r = s.run(&["--toolchain", "1.96.1", "--write"]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert!(
        r.out
            .contains("Wrote pantheon.pkl and 2 file(s). Next: run mise lock, then commit."),
        "{}",
        r.out
    );
    assert_eq!(s.get("new.toml"), "n = 1\n");
    assert_eq!(s.get(".github/workflows/ci.yml"), WORKFLOW);
    assert_eq!(s.get("README.md"), "# mine\n");
    assert!(
        s.get("pantheon.pkl")
            .contains("v0.2.0/pantheon-scaffold@0.2.0")
    );
    assert!(
        s.get(".pkl-gen/manifest.json")
            .contains("\"packageVersion\": \"0.2.0\"")
    );
}

#[test]
fn a_render_that_would_be_refused_for_another_reason_stops_before_anything_is_written() {
    // The file is identical to the render, but a manifest from an earlier render records another hash: the render refuses a file
    // that was edited by hand since then, which the classification alone does not see.
    let s = setup(&[
        ("a.toml", "owned", "x = 1\n"),
        ("new.toml", "owned", "n = 1\n"),
    ]);
    s.put("a.toml", "x = 1\n");
    s.put(
        ".pkl-gen/manifest.json",
        &format!(
            "{{\n  \"manifestVersion\": 1,\n  \"files\": {{\n    \"a.toml\": {{\n      \"class\": \"owned\",\n      \"hash\": \"{}\"\n    }}\n  }}\n}}\n",
            "0".repeat(64)
        ),
    );
    for args in [
        &["--toolchain", "1.96.1"][..],
        &["--toolchain", "1.96.1", "--write"][..],
    ] {
        let r = s.run(args);
        assert_eq!(r.code, 3, "{} {}", r.out, r.err);
        assert!(r.err.contains("a.toml: was edited by hand"), "{}", r.err);
        assert!(!s.exists("pantheon.pkl") && !s.exists("new.toml"));
    }
}

#[test]
fn an_existing_pantheon_pkl_is_refused_and_left_alone() {
    let s = clean();
    s.put("pantheon.pkl", "mine\n");
    let r = s.run(&["--toolchain", "1.96.1", "--write"]);
    assert_eq!(r.code, 3, "{} {}", r.out, r.err);
    assert!(r.err.contains("pantheon.pkl already exists"), "{}", r.err);
    assert!(r.err.contains("migrate writes it once"), "{}", r.err);
    assert_eq!(s.get("pantheon.pkl"), "mine\n");
}

#[test]
fn a_missing_toolchain_is_a_usage_error_that_names_migrate() {
    let s = clean();
    let r = s.run(&[]);
    assert_eq!(r.code, 2, "{} {}", r.out, r.err);
    assert!(
        r.err
            .contains("migrate needs --toolchain, for example --toolchain 1.96.1"),
        "{}",
        r.err
    );
}

#[test]
fn a_seed_that_exists_and_an_external_file_are_named_and_not_counted() {
    let s = setup(&[
        ("SECURITY.md", "seed", "theirs\n"),
        (".github/workflows/release.yml", "external", ""),
    ]);
    s.put("SECURITY.md", "mine\n");
    let r = s.run(&["--toolchain", "1.96.1"]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert!(r.out.contains("seed-kept  SECURITY.md"), "{}", r.out);
    assert!(
        r.out.contains("external   .github/workflows/release.yml"),
        "{}",
        r.out
    );
    assert!(
        r.out
            .contains("migrate: 0 identical, 0 drifted, 0 new, 0 per-repo."),
        "{}",
        r.out
    );
}

/// A repository with one drifted file `a.toml`, and a contract that, once the Preset keeps the file, renders the file as it is.
fn drifted() -> Setup {
    let s = setup_with_presets(
        &[
            ("a.toml", "owned", "x = 1\n"),
            ("new.toml", "owned", "n = 1\n"),
        ],
        &[
            ("a.toml", "owned", "x = 2\n"),
            ("new.toml", "owned", "n = 1\n"),
        ],
    );
    s.put("a.toml", "x = 2\n");
    s.put("README.md", "# mine\n");
    s
}

#[test]
fn as_presets_in_a_dry_run_lists_the_preset_and_writes_nothing() {
    let s = drifted();
    let r = s.run(&["--toolchain", "1.96.1", "--as-presets"]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert!(
        r.out.contains("preset     presets/a.pkl  keeps a.toml"),
        "{}",
        r.out
    );
    assert!(
        r.out.contains("migrate: 0 identical, 1 drifted, 1 new, 1 per-repo. Nothing was written. Run again with --write to write pantheon.pkl, 1 Preset(s) and the new files."),
        "{}",
        r.out
    );
    assert!(!s.exists("presets/a.pkl") && !s.exists("pantheon.pkl") && !s.exists("new.toml"));
}

#[test]
fn as_presets_with_write_keeps_the_file_in_a_preset_and_records_it() {
    let s = drifted();
    let r = s.run(&["--toolchain", "1.96.1", "--write", "--as-presets"]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert!(
        r.out.contains(
            "Wrote pantheon.pkl, 1 Preset(s) and 1 file(s). Next: run mise lock, then commit."
        ),
        "{}",
        r.out
    );
    assert_eq!(s.get("a.toml"), "x = 2\n");
    assert_eq!(s.get("new.toml"), "n = 1\n");
    let preset = s.get("presets/a.pkl");
    assert!(
        preset.contains("content = #\"\"\"\nx = 2\n\n\"\"\"#"),
        "{preset}"
    );
    let config = s.get("pantheon.pkl");
    assert!(
        config.contains("presets {\n  \"presets/a.pkl\"\n}"),
        "{config}"
    );
    assert!(
        config.contains("files = preset0.apply(standard)"),
        "{config}"
    );
    let manifest = s.get(".pkl-gen/manifest.json");
    assert!(
        manifest.contains("\"presets\": {\n    \"presets/a.pkl\": \""),
        "{manifest}"
    );
    assert!(
        manifest.contains("\"packageVersion\": \"0.2.0\""),
        "{manifest}"
    );
}

#[test]
fn when_the_presets_do_not_reproduce_a_file_nothing_is_written() {
    // The contract with Presets still renders `x = 3`, so the Preset has not kept the file.
    let s = setup_with_presets(
        &[("a.toml", "owned", "x = 1\n")],
        &[("a.toml", "owned", "x = 3\n")],
    );
    s.put("a.toml", "x = 2\n");
    let r = s.run(&["--toolchain", "1.96.1", "--write", "--as-presets"]);
    assert_eq!(r.code, 1, "{} {}", r.out, r.err);
    assert!(
        r.err
            .contains("the Presets do not reproduce 1 file(s), so nothing was written:\n  a.toml"),
        "{}",
        r.err
    );
    assert!(!s.exists("presets/a.pkl") && !s.exists("pantheon.pkl"));
    assert_eq!(s.get("a.toml"), "x = 2\n");
}

#[test]
fn a_drifted_file_that_is_not_text_stops_the_run_and_writes_nothing() {
    let s = setup(&[("a.bin", "owned", "x\n")]);
    std::fs::write(s.repo.path().join("a.bin"), [0xff, 0xfe]).unwrap();
    let r = s.run(&["--toolchain", "1.96.1", "--write", "--as-presets"]);
    assert_eq!(r.code, 1, "{} {}", r.out, r.err);
    assert!(
        r.err.contains("a.bin cannot be kept as a Preset"),
        "{}",
        r.err
    );
    assert!(!s.exists("pantheon.pkl") && !s.exists("presets"));
}

#[test]
fn a_preset_does_not_take_the_name_of_a_file_already_under_presets() {
    let s = drifted();
    s.put("presets/a.pkl", "mine\n");
    let r = s.run(&["--toolchain", "1.96.1", "--write", "--as-presets"]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert_eq!(s.get("presets/a.pkl"), "mine\n");
    assert!(s.exists("presets/a-2.pkl"));
    assert!(s.get("pantheon.pkl").contains("\"presets/a-2.pkl\""));
}

#[test]
fn a_refusal_found_by_the_plan_stops_the_run_before_any_preset_is_written() {
    // The manifest records another hash for new.toml than the file has: the render refuses a file edited by hand since then.
    let s = drifted();
    s.put("new.toml", "n = 1\n");
    s.put(
        ".pkl-gen/manifest.json",
        &format!(
            "{{\n  \"manifestVersion\": 1,\n  \"files\": {{\n    \"new.toml\": {{\n      \"class\": \"owned\",\n      \"hash\": \"{}\"\n    }}\n  }}\n}}\n",
            "0".repeat(64)
        ),
    );
    let r = s.run(&["--toolchain", "1.96.1", "--write", "--as-presets"]);
    assert_eq!(r.code, 3, "{} {}", r.out, r.err);
    assert!(r.err.contains("new.toml: was edited by hand"), "{}", r.err);
    assert!(!s.exists("presets/a.pkl") && !s.exists("pantheon.pkl"));
}

impl Setup {
    /// Makes the repository a git work tree with everything committed.
    fn commit_all(&self) {
        let root = self.repo.path().display().to_string();
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["add", "-A"],
            &["commit", "-q", "-m", "snapshot"],
        ] {
            let mut command = Command::new("git");
            command
                .args([
                    "-C",
                    &root,
                    "-c",
                    "user.name=tests",
                    "-c",
                    "user.email=tests@example.invalid",
                ])
                .args(["-c", "commit.gpgsign=false"])
                .args(args);
            for (key, _) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("GIT_") {
                    command.env_remove(key);
                }
            }
            let out = command.output().expect("git runs");
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}

#[test]
fn accept_builtins_in_a_dry_run_reports_the_git_check_and_what_it_would_replace_and_writes_nothing()
{
    let s = drifted();
    s.commit_all();
    let r = s.run(&["--toolchain", "1.96.1", "--accept-builtins"]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    for line in [
        "drifted    a.toml",
        "git guard: ok, 1 file(s) to replace are tracked and the tree is clean",
        "would replace 1 file(s)",
        "Nothing was written.",
    ] {
        assert!(r.out.contains(line), "{line}\n{}", r.out);
    }
    assert!(
        !r.out.contains("replaced by --accept-builtins"),
        "{}",
        r.out
    );
    assert_eq!(s.get("a.toml"), "x = 2\n");
    assert!(!s.exists("pantheon.pkl") && !s.exists("new.toml"));
}

#[test]
fn accept_builtins_in_a_dry_run_outside_git_says_a_write_would_be_refused() {
    let s = drifted();
    let r = s.run(&["--toolchain", "1.96.1", "--accept-builtins"]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert!(
        r.out.contains("git guard: a write would be refused:\n  not a git repository: files cannot be replaced without git"),
        "{}",
        r.out
    );
    assert!(r.out.contains("would replace 1 file(s)"), "{}", r.out);
    assert!(!r.out.contains("git guard: ok"), "{}", r.out);
    assert_eq!(s.get("a.toml"), "x = 2\n");
}

#[test]
fn accept_builtins_with_write_replaces_the_drifted_file_and_names_it_last() {
    let s = drifted();
    s.commit_all();
    let r = s.run(&["--toolchain", "1.96.1", "--accept-builtins", "--write"]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert_eq!(s.get("a.toml"), "x = 1\n");
    assert!(s.exists("pantheon.pkl") && s.exists("new.toml") && s.exists(".pkl-gen/manifest.json"));
    assert!(
        r.out.ends_with("replaced by --accept-builtins (the git diff of the adoption commit is the full record):\n  a.toml"),
        "{}",
        r.out
    );
    assert!(!r.out.contains("would replace"), "{}", r.out);
    assert_eq!(
        s.get("README.md"),
        "# mine\n",
        "the repository's own files are left alone"
    );
}

#[test]
fn accept_builtins_with_write_is_refused_while_a_tracked_file_has_uncommitted_changes() {
    let s = drifted();
    s.commit_all();
    s.put("README.md", "# edited\n");
    let r = s.run(&["--toolchain", "1.96.1", "--accept-builtins", "--write"]);
    assert_eq!(r.code, 3, "{} {}", r.out, r.err);
    assert!(
        r.err.contains("refused: --accept-builtins needs git to hold what it replaces, nothing was written:\n  the tracked tree is not clean"),
        "{}",
        r.err
    );
    assert_eq!(s.get("a.toml"), "x = 2\n");
    assert!(!s.exists("pantheon.pkl") && !s.exists("new.toml"));
    assert!(
        !r.out.contains("replaced by --accept-builtins"),
        "{}",
        r.out
    );
}

#[test]
fn accept_builtins_with_write_is_refused_outside_git() {
    let s = drifted();
    let r = s.run(&["--toolchain", "1.96.1", "--accept-builtins", "--write"]);
    assert_eq!(r.code, 3, "{} {}", r.out, r.err);
    assert!(r.err.contains("not a git repository"), "{}", r.err);
    assert_eq!(s.get("a.toml"), "x = 2\n");
    assert!(!s.exists("pantheon.pkl"));
}

#[test]
fn accept_builtins_with_nothing_drifted_replaces_nothing() {
    let s = clean();
    let r = s.run(&["--toolchain", "1.96.1", "--accept-builtins"]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert!(
        r.out
            .contains("no file has drifted, so --accept-builtins replaces nothing"),
        "{}",
        r.out
    );
    assert!(!r.out.contains("git guard"), "{}", r.out);
}

#[test]
fn accept_builtins_cannot_be_combined_with_as_presets_and_is_refused_before_anything_is_evaluated()
{
    let s = drifted();
    let r = s.run(&["--toolchain", "1.96.1", "--accept-builtins", "--as-presets"]);
    assert_eq!(r.code, 2, "{} {}", r.out, r.err);
    assert!(
        r.err
            .contains("--accept-builtins cannot be used with --as-presets"),
        "{}",
        r.err
    );
    assert!(r.out.is_empty(), "{}", r.out);
}

#[test]
fn features_all_is_written_to_the_config_by_migrate_and_default_writes_nothing() {
    let s = setup(&[("a.toml", "owned", "x = 1\n")]);
    s.put("a.toml", "x = 1\n");
    let r = s.run(&["--toolchain", "1.96.1", "--features", "all", "--write"]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert!(
        s.get("pantheon.pkl").contains("  features = \"all\"\n}\n"),
        "{}",
        s.get("pantheon.pkl")
    );

    let d = setup(&[("a.toml", "owned", "x = 1\n")]);
    d.put("a.toml", "x = 1\n");
    let r = d.run(&[
        "--toolchain",
        "1.96.1",
        "--features",
        "all",
        "--features",
        "default",
        "--write",
    ]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert!(
        !d.get("pantheon.pkl").contains("features"),
        "{}",
        d.get("pantheon.pkl")
    );
}

#[test]
fn features_all_is_written_to_the_config_when_the_drifted_files_are_kept_as_presets() {
    let s = drifted();
    let r = s.run(&[
        "--toolchain",
        "1.96.1",
        "--features",
        "all",
        "--write",
        "--as-presets",
    ]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    let config = s.get("pantheon.pkl");
    assert!(config.contains("  features = \"all\"\n}\n"), "{config}");
    assert!(config.contains("presets {"), "{config}");
}

#[test]
fn features_with_a_value_other_than_default_or_all_is_a_usage_error_and_writes_nothing() {
    let s = drifted();
    let r = s.run(&["--toolchain", "1.96.1", "--features", "foo,bar", "--write"]);
    assert_eq!(r.code, 2, "{} {}", r.out, r.err);
    assert!(
        r.err.contains("--features must be default or all"),
        "{}",
        r.err
    );
    assert!(r.out.is_empty() && !s.exists("pantheon.pkl"), "{}", r.out);
}
