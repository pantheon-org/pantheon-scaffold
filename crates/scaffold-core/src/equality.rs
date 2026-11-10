//! The P.1 equality rule (plan decision 73): two workflows are equal when both parse to equal data, comments and
//! formatting are ignored, `run:` scripts are compared byte for byte (a parsed string is its exact text), null and `{}`
//! are equal inside the `on:` map, and a duplicate mapping key in either file is a failure. Ported from
//! `runner/src/equality.ts`, which parses with `Bun.YAML`; here the parser is `saphyr`.

use serde_json::Value;

#[derive(Debug, PartialEq, Eq)]
pub struct Comparison {
    pub equal: bool,
    pub differences: Vec<String>,
    pub duplicate_keys: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct YamlError(pub String);

impl std::fmt::Display for YamlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for YamlError {}

/// Parses YAML text into JSON data. Several documents become an array, none become null.
pub fn parse_yaml(text: &str) -> Result<Value, YamlError> {
    use saphyr::{YamlLoader, YamlOwned};
    let invalid = |e: saphyr::ScanError| YamlError(format!("invalid YAML: {e}"));
    // A duplicate key is not an error here: the last value wins, as in the TypeScript runner, and `find_duplicate_keys`
    // reports it separately.
    let mut loader = YamlLoader::<YamlOwned>::default();
    loader.allow_duplicate_keys(true);
    saphyr_parser::Parser::new_from_str(text)
        .load(&mut loader, true)
        .map_err(invalid)?;
    if let Some(e) = loader.error() {
        return Err(invalid(e.clone()));
    }
    let docs = loader.into_documents();
    let mut values = docs.iter().map(to_json).collect::<Result<Vec<_>, _>>()?;
    Ok(match values.len() {
        0 => Value::Null,
        1 => values.remove(0),
        _ => Value::Array(values),
    })
}

fn to_json(node: &saphyr::YamlOwned) -> Result<Value, YamlError> {
    use saphyr::{ScalarOwned, YamlOwned};
    Ok(match node {
        YamlOwned::Value(ScalarOwned::Null) => Value::Null,
        YamlOwned::Value(ScalarOwned::Boolean(b)) => Value::Bool(*b),
        YamlOwned::Value(ScalarOwned::Integer(i)) => number(*i as f64),
        YamlOwned::Value(ScalarOwned::FloatingPoint(f)) => number(f.into_inner()),
        YamlOwned::Value(ScalarOwned::String(s)) | YamlOwned::Representation(s, ..) => {
            Value::String(s.clone())
        }
        YamlOwned::Sequence(items) => {
            Value::Array(items.iter().map(to_json).collect::<Result<_, _>>()?)
        }
        YamlOwned::Mapping(map) => {
            let mut out = serde_json::Map::new();
            let mut merged = Vec::new();
            for (key, value) in map {
                let key = key_text(key)?;
                if key == "<<" {
                    match to_json(value)? {
                        Value::Object(m) => merged.push(m),
                        Value::Array(items) => {
                            merged.extend(items.into_iter().filter_map(|i| i.as_object().cloned()))
                        }
                        other => {
                            out.insert(key, other);
                        }
                    }
                } else {
                    out.insert(key, to_json(value)?);
                }
            }
            // A merge key adds what the mapping does not already have; an earlier merged mapping wins over a later one.
            for m in merged {
                for (k, v) in m {
                    out.entry(k).or_insert(v);
                }
            }
            Value::Object(out)
        }
        YamlOwned::Tagged(_, inner) => to_json(inner)?,
        YamlOwned::Alias(_) => {
            return Err(YamlError(
                "invalid YAML: an alias is not supported".to_owned(),
            ));
        }
        YamlOwned::BadValue => {
            return Err(YamlError(
                "invalid YAML: a value could not be read".to_owned(),
            ));
        }
    })
}

/// Every number is a double, as in JavaScript: an integer past 2^53 loses its low digits, and a whole number reads as an
/// integer, because `1.0` and `1` are the same number there.
fn number(f: f64) -> Value {
    let whole = f.is_finite() && f.fract() == 0.0;
    if whole && (0.0..1.8e19).contains(&f) {
        Value::from(f as u64)
    } else if whole && (-9.2e18..0.0).contains(&f) {
        Value::from(f as i64)
    } else {
        // JSON has no infinity or NaN: `JSON.stringify` turns them into null, so do the same.
        serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number)
    }
}

