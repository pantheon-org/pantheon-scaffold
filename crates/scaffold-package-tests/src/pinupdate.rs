//! A stand-in for `main` and for the tar of a pin update, and a runner of `.github/scripts/apply-pin-update.sh`, for the tests of the two scripts of the
//! `update-pins` workflow.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::pkg::root;
use crate::repo::{put, temp_dir};
use crate::tarball::{Kind, Member, build};

pub fn sha(c: char) -> String {
    c.to_string().repeat(40)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub const WORKFLOW: &str = ".github/workflows/rust-ci.yml";
pub const PINS: &str = "pkl/Pins.pkl";
pub const MANIFEST: &str = ".pkl-gen/manifest.json";
pub const FRESHNESS: &str =
    "{\n  \"schemaVersion\": 1,\n  \"lastSuccessfulPinCheck\": \"2026-11-17T04:23:10Z\"\n}\n";

/// A rendered workflow with two pinned actions and one line that is not a pin.
pub fn workflow(checkout: char, mise: char, extra: &str) -> String {
    format!(
        "name: ci\non:\n  push:\npermissions:\n  contents: read\njobs:\n  a:\n    runs-on: ubuntu-latest\n    steps:\n    - uses: actions/checkout@{} # v7.0.1\n    - uses: jdx/mise-action@{} # v5.2.7\n    - run: echo hi\n{extra}",
        sha(checkout),
        sha(mise)
    )
}

pub fn pins(checkout: char, mise: char) -> String {
    format!(
        "module pins\nclass Pin {{ action: String; sha: String; version: String }}\ncheckout: Pin = new {{ action = \"actions/checkout\"; sha = \"{}\"; version = \"v7.0.1\" }}\nmiseAction: Pin = new {{ action = \"jdx/mise-action\"; sha = \"{}\"; version = \"v5.2.7\" }}\n",
        sha(checkout),
        sha(mise)
    )
}

/// The manifest of the fixture: the workflow is owned with this hash, and a seed has a hash that is not that of its file.
pub fn manifest(workflow_hash: &str) -> String {
    format!(
        "{{\n  \"manifestVersion\": 1,\n  \"files\": {{\n    \"{WORKFLOW}\": {{\n      \"class\": \"owned\",\n      \"hash\": \"{workflow_hash}\"\n    }},\n    \"README.md\": {{\n      \"class\": \"seed\",\n      \"hash\": \"{}\"\n    }}\n  }}\n}}\n",
        "9".repeat(64)
    )
}

/// A checkout of `main`: the owned workflow with the pins `a` and `b`, two hand-written workflows the manifest does not list, `pkl/Pins.pkl`, and the
/// manifest.
pub fn main_repo() -> tempfile::TempDir {
    let dir = temp_dir();
    let wf = workflow('a', 'b', "");
    put(dir.path(), WORKFLOW, &wf);
    put(
        dir.path(),
        ".github/workflows/ci.yml",
        &workflow('a', 'b', "    - run: echo ci\n"),
    );
    put(
        dir.path(),
        ".github/workflows/update-pins.yml",
        &workflow('a', 'b', "    - run: echo up\n"),
    );
    put(
        dir.path(),
        ".github/scripts/apply-pin-update.sh",
        "#!/usr/bin/env bash\necho main's copy\n",
    );
    put(dir.path(), "README.md", "# demo\n");
    put(dir.path(), PINS, &pins('a', 'b'));
    put(dir.path(), MANIFEST, &manifest(&sha256_hex(wf.as_bytes())));
    dir
}

/// Every file under a folder, path to content, for a before and after comparison.
pub fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, current: &Path, found: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(current).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, found);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                found.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut found = BTreeMap::new();
    walk(dir, dir, &mut found);
    found
}

/// The members of a tar that the collect script would make for a pin update that moves the pins to `c` and `d`, as a list a test edits before
/// it builds the tar.
pub struct Artifact {
    pub members: Vec<Member>,
}

