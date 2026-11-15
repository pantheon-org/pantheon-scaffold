//! The steps of the release workflow that are code and not shell. Ported from `runner/src/release.ts` and
//! `runner/scripts/release-assets.ts` (Phase 1 of `docs/ts-tooling-rust-port.md`).
//!
//! The command is `scaffold-release release-assets <version | check-tag <tag> | checksums <dir> <version>>`. It reads
//! `pkl/PklProject` under the directory it is run from, so the workflow runs it from the checkout root.

pub mod promote;

use std::io::Write;
use std::path::Path;

use scaffold_core::checksums::{Checksum, format_checksums};
use scaffold_core::semver::parse_semver;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const TARGETS: [&str; 3] = ["linux-x64", "linux-arm64", "darwin-arm64"];

pub fn binary_name(target: &str) -> String {
    format!("pantheon-scaffold-{target}")
}

fn package_file(version: &str) -> String {
    format!("pantheon-scaffold@{version}")
}

/// Every file a release carries. The package files come from `pkl project package`, the binaries from the build.
pub fn asset_names(version: &str) -> Vec<String> {
    let pkg = package_file(version);
    let mut names = vec!["checksums.txt".to_owned()];
    names.extend(TARGETS.iter().map(|t| binary_name(t)));
    names.extend([
        pkg.clone(),
        format!("{pkg}.sha256"),
        format!("{pkg}.zip"),
        format!("{pkg}.zip.sha256"),
    ]);
    names.sort();
    names
}

/// The package version, from the one `version = "..."` line of `pkl/PklProject`.
pub fn pkl_project_version(text: &str) -> Result<String, String> {
    static LINE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"(?m)^[ \t]*version[ \t]*=[ \t]*"([^"]+)""#).expect("valid")
    });
    let mut hits = LINE.captures_iter(text);
    match (hits.next(), hits.next()) {
        (None, _) => Err("PklProject has no version line".to_owned()),
        (Some(_), Some(_)) => Err("PklProject has more than one version line".to_owned()),
        (Some(hit), None) => Ok(hit[1].to_owned()),
    }
}

pub(crate) fn json(text: &str) -> String {
    serde_json::to_string(text).expect("a string serialises")
}

/// The version of a release tag `v<version>`, when what follows the `v` is a version.
pub(crate) fn tag_version(tag: &str) -> Option<String> {
    static TAG: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^v([0-9A-Za-z.-]+)$").expect("valid"));
    TAG.captures(tag)
        .map(|m| m[1].to_owned())
        .filter(|v| parse_semver(v).is_some())
}

/// A release is cut from a tag `v<version>`, and the version must be the one in `pkl/PklProject`, because the package URI is
/// built from it.
pub fn check_tag(tag: &str, pkl_project: &str) -> Result<String, String> {
    let version = tag_version(tag).ok_or_else(|| {
        format!(
            "{} is not a release tag (expected v<major>.<minor>.<patch>[-pre-release])",
            json(tag)
        )
    })?;
    let declared = pkl_project_version(pkl_project)?;
    if declared != version {
        return Err(format!(
            "tag {tag} does not match the PklProject version {declared}"
        ));
    }
    Ok(version)
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The text of a one-line file such as `x.sha256`, without its line break.
fn bare(path: &Path) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&read(path)?).trim().to_owned())
}

