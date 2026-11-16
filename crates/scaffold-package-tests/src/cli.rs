//! The command line of the runner, run in process.

use std::path::Path;

/// What a run printed and its exit code. The text is trimmed.
pub struct Out {
    pub code: i32,
    pub out: String,
    pub err: String,
}

/// Runs `pantheon-scaffold <args>` from a directory, through the same function the binary calls.
pub fn run(args: &[&str], cwd: &Path) -> Out {
    let argv: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = scaffold_cli::run(&argv, cwd, &mut out, &mut err);
    Out {
        code,
        out: String::from_utf8_lossy(&out).trim().to_owned(),
        err: String::from_utf8_lossy(&err).trim().to_owned(),
    }
}
