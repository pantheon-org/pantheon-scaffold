//! Turns a snapshot folder made by `snapshot` into the frozen snapshot that CI compares against: for each configuration the exit codes of
//! contract, render and check, every file written (path, mode, sha256) and the sha256 of the manifest. Messages are left out, because a
//! message may change when the runner changes; what a repository gets must not. Ported from
//! `runner/scripts/builtins-baseline/freeze.ts`.

use std::io::Write;
use std::path::Path;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::json::{resolve, sort_like_javascript, stringify_indent_1};

pub const USAGE: &str = "usage: scaffold-release baseline freeze <snapshot dir> > baseline.json";

/// The frozen text of a snapshot folder, with a line break at the end.
pub fn freeze_text(dir: &Path) -> Result<String, String> {
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|n| n.ends_with(".json"))
        .collect();
    sort_like_javascript(&mut files);
    let mut frozen = Map::new();
    for file in files {
        let path = dir.join(&file);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let x: Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let manifest = x["manifest"].as_str().unwrap_or("");
        let sha: String = Sha256::digest(manifest.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        frozen.insert(
            file.trim_end_matches(".json").to_owned(),
            json!({
                "codes": [x["contract"]["code"], x["render"]["code"], x["check"]["code"]],
                "files": x["files"],
                "manifest": sha,
            }),
        );
    }
    Ok(stringify_indent_1(&Value::Object(frozen)) + "\n")
}

/// Prints the frozen text. Exit 2 for a usage error, 1 when a file cannot be read.
pub fn freeze(args: &[&str], cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Some(dir) = args.first() else {
        let _ = writeln!(err, "{USAGE}");
        return 2;
    };
    match freeze_text(&resolve(dir, cwd)) {
        Ok(text) => {
            let _ = write!(out, "{text}");
            0
        }
        Err(message) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}
