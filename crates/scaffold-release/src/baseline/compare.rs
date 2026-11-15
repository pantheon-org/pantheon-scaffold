//! Compares two snapshot folders made by `snapshot`, configuration by configuration and field by field. Ported from
//! `runner/scripts/builtins-baseline/compare.ts`.
//!
//! A frozen baseline (made by `freeze`) holds the exit codes, the files and the manifest hash only, and is compared with the same fields
//! of the candidate. Exit 1 on any difference, naming the configuration, the field and the first differing line. stderr is compared
//! after the line numbers and source snippets of pkl error messages are removed (allowed to differ).

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::json::{quote, resolve, slice_utf16, sort_like_javascript};

pub const USAGE: &str = "usage: scaffold-release baseline compare <baseline dir or frozen baseline.json> <candidate dir>";

pub fn parse_args(args: &[&str], cwd: &Path) -> Result<(PathBuf, PathBuf), String> {
    match args {
        [a, b, ..] => Ok((resolve(a, cwd), resolve(b, cwd))),
        _ => Err(USAGE.to_owned()),
    }
}

/// The names of the `.json` files of a folder, sorted.
fn names(dir: &Path) -> Result<Vec<String>, String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|n| n.ends_with(".json"))
        .collect();
    sort_like_javascript(&mut found);
    Ok(found)
}

