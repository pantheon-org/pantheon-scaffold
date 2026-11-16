//! Ported from `runner/test/release-workflow.test.ts`: properties of `.github/workflows/release.yml` that make publishing safe. They are read
//! from the file, so a change that breaks one fails here.

use regex::Regex;
use scaffold_package_tests::pkg::read;
use scaffold_package_tests::workflow::{Workflow, needs, run_of, steps_of, uses_of, writes};
use serde_json::{Value, json};
use std::sync::LazyLock;

static WF: LazyLock<Workflow> = LazyLock::new(|| Workflow::load(".github/workflows/release.yml"));
static CI: LazyLock<Workflow> = LazyLock::new(|| Workflow::load(".github/workflows/ci.yml"));

fn release_runs(job: &str) -> Vec<&'static str> {
    WF.steps(job).iter().map(run_of).collect()
}

fn index_of(steps: &[Value], pred: impl Fn(&Value) -> bool) -> Option<usize> {
    steps.iter().position(pred)
}

#[test]
fn it_runs_on_a_v_tag_push_or_by_hand_never_on_pull_request_target_or_a_branch_push() {
    let mut keys: Vec<&str> = WF.doc["on"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["push", "workflow_dispatch"]);
    assert_eq!(WF.doc["on"]["push"], json!({ "tags": ["v[0-9]*"] }));
    assert!(!WF.text.contains("pull_request_target"));
}

#[test]
fn the_manual_run_is_a_dry_run_unless_told_otherwise() {
    let dry = &WF.doc["on"]["workflow_dispatch"]["inputs"]["dry_run"];
    assert_eq!(dry["type"], "boolean");
    assert_eq!(dry["default"], true);
}

#[test]
fn nothing_has_permissions_by_default_and_every_job_asks_for_its_own() {
    assert_eq!(WF.doc["permissions"], json!({}));
    for (name, job) in WF.jobs() {
        assert!(!job["permissions"].is_null(), "job {name}");
    }
}

#[test]
fn only_the_release_job_can_write_contents_attest_and_use_the_id_token() {
    for (name, job) in WF.jobs() {
        let mut got: Vec<String> = writes(job).into_iter().map(|(k, _)| k).collect();
        got.sort();
        let want: Vec<&str> = if name == "release" {
            vec!["attestations", "contents", "id-token"]
        } else {
            vec![]
        };
        assert_eq!(got, want, "job {name}");
    }
}

#[test]
fn every_action_is_pinned_by_a_40_character_sha() {
    let pinned = Regex::new(r"@[0-9a-f]{40}$").unwrap();
    let uses: Vec<&str> = WF
        .all_steps()
        .iter()
        .map(|(_, s)| uses_of(s))
        .filter(|u| !u.is_empty() && !u.starts_with("./"))
        .collect();
    assert!(uses.len() > 5);
    for u in uses {
        assert!(pinned.is_match(u), "{u}");
    }
}

#[test]
fn it_uses_no_secrets_and_persists_no_checkout_credentials() {
    assert!(!Regex::new(r"secrets\.").unwrap().is_match(&WF.text));
    for (job, step) in WF.all_steps() {
        if uses_of(step).starts_with("actions/checkout@") {
            assert_eq!(step["with"]["persist-credentials"], false, "{job}");
        }
    }
}

#[test]
fn no_run_script_interpolates_text_a_pusher_controls_tags_refs_and_inputs_go_through_env() {
    let interpolates =
        Regex::new(r"\$\{\{\s*(github\.(ref|ref_name|head_ref|event)|inputs\.)").unwrap();
    for (job, step) in WF.all_steps() {
        assert!(
            !interpolates.is_match(run_of(step)),
            "{job}: {}",
            step["name"]
        );
    }
}

