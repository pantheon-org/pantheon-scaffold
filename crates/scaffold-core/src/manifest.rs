//! The manifest the runner keeps in `.pkl-gen/manifest.json`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::contract::FileClass;

pub const MANIFEST_VERSION: u32 = 1;
pub const TOOL_DIR: &str = ".pkl-gen";
pub const MANIFEST_PATH: &str = ".pkl-gen/manifest.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub class: FileClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub remove_on_detach: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub package_version: Option<String>,
    pub files: BTreeMap<String, ManifestEntry>,
    /// The Preset files the last render applied, by path, with the hash of each: inputs, never written by the runner.
    pub presets: BTreeMap<String, String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ManifestError(pub String);

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ManifestError {}

impl Manifest {
    pub fn empty() -> Self {
        Self {
            package_version: None,
            files: BTreeMap::new(),
            presets: BTreeMap::new(),
        }
    }
}

pub fn sha256(text: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The files map, written in the order JavaScript's `Array.sort` gives (UTF-16 code units), so the text matches the
/// TypeScript runner's byte for byte.
struct Files<'a>(&'a BTreeMap<String, ManifestEntry>);

impl Serialize for Files<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut entries: Vec<_> = self.0.iter().collect();
        entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
        let mut map = s.serialize_map(Some(entries.len()))?;
        for (path, entry) in entries {
            map.serialize_entry(path, entry)?;
        }
        map.end()
    }
}

/// The presets map, in the same order as the files map.
struct Presets<'a>(&'a BTreeMap<String, String>);

impl Serialize for Presets<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut entries: Vec<_> = self.0.iter().collect();
        entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
        let mut map = s.serialize_map(Some(entries.len()))?;
        for (path, hash) in entries {
            map.serialize_entry(path, hash)?;
        }
        map.end()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document<'a> {
    manifest_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_version: Option<&'a str>,
    files: Files<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presets: Option<Presets<'a>>,
}

/// Deterministic text: sorted keys, two-space indent, one trailing newline, so a merge conflict stays small.
pub fn serialise_manifest(m: &Manifest) -> String {
    let document = Document {
        manifest_version: MANIFEST_VERSION,
        package_version: m.package_version.as_deref(),
        files: Files(&m.files),
        presets: (!m.presets.is_empty()).then_some(Presets(&m.presets)),
    };
    let mut text = serde_json::to_string_pretty(&document).expect("a manifest always serialises");
    text.push('\n');
    text
}

fn fail<T>(message: impl AsRef<str>) -> Result<T, ManifestError> {
    Err(ManifestError(format!(
        "{MANIFEST_PATH} {}",
        message.as_ref()
    )))
}

/// Reads the text of a manifest. The checks run in the TypeScript runner's order, so the first problem reported is the
/// same one. An entry that is not a `{class, hash?, removeOnDetach?}` object is also refused, which the TypeScript
/// runner does not check.
pub fn parse_manifest(text: &str) -> Result<Manifest, ManifestError> {
    use serde_json::Value;
    let Ok(parsed) = serde_json::from_str::<Value>(text) else {
        return fail("is not valid JSON (a merge conflict? run with --repair)");
    };
    let version = parsed.get("manifestVersion");
    if !version.is_some_and(|v| v.as_f64() == Some(f64::from(MANIFEST_VERSION))) {
        let shown = version.map_or_else(|| "undefined".to_owned(), Value::to_string);
        return fail(format!(
            "has manifestVersion {shown}, this runner supports {MANIFEST_VERSION}"
        ));
    }
    let Some(Value::Object(files)) = parsed.get("files") else {
        return fail("has no files map");
    };
    let package_version = match parsed.get("packageVersion") {
        None => None,
        Some(Value::String(v)) => Some(v.clone()),
        Some(_) => return fail("has a packageVersion that is not text"),
    };
    let presets = match parsed.get("presets") {
        None => BTreeMap::new(),
        Some(Value::Object(map)) => {
            let mut presets = BTreeMap::new();
            for (path, hash) in map {
                let Value::String(hash) = hash else {
                    return fail(format!("has a hash for preset {path:?} that is not text"));
                };
                presets.insert(path.clone(), hash.clone());
            }
            presets
        }
        Some(_) => return fail("has a presets map that is not an object"),
    };
    let mut entries = BTreeMap::new();
    for (path, value) in files {
        match serde_json::from_value::<ManifestEntry>(value.clone()) {
            Ok(entry) => entries.insert(path.clone(), entry),
            Err(_) => return fail(format!("has an invalid entry for {path:?}")),
        };
    }
    Ok(Manifest {
        package_version,
        files: entries,
        presets,
    })
}

