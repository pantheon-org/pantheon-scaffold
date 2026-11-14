//! The name of a repository's configuration file, and the one error for the name it had before.

use std::path::Path;

/// The configuration file of a repository, the file the runner renders from.
pub const CONFIG_NAME: &str = "scaffold.pkl";

/// The name until the rename. It is kept only to refuse it with a clear message.
const OLD_CONFIG_NAME: &str = "pantheon.pkl";

/// The one error: names the new file and points at the upgrade note.
fn renamed() -> String {
    format!(
        "{OLD_CONFIG_NAME} was renamed to {CONFIG_NAME}: rename the file (see docs/adopting.md, Upgrading)."
    )
}

/// Refuses a config path whose file name is the old one. The match is exact and case-sensitive, and a directory of that name
/// is not a config file, so it is let through.
pub fn refuse_old_name(path: &Path) -> Result<(), String> {
    if path.file_name().is_some_and(|n| n == OLD_CONFIG_NAME) && !path.is_dir() {
        return Err(renamed());
    }
    Ok(())
}

/// Refuses a root that holds the old config file and not the new one, for a command that looks for the config in the root.
pub fn refuse_old_name_in_root(root: &Path) -> Result<(), String> {
    if !root.join(CONFIG_NAME).exists() && root.join(OLD_CONFIG_NAME).is_file() {
        return Err(renamed());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MESSAGE: &str = "pantheon.pkl was renamed to scaffold.pkl: rename the file (see docs/adopting.md, Upgrading).";

    #[test]
    fn a_path_named_pantheon_pkl_is_refused_with_the_one_message() {
        assert_eq!(
            refuse_old_name(Path::new("pantheon.pkl")),
            Err(MESSAGE.into())
        );
    }

    #[test]
    fn a_nested_path_with_the_old_file_name_is_refused_too() {
        assert_eq!(
            refuse_old_name(Path::new("conf/pantheon.pkl")),
            Err(MESSAGE.into())
        );
    }

    #[test]
    fn the_new_name_and_other_names_are_let_through() {
        for name in [
            "scaffold.pkl",
            "conf/other.pkl",
            "pantheon.pkl.bak",
            "old-pantheon.pkl",
            "Pantheon.pkl",
        ] {
            assert_eq!(refuse_old_name(Path::new(name)), Ok(()), "{name}");
        }
    }

    #[test]
    fn a_directory_named_pantheon_pkl_is_not_a_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().join("pantheon.pkl");
        std::fs::create_dir(&d).unwrap();
        assert_eq!(refuse_old_name(&d), Ok(()));
    }

    #[test]
    fn a_root_with_only_the_old_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pantheon.pkl"), "x").unwrap();
        assert_eq!(refuse_old_name_in_root(dir.path()), Err(MESSAGE.into()));
    }

    #[test]
    fn a_root_with_both_files_or_neither_is_let_through() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(refuse_old_name_in_root(dir.path()), Ok(()));
        std::fs::write(dir.path().join("pantheon.pkl"), "x").unwrap();
        std::fs::write(dir.path().join("scaffold.pkl"), "y").unwrap();
        assert_eq!(refuse_old_name_in_root(dir.path()), Ok(()));
    }

    #[test]
    fn a_root_where_the_old_name_is_a_directory_is_let_through() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("pantheon.pkl")).unwrap();
        assert_eq!(refuse_old_name_in_root(dir.path()), Ok(()));
    }
}
