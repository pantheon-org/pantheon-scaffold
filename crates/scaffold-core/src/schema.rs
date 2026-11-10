//! A small JSON Schema validator for the subset `contract.schema.json` uses: type, const, enum, required, properties,
//! additionalProperties (boolean), items, pattern, minLength and minimum. It has no schema dependency by design, and
//! reports the same messages, in the same order, as `runner/src/schema.ts`.

use serde_json::Value;

/// The problems found in `value`, each prefixed with its path (`#`, `#/files/0/mode`). Empty when it is valid.
pub fn validate(value: &Value, schema: &Value, path: &str) -> Vec<String> {
    let mut errors = Vec::new();
    if let Some(expected) = schema.get("const")
        && value != expected
    {
        errors.push(format!("{path}: must be {expected}"));
    }
    if let Some(Value::Array(allowed)) = schema.get("enum")
        && !allowed.contains(value)
    {
        errors.push(format!(
            "{path}: must be one of {}",
            Value::Array(allowed.clone())
        ));
    }
    if let Some(Value::String(expected)) = schema.get("type") {
        let actual = type_of(value);
        if actual != expected && !(expected == "number" && actual == "integer") {
            errors.push(format!("{path}: must be {expected}, got {actual}"));
            return errors;
        }
    }
    match value {
        Value::String(text) => check_string(text, schema, path, &mut errors),
        Value::Number(n) => {
            if let (Some(min), Some(v)) =
                (schema.get("minimum").and_then(Value::as_f64), n.as_f64())
                && v < min
            {
                errors.push(format!("{path}: below {}", schema["minimum"]));
            }
        }
        Value::Array(items) => {
            if let Some(item_schema) = schema.get("items") {
                for (i, item) in items.iter().enumerate() {
                    errors.extend(validate(item, item_schema, &format!("{path}/{i}")));
                }
            }
        }
        Value::Object(object) => check_object(object, schema, path, &mut errors),
        _ => {}
    }
    errors
}

/// The type names JavaScript's checks give: a whole number is an integer even when written `2.0`.
fn type_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::Number(n) if n.as_f64().is_some_and(|f| f.fract() == 0.0) => "integer",
        Value::Number(_) => "number",
    }
}

fn check_string(text: &str, schema: &Value, path: &str, errors: &mut Vec<String>) {
    if let Some(min) = schema.get("minLength").and_then(Value::as_u64)
        && (text.encode_utf16().count() as u64) < min
    {
        errors.push(format!("{path}: shorter than {min}"));
    }
    if let Some(Value::String(pattern)) = schema.get("pattern")
        && !regex::Regex::new(pattern).is_ok_and(|re| re.is_match(text))
    {
        errors.push(format!("{path}: does not match {pattern}"));
    }
}