/// A key as JavaScript turns it into a property name: an array joins its items with commas, an object is
/// `[object Object]`.
fn key_text(key: &saphyr::YamlOwned) -> Result<String, YamlError> {
    Ok(js_string(&to_json(key)?))
}

fn js_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .map(|i| {
                if i.is_null() {
                    String::new()
                } else {
                    js_string(i)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_owned(),
        other => other.to_string(),
    }
}

fn block_start() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^(?P<indent>\s*)(?:-\s+)?[^#\n]*:\s*[|>][-+0-9]*\s*(?:#.*)?$")
            .expect("valid")
    });
    &RE
}

fn key_pattern() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r##"^(?:- +)*(?P<key>"[^"]*"|'[^']*'|[^\s#:'"{}\[\],&*!|>%@`-][^:#]*?|-[^\s:][^:#]*?):(?:\s|$)"##,
        )
        .expect("valid")
    });
    &RE
}

/// The `- ` prefix of a list item line: the text before the first dash and the dashes with their spaces.
fn sequence_prefix(line: &str) -> Option<regex::Captures<'_>> {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^(\s*)((?:- +)+)").expect("valid"));
    RE.captures(line)
}

fn indent_of(line: &str) -> usize {
    line.chars().count() - line.trim_start().chars().count()
}

/// Where a block scalar's content must be indented beyond, when `line` opens one.
fn block_scalar_column(line: &str) -> Option<usize> {
    let block = block_start().captures(line)?;
    Some(match sequence_prefix(line) {
        Some(seq) => seq[0].chars().count(),
        None => block["indent"].chars().count(),
    })
}

/// Block-style duplicate key detection. Flow mappings on one line and block scalar contents are not inspected.
pub fn find_duplicate_keys(text: &str) -> Vec<String> {
    let mut dups = Vec::new();
    let mut stack: Vec<(usize, std::collections::HashSet<String>)> = Vec::new();
    let mut block_key_column: Option<usize> = None;
    for (i, raw) in text.split('\n').enumerate() {
        let indent = indent_of(raw);
        if let Some(column) = block_key_column {
            if raw.trim().is_empty() || indent > column {
                continue;
            }
            block_key_column = None;
        }
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed == "---" {
            continue;
        }
        block_key_column = block_scalar_column(raw);
        let seq = sequence_prefix(raw);
        let key_indent = seq
            .as_ref()
            .map_or(indent, |s| s[1].chars().count() + s[2].chars().count());
        let found = key_pattern().captures(trimmed);
        let pops = |top: usize| {
            if seq.is_some() {
                top >= key_indent
            } else {
                top > key_indent
            }
        };
        while stack.last().is_some_and(|(top, _)| pops(*top)) {
            stack.pop();
        }
        let Some(found) = found else { continue };
        if stack.last().is_none_or(|(top, _)| *top < key_indent) {
            stack.push((key_indent, std::collections::HashSet::new()));
        }
        let key = found["key"].trim().to_owned();
        let (_, keys) = stack.last_mut().expect("a level was just pushed");
        if keys.contains(&key) {
            dups.push(format!("line {}: {key}", i + 1));
        }
        keys.insert(key);
    }
    dups
}

/// The `on` map treats a null value and `{}` as the same, because both mean "run on this event with no options".
fn normalise(doc: Value) -> Value {
    let Value::Object(mut map) = doc else {
        return doc;
    };
    if let Some(Value::Object(on)) = map.get_mut("on") {
        for value in on.values_mut() {
            if value.is_null() {
                *value = Value::Object(serde_json::Map::new());
            }
        }
    }
    Value::Object(map)
}