#[test]
fn only_the_release_job_publishes_through_publish_release_sh_and_never_on_a_dry_run() {
    let publishers: Vec<(&str, &Value)> = WF
        .all_steps()
        .into_iter()
        .filter(|(_, s)| run_of(s).contains("publish-release.sh"))
        .collect();
    assert_eq!(
        publishers.iter().map(|(job, _)| *job).collect::<Vec<_>>(),
        ["release"]
    );
    assert_eq!(publishers[0].1["if"], "github.event_name == 'push'");
    // No other step creates, edits or uploads to a release.
    let forbidden = Regex::new(r"gh release (create|edit|upload)|releases/\$").unwrap();
    for (job, step) in WF.all_steps() {
        if !run_of(step).contains("publish-release.sh") && !run_of(step).is_empty() {
            assert!(!forbidden.is_match(run_of(step)), "{job}: {}", step["name"]);
        }
    }
}

#[test]
fn publish_release_sh_makes_a_pre_release_adopts_only_a_draft_for_the_tag_and_never_re_publishes() {
    let script = read(".github/scripts/publish-release.sh");
    let creates: Vec<&str> = script
        .split('\n')
        .filter(|l| l.contains("gh release create"))
        .collect();
    assert_eq!(creates.len(), 1);
    assert!(creates[0].contains("--prerelease"));
    assert!(creates[0].contains("--verify-tag"));
    // A draft is published only as a pre-release.
    assert!(
        Regex::new(r"-F draft=false -F prerelease=true")
            .unwrap()
            .is_match(&script)
    );
    assert!(script.contains("select(.draft and .tag_name == \\\"$TAG\\\")"));
    // A published release stops it, and more than one draft stops it.
    assert!(
        Regex::new(
            r#"elif gh release view "\$TAG"[^\n]*\n[^\n]*never re-published[^\n]*\n\s*exit 1"#
        )
        .unwrap()
        .is_match(&script)
    );
    assert!(
        Regex::new(r"-gt 1 \]; then[\s\S]*exit 1")
            .unwrap()
            .is_match(&script)
    );
    assert!(
        !Regex::new(r"--clobber|--draft=false")
            .unwrap()
            .is_match(&script)
    );
    assert!(!script.contains("gh release delete"));
}

#[test]
fn the_attestation_is_made_only_on_a_real_release_and_covers_the_assets() {
    let attest: Vec<(&str, &Value)> = WF
        .all_steps()
        .into_iter()
        .filter(|(_, s)| uses_of(s).starts_with("actions/attest-build-provenance@"))
        .collect();
    assert_eq!(attest.len(), 1);
    assert_eq!(attest[0].0, "release");
    assert_eq!(attest[0].1["if"], "github.event_name == 'push'");
    assert!(
        attest[0].1["with"]["subject-path"]
            .as_str()
            .unwrap()
            .contains("dist/")
    );
}

#[test]
fn the_tag_is_checked_against_pkl_project_before_anything_is_built() {
    assert!(
        WF.steps("prepare")
            .iter()
            .any(|s| run_of(s).contains("release-assets check-tag"))
    );
    for (name, after) in [
        ("prepare", "tool"),
        ("package", "prepare"),
        ("build", "prepare"),
        ("smoke", "build"),
        ("release", "smoke"),
    ] {
        assert!(needs(WF.job(name)).contains(&after.to_owned()), "{name}");
    }
    assert!(needs(WF.job("release")).contains(&"package".to_owned()));
}