/// How JavaScript prints the `version` of the package metadata in its message: a missing one is `undefined`.
fn shown(version: Option<&Value>) -> String {
    match version {
        None => "undefined".to_owned(),
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}

/// The contents of `checksums.txt`: the three binaries, the package metadata file (its checksum is the one pkl checks with
/// `::sha256`) and the zip. Before it is written the package files are checked against each other, so a release that
/// disagrees with itself is never attested.
pub fn assemble_checksums(dir: &Path, version: &str) -> Result<String, String> {
    let missing: Vec<String> = asset_names(version)
        .into_iter()
        .filter(|n| n != "checksums.txt" && !dir.join(n).exists())
        .collect();
    if !missing.is_empty() {
        return Err(format!("missing release files: {}", missing.join(", ")));
    }

    let pkg = package_file(version);
    let meta_bytes = read(&dir.join(&pkg))?;
    let zip_bytes = read(&dir.join(format!("{pkg}.zip")))?;
    let meta: Value = serde_json::from_slice(&meta_bytes).map_err(|e| format!("{pkg}: {e}"))?;
    if meta.get("version").and_then(Value::as_str) != Some(version) {
        return Err(format!(
            "the package metadata says version {}, expected {version}",
            shown(meta.get("version"))
        ));
    }
    let zip_sum = sha256(&zip_bytes);
    let meta_sum = sha256(&meta_bytes);
    let declared = meta
        .get("packageZipChecksums")
        .and_then(|c| c.get("sha256"))
        .and_then(Value::as_str);
    if declared != Some(zip_sum.as_str()) {
        return Err(format!(
            "{pkg}.zip does not match the checksum in the package metadata"
        ));
    }
    if bare(&dir.join(format!("{pkg}.sha256")))? != meta_sum {
        return Err(format!("{pkg}.sha256 does not match the metadata file"));
    }
    if bare(&dir.join(format!("{pkg}.zip.sha256")))? != zip_sum {
        return Err(format!("{pkg}.zip.sha256 does not match the zip"));
    }

    let mut entries = Vec::new();
    for target in TARGETS {
        let name = binary_name(target);
        entries.push(Checksum {
            sha256: sha256(&read(&dir.join(&name))?),
            name,
        });
    }
    entries.push(Checksum {
        sha256: meta_sum,
        name: pkg.clone(),
    });
    entries.push(Checksum {
        sha256: zip_sum,
        name: format!("{pkg}.zip"),
    });
    format_checksums(&entries)
}

const USAGE: &str =
    "usage: scaffold-release release-assets check-tag <tag> | version | checksums <dir> <version>";

fn project_text(root: &Path) -> Result<String, String> {
    let path = root.join("pkl/PklProject");
    std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
}

fn release_assets(args: &[&str], root: &Path, out: &mut dyn Write) -> Result<(), String> {
    let line = |out: &mut dyn Write, text: &str| writeln!(out, "{text}").map_err(|e| e.to_string());
    match args {
        ["version"] => line(out, &pkl_project_version(&project_text(root)?)?),
        ["check-tag", tag] => line(out, &check_tag(tag, &project_text(root)?)?),
        ["checksums", dir, version] => {
            let text = assemble_checksums(Path::new(dir), version)?;
            let file = Path::new(dir).join("checksums.txt");
            std::fs::write(&file, &text).map_err(|e| format!("{}: {e}", file.display()))?;
            line(out, text.trim_end())
        }
        _ => Err(String::new()),
    }
}

/// Both commands, for a command line that names neither.
const ALL_USAGE: &str = "usage: scaffold-release release-assets check-tag <tag> | version | checksums <dir> <version>\n       scaffold-release promote check <tag> | promote <tag>";

/// Runs the command. Exit 0 on success, 1 for a refusal (its message on `err`), 2 for a usage error.
pub fn run(args: &[String], root: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let rest = match args.split_first() {
        Some((&"release-assets", rest)) => rest,
        Some((&"promote", rest)) => return promote::command(rest, root, out, err),
        _ => {
            let _ = writeln!(err, "{ALL_USAGE}");
            return 2;
        }
    };
    match release_assets(rest, root, out) {
        Ok(()) => 0,
        Err(message) if message.is_empty() => {
            let _ = writeln!(err, "{USAGE}");
            2
        }
        Err(message) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const VERSION: &str = "0.2.0";

    fn write(dir: &Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).unwrap();
    }

    /// A release folder with a consistent package and fake binaries. Returns the folder.
    fn release_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let pkg = package_file(VERSION);
        let zip = b"zip bytes".to_vec();
        let meta = serde_json::json!({
            "version": VERSION,
            "packageZipChecksums": { "sha256": sha256(&zip) },
        })
        .to_string();
        write(dir.path(), &format!("{pkg}.zip"), &zip);
        write(dir.path(), &pkg, meta.as_bytes());
        write(
            dir.path(),
            &format!("{pkg}.sha256"),
            format!("{}\n", sha256(meta.as_bytes())).as_bytes(),
        );
        write(
            dir.path(),
            &format!("{pkg}.zip.sha256"),
            format!("{}\n", sha256(&zip)).as_bytes(),
        );
        for target in TARGETS {
            write(
                dir.path(),
                &binary_name(target),
                format!("binary for {target}\n").as_bytes(),
            );
        }
        dir
    }

    #[test]
    fn pkl_project_version_reads_the_one_version_line_of_the_package_block() {
        assert_eq!(
            pkl_project_version("package {\n  name = \"x\"\n  version = \"1.2.3\"\n}\n").unwrap(),
            "1.2.3"
        );
        assert!(
            pkl_project_version("package {}\n")
                .unwrap_err()
                .contains("no version")
        );
        assert!(
            pkl_project_version("version = \"1.0.0\"\nversion = \"2.0.0\"\n")
                .unwrap_err()
                .contains("more than one")
        );
    }

    #[test]
    fn a_tag_that_matches_the_pkl_project_version_passes_pre_release_tags_included() {
        assert_eq!(check_tag("v0.2.0", "version = \"0.2.0\"").unwrap(), "0.2.0");
        assert_eq!(
            check_tag("v0.2.0-rc.1", "version = \"0.2.0-rc.1\"").unwrap(),
            "0.2.0-rc.1"
        );
    }

    #[test]
    fn the_real_pkl_project_of_this_repository_agrees_with_its_own_tag() {
        let text = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../pkl/PklProject"),
        )
        .unwrap();
        let version = pkl_project_version(&text).unwrap();
        assert_eq!(check_tag(&format!("v{version}"), &text).unwrap(), version);
    }

    #[test]
    fn a_tag_that_is_not_v_followed_by_a_version_is_refused() {
        for tag in [
            "0.1.0",
            "v0.1",
            "latest",
            "runner-v0.1.0",
            "v0.1.0+build",
            "refs/tags/v0.1.0",
            "v0.1.0\n",
            "",
        ] {
            let error = check_tag(tag, "version = \"0.1.0\"").unwrap_err();
            assert!(error.contains("not a release tag"), "{tag:?}: {error}");
        }
        assert_eq!(
            check_tag("latest", "version = \"0.1.0\"").unwrap_err(),
            "\"latest\" is not a release tag (expected v<major>.<minor>.<patch>[-pre-release])"
        );
    }

    #[test]
    fn a_tag_that_differs_from_the_pkl_project_version_is_refused() {
        assert_eq!(
            check_tag("v0.1.0", "version = \"0.0.0\"").unwrap_err(),
            "tag v0.1.0 does not match the PklProject version 0.0.0"
        );
    }

    #[test]
    fn a_bad_tag_is_refused_before_pkl_project_is_read() {
        // Same order as the TypeScript: the tag is checked first, so a missing version line does not hide it.
        assert!(
            check_tag("latest", "package {}")
                .unwrap_err()
                .contains("not a release tag")
        );
        assert!(
            check_tag("v0.1.0", "package {}")
                .unwrap_err()
                .contains("no version")
        );
    }

    #[test]
    fn the_release_carries_the_package_files_the_three_binaries_and_the_checksums() {
        assert_eq!(
            asset_names("0.1.0"),
            [
                "checksums.txt",
                "pantheon-scaffold-darwin-arm64",
                "pantheon-scaffold-linux-arm64",
                "pantheon-scaffold-linux-x64",
                "pantheon-scaffold@0.1.0",
                "pantheon-scaffold@0.1.0.sha256",
                "pantheon-scaffold@0.1.0.zip",
                "pantheon-scaffold@0.1.0.zip.sha256",
            ]
        );
    }

    #[test]
    fn checksums_list_the_binaries_the_package_metadata_and_the_zip_and_parse_back() {
        let dir = release_dir();
        let text = assemble_checksums(dir.path(), VERSION).unwrap();
        let parsed = scaffold_core::checksums::parse_checksums(&text).unwrap();
        let names: Vec<&str> = parsed.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "pantheon-scaffold-darwin-arm64",
                "pantheon-scaffold-linux-arm64",
                "pantheon-scaffold-linux-x64",
                "pantheon-scaffold@0.2.0",
                "pantheon-scaffold@0.2.0.zip",
            ]
        );
        let meta = std::fs::read(dir.path().join("pantheon-scaffold@0.2.0")).unwrap();
        assert_eq!(parsed[3].sha256, sha256(&meta));
        assert_eq!(parsed[2].sha256, sha256(b"binary for linux-x64\n"));
    }

    #[test]
    fn the_metadata_checksum_is_the_one_pkl_checks_with_the_same_as_the_sha256_file() {
        let dir = release_dir();
        let text = assemble_checksums(dir.path(), VERSION).unwrap();
        let on_disk =
            std::fs::read_to_string(dir.path().join("pantheon-scaffold@0.2.0.sha256")).unwrap();
        assert!(text.contains(&format!("{}  pantheon-scaffold@0.2.0\n", on_disk.trim())));
    }

    #[test]
    fn the_checksums_use_two_spaces_and_end_with_one_line_break() {
        let text = assemble_checksums(release_dir().path(), VERSION).unwrap();
        assert!(text.ends_with('\n') && !text.ends_with("\n\n"));
        assert!(text.lines().all(|l| l.as_bytes()[64..66] == *b"  "));
    }

    #[test]
    fn a_missing_binary_or_package_file_is_an_error_that_lists_what_is_missing() {
        let dir = release_dir();
        std::fs::remove_file(dir.path().join("pantheon-scaffold-linux-arm64")).unwrap();
        std::fs::remove_file(dir.path().join("pantheon-scaffold@0.2.0.zip")).unwrap();
        assert_eq!(
            assemble_checksums(dir.path(), VERSION).unwrap_err(),
            "missing release files: pantheon-scaffold-linux-arm64, pantheon-scaffold@0.2.0.zip"
        );
    }

    #[test]
    fn a_zip_that_does_not_match_the_checksum_in_the_metadata_is_refused() {
        let dir = release_dir();
        write(dir.path(), "pantheon-scaffold@0.2.0.zip", b"tampered");
        assert_eq!(
            assemble_checksums(dir.path(), VERSION).unwrap_err(),
            "pantheon-scaffold@0.2.0.zip does not match the checksum in the package metadata"
        );
    }

    #[test]
    fn a_sha256_file_that_does_not_match_its_file_is_refused() {
        let dir = release_dir();
        write(
            dir.path(),
            "pantheon-scaffold@0.2.0.sha256",
            "0".repeat(64).as_bytes(),
        );
        assert_eq!(
            assemble_checksums(dir.path(), VERSION).unwrap_err(),
            "pantheon-scaffold@0.2.0.sha256 does not match the metadata file"
        );
    }

    #[test]
    fn a_zip_sha256_file_that_does_not_match_the_zip_is_refused() {
        let dir = release_dir();
        write(
            dir.path(),
            "pantheon-scaffold@0.2.0.zip.sha256",
            "0".repeat(64).as_bytes(),
        );
        assert_eq!(
            assemble_checksums(dir.path(), VERSION).unwrap_err(),
            "pantheon-scaffold@0.2.0.zip.sha256 does not match the zip"
        );
    }

    #[test]
    fn metadata_for_another_version_is_refused_even_when_its_sha256_file_matches() {
        let dir = release_dir();
        let zip = std::fs::read(dir.path().join("pantheon-scaffold@0.2.0.zip")).unwrap();
        let meta = serde_json::json!({
            "version": "9.9.9",
            "packageZipChecksums": { "sha256": sha256(&zip) },
        })
        .to_string();
        write(dir.path(), "pantheon-scaffold@0.2.0", meta.as_bytes());
        write(
            dir.path(),
            "pantheon-scaffold@0.2.0.sha256",
            sha256(meta.as_bytes()).as_bytes(),
        );
        assert_eq!(
            assemble_checksums(dir.path(), VERSION).unwrap_err(),
            "the package metadata says version 9.9.9, expected 0.2.0"
        );
    }

    #[test]
    fn metadata_with_no_version_says_undefined_like_the_typescript() {
        let dir = release_dir();
        write(dir.path(), "pantheon-scaffold@0.2.0", b"{}");
        assert_eq!(
            assemble_checksums(dir.path(), VERSION).unwrap_err(),
            "the package metadata says version undefined, expected 0.2.0"
        );
    }

    #[test]
    fn metadata_that_is_not_json_is_refused_naming_the_file() {
        let dir = release_dir();
        write(dir.path(), "pantheon-scaffold@0.2.0", b"not json");
        let error = assemble_checksums(dir.path(), VERSION).unwrap_err();
        assert!(error.starts_with("pantheon-scaffold@0.2.0: "), "{error}");
    }

    fn run_in(root: &Path, args: &[&str]) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&args, root, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn project(version: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("pkl")).unwrap();
        std::fs::write(
            root.path().join("pkl/PklProject"),
            format!("package {{\n  version = \"{version}\"\n}}\n"),
        )
        .unwrap();
        root
    }

    #[test]
    fn the_command_prints_the_version_for_a_matching_tag_and_exits_1_for_another() {
        let root = project("0.2.0");
        assert_eq!(
            run_in(root.path(), &["release-assets", "check-tag", "v0.2.0"]),
            (0, "0.2.0\n".to_owned(), String::new())
        );
        let (code, out, err) = run_in(root.path(), &["release-assets", "check-tag", "v9.9.9"]);
        assert_eq!((code, out.as_str()), (1, ""));
        assert_eq!(
            err,
            "tag v9.9.9 does not match the PklProject version 0.2.0\n"
        );
    }

    #[test]
    fn the_command_prints_the_pkl_project_version_for_a_dry_run_that_has_no_tag() {
        let root = project("0.3.0-rc.1");
        assert_eq!(
            run_in(root.path(), &["release-assets", "version"]),
            (0, "0.3.0-rc.1\n".to_owned(), String::new())
        );
    }

    #[test]
    fn the_command_writes_checksums_txt_into_the_release_folder_and_prints_it() {
        let root = project("0.2.0");
        let dir = release_dir();
        let path = dir.path().to_str().unwrap();
        let (code, out, err) = run_in(root.path(), &["release-assets", "checksums", path, "0.2.0"]);
        assert_eq!((code, err.as_str()), (0, ""));
        let written = std::fs::read_to_string(dir.path().join("checksums.txt")).unwrap();
        assert_eq!(out, written.trim_end().to_owned() + "\n");
        assert_eq!(written.lines().count(), 5);
    }

    #[test]
    fn a_refused_release_folder_writes_no_checksums_txt() {
        let root = project("0.2.0");
        let dir = release_dir();
        std::fs::remove_file(dir.path().join("pantheon-scaffold-linux-x64")).unwrap();
        let path = dir.path().to_str().unwrap();
        let (code, _, err) = run_in(root.path(), &["release-assets", "checksums", path, "0.2.0"]);
        assert_eq!(code, 1);
        assert!(err.contains("missing release files"), "{err}");
        assert!(!dir.path().join("checksums.txt").exists());
    }

    #[test]
    fn a_missing_pkl_project_is_a_refusal_naming_the_file() {
        let root = tempfile::tempdir().unwrap();
        let (code, _, err) = run_in(root.path(), &["release-assets", "version"]);
        assert_eq!(code, 1);
        assert!(err.contains("PklProject"), "{err}");
    }

    #[test]
    fn a_usage_error_exits_2_with_the_usage_line() {
        let root = project("0.2.0");
        for args in [
            &["release-assets"][..],
            &["release-assets", "checksums", "only-one-arg"][..],
            &["release-assets", "check-tag"][..],
            &["release-assets", "version", "extra"][..],
            &["release-assets", "nope"][..],
        ] {
            let (code, out, err) = run_in(root.path(), args);
            assert_eq!(code, 2, "{args:?}");
            assert_eq!(out, "", "{args:?}");
            assert_eq!(err, format!("{USAGE}\n"), "{args:?}");
        }
    }

    #[test]
    fn a_command_line_that_names_no_command_prints_both_usages() {
        let root = project("0.2.0");
        for args in [&[][..], &["something-else", "version"][..]] {
            let (code, out, err) = run_in(root.path(), args);
            assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
            assert_eq!(err, format!("{ALL_USAGE}\n"), "{args:?}");
            assert!(err.contains("release-assets") && err.contains("promote"));
        }
    }

    #[test]
    fn a_promote_usage_error_exits_2_with_its_own_usage_line() {
        let root = project("0.2.0");
        for args in [
            &["promote"][..],
            &["promote", "check"][..],
            &["promote", "nope", "v0.2.0"][..],
            &["promote", "check", "v0.2.0", "extra"][..],
        ] {
            let (code, out, err) = run_in(root.path(), args);
            assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
            assert_eq!(err, format!("{}\n", promote::USAGE), "{args:?}");
        }
    }
}