fn shown(value: Option<&Value>) -> String {
    value.map_or_else(|| "undefined".to_owned(), Value::to_string)
}

fn diff(a: Option<&Value>, b: Option<&Value>, path: &str, out: &mut Vec<String>) {
    if a == b {
        return;
    }
    match (a, b) {
        (Some(Value::Object(left)), Some(Value::Object(right))) => {
            let mut keys: Vec<&String> = left.keys().collect();
            keys.extend(right.keys().filter(|k| !left.contains_key(*k)));
            for key in keys {
                diff(left.get(key), right.get(key), &format!("{path}.{key}"), out);
            }
        }
        (Some(Value::Array(left)), Some(Value::Array(right))) if left.len() == right.len() => {
            for (i, (x, y)) in left.iter().zip(right).enumerate() {
                diff(Some(x), Some(y), &format!("{path}[{i}]"), out);
            }
        }
        _ => out.push(format!("{path}: {} vs {}", shown(a), shown(b))),
    }
}

pub fn compare_workflows(canonical: &str, rendered: &str) -> Result<Comparison, YamlError> {
    let mut differences = Vec::new();
    let left = normalise(parse_yaml(canonical)?);
    let right = normalise(parse_yaml(rendered)?);
    diff(Some(&left), Some(&right), "$", &mut differences);
    let duplicate_keys: Vec<String> = find_duplicate_keys(canonical)
        .into_iter()
        .map(|d| format!("canonical {d}"))
        .chain(
            find_duplicate_keys(rendered)
                .into_iter()
                .map(|d| format!("rendered {d}")),
        )
        .collect();
    Ok(Comparison {
        equal: differences.is_empty() && duplicate_keys.is_empty(),
        differences,
        duplicate_keys,
    })
}

/// The lines that carry a directive comment, trimmed and with runs of whitespace collapsed.
pub fn directive_comments(text: &str) -> Vec<String> {
    static DIRECTIVE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"#\s*zizmor:\s*ignore\[[^\]]*\]").expect("valid")
    });
    static QUOTED_KEY: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"^(?:'([^'"]+)'|"([^'"]+)"):"#).expect("valid")
    });
    static SPACES: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"\s+").expect("valid"));
    let mut out = Vec::new();
    let mut block_key_column: Option<usize> = None;
    for raw in text.split('\n') {
        if let Some(column) = block_key_column
            && (raw.trim().is_empty() || indent_of(raw) > column)
        {
            continue;
        }
        block_key_column = block_scalar_column(raw);
        // The renderer quotes the key `on` (it is a YAML 1.1 boolean), so a quoted key is the same line as a bare one.
        if DIRECTIVE.is_match(raw) {
            let unquoted = QUOTED_KEY.replace(raw.trim(), "${1}${2}:");
            out.push(SPACES.replace_all(&unquoted, " ").into_owned());
        }
    }
    out
}

