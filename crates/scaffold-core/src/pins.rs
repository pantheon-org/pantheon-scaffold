//! Reading and writing the pins `upgrade` moves: the package version and checksum in `pantheon.pkl`, and the runner version
//! in `mise.toml`. Ported from `runner/src/pins.ts`.
//!
//! The TypeScript matches the section form of the runner's `mise.toml` entry with a look-ahead pattern, which the `regex`
//! crate does not have, so that one form is found by reading the lines. Everything else is a plain pattern.

use std::path::Path;

/// The mise tool key the runner binary is installed under.
pub const RUNNER_TOOL: &str = "github:pantheon-org/pantheon-scaffold";

#[derive(Debug, PartialEq, Eq)]
pub struct PinError(pub String);

impl std::fmt::Display for PinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for PinError {}

#[derive(Debug, PartialEq, Eq)]
pub struct Pins {
    pub package_version: String,
    pub checksum: Option<String>,
    pub runner_version: String,
}

pub struct NewPins<'a> {
    pub version: &'a str,
    pub checksum: &'a str,
}

// All pin locations live in this file, so a consumer that keeps the pin somewhere else changes one place.
fn amends_pattern() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r#"(amends "package://github\.com/pantheon-org/pantheon-scaffold/releases/download/v)([^/"]+)(/pantheon-scaffold@)([^:#"]+)(?:::sha256:([0-9a-f]{64}))?(#/Config\.pkl")"#,
        )
        .expect("valid")
    });
    &RE
}

// Every import of the same package (Builtins.pkl, Tools.pkl, ...) has to move with the amends line, or the render mixes two
// versions of the package.
fn import_pattern() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r#"(import\*? "package://github\.com/pantheon-org/pantheon-scaffold/releases/download/v)([^/"]+)(/pantheon-scaffold@)([^:#"]+)(?:::sha256:[0-9a-f]{64})?(#/[^"]+")"#,
        )
        .expect("valid")
    });
    &RE
}

/// Where the runner's version sits in `mise.toml`: the span of the version text.
struct ToolHit {
    version: std::ops::Range<usize>,
}

fn tool_string_pattern() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(&format!(
            r#"(?m)^(\s*"{}"\s*=\s*")([^"]+)(")"#,
            regex::escape(RUNNER_TOOL)
        ))
        .expect("valid")
    });
    &RE
}

fn tool_table_pattern() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(&format!(
            r#"(?m)^(\s*"{}"\s*=\s*\{{[^}}\n]*?\bversion\s*=\s*")([^"]+)(")"#,
            regex::escape(RUNNER_TOOL)
        ))
        .expect("valid")
    });
    &RE
}

/// The section form mise writes when a tool has options: `[tools."..."]` followed by lines that do not open another
/// section, one of them the version. The first version line that has a value is the one. Found by reading the lines,
/// because the TypeScript pattern for it uses a look-ahead.
fn section_hits(mise: &str) -> Vec<ToolHit> {
    let header = regex::Regex::new(&format!(
        r#"^\[tools\."{}"\][ \t]*$"#,
        regex::escape(RUNNER_TOOL)
    ))
    .expect("valid");
    let version_line = regex::Regex::new(r#"^[ \t]*version[ \t]*=[ \t]*""#).expect("valid");
    let mut lines = Vec::new();
    let mut offset = 0;
    for line in mise.split('\n') {
        lines.push((offset, line));
        offset += line.len() + 1;
    }
    let mut hits = Vec::new();
    for (i, (_, line)) in lines.iter().enumerate() {
        // A header with nothing after it has no version line to find, so it is not an entry.
        if !header.is_match(line) {
            continue;
        }
        for (line_start, text) in &lines[i + 1..] {
            if text.starts_with('[') {
                break;
            }
            let Some(opening) = version_line.find(text) else {
                continue;
            };
            let value_start = line_start + opening.end();
            let Some(length) = mise[value_start..].find('"').filter(|n| *n > 0) else {
                continue;
            };
            hits.push(ToolHit {
                version: value_start..value_start + length,
            });
            break;
        }
    }
    hits
}

fn tool_hits(mise: &str) -> Vec<ToolHit> {
    let from_pattern = |re: &regex::Regex| -> Vec<ToolHit> {
        re.captures_iter(mise)
            .map(|c| ToolHit {
                version: c.get(2).expect("the version group").range(),
            })
            .collect()
    };
    let mut hits = from_pattern(tool_string_pattern());
    hits.extend(from_pattern(tool_table_pattern()));
    hits.extend(section_hits(mise));
    hits
}

fn tool_match(mise: &str) -> Result<ToolHit, PinError> {
    let mut hits = tool_hits(mise);
    match hits.len() {
        0 => Err(PinError(format!(
            "mise.toml: no {RUNNER_TOOL} entry with a version"
        ))),
        1 => Ok(hits.remove(0)),
        _ => Err(PinError(format!(
            "mise.toml: more than one {RUNNER_TOOL} entry, the runner will not guess which to change"
        ))),
    }
}

fn read_text(root: &Path, file: &str) -> Result<String, PinError> {
    let path = root.join(file);
    if !path.exists() {
        return Err(PinError(format!("{file} not found in {}", root.display())));
    }
    std::fs::read_to_string(&path).map_err(|e| PinError(format!("{file}: {e}")))
}

/// The one amends line of the package. None, or more than one, is an error, so the runner never guesses which to change.
fn only_amends<'t>(text: &'t str, file: &str) -> Result<regex::Captures<'t>, PinError> {
    let what = "amends line for the pantheon-scaffold package";
    let mut hits = amends_pattern().captures_iter(text);
    match (hits.next(), hits.next()) {
        (None, _) => Err(PinError(format!("{file}: no {what}"))),
        (Some(_), Some(_)) => Err(PinError(format!(
            "{file}: more than one {what}, the runner will not guess which to change"
        ))),
        (Some(hit), None) => Ok(hit),
    }
}

