//! Renders a contract file's Jinja template with the context the pkl package passed in (decisions 4 to 6 of
//! `docs/templates-design.md`).

use crate::contract::ContractError;
use minijinja::syntax::SyntaxConfig;
use minijinja::value::Serde;
use minijinja::{AutoEscape, Environment, UndefinedBehavior, Value};

fn environment() -> Result<Environment<'static>, minijinja::Error> {
    let mut env = Environment::new();
    // A variable a template reads must be in its context, so a conditional is never silently false.
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    // The default turns JSON escaping on for `.json` and `.yaml` names, which would change quotes and slashes.
    env.set_auto_escape_callback(|_| AutoEscape::None);
    env.set_syntax(
        SyntaxConfig::builder()
            .keep_trailing_newline(true)
            .trim_blocks(true)
            .lstrip_blocks(true)
            .build()?,
    );
    Ok(env)
}

/// Renders `source` for the file at `path`. An error names the file and the template line, and nothing is partly rendered.
pub fn render(
    path: &str,
    source: &str,
    context: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, ContractError> {
    let fail = |e: minijinja::Error| ContractError(format!("{path}: template error: {e:#}"));
    let mut env = environment().map_err(fail)?;
    env.add_template_owned(path.to_owned(), source.to_owned())
        .map_err(fail)?;
    let template = env.get_template(path).map_err(fail)?;
    template.render(Value::from(Serde(context))).map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        v.as_object().expect("an object").clone()
    }

    #[test]
    fn a_value_is_filled_in_and_the_trailing_newline_is_kept() {
        let out = render(
            "a.toml",
            "rust = \"{{ toolchain }}\"\n",
            &ctx(json!({"toolchain": "1.96.1"})),
        )
        .unwrap();
        assert_eq!(out, "rust = \"1.96.1\"\n");
    }

    #[test]
    fn a_block_on_its_own_lines_leaves_no_blank_line() {
        let src = "a = 1\n{% if on %}\nb = 2\n{% endif %}\nz = 3\n";
        assert_eq!(
            render("b.toml", src, &ctx(json!({"on": true}))).unwrap(),
            "a = 1\nb = 2\nz = 3\n"
        );
        assert_eq!(
            render("b.toml", src, &ctx(json!({"on": false}))).unwrap(),
            "a = 1\nz = 3\n"
        );
    }

    #[test]
    fn a_list_joins_and_a_string_version_keeps_its_form() {
        let src = "v = {{ v }}\np = [{% for p in ps %}\"{{ p }}\"{% if not loop.last %}, {% endif %}{% endfor %}]\n";
        let out = render("c.toml", src, &ctx(json!({"v": "1.90", "ps": ["a", "b"]}))).unwrap();
        assert_eq!(out, "v = 1.90\np = [\"a\", \"b\"]\n");
    }

    #[test]
    fn raw_keeps_text_that_is_not_jinja() {
        let out = render(
            "d",
            "x {% raw %}{{files}} ${{ github.ref }} ${#v}{% endraw +%}\n",
            &ctx(json!({})),
        )
        .unwrap();
        assert_eq!(out, "x {{files}} ${{ github.ref }} ${#v}\n");
    }

    #[test]
    fn quotes_and_slashes_are_not_escaped_in_json_and_yaml_names() {
        let out = render(
            "s.json",
            "{\"a\": \"{{ v }}\"}\n",
            &ctx(json!({"v": "a/b \"q\" <c>"})),
        )
        .unwrap();
        assert_eq!(out, "{\"a\": \"a/b \"q\" <c>\"}\n");
    }

    #[test]
    fn an_undefined_variable_is_an_error_naming_the_file_and_line() {
        let e = render("e.yml", "ok\na: {{ missing }}\n", &ctx(json!({})))
            .unwrap_err()
            .to_string();
        assert!(
            e.starts_with("e.yml: template error: undefined value"),
            "{e}"
        );
        assert!(e.contains("e.yml:2"), "{e}");
    }

    #[test]
    fn an_undefined_variable_in_a_condition_is_an_error_too() {
        let e = render("f", "{% if nope %}x{% endif %}\n", &ctx(json!({})))
            .unwrap_err()
            .to_string();
        assert!(e.contains("nope"), "{e}");
    }

    #[test]
    fn a_syntax_error_names_the_file_and_line() {
        let e = render("g.toml", "ok\nbad {% if %}\n", &ctx(json!({})))
            .unwrap_err()
            .to_string();
        assert!(e.starts_with("g.toml: template error: syntax error"), "{e}");
        assert!(e.contains("g.toml:2"), "{e}");
    }

    #[test]
    fn a_boolean_prints_python_style_so_a_template_spells_out_true_and_false() {
        let c = ctx(json!({"on": false}));
        assert_eq!(render("i", "{{ on }}\n", &c).unwrap(), "False\n");
        assert_eq!(
            render("i", "{{ 'true' if on else 'false' }}\n", &c).unwrap(),
            "false\n"
        );
    }

    #[test]
    fn a_bare_hash_after_a_brace_starts_a_comment_and_is_refused() {
        assert!(render("h.sh", "n=${#var}\n", &ctx(json!({}))).is_err());
    }
}
