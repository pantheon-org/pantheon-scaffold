# Builtin templates: design record

Status: accepted goal, design in progress (PR B). Date: 14-11-2026.

Every toml, yaml and json file a builtin ships, and every template that uses `@@NAME@@` today, becomes a Jinja template
rendered by [minijinja](https://github.com/mitsuhiko/minijinja) in the Rust runner. The values come from a context built
in pkl and passed in. A template holds no changeable value, and each variable has one definition.

## Decisions

| # | Decision | Why |
| --- | --- | --- |
| 1 | The runner renders, after pkl. The contract carries `template` and `context` for a file, not `content`. | pkl has no Jinja engine, and the runner is the one place that already writes, checks and hashes files. |
| 2 | `schemaVersion` goes from 1 to 2, a hard cut. A runner that reads a higher version says "the package needs a newer runner". | A version 1 runner cannot render a template, so it must refuse the package, not write an empty file. |
| 3 | Templates are named `*.j2`. | The suffix says the file is not valid toml, yaml or json until rendered. |
| 4 | Undefined variables are errors (strict). | A conditional that reads a variable the context lacks would silently be false. |
| 5 | Autoescape is off, set by an explicit callback. | The default turns on JSON escaping for `.json` and `.yaml` names, which would change quotes and slashes. |
| 6 | A file is rendered once, when the contract is parsed. Check, apply, migrate, detach, upgrade and the manifest hash all see rendered text. | One render means one answer. A render error comes before anything is written. Version comments are added after Jinja. |
| 7 | `contract --json` prints the contract after rendering. | The package tests read what the package renders. |
| 8 | A file with `content` and a file with `template` are mutually exclusive, and `context` needs `template`. The runner checks this in `contract.rs`. | The in-house schema validator has no `oneOf`. |
| 9 | Context values are strings unless a test proves otherwise. | A JSON number loses its form: `1.90` renders as `1.9`. A version must be a string. |
| 10 | A preset replaces whole context keys. | Merging nested keys makes it unclear which definition won. |

## minijinja

- Version: `=3.1.0`, pinned exactly. Published 07-11-2026, which meets the 7-day cooldown in `.github/dependabot.yml`.
  Licence Apache-2.0, already allowed by `deny.toml`. Minimum Rust 1.70.
- Features: default features off, then `builtins`, `serde`. Add `json` only when a template uses `tojson`, and
  `preserve_order` only when a template walks a mapping.
- Crates it adds beyond what the runner already uses: none that are new to the tree, apart from minijinja itself
  (`serde`, `serde_json`, `indexmap`, `itoa` and `zmij` are already there through `serde_json`). To confirm with
  `cargo deny check` when the dependency lands.
- Whitespace is set on the syntax config, not the environment: `keep_trailing_newline(true)`, `trim_blocks(true)`,
  `lstrip_blocks(true)`. The context goes in as `Value::from(minijinja::value::Serde(ctx))`.

## Checked behaviour (minijinja 3.1.0)

| Case | Result |
| --- | --- |
| Template ending in a newline | newline kept |
| `{% if %}` block on its own lines | no blank line left (`trim_blocks` plus `lstrip_blocks`) |
| `{% raw %}{{files}}{% endraw %}` | `{{files}}` kept: use it for hk's `{{files}}` and GitHub `${{ }}` |
| Missing variable, inside `{{ }}` or `{% if %}` | error naming the variable, file and line |
| Syntax error | error naming the file and line, with the source line |
| `${#var}` in a shell script | **fails** (`{#` starts a comment): wrap in `{% raw %}` |
| CRLF in a template | kept as is |
| Context number `1.90` | rendered `1.9`: pass versions as strings |

## Whitespace policy

`trim_blocks` and `lstrip_blocks` on, so block tags on their own line leave no trace. The policy is checked against the
baseline: the default render of every configuration must equal the frozen snapshot byte for byte.

## Baseline

`runner/scripts/builtins-baseline/baseline.json` is the frozen snapshot of 55 configurations (exit codes, files, manifest
hash). The `baseline` CI job compares against it, so each conversion commit is proven byte for byte.

## Strict-undefined rule

A variable a template reads is always in the context, set by pkl with a default. Optional text is an empty string or a
boolean, never an absent key.
