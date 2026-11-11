//! The Preset files a configuration applies (docs/presets-design.md): reading and hashing the listed ones, finding the files in
//! `presets/` that nothing applies, and noticing a Preset that changed since the last render. A Preset is an input of the render,
//! never an output, so the runner reads these files and never writes or deletes them. Paths are relative to the directory of the
//! config file.

use std::collections::BTreeMap;
use std::path::Path;

use crate::manifest::sha256;

#[derive(Debug, PartialEq, Eq)]
pub struct PresetError(pub String);

impl std::fmt::Display for PresetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for PresetError {}

/// The hash of every listed Preset, by path. A listed file that is not there, or cannot be read as text, is an error naming the
/// config that listed it.
pub fn read_listed(
    config_dir: &Path,
    config_name: &str,
    listed: &[String],
) -> Result<BTreeMap<String, String>, PresetError> {
    let mut hashes = BTreeMap::new();
    for path in listed {
        let full = config_dir.join(path);
        if !full.is_file() {
            return Err(PresetError(format!(
                "preset {path} is listed by {config_name} but the file does not exist"
            )));
        }
        let text = std::fs::read_to_string(&full)
            .map_err(|e| PresetError(format!("preset {path} could not be read: {e}")))?;
        hashes.insert(path.clone(), sha256(&text));
    }
    Ok(hashes)
}

/// The `.pkl` files under `presets/` that are not in `listed`, sorted, as paths relative to the config directory.
pub fn unused(config_dir: &Path, listed: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    collect(config_dir, "presets", &mut found);
    found.retain(|path| !listed.contains(path));
    found.sort();
    found
}

fn collect(config_dir: &Path, relative: &str, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(config_dir.join(relative)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{relative}/{name}");
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect(config_dir, &path, found);
        } else if kind.is_file() && name.ends_with(".pkl") {
            found.push(path);
        }
    }
}

/// The listed Presets whose hash differs from the one the last render recorded. A Preset the manifest does not record is not
/// reported: nothing is known about it yet, and the next render records it.
pub fn changed(
    recorded: &BTreeMap<String, String>,
    current: &BTreeMap<String, String>,
) -> Vec<String> {
    current
        .iter()
        .filter(|(path, hash)| recorded.get(*path).is_some_and(|before| before != *hash))
        .map(|(path, _)| path.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::put;

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn listed(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn a_listed_preset_is_hashed_by_its_text() {
        let d = dir();
        put(d.path(), "presets/ci.pkl", "function f() = 1\n");
        let hashes = read_listed(d.path(), "pantheon.pkl", &listed(&["presets/ci.pkl"])).unwrap();
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes["presets/ci.pkl"], sha256("function f() = 1\n"));
    }

    #[test]
    fn no_listed_presets_is_an_empty_map() {
        assert!(
            read_listed(dir().path(), "pantheon.pkl", &[])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_missing_preset_names_the_file_and_the_config_that_listed_it() {
        let d = dir();
        assert_eq!(
            read_listed(d.path(), "conf.pkl", &listed(&["presets/gone.pkl"]))
                .unwrap_err()
                .to_string(),
            "preset presets/gone.pkl is listed by conf.pkl but the file does not exist"
        );
    }

    #[test]
    fn a_directory_where_a_preset_should_be_is_missing_not_unreadable() {
        let d = dir();
        std::fs::create_dir_all(d.path().join("presets/ci.pkl")).unwrap();
        let message = read_listed(d.path(), "p.pkl", &listed(&["presets/ci.pkl"]))
            .unwrap_err()
            .to_string();
        assert!(
            message.ends_with("but the file does not exist"),
            "{message}"
        );
    }

    #[test]
    fn a_preset_that_is_not_text_is_an_error_naming_it() {
        let d = dir();
        std::fs::create_dir_all(d.path().join("presets")).unwrap();
        std::fs::write(d.path().join("presets/ci.pkl"), [0xff, 0xfe, 0x00]).unwrap();
        let message = read_listed(d.path(), "p.pkl", &listed(&["presets/ci.pkl"]))
            .unwrap_err()
            .to_string();
        assert!(
            message.starts_with("preset presets/ci.pkl could not be read: "),
            "{message}"
        );
    }

    #[test]
    fn the_first_missing_preset_in_the_list_order_is_the_one_reported() {
        let d = dir();
        put(d.path(), "presets/b.pkl", "x");
        let message = read_listed(
            d.path(),
            "p.pkl",
            &listed(&["presets/a.pkl", "presets/b.pkl", "presets/c.pkl"]),
        )
        .unwrap_err()
        .to_string();
        assert!(message.contains("presets/a.pkl"), "{message}");
    }

    #[test]
    fn unused_finds_pkl_files_under_presets_that_nothing_lists_sorted() {
        let d = dir();
        put(d.path(), "presets/used.pkl", "x");
        put(d.path(), "presets/zebra.pkl", "x");
        put(d.path(), "presets/deep/er/apple.pkl", "x");
        put(d.path(), "presets/notes.md", "x");
        put(d.path(), "other/outside.pkl", "x");
        assert_eq!(
            unused(d.path(), &listed(&["presets/used.pkl"])),
            ["presets/deep/er/apple.pkl", "presets/zebra.pkl"]
        );
    }

    #[test]
    fn unused_is_empty_without_a_presets_directory_or_when_everything_is_listed() {
        let d = dir();
        assert!(unused(d.path(), &[]).is_empty());
        put(d.path(), "presets/a.pkl", "x");
        assert!(unused(d.path(), &listed(&["presets/a.pkl"])).is_empty());
    }

    #[test]
    fn a_directory_named_like_a_pkl_file_is_not_a_preset() {
        let d = dir();
        std::fs::create_dir_all(d.path().join("presets/odd.pkl")).unwrap();
        assert!(unused(d.path(), &[]).is_empty());
    }

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn changed_reports_a_recorded_preset_whose_hash_differs() {
        let recorded = map(&[("presets/a.pkl", "1"), ("presets/b.pkl", "2")]);
        let current = map(&[("presets/a.pkl", "1"), ("presets/b.pkl", "9")]);
        assert_eq!(changed(&recorded, &current), ["presets/b.pkl"]);
    }

    #[test]
    fn changed_says_nothing_of_a_preset_the_manifest_does_not_record_or_one_no_longer_listed() {
        let recorded = map(&[("presets/gone.pkl", "1")]);
        let current = map(&[("presets/new.pkl", "2")]);
        assert!(changed(&recorded, &current).is_empty());
        assert!(changed(&BTreeMap::new(), &current).is_empty());
    }
}