// Phase 1 of docs/ts-tooling-rust-port.md: the release-assets step is the Rust tool crates/scaffold-release. Building it runs the build scripts
// of every crate it depends on, so it is built in a job that cannot write and has no token, and the jobs that can write (or that hold the
// token to read releases) only download the finished binary and check its checksum before they run it.
#[test]
fn the_release_tool_is_built_in_a_job_that_cannot_write_and_holds_no_token_and_prepare_and_release_only_download_it()
 {
    let tool = WF.job("tool");
    assert_eq!(tool["permissions"], json!({ "contents": "read" }));
    assert!(tool["env"].is_null());
    let secret = Regex::new(r"github\.token|secrets\.").unwrap();
    for step in steps_of(tool) {
        assert!(!secret.is_match(&step.to_string()), "{}", step["name"]);
    }
    assert!(
        steps_of(tool)
            .iter()
            .any(|s| run_of(s).contains("cargo build --release --locked -p scaffold-release"))
    );
    let upload = steps_of(tool)
        .iter()
        .find(|s| uses_of(s).starts_with("actions/upload-artifact@"))
        .unwrap();
    assert_eq!(upload["with"]["name"], "scaffold-release");
    for name in ["prepare", "release"] {
        let job = WF.job(name);
        assert!(needs(job).contains(&"tool".to_owned()), "{name}");
        let steps = steps_of(job);
        let download = index_of(steps, |s| {
            uses_of(s).starts_with("actions/download-artifact@")
                && s["with"]["name"] == "scaffold-release"
        })
        .unwrap_or_else(|| panic!("{name} downloads the tool"));
        let verify = index_of(steps, |s| run_of(s).contains("sha256sum --check --strict"))
            .unwrap_or_else(|| panic!("{name} checks its checksum"));
        assert!(verify > download, "{name}");
        let first_run = index_of(steps, |s| {
            run_of(s).contains("tool/scaffold-release release-assets")
        })
        .unwrap_or_else(|| panic!("{name} runs it"));
        assert!(first_run > verify, "{name}");
    }
}

#[test]
fn no_job_that_can_write_runs_cargo() {
    let cargo = Regex::new(r"\bcargo\b").unwrap();
    for (name, job) in WF.jobs() {
        let can_write = job["permissions"]
            .as_object()
            .is_some_and(|p| p.values().any(|v| v == "write"));
        let runs_cargo = steps_of(job).iter().any(|s| cargo.is_match(run_of(s)));
        assert!(!(can_write && runs_cargo), "job {name}");
    }
}

#[test]
fn prepare_and_release_have_no_job_wide_token_in_their_environment_so_the_tool_runs_without_one() {
    for name in ["prepare", "release"] {
        assert!(WF.job(name)["env"]["GITHUB_TOKEN"].is_null(), "{name}");
    }
}

#[test]
fn a_dry_run_also_checks_the_tag_this_version_would_get_so_check_tag_is_exercised_before_a_real_tag()
 {
    let step = WF
        .steps("prepare")
        .iter()
        .find(|s| run_of(s).contains("release-assets check-tag \"v$VERSION\""))
        .expect("the dry run check");
    assert_eq!(step["if"], "github.ref_type != 'tag'");
    assert_eq!(
        step["env"]["VERSION"],
        "${{ steps.version.outputs.version }}"
    );
}

// Task 2.5: the runner is the Rust one, built natively on each target by the same script ci.yml runs.
fn targets() -> Value {
    json!([
        { "os": "ubuntu-latest", "target": "linux-x64" },
        { "os": "ubuntu-24.04-arm", "target": "linux-arm64" },
        { "os": "macos-latest", "target": "darwin-arm64" },
    ])
}

#[test]
fn the_runner_is_built_on_a_machine_of_each_target_in_a_matrix_and_a_failure_on_one_does_not_hide_the_others()
 {
    let build = WF.job("build");
    assert_eq!(build["strategy"]["matrix"]["include"], targets());
    assert_eq!(build["strategy"]["fail-fast"], false);
    assert_eq!(WF.job("smoke")["strategy"]["matrix"]["include"], targets());
}

