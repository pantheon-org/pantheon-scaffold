//! The write lock in `.pkl-gen/lock`, ported from `runner/src/lock.ts`.

use std::path::{Path, PathBuf};
use std::time::Duration;

pub const LOCK_PATH: &str = ".pkl-gen/lock";
pub const STALE_AFTER: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, PartialEq, Eq)]
pub struct LockError(pub String);

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for LockError {}

pub struct LockOptions {
    /// On CI only the age of a lock counts, because the process id of another job means nothing.
    pub ci: bool,
    pub stale_after: Duration,
}

impl Default for LockOptions {
    fn default() -> Self {
        Self {
            ci: std::env::var_os("CI").is_some_and(|v| !v.is_empty()),
            stale_after: STALE_AFTER,
        }
    }
}

/// Held while the runner writes. Dropping it, or calling `release`, removes the lock file.
pub struct Lock {
    path: PathBuf,
}

impl Lock {
    pub fn release(self) {}
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Whether a process with this id exists. `ps` answers for every user's processes, unlike a signal check, which cannot
/// see a process owned by someone else without an unsafe call.
fn alive(pid: u32) -> bool {
    std::process::Command::new("ps")
        .args(["-p", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn iso(time: std::time::SystemTime) -> String {
    use time::OffsetDateTime;
    let format = time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
    );
    OffsetDateTime::from(time)
        .format(&format)
        .expect("a time formats")
}

/// The holder of the lock file: its process id (when readable) and when it started.
fn holder(path: &Path) -> (Option<u32>, std::time::SystemTime) {
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};
    let parsed = std::fs::read_to_string(path).ok().and_then(|text| {
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        let pid = u32::try_from(json.get("pid")?.as_u64()?).ok()?;
        let started = OffsetDateTime::parse(json.get("started")?.as_str()?, &Rfc3339).ok()?;
        Some((pid, std::time::SystemTime::from(started)))
    });
    match parsed {
        Some((pid, started)) => (Some(pid), started),
        None => {
            let modified = std::fs::metadata(path)
                .and_then(|m| m.modified())
                .unwrap_or_else(|_| std::time::SystemTime::now());
            (None, modified)
        }
    }
}

/// Strictly older: a lock exactly `stale_after` old is still held.
fn is_older_than(
    now: std::time::SystemTime,
    started: std::time::SystemTime,
    stale_after: Duration,
) -> bool {
    now.duration_since(started)
        .is_ok_and(|age| age > stale_after)
}

/// Takes the write lock. Locally a lock is stale when its process is gone or it is older than `stale_after`. On CI only
/// the age counts. `check` never takes the lock, so checks run concurrently.
pub fn acquire_lock(root: &Path, opts: &LockOptions) -> Result<Lock, LockError> {
    use std::io::Write;
    let path = root.join(LOCK_PATH);
    let io = |e: std::io::Error| LockError(format!("{LOCK_PATH} could not be written: {e}"));
    std::fs::create_dir_all(root.join(".pkl-gen")).map_err(io)?;
    for _ in 0..2 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                let started = iso(std::time::SystemTime::now());
                write!(
                    file,
                    "{{\"pid\":{},\"started\":\"{started}\"}}",
                    std::process::id()
                )
                .map_err(io)?;
                return Ok(Lock { path });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io(e)),
        }
        if !path.exists() {
            continue;
        }
        let (pid, started) = holder(&path);
        let old = is_older_than(std::time::SystemTime::now(), started, opts.stale_after);
        let gone = !opts.ci && pid.is_some_and(|pid| !alive(pid));
        if !(old || gone) {
            let by = pid.map_or_else(String::new, |pid| format!(" by process {pid}"));
            return Err(LockError(format!(
                "{LOCK_PATH} is held{by} since {}, another run is writing",
                iso(started)
            )));
        }
        let _ = std::fs::remove_file(&path);
    }
    Err(LockError(format!(
        "{LOCK_PATH} could not be taken, another run won the race"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::put;
    use time::OffsetDateTime;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn local() -> LockOptions {
        LockOptions {
            ci: false,
            stale_after: STALE_AFTER,
        }
    }

    fn ci() -> LockOptions {
        LockOptions {
            ci: true,
            stale_after: STALE_AFTER,
        }
    }

    /// The id of a process that has already finished.
    fn dead_pid() -> u32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    fn write_lock(root: &Path, pid: u32, started_ago: Duration) {
        let started = OffsetDateTime::now_utc() - started_ago;
        let text = started
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        put(
            root,
            LOCK_PATH,
            &format!("{{\"pid\":{pid},\"started\":\"{text}\"}}"),
        );
    }

    fn held(root: &Path) -> bool {
        root.join(LOCK_PATH).exists()
    }

    #[test]
    fn acquiring_creates_the_lock_file_with_this_process_and_releasing_removes_it() {
        let dir = tmp();
        let lock = acquire_lock(dir.path(), &local()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(LOCK_PATH)).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["pid"], std::process::id());
        assert!(json["started"].as_str().unwrap().ends_with('Z'));
        lock.release();
        assert!(!held(dir.path()));
    }

    #[test]
    fn the_default_stale_age_is_ten_minutes() {
        assert_eq!(STALE_AFTER, Duration::from_secs(600));
        assert_eq!(LockOptions::default().stale_after, STALE_AFTER);
    }

    #[test]
    fn a_lock_exactly_as_old_as_the_limit_is_not_yet_stale() {
        let start = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let limit = Duration::from_secs(10);
        assert!(!is_older_than(start + limit, start, limit));
        assert!(is_older_than(
            start + limit + Duration::from_millis(1),
            start,
            limit
        ));
        assert!(
            !is_older_than(start, start + limit, limit),
            "a start in the future is never old"
        );
    }

    #[test]
    fn a_write_failure_is_reported_as_such_not_as_a_held_lock() {
        use std::os::unix::fs::PermissionsExt;
        let uid = std::process::Command::new("id")
            .arg("-u")
            .output()
            .unwrap()
            .stdout;
        if String::from_utf8_lossy(&uid).trim() == "0" {
            return; // root ignores directory permissions, so the failure cannot be provoked
        }
        let dir = tmp();
        std::fs::create_dir(dir.path().join(".pkl-gen")).unwrap();
        std::fs::set_permissions(
            dir.path().join(".pkl-gen"),
            std::fs::Permissions::from_mode(0o555),
        )
        .unwrap();
        let message = acquire_lock(dir.path(), &local())
            .err()
            .unwrap()
            .to_string();
        std::fs::set_permissions(
            dir.path().join(".pkl-gen"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert!(
            message.starts_with(".pkl-gen/lock could not be written: "),
            "{message}"
        );
    }

    #[test]
    fn dropping_the_lock_releases_it() {
        let dir = tmp();
        drop(acquire_lock(dir.path(), &local()).unwrap());
        assert!(!held(dir.path()));
    }

    #[test]
    fn a_second_acquire_while_the_lock_is_held_fails_and_names_the_holder() {
        let dir = tmp();
        let _lock = acquire_lock(dir.path(), &local()).unwrap();
        let message = acquire_lock(dir.path(), &local())
            .err()
            .unwrap()
            .to_string();
        assert!(
            message.starts_with(&format!(
                ".pkl-gen/lock is held by process {} since 20",
                std::process::id()
            )),
            "{message}"
        );
        assert!(message.ends_with("Z, another run is writing"), "{message}");
    }

    #[test]
    fn locally_a_lock_held_by_a_process_that_no_longer_exists_is_recovered() {
        let dir = tmp();
        write_lock(dir.path(), dead_pid(), Duration::from_secs(1));
        let _lock = acquire_lock(dir.path(), &local()).unwrap();
        assert!(held(dir.path()));
    }

    #[test]
    fn locally_a_lock_held_by_a_live_process_that_is_young_is_respected() {
        let dir = tmp();
        write_lock(dir.path(), std::process::id(), Duration::from_secs(1));
        assert!(acquire_lock(dir.path(), &local()).is_err());
    }

    #[test]
    fn locally_an_old_lock_is_recovered_even_if_the_process_id_is_alive() {
        let dir = tmp();
        write_lock(dir.path(), std::process::id(), Duration::from_secs(11 * 60));
        assert!(acquire_lock(dir.path(), &local()).is_ok());
    }

    #[test]
    fn on_ci_the_process_id_is_ignored_so_a_young_lock_of_a_dead_process_is_respected() {
        let dir = tmp();
        write_lock(dir.path(), dead_pid(), Duration::from_secs(1));
        assert!(acquire_lock(dir.path(), &ci()).is_err());
    }

    #[test]
    fn on_ci_an_old_lock_is_recovered() {
        let dir = tmp();
        write_lock(dir.path(), dead_pid(), Duration::from_secs(11 * 60));
        assert!(acquire_lock(dir.path(), &ci()).is_ok());
    }

    #[test]
    fn the_stale_age_is_configurable() {
        let dir = tmp();
        write_lock(dir.path(), std::process::id(), Duration::from_secs(5));
        let opts = LockOptions {
            ci: false,
            stale_after: Duration::from_secs(1),
        };
        assert!(acquire_lock(dir.path(), &opts).is_ok());
    }

    #[test]
    fn an_unreadable_lock_file_is_never_silently_deleted_while_young() {
        let dir = tmp();
        put(dir.path(), LOCK_PATH, "not json");
        let message = acquire_lock(dir.path(), &local())
            .err()
            .unwrap()
            .to_string();
        assert!(
            message.starts_with(".pkl-gen/lock is held since "),
            "{message}"
        );
        assert!(held(dir.path()));
    }

    #[test]
    fn an_unreadable_lock_file_is_recovered_once_old() {
        let dir = tmp();
        put(dir.path(), LOCK_PATH, "not json");
        let opts = LockOptions {
            ci: false,
            stale_after: Duration::ZERO,
        };
        std::thread::sleep(Duration::from_millis(5));
        assert!(acquire_lock(dir.path(), &opts).is_ok());
    }

    #[test]
    fn a_lock_with_a_bad_date_falls_back_to_the_file_time() {
        let dir = tmp();
        put(
            dir.path(),
            LOCK_PATH,
            &format!(
                "{{\"pid\":{},\"started\":\"yesterday\"}}",
                std::process::id()
            ),
        );
        let message = acquire_lock(dir.path(), &local())
            .err()
            .unwrap()
            .to_string();
        assert!(
            message.contains("held by process") || message.contains("held since"),
            "{message}"
        );
    }
}
