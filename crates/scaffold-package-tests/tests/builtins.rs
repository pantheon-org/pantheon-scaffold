//! Ported from `runner/test/builtins.test.ts`: the slice, three workflows and five verbatim files, rendered from the first builtins.

use scaffold_package_tests::pkg::{
    asset_path, content, expect_pins, file, git_blob_sha, mode, paths, read, render, unpin,
    upstream_json,
};
use scaffold_package_tests::{compare_workflows, parse_toml};
use serde_json::Value;
use std::sync::LazyLock;

static SLICE: LazyLock<Value> = LazyLock::new(|| render("slice"));

#[test]
fn the_slice_renders_the_three_workflows_five_verbatim_files_and_no_external_file() {
    assert_eq!(
        paths(&SLICE),
        [
            ".github/scripts/cargo-deny.sh",
            ".github/scripts/check-inline-scripts.sh",
            ".github/scripts/check-toolchain-version.sh",
            ".github/workflows/cargo-deny.yml",
            ".github/workflows/inline-scripts.yml",
            ".github/workflows/rust-ci.yml",
            "deny.toml",
            "rust-toolchain.toml",
        ]
    );
}

fn yml_equals_the_pinned_copy(name: &str) {
    let upstream = read(&format!(
        "crates/scaffold-package-tests/upstream/workflows/{name}.yml"
    ));
    let rendered = content(&SLICE, &format!(".github/workflows/{name}.yml"));
    let result = compare_workflows(&unpin(&upstream), &unpin(rendered)).unwrap();
    assert!(result.equal, "{:?}", result.differences);
    assert!(result.duplicate_keys.is_empty());
}

#[test]
fn cargo_deny_yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule() {
    yml_equals_the_pinned_copy("cargo-deny");
}

#[test]
fn inline_scripts_yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule() {
    yml_equals_the_pinned_copy("inline-scripts");
}

#[test]
fn rust_ci_yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule() {
    yml_equals_the_pinned_copy("rust-ci");
}

fn script_is_the_asset_and_executable(name: &str) {
    let path = format!(".github/scripts/{name}.sh");
    assert_eq!(
        content(&SLICE, &path),
        read(&asset_path(&format!("{name}.sh")))
    );
    assert_eq!(mode(&SLICE, &path), "0755");
}

#[test]
fn cargo_deny_sh_is_byte_for_byte_the_asset_and_executable() {
    script_is_the_asset_and_executable("cargo-deny");
}

#[test]
fn check_inline_scripts_sh_is_byte_for_byte_the_asset_and_executable() {
    script_is_the_asset_and_executable("check-inline-scripts");
}

#[test]
fn check_toolchain_version_sh_is_byte_for_byte_the_asset_and_executable() {
    script_is_the_asset_and_executable("check-toolchain-version");
}

#[test]
fn deny_toml_renders_as_valid_toml_with_the_shared_licence_list_and_is_not_executable() {
    let toml = parse_toml(content(&SLICE, "deny.toml"));
    let allow = toml["licenses"]["allow"]
        .as_array()
        .expect("licenses.allow");
    assert!(allow.iter().any(|l| l.as_str() == Some("Apache-2.0")));
    assert_eq!(mode(&SLICE, "deny.toml"), "0644");
}

#[test]
fn rust_toolchain_toml_carries_the_one_toolchain_option() {
    let asset = read(&asset_path("rust-toolchain.toml.j2"));
    assert_eq!(
        content(&SLICE, "rust-toolchain.toml"),
        asset.replacen("{{ toolchain }}", "1.96.1", 1)
    );
}

#[test]
fn the_toolchain_option_reaches_both_workflows_that_install_it() {
    for name in ["cargo-deny", "rust-ci"] {
        assert!(
            content(&SLICE, &format!(".github/workflows/{name}.yml"))
                .contains("rustup toolchain install 1.96.1"),
            "{name}"
        );
    }
}

#[test]
fn the_slice_uses_no_escape_hatch() {
    assert_eq!(SLICE["escapeHatch"]["uses"], 0);
}

#[test]
fn the_only_pin_is_checkout_at_its_release_tag() {
    expect_pins(&SLICE["pins"], &["actions/checkout"]);
}

/// Non-circular byte fidelity: the git blob hash of what pkl renders must equal the blob hash of the pinned upstream file. This is the
/// five TypeScript tests, one for each entry of `blobs.json`.
#[test]
fn each_verbatim_file_renders_byte_for_byte_as_the_pinned_pantheon_adr_file() {
    let blobs = upstream_json("blobs.json");
    let blobs = blobs.as_object().expect("blobs.json is an object");
    assert_eq!(blobs.len(), 5);
    for (path, blob) in blobs {
        assert_eq!(
            git_blob_sha(content(&SLICE, path)),
            blob.as_str().unwrap(),
            "{path}"
        );
        assert!(file(&SLICE, path).is_object());
    }
}