pub fn read_pins(root: &Path, config: &str) -> Result<Pins, PinError> {
    let pkl = read_text(root, config)?;
    let amends = only_amends(&pkl, config)?;
    let mise = read_text(root, "mise.toml")?;
    let tool = tool_match(&mise)?;
    Ok(Pins {
        package_version: amends[4].to_owned(),
        checksum: amends.get(5).map(|m| m.as_str().to_owned()),
        runner_version: mise[tool.version.clone()].to_owned(),
    })
}

/// How many imports of the package the config has, which move with the amends line.
pub fn count_package_imports(root: &Path, config: &str) -> Result<usize, PinError> {
    Ok(import_pattern()
        .find_iter(&read_text(root, config)?)
        .count())
}

fn check_next(next: &NewPins) -> Result<(), PinError> {
    if crate::semver::parse_semver(next.version).is_none() {
        return Err(PinError(format!(
            "{} is not a version",
            serde_json::to_string(next.version).expect("a string serialises")
        )));
    }
    if next.checksum.len() != 64
        || !next
            .checksum
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(PinError("the checksum is not 64 hex characters".to_owned()));
    }
    Ok(())
}

/// The config text with the amends line and every import of the package moved to the new version and checksum.
fn moved_config(pkl: &str, next: &NewPins) -> String {
    let (version, checksum) = (next.version, next.checksum);
    let new_pkl = amends_pattern().replace_all(pkl, |c: &regex::Captures| {
        format!(
            "{}{version}{}{version}::sha256:{checksum}{}",
            &c[1], &c[3], &c[6]
        )
    });
    import_pattern()
        .replace_all(&new_pkl, |c: &regex::Captures| {
            format!(
                "{}{version}{}{version}::sha256:{checksum}{}",
                &c[1], &c[3], &c[5]
            )
        })
        .into_owned()
}

/// What the config would read after `write_pins`, without writing anything. `upgrade` evaluates it first, so a config that the
/// new package cannot evaluate (a Preset that no longer fits) is found before any file is touched.
pub fn config_after(root: &Path, config: &str, next: &NewPins) -> Result<String, PinError> {
    check_next(next)?;
    let pkl = read_text(root, config)?;
    only_amends(&pkl, config)?;
    Ok(moved_config(&pkl, next))
}