fn load(dir: &Path, file: &str) -> Result<Value, String> {
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// What may differ in a pkl error: the source snippets, line numbers and stack frames, because a builtin that moves into a folder changes
/// its module name, path and line. The error message itself is compared.
///
/// One difference from the JavaScript, harmless for text the runner prints: `$` and `.` stop at `\n` only, where JavaScript's also stop
/// at `\r`, U+2028 and U+2029.
fn loose(text: &str) -> String {
    use std::sync::LazyLock;
    static PATTERNS: LazyLock<[regex::Regex; 4]> = LazyLock::new(|| {
        [
            regex::Regex::new(r"(?m)^\s*\d+ \|.*$").expect("valid"),
            regex::Regex::new(r"(?m)^\s*at .*\(.*\.pkl, line \d+\)\s*$").expect("valid"),
            regex::Regex::new(r"line \d+").expect("valid"),
            regex::Regex::new(r"(?m)^\s*\^+\s*$").expect("valid"),
        ]
    });
    let [snippets, frames, lines, carets] = &*PATTERNS;
    let text = snippets.replace_all(text, regex::NoExpand(""));
    let text = frames.replace_all(&text, regex::NoExpand(""));
    let text = lines.replace_all(&text, regex::NoExpand("line N"));
    carets.replace_all(&text, regex::NoExpand("")).into_owned()
}

/// Where two texts first differ, line by line.
fn first_diff(x: &str, y: &str) -> String {
    let l: Vec<&str> = x.split('\n').collect();
    let m: Vec<&str> = y.split('\n').collect();
    for i in 0..l.len().max(m.len()) {
        if l.get(i) != m.get(i) {
            let show =
                |line: Option<&&str>| slice_utf16(&quote(line.copied().unwrap_or("(none)")), 300);
            return format!(
                "line {}\n    baseline:  {}\n    candidate: {}",
                i + 1,
                show(l.get(i)),
                show(m.get(i))
            );
        }
    }
    "(equal)".to_owned()
}

/// What is in the first list and not in the second, five at most, as `a | b`.
fn only_in(first: &[String], second: &[String]) -> String {
    first
        .iter()
        .filter(|v| !second.contains(v))
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ")
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn code(value: &Value, step: &str) -> String {
    value
        .get(step)
        .and_then(|s| s.get("code"))
        .map_or_else(|| "undefined".to_owned(), ToString::to_string)
}

fn text<'a>(value: &'a Value, step: &str, field: &str) -> &'a str {
    value
        .get(step)
        .and_then(|s| s.get(field))
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn sets_differ(baseline: &[String], candidate: &[String]) -> Option<String> {
    (baseline != candidate).then(|| {
        let only = |a: &[String], b: &[String]| {
            a.iter()
                .filter(|n| !b.contains(n))
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        };
        format!(
            "configuration sets differ: only in baseline {}, only in candidate {}",
            only(baseline, candidate),
            only(candidate, baseline)
        )
    })
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The problems of a candidate folder against a frozen baseline, and how many configurations were compared.
fn against_frozen(frozen: &Value, candidate: &Path) -> Result<(usize, Vec<String>), String> {
    let mut problems = Vec::new();
    let mut found: Vec<String> = names(candidate)?
        .iter()
        .map(|n| n.trim_end_matches(".json").to_owned())
        .collect();
    sort_like_javascript(&mut found);
    let mut keys: Vec<String> = frozen
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    sort_like_javascript(&mut keys);
    problems.extend(sets_differ(&keys, &found));
    for name in keys.iter().filter(|k| found.contains(k)) {
        let y = load(candidate, &format!("{name}.json"))?;
        let x = &frozen[name];
        let got: Vec<String> = ["contract", "render", "check"]
            .iter()
            .map(|s| code(&y, s))
            .collect();
        let want: Vec<String> = x
            .get("codes")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(ToString::to_string).collect())
            .unwrap_or_default();
        if want != got {
            problems.push(format!(
                "{name}: exit codes {} vs {}",
                want.join(","),
                got.join(",")
            ));
        }
        let (fx, fy) = (strings(&x["files"]), strings(&y["files"]));
        if fx != fy {
            problems.push(format!(
                "{name}: files differ: only in baseline {}; only in candidate {}",
                only_in(&fx, &fy),
                only_in(&fy, &fx)
            ));
        }
        let manifest = y.get("manifest").and_then(Value::as_str).unwrap_or("");
        if x.get("manifest").and_then(Value::as_str) != Some(sha256_hex(manifest).as_str()) {
            problems.push(format!("{name}: manifest differs"));
        }
    }
    Ok((keys.len(), problems))
}

/// The problems of one snapshot folder against another, and how many configurations were compared.
fn between_folders(baseline: &Path, candidate: &Path) -> Result<(usize, Vec<String>), String> {
    let (na, nb) = (names(baseline)?, names(candidate)?);
    let mut problems = Vec::new();
    problems.extend(sets_differ(&na, &nb));
    let mut compared = 0;
    for f in na.iter().filter(|n| nb.contains(n)) {
        let (x, y) = (load(baseline, f)?, load(candidate, f)?);
        compared += 1;
        for step in ["contract", "render", "check"] {
            if code(&x, step) != code(&y, step) {
                problems.push(format!(
                    "{f}: {step} exit {} vs {}",
                    code(&x, step),
                    code(&y, step)
                ));
            }
            let (ox, oy) = (text(&x, step, "out"), text(&y, step, "out"));
            if ox != oy {
                problems.push(format!(
                    "{f}: {step} stdout differs at {}",
                    first_diff(ox, oy)
                ));
            }
            let (ex, ey) = (loose(text(&x, step, "err")), loose(text(&y, step, "err")));
            if ex != ey {
                problems.push(format!(
                    "{f}: {step} stderr differs at {}",
                    first_diff(&ex, &ey)
                ));
            }
        }
        let (fx, fy) = (strings(&x["files"]), strings(&y["files"]));
        if fx != fy {
            problems.push(format!(
                "{f}: files differ: only in baseline {}; only in candidate {}",
                only_in(&fx, &fy),
                only_in(&fy, &fx)
            ));
        }
        let (mx, my) = (
            x["manifest"].as_str().unwrap_or(""),
            y["manifest"].as_str().unwrap_or(""),
        );
        if mx != my {
            problems.push(format!("{f}: manifest differs at {}", first_diff(mx, my)));
        }
    }
    Ok((compared, problems))
}

/// Compares and prints the result. Exit 0 when nothing differs, 1 on any difference or any error.
pub fn compare(baseline: &Path, candidate: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let frozen = baseline.is_file();
    let result = if frozen {
        std::fs::read_to_string(baseline)
            .map_err(|e| format!("{}: {e}", baseline.display()))
            .and_then(|t| {
                serde_json::from_str::<Value>(&t)
                    .map_err(|e| format!("{}: {e}", baseline.display()))
            })
            .and_then(|v| against_frozen(&v, candidate))
    } else {
        between_folders(baseline, candidate)
    };
    let (compared, problems) = match result {
        Ok(found) => found,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 1;
        }
    };
    let _ = if frozen {
        writeln!(
            out,
            "{compared} configurations compared with the frozen baseline, {} differences",
            problems.len()
        )
    } else {
        writeln!(
            out,
            "{compared} configurations compared, {} differences",
            problems.len()
        )
    };
    if problems.is_empty() {
        return 0;
    }
    let _ = writeln!(err, "{}", problems.join("\n"));
    1
}
