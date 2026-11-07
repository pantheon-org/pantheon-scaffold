# pklr spike (06-11-2026)

Tests whether pklr (pure Rust pkl evaluator, https://github.com/jdx/pklr) can evaluate `../pkl-spike/template.pkl`.
Tested: pklr 5.2.3 (crates.io) against Apple pkl 0.32.2.

Run: `cargo build -j 2`, then `./target/debug/pklr-spike <file.pkl>` prints the module as JSON, or the error.
`probes/` holds the small pkl files behind each finding. Re-run any of them with the harness and with `pkl eval`.

## Findings

| Construct | pklr 5.2.3 |
|---|---|
| Consumer `amends` an open module and sets a typed property | Works |
| Late binding (`b = a + 1`, amender changes `a`) | Works |
| `read("file")`, `replaceAll`, `Mapping` with `when`, `for`, `toMap().mapValues` | Works |
| Typed `new Class { }` objects with defaults | Works |
| `output { files { ... } }` and renderers (`JsonRenderer`, `YamlRenderer`) | **Ignored.** The result is the module's properties as JSON, so `output.files` yields `{}`. |
| A module-level `local function` that reads an amended property, called inside `new Class { }` | **Bug.** It sees the base module's unamended value and fails with "Cannot find property". Eight-line repro: `probes/u_base.pkl` and `probes/u_c.pkl`. Apple pkl evaluates it correctly. |

## What works

With the function inlined (`probes/t_wa.pkl` and `probes/p_wa.pkl`), pklr evaluates the template. Its JSON for the `generated` property is identical to Apple pkl's (`jq -S` then `diff`), and the agnix toggle adds the file.

## Consequences for a pkl-based generator

- A runner that embeds pklr must take files from a data property such as `generated`, not from `output.files`.
- Avoid module-level functions that read amended properties. Use inline expressions until the pklr bug is fixed. Check upstream before reporting it.
- An embedded pklr removes the dependency on the `pkl` binary, but the evaluator is not complete, so templates must be tested against both.
- Not tested: `package://` imports, `import*` globs, constraints on types, `pkl:` stdlib modules beyond the base.
