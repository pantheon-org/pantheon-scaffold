//! The baseline scripts of `runner/scripts/builtins-baseline/`: `snapshot`, `compare`, `freeze`, `packaged` and `check` (which snapshots
//! the current directory and compares it with a baseline, as `check.sh` does).

pub mod compare;
pub mod freeze;
pub mod packaged;
pub mod snapshot;

#[cfg(test)]
mod tests;

use std::io::Write;
use std::path::Path;

use crate::json::resolve;

pub const USAGE: &str = "usage: scaffold-release baseline snapshot <tree root> <binary> <out dir> [--api old|new]\n       scaffold-release baseline compare <baseline dir or frozen baseline.json> <candidate dir>\n       scaffold-release baseline freeze <snapshot dir> > baseline.json\n       scaffold-release baseline packaged <tree root> <binary> <work dir>\n       scaffold-release baseline check <baseline> <binary> [work dir] [--api old|new]";

const CHECK_USAGE: &str = "usage: scaffold-release baseline check <baseline snapshot dir> <binary> [work dir] [--api old|new]";

/// Snapshots the current directory and compares it with a baseline, as `check.sh` does. The snapshot's own output goes to
/// `<work>/candidate.log`, and its last 20 lines are printed when it fails.
fn check(args: &[&str], cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let [baseline, binary, rest @ ..] = args else {
        let _ = writeln!(err, "{CHECK_USAGE}");
        return 2;
    };
    let (work, passed): (Option<&str>, &[&str]) = match rest.split_first() {
        Some((first, tail)) if *first != "--api" && !first.is_empty() => (Some(*first), tail),
        _ => (None, rest),
    };
    let tmp = match work {
        Some(dir) => resolve(dir, cwd),
        None => match tempfile::Builder::new().tempdir() {
            Ok(dir) => dir.keep(),
            Err(e) => {
                let _ = writeln!(err, "{e}");
                return 1;
            }
        },
    };
    let candidate = tmp.join("candidate");
    let _ = std::fs::remove_dir_all(&candidate);
    let mut snapshot_args = vec![".", *binary, candidate.to_str().unwrap_or("candidate")];
    snapshot_args.extend(passed);
    let parsed = match snapshot::parse_args(&snapshot_args, cwd) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 2;
        }
    };
    let (mut log_out, mut log_err) = (Vec::new(), Vec::new());
    let code = snapshot::snapshot(&parsed, &mut log_out, &mut log_err);
    let mut log = log_out;
    log.extend(log_err);
    let _ = std::fs::create_dir_all(&tmp);
    let _ = std::fs::write(tmp.join("candidate.log"), &log);
    if code != 0 {
        let text = String::from_utf8_lossy(&log);
        let lines: Vec<&str> = text.lines().collect();
        let _ = writeln!(
            out,
            "{}",
            lines[lines.len().saturating_sub(20)..].join("\n")
        );
        return 1;
    }
    compare::compare(&resolve(baseline, cwd), &candidate, out, err)
}

/// `baseline <command> ...`. Exit 2 for a usage error.
pub fn command(args: &[&str], cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let usage = |err: &mut dyn Write, message: &str| {
        let _ = writeln!(err, "{message}");
        2
    };
    match args.split_first() {
        Some((&"snapshot", rest)) => match snapshot::parse_args(rest, cwd) {
            Ok(parsed) => snapshot::snapshot(&parsed, out, err),
            Err(message) => usage(err, &message),
        },
        Some((&"compare", rest)) => match compare::parse_args(rest, cwd) {
            Ok((baseline, candidate)) => compare::compare(&baseline, &candidate, out, err),
            Err(message) => usage(err, &message),
        },
        Some((&"freeze", rest)) => freeze::freeze(rest, cwd, out, err),
        Some((&"packaged", rest)) => match packaged::parse_args(rest, cwd) {
            Ok((tree, binary, work)) => packaged::packaged(&tree, &binary, &work, out, err),
            Err(message) => usage(err, &message),
        },
        Some((&"check", rest)) => check(rest, cwd, out, err),
        _ => usage(err, USAGE),
    }
}