fn check_object(
    object: &serde_json::Map<String, Value>,
    schema: &Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    if let Some(Value::Array(required)) = schema.get("required") {
        for key in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(key) {
                errors.push(format!("{path}: missing {key}"));
            }
        }
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    for (key, sub) in properties.into_iter().flatten() {
        if let Some(child) = object.get(key) {
            errors.extend(validate(child, sub, &format!("{path}/{key}")));
        }
    }
    if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
        for key in object
            .keys()
            .filter(|k| !properties.is_some_and(|p| p.contains_key(*k)))
        {
            errors.push(format!("{path}: unexpected {key}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> Value {
        json!({
            "type": "object",
            "required": ["a"],
            "additionalProperties": false,
            "properties": {
                "a": { "type": "integer", "minimum": 1 },
                "b": { "enum": ["x", "y"] },
                "c": { "type": "string", "pattern": "^[0-9a-f]{4}$", "minLength": 4 },
                "d": { "type": "array", "items": { "type": "string" } },
                "v": { "const": 1 }
            }
        })
    }

    fn check(value: Value) -> Vec<String> {
        validate(&value, &schema(), "#")
    }

    #[test]
    fn accepts_a_valid_value() {
        assert!(check(json!({ "a": 1, "b": "x", "c": "ab12", "d": ["z"], "v": 1 })).is_empty());
    }

    #[test]
    fn reports_a_missing_required_key() {
        assert_eq!(check(json!({})), ["#: missing a"]);
    }

    #[test]
    fn reports_an_unexpected_key() {
        assert_eq!(check(json!({ "a": 1, "z": 1 })), ["#: unexpected z"]);
    }

    #[test]
    fn reports_a_wrong_type_with_its_path() {
        assert_eq!(
            check(json!({ "a": "1" })),
            ["#/a: must be integer, got string"]
        );
    }

    #[test]
    fn a_float_is_not_an_integer_but_a_whole_float_is() {
        assert_eq!(
            check(json!({ "a": 1.5 })),
            ["#/a: must be integer, got number"]
        );
        assert!(check(json!({ "a": 2.0 })).is_empty());
    }

    #[test]
    fn reports_a_minimum() {
        assert_eq!(check(json!({ "a": 0 })), ["#/a: below 1"]);
    }

    #[test]
    fn reports_an_enum_miss() {
        assert_eq!(
            check(json!({ "a": 1, "b": "q" })),
            [r##"#/b: must be one of ["x","y"]"##]
        );
    }

    #[test]
    fn reports_a_pattern_miss_and_a_short_string_in_that_order() {
        assert_eq!(
            check(json!({ "a": 1, "c": "zz" })),
            ["#/c: shorter than 4", "#/c: does not match ^[0-9a-f]{4}$"]
        );
    }

    #[test]
    fn minimum_length_counts_utf16_units_like_javascript() {
        // One emoji is two UTF-16 units, so it is long enough for minLength 2 in JavaScript.
        let s = json!({ "type": "string", "minLength": 2 });
        assert!(validate(&json!("\u{1F600}"), &s, "#").is_empty());
    }

    #[test]
    fn reports_an_item_error_with_an_index() {
        assert_eq!(
            check(json!({ "a": 1, "d": ["ok", 3] })),
            ["#/d/1: must be string, got integer"]
        );
    }

    #[test]
    fn reports_a_const_miss() {
        assert_eq!(check(json!({ "a": 1, "v": 2 })), ["#/v: must be 1"]);
    }

    #[test]
    fn a_wrong_type_stops_the_checks_below_it() {
        assert_eq!(
            validate(
                &json!("x"),
                &json!({ "type": "object", "required": ["a"] }),
                "#"
            ),
            ["#: must be object, got string"]
        );
    }

    #[test]
    fn the_number_type_takes_integers_and_floats_but_nothing_else() {
        let s = json!({ "type": "number" });
        assert!(validate(&json!(3), &s, "#").is_empty());
        assert!(validate(&json!(1.5), &s, "#").is_empty());
        assert_eq!(
            validate(&json!("x"), &s, "#"),
            ["#: must be number, got string"]
        );
        assert_eq!(
            validate(&json!(3), &json!({ "type": "string" }), "#"),
            ["#: must be string, got integer"]
        );
    }

    #[test]
    fn null_and_array_have_their_own_type_names() {
        let s = json!({ "type": "string" });
        assert_eq!(
            validate(&Value::Null, &s, "#"),
            ["#: must be string, got null"]
        );
        assert_eq!(
            validate(&json!([]), &s, "#"),
            ["#: must be string, got array"]
        );
    }

    #[test]
    fn problems_come_out_in_the_order_the_schema_declares_its_properties() {
        let s = json!({ "properties": { "z": { "type": "string" }, "a": { "type": "string" } } });
        assert_eq!(
            validate(&json!({ "a": 1, "z": 1 }), &s, "#"),
            [
                "#/z: must be string, got integer",
                "#/a: must be string, got integer"
            ]
        );
    }
}