/// Differences between the directive comments of the canonical file and of the render, as a multiset.
pub fn compare_directives(canonical: &str, rendered: &str) -> Vec<String> {
    let want = directive_comments(canonical);
    let mut left = directive_comments(rendered);
    let mut out = Vec::new();
    for directive in want {
        match left.iter().position(|d| *d == directive) {
            Some(i) => {
                left.remove(i);
            }
            None => out.push(format!("missing in the render: {directive}")),
        }
    }
    out.extend(
        left.into_iter()
            .map(|d| format!("not in the canonical file: {d}")),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const A: &str = "name: X\non:\n  push:\n    branches: [main]\n  workflow_dispatch:\njobs:\n  j:\n    runs-on: ubuntu-latest\n    steps:\n      - run: |\n          echo one\n          echo two\n";

    fn cmp(a: &str, b: &str) -> Comparison {
        compare_workflows(a, b).unwrap()
    }

    #[test]
    fn identical_text_is_equal() {
        assert!(cmp(A, A).equal);
    }

    #[test]
    fn comments_key_order_quoting_and_block_lists_do_not_matter() {
        let b = "# a comment\njobs:\n  j:\n    steps:\n    - run: |\n        echo one\n        echo two\n    runs-on: \"ubuntu-latest\"\n'on':\n  workflow_dispatch: {}\n  push:\n    branches:\n    - main\nname: X\n";
        assert_eq!(
            cmp(A, b),
            Comparison {
                equal: true,
                differences: vec![],
                duplicate_keys: vec![]
            }
        );
    }

    #[test]
    fn null_and_an_empty_map_are_equal_inside_on() {
        assert!(
            cmp(
                "on:\n  workflow_dispatch:\n",
                "on:\n  workflow_dispatch: {}\n"
            )
            .equal
        );
    }

    #[test]
    fn null_and_an_empty_map_are_not_equal_outside_on() {
        assert!(!cmp("a:\n  b:\n", "a:\n  b: {}\n").equal);
    }

    #[test]
    fn a_one_character_difference_in_a_run_script_is_a_difference() {
        let r = cmp(A, &A.replace("echo two", "echo twO"));
        assert!(!r.equal);
        assert_eq!(
            r.differences,
            ["$.jobs.j.steps[0].run: \"echo one\\necho two\\n\" vs \"echo one\\necho twO\\n\""]
        );
    }

    #[test]
    fn a_trailing_newline_difference_in_a_run_script_is_a_difference() {
        assert!(!cmp("a:\n  - run: |\n      x\n", "a:\n  - run: |-\n      x\n").equal);
    }

    #[test]
    fn an_extra_key_is_a_difference_shown_as_undefined() {
        assert_eq!(
            cmp(A, &format!("{A}extra: 1\n")).differences,
            ["$.extra: undefined vs 1"]
        );
        assert_eq!(
            cmp(&format!("{A}extra: 1\n"), A).differences,
            ["$.extra: 1 vs undefined"]
        );
    }

    #[test]
    fn arrays_of_different_length_are_one_difference() {
        assert_eq!(
            cmp("a: [1, 2]\n", "a: [1]\n").differences,
            ["$.a: [1,2] vs [1]"]
        );
    }

    #[test]
    fn a_whole_number_float_equals_the_integer() {
        assert!(cmp("a: 1\n", "a: 1.0\n").equal);
        assert!(!cmp("a: 1\n", "a: 1.5\n").equal);
    }

    #[test]
    fn a_duplicate_key_in_the_rendered_file_fails_the_comparison() {
        let r = cmp("a: 1\n", "a: 1\na: 1\n");
        assert!(!r.equal);
        assert_eq!(r.duplicate_keys, ["rendered line 2: a"]);
    }

    #[test]
    fn a_duplicate_key_in_the_canonical_file_is_labelled_canonical() {
        assert_eq!(
            cmp("a: 1\na: 1\n", "a: 1\n").duplicate_keys,
            ["canonical line 2: a"]
        );
    }

    #[test]
    fn text_that_is_not_yaml_is_an_error() {
        assert!(
            compare_workflows("a: [1\n", "a: 1\n")
                .unwrap_err()
                .to_string()
                .starts_with("invalid YAML: ")
        );
    }

    #[test]
    fn yaml_one_one_booleans_are_strings_like_bun() {
        // Core schema: `yes`, `on` and `y` are plain strings, only true and false are booleans.
        assert_eq!(
            parse_yaml("a: yes\nb: on\nc: true\n").unwrap(),
            json!({ "a": "yes", "b": "on", "c": true })
        );
    }

    #[test]
    fn scalars_resolve_by_the_core_schema() {
        let v = parse_yaml("a: ~\nb: null\nc: 12\nd: 0o17\ne: 0x1f\nf: 1e3\ng: '12'\nh: 007\n")
            .unwrap();
        assert_eq!(
            v,
            json!({ "a": null, "b": null, "c": 12, "d": 15, "e": 31, "f": 1000, "g": "12", "h": 7 })
        );
    }

    #[test]
    fn no_document_is_null_and_several_are_an_array() {
        assert_eq!(parse_yaml("").unwrap(), Value::Null);
        assert_eq!(
            parse_yaml("a: 1\n---\nb: 2\n").unwrap(),
            json!([{ "a": 1 }, { "b": 2 }])
        );
    }

    #[test]
    fn non_string_keys_become_their_text() {
        assert_eq!(
            parse_yaml("1: a\ntrue: b\n").unwrap(),
            json!({ "1": "a", "true": "b" })
        );
    }

    #[test]
    fn an_alias_is_replaced_by_the_value_it_names() {
        assert_eq!(
            parse_yaml("a: &x 1\nb: *x\n").unwrap(),
            json!({ "a": 1, "b": 1 })
        );
    }

    #[test]
    fn with_a_duplicate_key_the_last_value_wins() {
        assert_eq!(parse_yaml("a: 1\na: 2\n").unwrap(), json!({ "a": 2 }));
    }

    #[test]
    fn duplicate_keys_same_key_in_different_list_items_is_fine() {
        assert!(
            find_duplicate_keys("steps:\n  - name: a\n    run: x\n  - name: b\n    run: y\n")
                .is_empty()
        );
    }

    #[test]
    fn duplicate_keys_same_key_in_different_mappings_is_fine() {
        assert!(find_duplicate_keys("a:\n  name: x\nb:\n  name: y\n").is_empty());
    }

    #[test]
    fn duplicate_keys_a_repeated_key_in_one_nested_mapping_is_found() {
        assert_eq!(
            find_duplicate_keys("jobs:\n  j:\n    name: x\n    runs-on: a\n    name: y\n"),
            ["line 5: name"]
        );
    }

    #[test]
    fn duplicate_keys_block_scalar_content_is_ignored() {
        assert!(find_duplicate_keys("run: |\n  a: 1\n  a: 2\nother: 1\n").is_empty());
    }

    #[test]
    fn duplicate_keys_comments_and_the_document_marker_are_ignored() {
        assert!(find_duplicate_keys("---\n# a: 1\na: 1\n# a: 2\n").is_empty());
    }

    #[test]
    fn duplicate_keys_quoted_keys_count() {
        assert_eq!(find_duplicate_keys("'on': 1\n'on': 2\n"), ["line 2: 'on'"]);
    }

    #[test]
    fn duplicate_keys_a_repeated_key_inside_a_list_item_is_found() {
        assert_eq!(
            find_duplicate_keys("steps:\n  - name: a\n    name: b\n"),
            ["line 3: name"]
        );
    }

    #[test]
    fn numbers_follow_javascript_doubles_at_their_edges() {
        assert_eq!(number(0.0), json!(0));
        assert_eq!(number(5.0), json!(5));
        assert_eq!(number(-5.0), json!(-5));
        assert_eq!(
            number(1.8e19 - 4096.0),
            Value::from(17_999_999_999_999_995_904_u64)
        );
        assert!(number(1.8e19).is_f64(), "{}", number(1.8e19));
        assert!(number(-1.0e19).is_f64(), "{}", number(-1.0e19));
        assert_eq!(number(-9.0e18), Value::from(-9_000_000_000_000_000_000_i64));
        assert_eq!(number(-9.2e18), Value::from(-9_200_000_000_000_000_000_i64));
        assert!(number(-9.3e18).is_f64());
        assert_eq!(number(f64::INFINITY), Value::Null);
        assert_eq!(number(f64::NAN), Value::Null);
        assert_eq!(number(1.5), json!(1.5));
    }

    #[test]
    fn a_merge_key_adds_only_the_keys_the_mapping_lacks_and_the_first_merged_wins() {
        let text = "a: &a\n  k: 1\n  j: 2\nb: &b\n  j: 9\n  m: 4\nd:\n  <<: [*a, *b]\n  j: 3\n";
        assert_eq!(
            parse_yaml(text).unwrap()["d"],
            json!({ "j": 3, "k": 1, "m": 4 })
        );
    }

    #[test]
    fn a_complex_key_becomes_javascripts_text_for_it() {
        assert_eq!(
            parse_yaml("? [a, b]\n: c\n? [1, ~, 2]\n: d\n? {x: 1}\n: e\n").unwrap(),
            json!({ "a,b": "c", "1,,2": "d", "[object Object]": "e" })
        );
    }

    #[test]
    fn duplicate_keys_a_blank_line_or_a_comment_does_not_end_the_mapping() {
        assert_eq!(find_duplicate_keys("a:\n  b: 1\n\n  b: 2\n"), ["line 4: b"]);
        assert_eq!(
            find_duplicate_keys("a:\n  b: 1\n# note\n  b: 2\n"),
            ["line 4: b"]
        );
        assert_eq!(
            find_duplicate_keys("a:\n  b: 1\n---\n  b: 2\n"),
            ["line 4: b"]
        );
    }

    #[test]
    fn duplicate_keys_a_key_at_the_indent_of_a_block_scalars_key_ends_the_scalar() {
        assert_eq!(
            find_duplicate_keys("a: 1\nrun: |\n  x\na: 2\n"),
            ["line 4: a"]
        );
    }

    #[test]
    fn a_directive_on_the_line_after_a_block_scalar_is_found() {
        assert_eq!(
            directive_comments("run: |\n  x\nname: x # zizmor: ignore[a]\n"),
            ["name: x # zizmor: ignore[a]"]
        );
    }

    const WITH_ZIZMOR: &str = "name: x\non: # zizmor: ignore[dangerous-triggers] reads only the PR title\n  pull_request_target:\n    types: [opened]\n";
    const NOTE: &str = "on: # zizmor: ignore[dangerous-triggers] reads only the PR title";

    #[test]
    fn a_zizmor_ignore_comment_is_found_with_the_line_it_sits_on() {
        assert_eq!(directive_comments(WITH_ZIZMOR), [NOTE]);
    }

    #[test]
    fn ordinary_comments_are_not_directives() {
        assert!(
            directive_comments("name: x # a note\n# zizmor is a linter\non: push\n").is_empty()
        );
    }

    #[test]
    fn a_zizmor_directive_inside_a_run_script_is_part_of_the_script() {
        assert!(
            directive_comments(
                "jobs:\n  a:\n    steps:\n      - run: |\n          echo '# zizmor: ignore[x]'\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn the_same_directives_compare_equal_however_the_file_is_otherwise_formatted() {
        assert!(
            compare_directives(WITH_ZIZMOR, &WITH_ZIZMOR.replace("name: x", "name:    x"))
                .is_empty()
        );
    }

    #[test]
    fn a_dropped_directive_and_an_added_one_are_both_reported() {
        assert_eq!(
            compare_directives(
                WITH_ZIZMOR,
                "name: x\non:\n  pull_request_target:\n    types: [opened]\n"
            ),
            [format!("missing in the render: {NOTE}")]
        );
        assert_eq!(
            compare_directives("name: x\non: push\n", WITH_ZIZMOR),
            [format!("not in the canonical file: {NOTE}")]
        );
    }

    #[test]
    fn a_directive_moved_to_a_different_line_is_reported_twice() {
        let moved = WITH_ZIZMOR.replace(
            "on: # zizmor: ignore[dangerous-triggers] reads only the PR title\n  pull_request_target:",
            "on:\n  pull_request_target: # zizmor: ignore[dangerous-triggers] reads only the PR title",
        );
        assert_eq!(compare_directives(WITH_ZIZMOR, &moved).len(), 2);
    }

    #[test]
    fn a_directive_on_a_quoted_key_is_the_same_line_as_on_the_bare_key() {
        let quoted = WITH_ZIZMOR.replace("\non:", "\n'on':");
        assert!(compare_directives(WITH_ZIZMOR, &quoted).is_empty());
        assert_eq!(directive_comments(&quoted), [NOTE]);
        let double = WITH_ZIZMOR.replace("\non:", "\n\"on\":");
        assert_eq!(directive_comments(&double), [NOTE]);
    }
}