impl Artifact {
    /// The update `a`, `b` to `c`, `d`: the workflow, the manifest with the new hash, `pkl/Pins.pkl`, the record and the summary.
    pub fn valid() -> Self {
        let wf = workflow('c', 'd', "");
        let changed = format!("{WORKFLOW}\n{MANIFEST}\n{PINS}\n");
        Self {
            members: vec![
                Member::file("changed.txt", &changed),
                Member::file("freshness.json", FRESHNESS),
                Member::file("summary.md", "Moves the pinned actions.\n"),
                Member::dir("tree/"),
                Member::dir("tree/.github/"),
                Member::dir("tree/.github/workflows/"),
                Member::file(&format!("tree/{WORKFLOW}"), &wf),
                Member::dir("tree/.pkl-gen/"),
                Member::file(
                    &format!("tree/{MANIFEST}"),
                    &manifest(&sha256_hex(wf.as_bytes())),
                ),
                Member::dir("tree/pkl/"),
                Member::file(&format!("tree/{PINS}"), &pins('c', 'd')),
            ],
        }
    }

    /// The update when no pin moved: the record alone.
    pub fn quiet() -> Self {
        Self {
            members: vec![
                Member::file("changed.txt", ""),
                Member::file("freshness.json", FRESHNESS),
                Member::dir("tree/"),
            ],
        }
    }

    pub fn replace(mut self, name: &str, content: &str) -> Self {
        let slot = self
            .members
            .iter_mut()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("no member {name}"));
        slot.content = content.as_bytes().to_vec();
        self
    }

    pub fn remove(mut self, name: &str) -> Self {
        let before = self.members.len();
        self.members.retain(|m| m.name != name);
        assert_ne!(before, self.members.len(), "no member {name}");
        self
    }

    pub fn with(mut self, member: Member) -> Self {
        self.members.push(member);
        self
    }

    pub fn bytes(&self) -> Vec<u8> {
        build(&self.members)
    }
}

pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    /// The folder the script was asked to write the record to; it may not exist.
    pub out: PathBuf,
    _keep: tempfile::TempDir,
}

/// Runs `apply-pin-update.sh` of this repository in `repo` on these tar bytes. The digest is the right one unless the test gives another.
pub fn apply(repo: &Path, tar: &[u8], digest: Option<&str>, check_only: bool) -> Run {
    let keep = temp_dir();
    let tar_path = keep.path().join("pin-update.tar");
    std::fs::write(&tar_path, tar).unwrap();
    let out = keep.path().join("out");
    let right = sha256_hex(tar);
    let mut cmd = Command::new("bash");
    cmd.arg(root().join(".github/scripts/apply-pin-update.sh"))
        .arg(&tar_path)
        .arg(digest.unwrap_or(&right))
        .arg(&out)
        .current_dir(repo);
    if check_only {
        cmd.arg("--check-only");
    }
    let output = cmd.output().expect("bash runs");
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        out,
        _keep: keep,
    }
}

/// Asserts that the script refused with a message that has `needle`, wrote nothing in the checkout and did not make the output folder.
pub fn assert_refused(repo: &Path, before: &BTreeMap<String, Vec<u8>>, run: &Run, needle: &str) {
    assert_eq!(
        run.code, 1,
        "stdout: {}\nstderr: {}",
        run.stdout, run.stderr
    );
    assert!(
        run.stderr.contains("refused:") && run.stderr.contains(needle),
        "wanted a refusal naming {needle:?}, got: {}",
        run.stderr
    );
    assert_eq!(&snapshot(repo), before, "a refusal writes nothing");
    assert!(
        !run.out.exists(),
        "a refusal does not make the output folder"
    );
}

/// A link member that points at `target`.
pub fn link(name: &str, kind: Kind, target: &str) -> Member {
    Member::link(name, kind, target)
}