#[test]
fn each_build_installs_the_pinned_rust_toolchain_builds_with_the_release_tool_and_uploads_its_own_binary()
 {
    let steps = WF.steps("build");
    let toolchain = steps
        .iter()
        .find(|s| run_of(s).contains("rustup toolchain install"))
        .expect("the toolchain step");
    // Every version in the step, the one installed and the one made the default, is the one rust-toolchain.toml pins.
    let channel = Regex::new(r#"channel\s*=\s*"([^"]+)""#)
        .unwrap()
        .captures(&read("rust-toolchain.toml"))
        .expect("a channel")[1]
        .to_owned();
    assert!(Regex::new(r"^\d+\.\d+\.\d+$").unwrap().is_match(&channel));
    let versions: Vec<&str> = Regex::new(r"\d+\.\d+\.\d+")
        .unwrap()
        .find_iter(run_of(toolchain))
        .map(|m| m.as_str())
        .collect();
    assert_eq!(versions, [channel.as_str(), channel.as_str()]);
    let build = steps
        .iter()
        .find(|s| run_of(s).contains("scaffold-release build-rust"))
        .expect("the build step");
    assert!(run_of(build).contains("--target \"${{ matrix.target }}\""));
    assert!(run_of(build).contains("--version \"$VERSION\""));
    let upload = steps
        .iter()
        .find(|s| uses_of(s).starts_with("actions/upload-artifact@"))
        .expect("the upload step");
    assert_eq!(upload["with"]["name"], "binary-${{ matrix.target }}");
    assert_eq!(
        upload["with"]["path"],
        "dist/pantheon-scaffold-${{ matrix.target }}"
    );
    assert_eq!(upload["with"]["if-no-files-found"], "error");
}

#[test]
fn nothing_cross_compiles_and_the_type_script_build_script_is_gone_from_the_release() {
    assert!(!WF.text.contains("bun build --compile"));
    assert!(!WF.text.contains("runner/scripts/build.ts"));
}

// The static Linux builds compile ring's C code with zig cc. zig comes from mise, pinned and checksummed, in its own environment so that only
// the jobs that build a release binary download it. Nothing reads the Ubuntu archive: when its DNS failed on the runners, every release build
// failed with it.
fn installs_zig_through_mise_and_reads_no_archive(job: &Value) {
    assert_eq!(job["env"]["MISE_ENV"], "rust-build");
    assert!(
        steps_of(job)
            .iter()
            .any(|s| run_of(s) == "mise install --locked")
    );
    let joined = steps_of(job)
        .iter()
        .map(run_of)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !Regex::new(r"apt-get|musl-tools|musl-gcc")
            .unwrap()
            .is_match(&joined)
    );
}

#[test]
fn release_yml_build_installs_zig_through_mises_rust_build_environment_and_reads_no_package_archive()
 {
    installs_zig_through_mise_and_reads_no_archive(WF.job("build"));
}

#[test]
fn ci_yml_rust_binary_installs_zig_through_mises_rust_build_environment_and_reads_no_package_archive()
 {
    installs_zig_through_mise_and_reads_no_archive(CI.job("rust-binary"));
}

#[test]
fn no_other_job_gets_zig_so_no_other_job_depends_on_its_download_host() {
    for (name, job) in WF.jobs().into_iter().chain(CI.jobs()) {
        if name == "build" || name == "rust-binary" {
            continue;
        }
        assert!(job["env"]["MISE_ENV"].is_null(), "{name}");
    }
}

#[test]
fn the_rust_build_environment_pins_zig_to_one_exact_version_and_locks_it_for_the_three_platforms_with_a_checksum_each()
 {
    let toml: toml::Table = read("mise.rust-build.toml")
        .parse()
        .expect("mise.rust-build.toml parses");
    let tools = toml["tools"].as_table().unwrap();
    assert_eq!(tools.keys().collect::<Vec<_>>(), ["zig"]);
    let version = tools["zig"].as_str().unwrap();
    assert!(Regex::new(r"^\d+\.\d+\.\d+$").unwrap().is_match(version));
    let lock: toml::Table = read("mise.rust-build.lock")
        .parse()
        .expect("mise.rust-build.lock parses");
    let zig = lock["tools"]["zig"].as_array().unwrap()[0]
        .as_table()
        .unwrap();
    assert_eq!(zig["version"].as_str().unwrap(), version);
    let checksum = Regex::new(r"^sha256:[0-9a-f]{64}$").unwrap();
    for platform in ["linux-x64", "linux-arm64", "macos-arm64"] {
        let sum = zig[&format!("platforms.{platform}")]["checksum"]
            .as_str()
            .unwrap_or_else(|| panic!("{platform}"));
        assert!(checksum.is_match(sum), "{platform}");
    }
}