/// A missing manifest reads as empty.
pub fn read_manifest(root: &Path) -> Result<Manifest, ManifestError> {
    let path = root.join(MANIFEST_PATH);
    if !path.exists() {
        return Ok(Manifest::empty());
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_manifest(&text),
        Err(e) => fail(format!("could not be read: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::FileClass;

    fn entry(class: FileClass, hash: Option<&str>) -> ManifestEntry {
        ManifestEntry {
            class,
            hash: hash.map(str::to_owned),
            remove_on_detach: false,
        }
    }

    #[test]
    fn serialisation_is_sorted_indented_and_ends_with_one_newline() {
        let mut m = Manifest::empty();
        m.files
            .insert("b".into(), entry(FileClass::Owned, Some("2")));
        m.files.insert("a".into(), entry(FileClass::External, None));
        assert_eq!(
            serialise_manifest(&m),
            "{\n  \"manifestVersion\": 1,\n  \"files\": {\n    \"a\": {\n      \"class\": \"external\"\n    },\n    \"b\": {\n      \"class\": \"owned\",\n      \"hash\": \"2\"\n    }\n  }\n}\n"
        );
    }

    #[test]
    fn serialising_twice_gives_the_same_text() {
        let mut m = Manifest::empty();
        m.files
            .insert("x".into(), entry(FileClass::Seed, Some("h")));
        let once = serialise_manifest(&m);
        assert_eq!(serialise_manifest(&parse_manifest(&once).unwrap()), once);
    }

    #[test]
    fn the_package_version_and_the_detach_flag_are_kept_and_the_version_comes_right_after_the_manifest_version()
     {
        let mut m = Manifest::empty();
        m.package_version = Some("0.2.0".into());
        m.files.insert(
            "w.yml".into(),
            ManifestEntry {
                class: FileClass::Owned,
                hash: Some("h".into()),
                remove_on_detach: true,
            },
        );
        let text = serialise_manifest(&m);
        assert_eq!(
            text,
            "{\n  \"manifestVersion\": 1,\n  \"packageVersion\": \"0.2.0\",\n  \"files\": {\n    \"w.yml\": {\n      \"class\": \"owned\",\n      \"hash\": \"h\",\n      \"removeOnDetach\": true\n    }\n  }\n}\n"
        );
        assert_eq!(parse_manifest(&text).unwrap(), m);
    }

    #[test]
    fn an_empty_manifest_serialises_with_an_empty_files_map() {
        assert_eq!(
            serialise_manifest(&Manifest::empty()),
            "{\n  \"manifestVersion\": 1,\n  \"files\": {}\n}\n"
        );
    }

    #[test]
    fn keys_sort_by_utf16_code_unit_like_the_typescript_runner() {
        // U+FF5E is one UTF-16 unit (0xFF5E); U+1F600 is a surrogate pair (0xD83D first). JavaScript puts U+1F600 first,
        // while a byte-wise sort (UTF-8) puts it last.
        let mut m = Manifest::empty();
        m.files
            .insert("\u{FF5E}".into(), entry(FileClass::Seed, None));
        m.files
            .insert("\u{1F600}".into(), entry(FileClass::Seed, None));
        let text = serialise_manifest(&m);
        assert!(text.find('\u{1F600}').unwrap() < text.find('\u{FF5E}').unwrap());
    }

    #[test]
    fn a_missing_manifest_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_manifest(dir.path()).unwrap(), Manifest::empty());
    }

    #[test]
    fn a_manifest_round_trips_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = Manifest::empty();
        m.files
            .insert("a".into(), entry(FileClass::Owned, Some(&sha256("x"))));
        std::fs::create_dir_all(dir.path().join(TOOL_DIR)).unwrap();
        std::fs::write(dir.path().join(MANIFEST_PATH), serialise_manifest(&m)).unwrap();
        assert_eq!(read_manifest(dir.path()).unwrap(), m);
    }

    fn err(text: &str) -> String {
        parse_manifest(text).unwrap_err().to_string()
    }

    #[test]
    fn an_unknown_manifest_version_is_refused() {
        assert_eq!(
            err(r#"{"manifestVersion":2,"files":{}}"#),
            ".pkl-gen/manifest.json has manifestVersion 2, this runner supports 1"
        );
    }

    #[test]
    fn a_missing_manifest_version_is_refused_as_undefined() {
        assert_eq!(
            err(r#"{"files":{}}"#),
            ".pkl-gen/manifest.json has manifestVersion undefined, this runner supports 1"
        );
    }

    #[test]
    fn a_manifest_with_merge_conflict_markers_points_at_repair() {
        assert_eq!(
            err("<<<<<<< HEAD\n{}\n=======\n{}\n>>>>>>> x\n"),
            ".pkl-gen/manifest.json is not valid JSON (a merge conflict? run with --repair)"
        );
    }

    #[test]
    fn a_manifest_without_a_files_map_is_refused() {
        assert_eq!(
            err(r#"{"manifestVersion":1}"#),
            ".pkl-gen/manifest.json has no files map"
        );
        assert_eq!(
            err(r#"{"manifestVersion":1,"files":null}"#),
            ".pkl-gen/manifest.json has no files map"
        );
    }

    #[test]
    fn a_package_version_that_is_not_a_string_is_refused() {
        assert_eq!(
            err(r#"{"manifestVersion":1,"packageVersion":3,"files":{}}"#),
            ".pkl-gen/manifest.json has a packageVersion that is not text"
        );
    }

    #[test]
    fn sha256_matches_a_known_value() {
        assert_eq!(
            sha256("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn presets_are_written_after_the_files_sorted_and_read_back() {
        let mut m = Manifest::empty();
        m.presets.insert("presets/b.pkl".into(), "2".into());
        m.presets.insert("presets/a.pkl".into(), "1".into());
        let text = serialise_manifest(&m);
        assert_eq!(
            text,
            "{\n  \"manifestVersion\": 1,\n  \"files\": {},\n  \"presets\": {\n    \"presets/a.pkl\": \"1\",\n    \"presets/b.pkl\": \"2\"\n  }\n}\n"
        );
        assert_eq!(parse_manifest(&text).unwrap(), m);
    }

    #[test]
    fn a_manifest_with_no_presets_has_no_presets_key_and_reads_as_none() {
        assert!(!serialise_manifest(&Manifest::empty()).contains("presets"));
        let parsed = parse_manifest("{\"manifestVersion\":1,\"files\":{}}").unwrap();
        assert!(parsed.presets.is_empty());
    }

    #[test]
    fn preset_paths_sort_by_utf16_code_unit_like_the_files_do() {
        let mut m = Manifest::empty();
        m.presets.insert("presets/\u{FF5E}.pkl".into(), "x".into());
        m.presets.insert("presets/\u{1F600}.pkl".into(), "y".into());
        let text = serialise_manifest(&m);
        assert!(
            text.find("\u{1F600}").unwrap() < text.find("\u{FF5E}").unwrap(),
            "{text}"
        );
    }

    #[test]
    fn a_presets_value_that_is_not_a_map_of_text_is_refused() {
        assert_eq!(
            parse_manifest("{\"manifestVersion\":1,\"files\":{},\"presets\":[]}")
                .unwrap_err()
                .to_string(),
            ".pkl-gen/manifest.json has a presets map that is not an object"
        );
        assert_eq!(
            parse_manifest(
                "{\"manifestVersion\":1,\"files\":{},\"presets\":{\"presets/a.pkl\":3}}"
            )
            .unwrap_err()
            .to_string(),
            ".pkl-gen/manifest.json has a hash for preset \"presets/a.pkl\" that is not text"
        );
    }
}
