//! `init` through the real binary, against a release: a local server in place of GitHub (the release list, the known-bad list and
//! the checksums), and fake `gh` and `pkl` programs on the PATH. The fake pkl prints an empty contract for the package the config
//! amends, so these tests are about which release `init` picks and what it writes, not about what the package renders; that is the
//! scenarios of features/init.feature, which use the real package.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

fn h(c: char) -> String {
    c.to_string().repeat(64)
}

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

fn release(tag: &str, prerelease: bool) -> String {
    format!("{{\"tag_name\":\"{tag}\",\"prerelease\":{prerelease},\"draft\":false}}")
}

/// A server answering `routes` (path to body, anything else 404), and fake `gh` and `pkl`. The fake pkl answers with a contract for
/// the version named in the last `package://` address of the config it is given, so the preflight sees a version.
fn setup(routes: HashMap<String, String>) -> Setup {
    let bin = tempfile::tempdir().unwrap();
    std::fs::write(bin.path().join("calls"), "").unwrap();
    let gh = format!(
        "#!/bin/sh\necho \"gh $*\" >> \"{calls}\"\nif [ -n \"$GH_FAIL\" ]; then echo \"no valid attestation\" >&2; exit 1; fi\nexit 0\n",
        calls = bin.path().join("calls").display()
    );
    let pkl = "#!/bin/sh\nfor last; do :; done\nuri=$(grep -o 'package://[^\"#]*' \"$last\" | head -1)\necho \"{\\\"schemaVersion\\\":2,\\\"files\\\":[],\\\"pins\\\":[],\\\"escapeHatch\\\":{\\\"uses\\\":0,\\\"constructs\\\":0},\\\"packageUri\\\":\\\"$uri#/Config.pkl\\\"}\"\n";
    for (name, body) in [("gh", gh.as_str()), ("pkl", pkl)] {
        let path = bin.path().join(name);
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
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
    fn run(&self, args: &[&str], env: &[(&str, &str)]) -> Run {
        let path = format!(
            "{}:{}",
            self.bin.path().display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let root = self.repo.path().display().to_string();
        let releases = format!("{}/releases", self.base);
        let known_bad = format!("{}/known-bad.json", self.base);
        let mut command = Command::new(env!("CARGO_BIN_EXE_pantheon-scaffold"));
        command
            .arg("init")
            .args(["--toolchain", "1.96.1", "--root", &root, "--url", &releases])
            .args(args)
            .current_dir(self.repo.path())
            .env("PATH", path)
            .env("PANTHEON_SCAFFOLD_RELEASE_BASE", &self.base)
            .env("PANTHEON_SCAFFOLD_KNOWN_BAD_URL", &known_bad)
            .env_remove("GITHUB_TOKEN")
            .env_remove("GH_TOKEN")
            .env_remove("CI");
        for (name, value) in env {
            command.env(name, value);
        }
        let output = command.output().unwrap();
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

    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.bin.path().join("calls"))
            .unwrap()
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    }
}

fn checksums(version: &str) -> String {
    format!(
        "{}  pantheon-scaffold@{version}\n{}  other\n",
        h('c'),
        h('d')
    )
}

fn routes(list: &[String], bad: &str, versions: &[&str]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("/releases".to_owned(), format!("[{}]", list.join(",")));
    map.insert("/known-bad.json".to_owned(), bad.to_owned());
    for v in versions {
        map.insert(format!("/v{v}/checksums.txt"), checksums(v));
    }
    map
}

fn amends(version: &str) -> String {
    format!(
        "amends \"package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{version}/pantheon-scaffold@{version}::sha256:{}#/Config.pkl\"",
        h('c')
    )
}

const NOTHING_BAD: &str = "{\"versions\":[]}";

#[test]
fn init_picks_the_newest_stable_release_that_is_not_known_bad_and_pins_its_verified_checksum() {
    let list = [
        release("v0.4.0", true),
        release("v0.3.0", false),
        release("v0.2.0", false),
        release("v0.1.5", false),
    ];
    let bad = "{\"versions\":[{\"version\":\"0.3.0\",\"reason\":\"broken\"}]}";
    let s = setup(routes(&list, bad, &["0.2.0"]));
    let r = s.run(&["--write"], &[]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    let config = s.get("pantheon.pkl");
    assert!(config.contains(&amends("0.2.0")), "{config}");
    assert!(
        config.contains(
            "import \"package://github.com/pantheon-org/pantheon-scaffold/releases/download/v0.2.0/"
        ),
        "{config}"
    );
    assert!(
        r.out.contains("init: wrote pantheon.pkl and 0 file(s)"),
        "{}",
        r.out
    );
    // The manifest records the version the config was rendered from, as a render does.
    assert!(
        s.get(".pkl-gen/manifest.json")
            .contains("\"packageVersion\": \"0.2.0\"")
    );
    let calls = s.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        calls[0].starts_with("gh attestation verify ")
            && calls[0].contains("checksums.txt --repo pantheon-org/pantheon-scaffold"),
        "{}",
        calls[0]
    );
}

#[test]
fn a_package_version_on_the_command_line_is_used_instead_of_the_newest() {
    let list = [release("v0.3.0", false)];
    let s = setup(routes(&list, NOTHING_BAD, &["0.1.5"]));
    let r = s.run(&["--write", "--package-version", "v0.1.5"], &[]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert!(s.get("pantheon.pkl").contains(&amends("0.1.5")));
}

#[test]
fn a_dry_run_shows_the_config_with_the_release_and_writes_nothing() {
    let list = [release("v0.2.0", false)];
    let s = setup(routes(&list, NOTHING_BAD, &["0.2.0"]));
    let r = s.run(&[], &[]);
    assert_eq!(r.code, 0, "{} {}", r.out, r.err);
    assert!(
        r.out.starts_with("pantheon.pkl would be written:"),
        "{}",
        r.out
    );
    assert!(r.out.contains(&amends("0.2.0")), "{}", r.out);
    assert!(!s.exists("pantheon.pkl"));
}

#[test]
fn a_failed_attestation_stops_with_exit_1_and_writes_nothing() {
    let list = [release("v0.2.0", false)];
    let s = setup(routes(&list, NOTHING_BAD, &["0.2.0"]));
    let r = s.run(&["--write"], &[("GH_FAIL", "1")]);
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(r.err.contains("attestation check failed"), "{}", r.err);
    assert!(!s.exists("pantheon.pkl"));
}

#[test]
fn a_release_whose_checksums_cannot_be_fetched_fails_closed_with_nothing_written() {
    let list = [release("v0.2.0", false)];
    let s = setup(routes(&list, NOTHING_BAD, &[]));
    let r = s.run(&["--write", "--attempts", "1"], &[]);
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(!s.exists("pantheon.pkl"));
    assert!(s.calls().is_empty(), "{:?}", s.calls());
}

#[test]
fn with_no_release_that_qualifies_init_says_so_and_offers_a_version_flag() {
    let list = [release("v0.4.0", true)];
    let s = setup(routes(&list, NOTHING_BAD, &[]));
    let r = s.run(&["--write"], &[]);
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(
        r.err.contains("no release found that qualifies") && r.err.contains("--package-version"),
        "{}",
        r.err
    );
    assert!(!s.exists("pantheon.pkl"));
}

#[test]
fn a_release_on_the_known_bad_list_is_refused_even_when_named() {
    let bad = "{\"versions\":[{\"version\":\"0.2.0\",\"reason\":\"broken\"}]}";
    let s = setup(routes(&[], bad, &["0.2.0"]));
    let r = s.run(&["--write", "--package-version", "0.2.0"], &[]);
    assert_eq!(r.code, 3, "{} {}", r.out, r.err);
    assert!(
        r.err.contains("0.2.0") && r.err.contains("broken"),
        "{}",
        r.err
    );
    assert!(!s.exists("pantheon.pkl"));
}