#[test]
fn the_package_is_built_on_its_own_and_uploaded_as_package_files() {
    let steps = WF.steps("package");
    assert!(
        steps
            .iter()
            .any(|s| run_of(s).contains("pkl project package"))
    );
    let upload = steps
        .iter()
        .find(|s| uses_of(s).starts_with("actions/upload-artifact@"))
        .unwrap();
    assert_eq!(upload["with"]["name"], "package-files");
}

#[test]
fn smoke_tests_the_binary_built_for_its_own_target_and_the_release_gets_the_package_and_all_three_binaries_into_one_folder()
 {
    let smoke = WF
        .steps("smoke")
        .iter()
        .find(|s| uses_of(s).starts_with("actions/download-artifact@"))
        .unwrap();
    assert_eq!(smoke["with"]["name"], "binary-${{ matrix.target }}");
    // The release tool is downloaded into tool/, beside the files that go into dist/; only the dist/ downloads are listed here.
    let steps = WF.steps("release");
    let downloads: Vec<usize> = (0..steps.len())
        .filter(|i| {
            uses_of(&steps[*i]).starts_with("actions/download-artifact@")
                && steps[*i]["with"]["name"] != "scaffold-release"
        })
        .collect();
    let with: Vec<&Value> = downloads.iter().map(|i| &steps[*i]["with"]).collect();
    assert_eq!(
        with,
        [
            &json!({ "name": "package-files", "path": "dist/" }),
            &json!({ "pattern": "binary-*", "merge-multiple": true, "path": "dist/" }),
        ]
    );
    let checksums = index_of(steps, |s| run_of(s).contains("release-assets checksums"))
        .expect("the checksums step");
    assert!(checksums > downloads[1]);
}

#[test]
fn the_release_workflow_and_the_pull_request_checks_build_the_binary_with_the_same_script() {
    assert!(CI.text.contains("scaffold-release build-rust --target"));
    assert!(CI.text.contains("scaffold-release smoke"));
}

// Phase 4 of docs/ts-tooling-rust-port.md: smoke runs from the Rust release tool, built per target in the unprivileged build job.
#[test]
fn the_smoke_job_runs_the_rust_release_tool_built_for_its_own_target_and_the_type_script_smoke_suite_is_no_longer_run_by_the_release()
 {
    let steps = WF.steps("smoke");
    let tool = steps
        .iter()
        .find(|s| {
            uses_of(s).starts_with("actions/download-artifact@")
                && s["with"]["name"] == "release-tool-${{ matrix.target }}"
        })
        .expect("the tool download");
    assert_eq!(tool["with"]["path"], "tool/");
    let run = steps.iter().map(run_of).collect::<Vec<_>>().join("\n");
    assert!(run.contains("tool/scaffold-release smoke \"dist/pantheon-scaffold-${{ matrix.target }}\" --version \"$VERSION\""));
    assert!(!run.contains("smoke.ts"));
    assert!(!WF.text.contains("smoke.ts"));
}

