//! Helpers the integration tests share.

use std::path::Path;
use std::process::Command;

/// Makes a script that was just written ready to run, as `testutil::settle` in the library does (an integration test cannot reach it):
/// running a file fails with ETXTBSY while a process forked by another test thread still holds a copy of its write descriptor, so the
/// script gets a guard line that makes a probe run exit at once, and the probe is repeated until it is no longer refused.
pub fn settle(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let text = std::fs::read_to_string(path).unwrap();
    let (shebang, rest) = text.split_once('\n').expect("a shebang line");
    std::fs::write(
        path,
        format!("{shebang}\n[ -z \"${{SCRIPT_PROBE:-}}\" ] || exit 0\n{rest}"),
    )
    .unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    for _ in 0..500 {
        match Command::new(path).env("SCRIPT_PROBE", "1").status() {
            Err(e) if e.raw_os_error() == Some(26) => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            other => {
                other.unwrap();
                return;
            }
        }
    }
    panic!("{} stayed busy for five seconds", path.display());
}
