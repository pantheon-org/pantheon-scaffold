//! Builds the package from a tree's `pkl/` folder, serves it locally in place of GitHub, renders the full composition through the package URI
//! for a few option sets, and compares each with the folder render of the same tree. Ported from
//! `runner/scripts/builtins-baseline/packaged.ts`.
//!
//! Checks: (1) every file under `pkl/` is in the package zip, (2) the packaged contract equals the folder contract (`packageUri` aside), so
//! `read()` of a submodule's assets and the types of `Config.*` work from a package URI. This is a local zip, not the published package: the
//! true `package://` path is proven only by `verify-release` at the next release.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::json::{resolve, sort_like_javascript};

pub const USAGE: &str = "usage: scaffold-release baseline packaged <tree root> <binary> <work dir>";

pub fn parse_args(args: &[&str], cwd: &Path) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    match args {
        [tree, binary, work, ..] => {
            Ok((resolve(tree, cwd), resolve(binary, cwd), resolve(work, cwd)))
        }
        _ => Err(USAGE.to_owned()),
    }
}

/// The files under a folder, relative to `base`.
fn walk(base: &Path, dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                found.extend(walk(base, &path));
            } else {
                found.push(
                    path.strip_prefix(base)
                        .expect("under base")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    found
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && hex(bytes[i + 1]).is_some()
            && hex(bytes[i + 2]).is_some()
        {
            out.push(
                (hex(bytes[i + 1]).expect("checked") * 16 + hex(bytes[i + 2]).expect("checked"))
                    as u8,
            );
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Answers one request with the file of that name in `folder`, or a 404.
fn answer(mut stream: TcpStream, folder: &Path) {
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => request.extend_from_slice(&buffer[..n]),
        }
    }
    let request = String::from_utf8_lossy(&request);
    let target = request
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");
    let name = percent_decode(path.rsplit('/').next().unwrap_or(""));
    let file = folder.join(name);
    let (status, body) = match (file.is_file(), std::fs::read(&file)) {
        (true, Ok(bytes)) => ("200 OK", bytes),
        _ => ("404 Not Found", Vec::new()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

/// Serves the files of `folder` on a free local port, and returns the port. The threads end with the process.
fn serve(folder: PathBuf) -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let folder = folder.clone();
            std::thread::spawn(move || answer(stream, &folder));
        }
    });
    Ok(port)
}

fn describe(contract: &Value) -> String {
    let files = contract
        .get("files")
        .and_then(Value::as_array)
        .map_or_else(|| "undefined".to_owned(), |a| a.len().to_string());
    let workflows = contract
        .get("workflows")
        .and_then(Value::as_object)
        .map_or(0, serde_json::Map::len);
    format!("({files} files, {workflows} workflows)")
}