// Phase 5 of docs/ts-tooling-rust-port.md: the runner is built by the Rust release tool, which the same job builds first.
#[test]
fn the_build_job_builds_the_release_tool_before_it_builds_the_runner_with_it_and_runs_no_type_script_to_do_so()
 {
    let steps = WF.steps("build");
    let tool = index_of(steps, |s| {
        run_of(s).contains("cargo build --release --locked -p scaffold-release")
    })
    .expect("the tool build");
    let runner = index_of(steps, |s| run_of(s).contains("scaffold-release build-rust"))
        .expect("the runner build");
    assert!(runner > tool);
    assert!(run_of(&steps[runner]).contains("target/release/scaffold-release build-rust"));
    assert!(
        !Regex::new(r"\bbun\b|build-rust\.ts")
            .unwrap()
            .is_match(&release_runs("build").join("\n"))
    );
    assert!(!WF.text.contains("build-rust.ts"));
}

#[test]
fn the_pull_request_checks_build_the_tool_before_the_runner_and_build_and_smoke_test_the_runner_on_every_platform()
 {
    let steps = CI.steps("rust-binary");
    let tool = index_of(steps, |s| {
        run_of(s).contains("cargo build --release --locked -p scaffold-release")
    })
    .expect("the tool build");
    let runner = index_of(steps, |s| run_of(s).contains("scaffold-release build-rust"))
        .expect("the runner build");
    assert!(runner > tool);
    let smoke =
        index_of(steps, |s| run_of(s).contains("scaffold-release smoke")).expect("the smoke step");
    assert!(smoke > runner);
    assert!(
        CI.job("rust-binary")["strategy"]["matrix"]
            .to_string()
            .contains("darwin-arm64")
    );
}

#[test]
fn the_release_tool_for_the_smoke_job_is_built_where_nothing_can_write_and_its_artifact_is_kept_out_of_the_released_files()
 {
    let build = WF.job("build");
    assert_eq!(build["permissions"], json!({ "contents": "read" }));
    assert!(
        steps_of(build)
            .iter()
            .any(|s| run_of(s).contains("cargo build --release --locked -p scaffold-release"))
    );
    let upload = steps_of(build)
        .iter()
        .find(|s| {
            uses_of(s).starts_with("actions/upload-artifact@")
                && s["with"]["name"]
                    .as_str()
                    .is_some_and(|n| n.starts_with("release-tool-"))
        })
        .expect("the tool upload");
    assert_eq!(upload["with"]["name"], "release-tool-${{ matrix.target }}");
    assert_eq!(upload["with"]["path"], "target/release/scaffold-release");
    // The release job downloads by the pattern binary-*; a name that matches it would be attested and published.
    let published = WF
        .steps("release")
        .iter()
        .find(|s| !s["with"]["pattern"].is_null())
        .expect("the pattern download")["with"]["pattern"]
        .as_str()
        .unwrap();
    assert_eq!(published, "binary-*");
    assert!(!"release-tool-linux-x64".starts_with(&published.replace('*', "")));
    assert!(!needs(WF.job("release")).contains(&"build".to_owned()));
}

// Phase 6 of docs/ts-tooling-rust-port.md: the TypeScript twins are deleted, and so are the differential scripts that compared them with the
// Rust tools.
#[test]
fn the_pull_request_checks_run_the_rust_smoke_and_baseline_scripts_and_no_differential_script_of_a_deleted_tool()
 {
    for gone in [
        "release-assets-diff",
        "promote-diff",
        "smoke-diff",
        "baseline-diff",
        "build-rust-diff",
    ] {
        assert!(!CI.text.contains(gone), "{gone}");
    }
    assert!(CI.text.contains("scaffold-release smoke"));
    // The runner needs pkl, which only mise puts on the PATH: without it every render exits 1.
    assert!(
        CI.text
            .contains("mise x -- target/debug/scaffold-release baseline snapshot")
    );
    assert!(CI.text.contains("mise x -- target/debug/scaffold-release baseline compare crates/scaffold-release/baseline/baseline.json"));
    assert!(!CI.text.contains("builtins-baseline/snapshot.ts"));
}

