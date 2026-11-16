//! What the TypeScript scripts get from JavaScript and Rust does not: `JSON.stringify(value, null, 1)`, the way JavaScript sorts and
//! cuts strings (by UTF-16 code unit), a path made absolute the way `path.resolve` does it, and the octal text of a file mode.

use serde::Serialize;
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

/// `JSON.stringify(value, null, 1)`: one space for each level, a space after each colon, nothing after the last line. Object keys keep the
/// order they were built in (the `preserve_order` feature of `serde_json`).
pub fn stringify_indent_1(value: &Value) -> String {
    let mut bytes = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
    value
        .serialize(&mut serializer)
        .expect("a value serialises");
    String::from_utf8(bytes).expect("JSON is UTF-8")
}

/// `JSON.stringify(text)`: the text in quotes, with its escapes.
pub fn quote(text: &str) -> String {
    serde_json::to_string(text).expect("a string serialises")
}

/// Sorts the way `Array.prototype.sort` does on strings: by UTF-16 code unit, which is not the same as by byte for text outside the
/// Basic Multilingual Plane.
pub fn sort_like_javascript(list: &mut [String]) {
    list.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
}

/// `text.slice(0, units)`: the first `units` UTF-16 code units. A surrogate pair cut in two becomes U+FFFD, as it prints in JavaScript.
pub fn slice_utf16(text: &str, units: usize) -> String {
    let all: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16_lossy(&all[..all.len().min(units)])
}

/// `path.resolve(p)`: the path made absolute against `cwd` and tidied by its text alone (`.` dropped, `..` removing the part before it),
/// without looking at the file system or following a link.
pub fn resolve(path: &str, cwd: &Path) -> PathBuf {
    let joined = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        cwd.join(path)
    };
    let mut out = PathBuf::new();
    for part in joined.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `(mode & 0o777).toString(8)`: the permission bits as octal text, with no leading zero.
pub fn octal_mode(mode: u32) -> String {
    format!("{:o}", mode & 0o777)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_pretty_text_has_one_space_for_each_level_like_json_stringify() {
        let value = json!({ "a": [1, 2], "b": { "c": "d" }, "e": [], "f": {}, "g": null });
        assert_eq!(
            stringify_indent_1(&value),
            "{\n \"a\": [\n  1,\n  2\n ],\n \"b\": {\n  \"c\": \"d\"\n },\n \"e\": [],\n \"f\": {},\n \"g\": null\n}"
        );
    }

    #[test]
    fn keys_keep_the_order_they_were_built_in() {
        let mut object = serde_json::Map::new();
        object.insert("z".to_owned(), json!(1));
        object.insert("a".to_owned(), json!(2));
        assert_eq!(
            stringify_indent_1(&Value::Object(object)),
            "{\n \"z\": 1,\n \"a\": 2\n}"
        );
    }

    #[test]
    fn strings_are_escaped_like_json_stringify() {
        let text = "q\"b\\n\n\r\t\u{8}\u{c}\u{1}\u{1f}é€\u{7f}\u{2028}😀";
        assert_eq!(
            quote(text),
            "\"q\\\"b\\\\n\\n\\r\\t\\b\\f\\u0001\\u001fé€\u{7f}\u{2028}😀\""
        );
        assert_eq!(
            stringify_indent_1(&json!([text])),
            format!("[\n {}\n]", quote(text))
        );
    }

    #[test]
    fn the_sort_is_by_utf16_unit_so_a_character_outside_the_bmp_sorts_before_a_high_bmp_one() {
        let mut list = vec![
            "\u{ffff}".to_owned(),
            "😀".to_owned(),
            "a".to_owned(),
            "B".to_owned(),
        ];
        sort_like_javascript(&mut list);
        assert_eq!(list, ["B", "a", "😀", "\u{ffff}"]);
        let mut by_bytes = vec!["\u{ffff}".to_owned(), "😀".to_owned()];
        by_bytes.sort();
        assert_eq!(
            by_bytes,
            ["\u{ffff}", "😀"],
            "bytes order them the other way round"
        );
    }

    #[test]
    fn a_slice_counts_utf16_units_and_a_cut_pair_is_a_replacement_character() {
        assert_eq!(slice_utf16("abcdef", 3), "abc");
        assert_eq!(slice_utf16("abc", 10), "abc");
        assert_eq!(slice_utf16("a😀b", 2), "a\u{fffd}");
        assert_eq!(slice_utf16("a😀b", 3), "a😀");
    }

    #[test]
    fn a_path_is_made_absolute_and_tidied_by_its_text_like_path_resolve() {
        let cwd = Path::new("/work/here");
        assert_eq!(resolve("a/b", cwd), Path::new("/work/here/a/b"));
        assert_eq!(resolve("./a/../b", cwd), Path::new("/work/here/b"));
        assert_eq!(resolve("/x/./y/../z", cwd), Path::new("/x/z"));
        assert_eq!(resolve("../up", cwd), Path::new("/work/up"));
        assert_eq!(resolve("/x//y/", cwd), Path::new("/x/y"));
    }

    #[test]
    fn a_mode_is_its_permission_bits_in_octal_with_no_leading_zero() {
        assert_eq!(octal_mode(0o100644), "644");
        assert_eq!(octal_mode(0o100755), "755");
        assert_eq!(octal_mode(0o100600), "600");
        assert_eq!(octal_mode(0o100007), "7");
        assert_eq!(octal_mode(0o100000), "0");
    }
}
