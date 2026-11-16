//! Ported from `runner/test/package-uri.test.ts`: what a package loaded from a package URI reports, and that the runner can read its version
//! from it. The real package is built, served locally in place of GitHub (as spike S.1 did) and a consumer file is rendered through it.

use scaffold_core::preflight::package_version_of;
use scaffold_package_tests::pkg::{eval, fixture, pkl_project_version, root};
use scaffold_package_tests::repo::temp_dir;
use scaffold_package_tests::serve::{Server, serve_folder};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

struct Package {
    out: PathBuf,
    server: Server,
}

/// The package, built once for this file and served for as long as the tests run.
fn package() -> &'static Package {
    static PACKAGE: OnceLock<Package> = OnceLock::new();
    PACKAGE.get_or_init(|| {
        let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("package-uri");
        let _ = std::fs::remove_dir_all(&out);
        let built = Command::new("pkl")
            .args(["project", "package"])
            .arg(root().join("pkl"))
            .arg("--output-path")
            .arg(&out)
            .output()
            .expect("pkl runs");
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let server = serve_folder(&out);
        Package { out, server }
    })
}

fn uri() -> String {
    let v = pkl_project_version();
    format!(
        "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{v}/pantheon-scaffold@{v}#/Config.pkl"
    )
}

/// Renders a consumer file through the served package and returns the contract. A real consumer pins the package with the checksum of its
/// metadata file, which lands in the URI before the `#`.
fn render_through_package(consumer_body: &str, pin_checksum: bool) -> Value {
    let package = package();
    let work = temp_dir();
    let version = pkl_project_version();
    let metadata = std::fs::read(package.out.join(format!("pantheon-scaffold@{version}"))).unwrap();
    let sha: String = Sha256::digest(&metadata)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let uri = if pin_checksum {
        uri().replace("#/Config.pkl", &format!("::sha256:{sha}#/Config.pkl"))
    } else {
        uri()
    };
    // @@BUILTINS@@ in a consumer body is the package's own Builtins module, loaded from the same package URI.
    let body = consumer_body.replace(
        "@@BUILTINS@@",
        &uri.replace("#/Config.pkl", "#/Builtins.pkl"),
    );
    let consumer = work.path().join("consumer");
    std::fs::create_dir(&consumer).unwrap();
    let config = consumer.join("scaffold.pkl");
    std::fs::write(&config, format!("amends \"{uri}\"\n{body}\n")).unwrap();
    let output = Command::new("pkl")
        .args(["eval", "-f", "json", "--cache-dir"])
        .arg(work.path().join("cache"))
        .arg("--http-rewrite")
        .arg(format!(
            "https://github.com/=http://localhost:{}/",
            package.server.port
        ))
        .args(["--allowed-resources", "package:,https:,http:"])
        .arg(&config)
        .output()
        .expect("pkl runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("pkl prints JSON")
}

#[test]
fn a_package_loaded_from_a_package_uri_reports_that_uri_and_the_runner_reads_its_version_from_it() {
    let json = render_through_package(r#"options { toolchain = "1.96.1" }"#, false);
    assert_eq!(json["packageUri"], uri());
    assert_eq!(
        package_version_of(json["packageUri"].as_str()).unwrap(),
        Some(pkl_project_version())
    );
    assert_eq!(json["requires"], json!({ "runner": "0.0.0" }));
}

#[test]
fn a_package_loaded_from_a_local_folder_has_no_package_uri() {
    let json = eval(&fixture("minimal")).unwrap();
    assert!(json.get("packageUri").is_none());
    assert_eq!(json["requires"], json!({ "runner": "0.0.0" }));
}

#[test]
fn a_file_marked_remove_on_detach_reaches_the_contract_and_an_unmarked_one_does_not_carry_the_key()
{
    let json = render_through_package(
        "options { toolchain = \"1.96.1\" }\nfiles { [\"up.txt\"] { content = \"x\\n\"; removeOnDetach = true }\n[\"b.txt\"] { content = \"y\\n\" } }",
        false,
    );
    let file = |path: &str| {
        json["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == path)
            .unwrap_or_else(|| panic!("no file {path}"))
    };
    assert_eq!(file("up.txt")["removeOnDetach"], true);
    assert!(file("b.txt").get("removeOnDetach").is_none());
}

#[test]
fn a_consumer_that_pins_the_package_with_its_checksum_still_gets_a_package_uri_the_runner_can_read_the_version_from()
 {
    let json = render_through_package(r#"options { toolchain = "1.96.1" }"#, true);
    let uri = json["packageUri"].as_str().unwrap();
    assert!(
        regex::Regex::new(r"::sha256:[0-9a-f]{64}#/Config\.pkl$")
            .unwrap()
            .is_match(uri),
        "{uri}"
    );
    assert_eq!(
        package_version_of(Some(uri)).unwrap(),
        Some(pkl_project_version())
    );
}

/// The runner pin in the rendered mise.toml is the version of the package that rendered it, with no runnerVersion option set.
fn the_mise_builtin_pins_the_runner_at_the_package_version(pin_checksum: bool) {
    let json = render_through_package(
        "import \"@@BUILTINS@@\"\noptions { toolchain = \"1.96.1\" }\nfiles { ...Builtins.miseFiles(options) }",
        pin_checksum,
    );
    let mise = json["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "mise.toml")
        .expect("a mise.toml");
    assert!(
        mise["template"]
            .as_str()
            .unwrap()
            .contains(r#""github:pantheon-org/pantheon-scaffold" = "{{ runner }}""#)
    );
    assert_eq!(mise["context"]["runner"], pkl_project_version());
}

#[test]
fn the_mise_builtin_pins_the_runner_at_the_package_version_when_loaded_from_a_package_uri() {
    the_mise_builtin_pins_the_runner_at_the_package_version(false);
}

#[test]
fn the_mise_builtin_pins_the_runner_at_the_package_version_when_loaded_from_a_package_uri_that_carries_the_pinned_checksum()
 {
    the_mise_builtin_pins_the_runner_at_the_package_version(true);
}