#[test]
fn the_release_is_verified_from_the_outside_after_it_is_published_on_all_three_platforms() {
    let verify = WF.job("verify-release");
    assert!(needs(verify).contains(&"release".to_owned()));
    assert_eq!(verify["if"], "github.event_name == 'push'");
    assert!(
        verify["strategy"]["matrix"]
            .to_string()
            .contains("ubuntu-24.04-arm")
    );
    let run = release_runs("verify-release").join("\n");
    assert!(run.contains("gh attestation verify"));
    assert!(run.contains("mise install --locked"));
    assert!(run.contains("upgrade"));
}

#[test]
fn two_releases_never_run_at_once_and_a_running_release_is_not_cancelled() {
    assert_eq!(
        WF.doc["concurrency"]["cancel-in-progress"],
        Value::Bool(false)
    );
}

#[test]
fn the_consumer_the_release_is_verified_with_limits_the_lock_to_the_supported_platforms_and_checks_the_assets_mise_chose()
 {
    let run = release_runs("verify-release").join("\n");
    assert!(run.contains("lockfile_platforms = [\"linux-x64\", \"linux-arm64\", \"macos-arm64\"]"));
    assert!(run.contains("pantheon-scaffold-(linux-x64|linux-arm64|darwin-arm64)"));
}

// Phase 5 wave E: the consumer smoke test. verify-release renders the whole standard set through the real package, as a consumer would.
#[test]
fn verify_release_renders_the_standard_workflows_and_files_through_the_published_package_checks_them_and_renders_again_with_nothing_to_write()
 {
    let step = WF
        .steps("verify-release")
        .iter()
        .find(|s| run_of(s).contains("standardWorkflows"))
        .expect("a step that renders the standard set");
    let run = run_of(step);
    assert!(run.contains("standardFiles"));
    assert!(run.contains("::sha256:"));
    assert!(
        Regex::new(r"pantheon-scaffold check ")
            .unwrap()
            .is_match(run)
    );
    assert!(run.contains("written 0 file(s)"));
    for f in [
        ".github/workflows/dist-check.yml",
        ".github/workflows/audit-skill.yml",
        ".github/workflows/scaffold-check.yml",
    ] {
        assert!(run.contains(f), "{f}");
    }
    assert!(!run.contains("${{"));
}

#[test]
fn verify_release_imports_builtins_from_the_same_package_uri_with_the_same_checksum_as_it_amends_config()
 {
    let step = WF
        .steps("verify-release")
        .iter()
        .find(|s| run_of(s).contains("standardWorkflows"))
        .unwrap();
    assert!(run_of(step).contains("#/Config.pkl"));
    assert!(run_of(step).contains("#/Builtins.pkl"));
}

// The notes of a release are the only text a consumer sees on the release page, so they link the Upgrading section, and promote must keep that
// sentence: it rewrites only the leading "Pre-release of" (`stable_wording` in crates/scaffold-release/src/promote.rs, which its own tests
// cover).
#[test]
fn the_release_notes_start_with_the_pre_release_wording_link_the_upgrading_section_and_keep_the_link_when_promoted()
 {
    let (_, step) = WF
        .all_steps()
        .into_iter()
        .find(|(_, s)| run_of(s).contains("publish-release.sh"))
        .expect("the publishing step");
    let notes = step["env"]["RELEASE_NOTES"]
        .as_str()
        .unwrap()
        .replace("${{ github.repository }}", "$GH_REPO")
        .replace("${{ github.ref_name }}", "$TAG");
    assert!(notes.starts_with("Pre-release of "));
    assert!(notes.contains("https://github.com/$GH_REPO/blob/$TAG/docs/adopting.md#upgrading"));
    assert!(!Regex::new(r"[`]|\$\(|\$\{").unwrap().is_match(&notes));
    let promoted = Regex::new(r"^Pre-release of")
        .unwrap()
        .replace(&notes, "Release of")
        .into_owned();
    assert!(promoted.starts_with("Release of "));
    assert!(promoted.contains("docs/adopting.md#upgrading"));
}