/// Rewrites both files in memory first, and writes either only when both rewrites worked.
pub fn write_pins(root: &Path, config: &str, next: &NewPins) -> Result<(), PinError> {
    check_next(next)?;
    let pkl = read_text(root, config)?;
    let mise = read_text(root, "mise.toml")?;
    only_amends(&pkl, config)?;
    let tool = tool_match(&mise)?;
    let version = next.version;
    let new_pkl = moved_config(&pkl, next);
    let new_mise = format!(
        "{}{version}{}",
        &mise[..tool.version.start],
        &mise[tool.version.end..]
    );
    std::fs::write(root.join(config), new_pkl.as_bytes())
        .map_err(|e| PinError(format!("{config}: {e}")))?;
    std::fs::write(root.join("mise.toml"), new_mise)
        .map_err(|e| PinError(format!("mise.toml: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{get, put};

    fn h(c: char) -> String {
        c.to_string().repeat(64)
    }

    fn amends(v: &str, sum: Option<&str>) -> String {
        let checksum = sum.map(|s| format!("::sha256:{s}")).unwrap_or_default();
        format!(
            "amends \"package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{v}/pantheon-scaffold@{v}{checksum}#/Config.pkl\""
        )
    }

    fn pkl(v: &str, sum: Option<&str>) -> String {
        format!(
            "{}\n\noptions {{ toolchain = \"1.96.1\" }}\n",
            amends(v, sum)
        )
    }

    fn mise(v: &str) -> String {
        format!("[tools]\nbun = \"1.4.2\"\n\"{RUNNER_TOOL}\" = \"{v}\"\npkl = \"0.32.2\"\n")
    }

    fn repo(v: &str, sum: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "pantheon.pkl", &pkl(v, sum));
        put(dir.path(), "mise.toml", &mise(v));
        dir
    }

    fn pins(root: &Path) -> Pins {
        read_pins(root, "pantheon.pkl").unwrap()
    }

    fn write(root: &Path, version: &str, checksum: &str) -> Result<(), PinError> {
        write_pins(root, "pantheon.pkl", &NewPins { version, checksum })
    }

    fn error(result: Result<Pins, PinError>) -> String {
        result.unwrap_err().to_string()
    }

    #[test]
    fn reads_the_package_version_its_checksum_and_the_runner_pin() {
        let dir = repo("0.1.0", Some(&h('a')));
        assert_eq!(
            pins(dir.path()),
            Pins {
                package_version: "0.1.0".into(),
                checksum: Some(h('a')),
                runner_version: "0.1.0".into()
            }
        );
    }

    #[test]
    fn a_file_with_no_checksum_yet_reads_as_none() {
        assert_eq!(pins(repo("0.1.0", None).path()).checksum, None);
    }

    #[test]
    fn writes_the_new_version_into_the_tag_the_file_name_and_the_runner_pin_and_the_new_checksum_touching_nothing_else()
     {
        let dir = repo("0.1.0", Some(&h('a')));
        write(dir.path(), "0.2.0", &h('c')).unwrap();
        assert_eq!(get(dir.path(), "pantheon.pkl"), pkl("0.2.0", Some(&h('c'))));
        assert_eq!(get(dir.path(), "mise.toml"), mise("0.2.0"));
    }

    #[test]
    fn a_checksum_is_added_when_there_was_none() {
        let dir = repo("0.1.0", None);
        write(dir.path(), "0.2.0", &h('c')).unwrap();
        assert!(get(dir.path(), "pantheon.pkl").contains(&format!(
            "pantheon-scaffold@0.2.0::sha256:{}#/Config.pkl",
            h('c')
        )));
    }

    #[test]
    fn a_pre_release_version_is_written_as_given() {
        let dir = repo("0.1.0", Some(&h('a')));
        write(dir.path(), "0.2.0-rc.1", &h('c')).unwrap();
        assert_eq!(pins(dir.path()).package_version, "0.2.0-rc.1");
    }

    #[test]
    fn the_table_form_of_the_runner_pin_is_rewritten_too() {
        let dir = repo("0.1.0", Some(&h('a')));
        put(
            dir.path(),
            "mise.toml",
            &format!(
                "[tools]\n\"{RUNNER_TOOL}\" = {{ version = \"0.1.0\", version_prefix = \"v\" }}\n"
            ),
        );
        write(dir.path(), "0.2.0", &h('c')).unwrap();
        assert_eq!(
            get(dir.path(), "mise.toml"),
            format!(
                "[tools]\n\"{RUNNER_TOOL}\" = {{ version = \"0.2.0\", version_prefix = \"v\" }}\n"
            )
        );
    }

    #[test]
    fn a_pantheon_pkl_with_no_amends_line_for_the_package_or_two_is_an_error_that_names_the_file() {
        let none = repo("0.1.0", None);
        put(
            none.path(),
            "pantheon.pkl",
            "amends \"../local/Config.pkl\"\n",
        );
        assert_eq!(
            error(read_pins(none.path(), "pantheon.pkl")),
            "pantheon.pkl: no amends line for the pantheon-scaffold package"
        );
        let two = repo("0.1.0", None);
        put(
            two.path(),
            "pantheon.pkl",
            &format!("{}\n{}\n", amends("0.1.0", None), amends("0.1.0", None)),
        );
        assert_eq!(
            error(read_pins(two.path(), "pantheon.pkl")),
            "pantheon.pkl: more than one amends line for the pantheon-scaffold package, the runner will not guess which to change"
        );
    }

    #[test]
    fn a_mise_toml_without_the_runner_tool_or_with_two_is_an_error() {
        let none = repo("0.1.0", None);
        put(none.path(), "mise.toml", "[tools]\nbun = \"1.4.2\"\n");
        assert_eq!(
            error(read_pins(none.path(), "pantheon.pkl")),
            format!("mise.toml: no {RUNNER_TOOL} entry with a version")
        );
        let two = repo("0.1.0", None);
        put(
            two.path(),
            "mise.toml",
            &format!("{}\"{RUNNER_TOOL}\" = \"0.1.0\"\n", mise("0.1.0")),
        );
        assert_eq!(
            error(read_pins(two.path(), "pantheon.pkl")),
            format!(
                "mise.toml: more than one {RUNNER_TOOL} entry, the runner will not guess which to change"
            )
        );
    }

    #[test]
    fn a_missing_file_is_an_error_not_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let message = error(read_pins(dir.path(), "pantheon.pkl"));
        assert_eq!(
            message,
            format!("pantheon.pkl not found in {}", dir.path().display())
        );
        let only_pkl = tempfile::tempdir().unwrap();
        put(only_pkl.path(), "pantheon.pkl", &pkl("0.1.0", None));
        assert_eq!(
            error(read_pins(only_pkl.path(), "pantheon.pkl")),
            format!("mise.toml not found in {}", only_pkl.path().display())
        );
    }

    #[test]
    fn nothing_is_written_when_one_of_the_two_files_cannot_be_rewritten() {
        let dir = repo("0.1.0", Some(&h('a')));
        put(dir.path(), "mise.toml", "[tools]\nbun = \"1.4.2\"\n");
        let before = get(dir.path(), "pantheon.pkl");
        assert!(write(dir.path(), "0.2.0", &h('c')).is_err());
        assert_eq!(get(dir.path(), "pantheon.pkl"), before);
    }

    #[test]
    fn a_version_or_checksum_that_is_not_valid_is_refused_before_anything_is_written() {
        let dir = repo("0.1.0", Some(&h('a')));
        let before = (
            get(dir.path(), "pantheon.pkl"),
            get(dir.path(), "mise.toml"),
        );
        assert_eq!(
            write(dir.path(), "latest", &h('c')),
            Err(PinError("\"latest\" is not a version".into()))
        );
        assert_eq!(
            write(dir.path(), "0.2.0", "abc"),
            Err(PinError("the checksum is not 64 hex characters".into()))
        );
        assert!(
            write(dir.path(), "0.2.0", &"A".repeat(64)).is_err(),
            "upper case hex is not accepted"
        );
        assert!(
            write(dir.path(), "0.2.0", &format!("{}\n", h('a'))).is_err(),
            "a trailing newline is not accepted"
        );
        assert_eq!(
            (
                get(dir.path(), "pantheon.pkl"),
                get(dir.path(), "mise.toml")
            ),
            before
        );
    }

    // The form mise itself writes when a tool has options: a section with a version line.
    fn section(v: &str, extra: &str) -> String {
        format!(
            "[settings]\nlockfile = true\n\n[tools]\npkl = \"0.32.2\"\n\n[tools.\"{RUNNER_TOOL}\"]\n{extra}version = \"{v}\"\n\n[env]\nX = \"1\"\nversion = \"9.9.9\"\n"
        )
    }

    #[test]
    fn the_section_form_of_the_runner_pin_is_read_and_rewritten_and_nothing_else_with_a_version_line_is_touched()
     {
        let dir = repo("0.1.0", Some(&h('a')));
        put(dir.path(), "mise.toml", &section("0.1.0", ""));
        assert_eq!(pins(dir.path()).runner_version, "0.1.0");
        write(dir.path(), "0.2.0", &h('c')).unwrap();
        assert_eq!(get(dir.path(), "mise.toml"), section("0.2.0", ""));
    }

    #[test]
    fn a_section_with_other_options_before_the_version_is_handled() {
        let dir = repo("0.1.0", Some(&h('a')));
        put(
            dir.path(),
            "mise.toml",
            &section("0.1.0", "version_prefix = \"v\"\n"),
        );
        write(dir.path(), "0.2.0", &h('c')).unwrap();
        assert_eq!(
            get(dir.path(), "mise.toml"),
            section("0.2.0", "version_prefix = \"v\"\n")
        );
    }

    #[test]
    fn the_pin_in_a_section_that_is_the_last_thing_in_the_file_is_handled() {
        let dir = repo("0.1.0", Some(&h('a')));
        put(
            dir.path(),
            "mise.toml",
            &format!("[tools.\"{RUNNER_TOOL}\"]\nversion = \"0.1.0\""),
        );
        write(dir.path(), "0.2.0", &h('c')).unwrap();
        assert_eq!(
            get(dir.path(), "mise.toml"),
            format!("[tools.\"{RUNNER_TOOL}\"]\nversion = \"0.2.0\"")
        );
    }

    #[test]
    fn a_section_header_with_trailing_blanks_and_a_version_line_with_extra_spacing_are_handled() {
        let dir = repo("0.1.0", Some(&h('a')));
        put(
            dir.path(),
            "mise.toml",
            &format!("[tools.\"{RUNNER_TOOL}\"] \t\n \tversion\t=   \"0.1.0\"\n"),
        );
        assert_eq!(pins(dir.path()).runner_version, "0.1.0");
        write(dir.path(), "0.2.0", &h('c')).unwrap();
        assert_eq!(
            get(dir.path(), "mise.toml"),
            format!("[tools.\"{RUNNER_TOOL}\"] \t\n \tversion\t=   \"0.2.0\"\n")
        );
    }

    #[test]
    fn a_version_that_appears_earlier_in_the_section_is_not_the_one_rewritten() {
        let dir = repo("0.1.0", Some(&h('a')));
        put(
            dir.path(),
            "mise.toml",
            &format!("[tools.\"{RUNNER_TOOL}\"]\n# used to be 0.1.0\nversion = \"0.1.0\"\n"),
        );
        write(dir.path(), "0.2.0", &h('c')).unwrap();
        assert_eq!(
            get(dir.path(), "mise.toml"),
            format!("[tools.\"{RUNNER_TOOL}\"]\n# used to be 0.1.0\nversion = \"0.2.0\"\n")
        );
    }

    #[test]
    fn an_empty_version_line_in_a_section_is_skipped_for_the_next_one_with_a_value() {
        let dir = repo("0.1.0", Some(&h('a')));
        put(
            dir.path(),
            "mise.toml",
            &format!("[tools.\"{RUNNER_TOOL}\"]\nversion = \"\"\nversion = \"0.1.0\"\n"),
        );
        assert_eq!(pins(dir.path()).runner_version, "0.1.0");
        write(dir.path(), "0.2.0", &h('c')).unwrap();
        assert_eq!(
            get(dir.path(), "mise.toml"),
            format!("[tools.\"{RUNNER_TOOL}\"]\nversion = \"\"\nversion = \"0.2.0\"\n")
        );
        put(
            dir.path(),
            "mise.toml",
            &format!("[tools.\"{RUNNER_TOOL}\"]\nversion = \"\"\n"),
        );
        assert!(
            error(read_pins(dir.path(), "pantheon.pkl")).contains(&format!("no {RUNNER_TOOL}"))
        );
    }

    #[test]
    fn a_section_with_no_version_line_is_an_error_and_so_is_a_section_plus_a_one_line_entry() {
        let none = repo("0.1.0", None);
        put(
            none.path(),
            "mise.toml",
            &format!(
                "[tools.\"{RUNNER_TOOL}\"]\nversion_prefix = \"v\"\n\n[env]\nversion = \"9.9.9\"\n"
            ),
        );
        assert!(
            error(read_pins(none.path(), "pantheon.pkl")).contains(&format!("no {RUNNER_TOOL}"))
        );
        let two = repo("0.1.0", None);
        put(
            two.path(),
            "mise.toml",
            &format!(
                "{}\n[tools]\n\"{RUNNER_TOOL}\" = \"0.1.0\"\n",
                section("0.1.0", "")
            ),
        );
        assert!(error(read_pins(two.path(), "pantheon.pkl")).contains("more than one"));
    }

    #[test]
    fn two_sections_for_the_runner_are_an_error() {
        let dir = repo("0.1.0", None);
        let one = format!("[tools.\"{RUNNER_TOOL}\"]\nversion = \"0.1.0\"\n");
        put(dir.path(), "mise.toml", &format!("{one}\n{one}"));
        assert!(error(read_pins(dir.path(), "pantheon.pkl")).contains("more than one"));
    }

    #[test]
    fn a_section_for_a_different_tool_is_not_mistaken_for_the_runner() {
        let dir = repo("0.1.0", None);
        put(
            dir.path(),
            "mise.toml",
            "[tools.\"github:other/tool\"]\nversion = \"1.0.0\"\n",
        );
        assert!(
            error(read_pins(dir.path(), "pantheon.pkl")).contains(&format!("no {RUNNER_TOOL}"))
        );
    }

    // A consumer that uses builtins also imports them from the package URI, and upgrade must move every one of them with the
    // amends line, or the render mixes the new Config with the old Builtins and fails.
    fn imp(file: &str, v: &str, sum: Option<&str>, keyword: &str) -> String {
        let checksum = sum.map(|s| format!("::sha256:{s}")).unwrap_or_default();
        format!(
            "{keyword} \"package://github.com/pantheon-org/pantheon-scaffold/releases/download/v{v}/pantheon-scaffold@{v}{checksum}#/{file}\""
        )
    }

    fn with_imports(v: &str, sum: Option<&str>, imports: &[String]) -> String {
        format!(
            "{}\n{}\n\noptions {{ toolchain = \"1.96.1\" }}\n",
            amends(v, sum),
            imports.join("\n")
        )
    }

    #[test]
    fn every_import_of_the_package_moves_to_the_new_version_and_checksum_with_the_amends_line_and_nothing_else_changes()
     {
        let dir = repo("0.1.0", Some(&h('a')));
        let before = [
            imp("Builtins.pkl", "0.1.0", Some(&h('a')), "import"),
            imp("Tools.pkl", "0.1.0", Some(&h('a')), "import"),
        ];
        put(
            dir.path(),
            "pantheon.pkl",
            &with_imports("0.1.0", Some(&h('a')), &before),
        );
        assert_eq!(count_package_imports(dir.path(), "pantheon.pkl"), Ok(2));
        write(dir.path(), "0.2.0", &h('c')).unwrap();
        let after = [
            imp("Builtins.pkl", "0.2.0", Some(&h('c')), "import"),
            imp("Tools.pkl", "0.2.0", Some(&h('c')), "import"),
        ];
        assert_eq!(
            get(dir.path(), "pantheon.pkl"),
            with_imports("0.2.0", Some(&h('c')), &after)
        );
    }

    #[test]
    fn an_import_star_and_an_import_with_no_checksum_yet_are_moved_too_and_get_the_checksum() {
        let dir = repo("0.1.0", Some(&h('a')));
        let before = [
            imp("Builtins.pkl", "0.1.0", None, "import"),
            imp("Pins.pkl", "0.1.0", Some(&h('a')), "import*"),
        ];
        put(
            dir.path(),
            "pantheon.pkl",
            &with_imports("0.1.0", Some(&h('a')), &before),
        );
        write(dir.path(), "0.2.0", &h('c')).unwrap();
        let after = [
            imp("Builtins.pkl", "0.2.0", Some(&h('c')), "import"),
            imp("Pins.pkl", "0.2.0", Some(&h('c')), "import*"),
        ];
        assert_eq!(
            get(dir.path(), "pantheon.pkl"),
            with_imports("0.2.0", Some(&h('c')), &after)
        );
    }

    #[test]
    fn an_import_that_was_left_on_an_older_version_is_brought_to_the_new_one() {
        let dir = repo("0.1.0", Some(&h('a')));
        put(
            dir.path(),
            "pantheon.pkl",
            &with_imports(
                "0.2.0",
                Some(&h('c')),
                &[imp("Builtins.pkl", "0.1.0", Some(&h('a')), "import")],
            ),
        );
        write(dir.path(), "0.3.0", &h('d')).unwrap();
        assert_eq!(
            get(dir.path(), "pantheon.pkl"),
            with_imports(
                "0.3.0",
                Some(&h('d')),
                &[imp("Builtins.pkl", "0.3.0", Some(&h('d')), "import")]
            )
        );
    }

    #[test]
    fn an_import_of_something_else_a_local_file_or_another_package_is_left_alone() {
        let dir = repo("0.1.0", Some(&h('a')));
        let others = [
            "import \"pkl:reflect\"".to_owned(),
            "import \"../local/Builtins.pkl\"".to_owned(),
            "import \"package://github.com/pantheon-org/pantheon-adr/releases/download/v1.0.0/pantheon-adr@1.0.0#/X.pkl\"".to_owned(),
        ];
        let mut before = others.to_vec();
        before.push(imp("Builtins.pkl", "0.1.0", Some(&h('a')), "import"));
        put(
            dir.path(),
            "pantheon.pkl",
            &with_imports("0.1.0", Some(&h('a')), &before),
        );
        assert_eq!(count_package_imports(dir.path(), "pantheon.pkl"), Ok(1));
        write(dir.path(), "0.2.0", &h('c')).unwrap();
        let mut after = others.to_vec();
        after.push(imp("Builtins.pkl", "0.2.0", Some(&h('c')), "import"));
        assert_eq!(
            get(dir.path(), "pantheon.pkl"),
            with_imports("0.2.0", Some(&h('c')), &after)
        );
    }

    #[test]
    fn a_config_with_no_imports_counts_none() {
        assert_eq!(
            count_package_imports(repo("0.1.0", None).path(), "pantheon.pkl"),
            Ok(0)
        );
        assert!(
            count_package_imports(tempfile::tempdir().unwrap().path(), "pantheon.pkl").is_err()
        );
    }
    #[test]
    fn config_after_is_what_write_pins_would_write_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let text = format!(
            "{}\nimport \"package://github.com/pantheon-org/pantheon-scaffold/releases/download/v0.1.0/pantheon-scaffold@0.1.0#/Builtins.pkl\"\n",
            pkl("0.1.0", None)
        );
        put(dir.path(), "pantheon.pkl", &text);
        put(dir.path(), "mise.toml", &mise("0.1.0"));
        let next = NewPins {
            version: "0.2.0",
            checksum: &h('c'),
        };
        let after = config_after(dir.path(), "pantheon.pkl", &next).unwrap();
        assert_eq!(get(dir.path(), "pantheon.pkl"), text, "nothing was written");
        write_pins(dir.path(), "pantheon.pkl", &next).unwrap();
        assert_eq!(after, get(dir.path(), "pantheon.pkl"));
        assert!(after.contains(&format!(
            "pantheon-scaffold@0.2.0::sha256:{}#/Config.pkl",
            h('c')
        )));
        assert!(after.contains(&format!(
            "pantheon-scaffold@0.2.0::sha256:{}#/Builtins.pkl",
            h('c')
        )));
    }

    #[test]
    fn config_after_refuses_what_write_pins_refuses() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "pantheon.pkl", &pkl("0.1.0", None));
        let bad_version = NewPins {
            version: "banana",
            checksum: &h('c'),
        };
        assert_eq!(
            config_after(dir.path(), "pantheon.pkl", &bad_version)
                .unwrap_err()
                .0,
            "\"banana\" is not a version"
        );
        let bad_sum = NewPins {
            version: "0.2.0",
            checksum: "abc",
        };
        assert_eq!(
            config_after(dir.path(), "pantheon.pkl", &bad_sum)
                .unwrap_err()
                .0,
            "the checksum is not 64 hex characters"
        );
        put(dir.path(), "none.pkl", "options { }\n");
        let ok = NewPins {
            version: "0.2.0",
            checksum: &h('c'),
        };
        assert!(
            config_after(dir.path(), "none.pkl", &ok)
                .unwrap_err()
                .0
                .contains("no amends line")
        );
        assert!(
            config_after(dir.path(), "missing.pkl", &ok)
                .unwrap_err()
                .0
                .contains("not found")
        );
    }
}