/// Builds, serves and compares. Exit 1 when a file is missing from the zip or a packaged contract differs from the folder one.
pub fn packaged(
    tree: &Path,
    binary: &Path,
    work: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let _ = binary;
    match run(tree, work, out, err) {
        Ok(problems) => i32::from(problems),
        Err(message) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}

fn run(tree: &Path, work: &Path, out: &mut dyn Write, err: &mut dyn Write) -> Result<bool, String> {
    let io = |path: &Path| {
        let path = path.to_owned();
        move |e: std::io::Error| format!("{}: {e}", path.display())
    };
    let _ = std::fs::remove_dir_all(work);
    std::fs::create_dir_all(work).map_err(io(work))?;
    let build = Command::new("pkl")
        .args(["project", "package"])
        .arg(tree.join("pkl"))
        .arg("--output-path")
        .arg(work.join("pkg"))
        .output()
        .map_err(|e| format!("pkl: {e}"))?;
    if !build.status.success() {
        let _ = writeln!(err, "{}", String::from_utf8_lossy(&build.stderr));
        return Ok(true);
    }
    let project_path = tree.join("pkl/PklProject");
    let project = std::fs::read_to_string(&project_path).map_err(io(&project_path))?;
    let version = regex::Regex::new(r#"version = "([^"]+)""#)
        .expect("valid")
        .captures(&project)
        .map(|m| m[1].to_owned())
        .ok_or("PklProject has no version line")?;
    let zip = work
        .join("pkg")
        .join(format!("pantheon-scaffold@{version}.zip"));

    // (1) The zip lists every file of pkl/.
    let mut on_disk: Vec<String> = walk(&tree.join("pkl"), &tree.join("pkl"))
        .into_iter()
        .filter(|p| p != "PklProject" && p != "PklProject.deps.json")
        .collect();
    sort_like_javascript(&mut on_disk);
    let listing = Command::new("unzip")
        .arg("-Z1")
        .arg(&zip)
        .output()
        .map_err(|e| format!("unzip: {e}"))?;
    let mut listed: Vec<String> = String::from_utf8_lossy(&listing.stdout)
        .split('\n')
        .filter(|l| !l.is_empty() && !l.ends_with('/'))
        .map(str::to_owned)
        .collect();
    sort_like_javascript(&mut listed);
    let missing: Vec<&String> = on_disk.iter().filter(|p| !listed.contains(p)).collect();
    let extra = listed.iter().filter(|p| !on_disk.contains(p)).count();
    let say =
        |out: &mut dyn Write, line: String| writeln!(out, "{line}").map_err(|e| e.to_string());
    say(
        out,
        format!(
            "zip: {} entries, {} files on disk, {} missing from the zip, {extra} extra",
            listed.len(),
            on_disk.len(),
            missing.len()
        ),
    )?;
    if !missing.is_empty() {
        say(
            out,
            format!(
                "  missing: {}",
                missing
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )?;
    }

    // (2) Packaged contract equals folder contract.
    let base = format!(
        "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{version}/pantheon-scaffold@{version}"
    );
    let port = serve(work.join("pkg"))?;
    let sets = [
        (
            "skills-binary-composite-all",
            "skillName = \"demo-skill\"\n  crateName = \"demo\"\n  skills = true\n  binary = true\n  compositeAction = true\n  features = \"all\"",
        ),
        ("library", "skills = false\n  binary = false"),
    ];
    let mut problems = false;
    for (name, body) in sets {
        let dir = work.join(name);
        std::fs::create_dir_all(&dir).map_err(io(&dir))?;
        let common = format!("toolchain = \"1.96.1\"\n  runnerVersion = \"0.1.9\"\n  {body}");
        let composition = "workflows { ...Builtins.standardWorkflows(options) }\nfiles {\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles(options)\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}\n";
        let tree_text = tree.display();
        std::fs::write(dir.join("scaffold.pkl"), format!("amends \"{base}#/Config.pkl\"\nimport \"{base}#/Builtins.pkl\"\noptions {{\n  {common}\n}}\n{composition}")).map_err(io(&dir))?;
        std::fs::write(dir.join("folder.pkl"), format!("amends \"file://{tree_text}/pkl/Config.pkl\"\nimport \"file://{tree_text}/pkl/Builtins.pkl\"\noptions {{\n  {common}\n}}\n{composition}")).map_err(io(&dir))?;
        let packaged = Command::new("pkl")
            .args(["eval", "-f", "json", "--cache-dir"])
            .arg(dir.join("cache"))
            .args([
                "--http-rewrite",
                &format!("https://github.com/=http://localhost:{port}/"),
                "--allowed-resources",
                "package:,https:,http:",
            ])
            .arg(dir.join("scaffold.pkl"))
            .output()
            .map_err(|e| format!("pkl: {e}"))?;
        if !packaged.status.success() {
            say(
                out,
                format!(
                    "{name}: packaged render FAILED\n{}",
                    String::from_utf8_lossy(&packaged.stderr)
                ),
            )?;
            problems = true;
            continue;
        }
        let folder = Command::new("pkl")
            .args(["eval", "-f", "json", "--allowed-resources", "file:"])
            .arg(dir.join("folder.pkl"))
            .output()
            .map_err(|e| format!("pkl: {e}"))?;
        if !folder.status.success() {
            say(
                out,
                format!(
                    "{name}: folder render FAILED\n{}",
                    String::from_utf8_lossy(&folder.stderr)
                ),
            )?;
            problems = true;
            continue;
        }
        let parse = |bytes: &[u8]| -> Result<Value, String> {
            let mut value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
            if let Some(object) = value.as_object_mut() {
                object.shift_remove("packageUri");
            }
            Ok(value)
        };
        let (a, b) = (parse(&packaged.stdout)?, parse(&folder.stdout)?);
        let same = serde_json::to_string(&a).ok() == serde_json::to_string(&b).ok();
        say(
            out,
            format!(
                "{name}: packaged contract {} the folder contract {}",
                if same { "equals" } else { "DIFFERS from" },
                describe(&a)
            ),
        )?;
        problems |= !same;
    }
    Ok(!missing.is_empty() || problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_percent_escape_is_decoded_and_anything_else_is_left_alone() {
        assert_eq!(
            percent_decode("pantheon-scaffold%400.1.0.zip"),
            "pantheon-scaffold@0.1.0.zip"
        );
        assert_eq!(percent_decode("a%2Fb%20c"), "a/b c");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("%4"), "%4");
    }

    #[test]
    fn a_contract_is_described_by_its_files_and_workflows() {
        assert_eq!(
            describe(&serde_json::json!({ "files": [1, 2], "workflows": { "a": 1 } })),
            "(2 files, 1 workflows)"
        );
        assert_eq!(
            describe(&serde_json::json!({})),
            "(undefined files, 0 workflows)"
        );
    }

    #[test]
    fn the_server_answers_a_file_by_the_last_segment_of_the_path_and_a_404_for_the_rest() {
        let folder = tempfile::tempdir().unwrap();
        std::fs::write(
            folder.path().join("pantheon-scaffold@1.0.0.zip"),
            b"zip bytes",
        )
        .unwrap();
        let port = serve(folder.path().to_path_buf()).unwrap();
        let get = |path: &str| {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let mut text = Vec::new();
            stream.read_to_end(&mut text).unwrap();
            String::from_utf8_lossy(&text).into_owned()
        };
        let found = get("/a/b/pantheon-scaffold%401.0.0.zip?x=1");
        assert!(found.starts_with("HTTP/1.1 200 OK\r\n"), "{found}");
        assert!(
            found.contains("Content-Length: 9\r\n") && found.ends_with("zip bytes"),
            "{found}"
        );
        assert!(get("/missing.zip").starts_with("HTTP/1.1 404 Not Found\r\n"));
    }
}
