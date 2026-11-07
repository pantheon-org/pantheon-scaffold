# hk-style pantheon.pkl spike (06-11-2026)

A repo-level `pantheon.pkl` amends `Config.pkl`, reuses a builtin from `Builtins.pkl` and defines a custom workflow inline, the way `hk.pkl` amends hk's `Config.pkl` and imports its `Builtins.pkl`.

- `Config.pkl`: typed `Workflow`, `Job` and `Step` classes, a `workflows` mapping, `extraFiles`, and a `generated` mapping that renders each workflow to YAML (header comment, then `YamlRenderer`). Output is also written as `output.files` plus a manifest.
- `Builtins.pkl`: `cargoDeny(toolchain)`, a function returning a `Workflow` that mirrors `pantheon-adr`'s `cargo-deny.yml`. Pins live here.
- `pantheon.pkl`: the consumer. `["cargo-deny"] = Builtins.cargoDeny(toolchain)` reuses the builtin, and `["docs-check"] = new Workflow { ... }` is a custom workflow.
- `runner.ts` (copy of the earlier spike): `bun runner.ts pantheon.pkl target` seeds once, keeps a manifest, deletes orphans and sets the executable bit.
- `compare.ts`: parses two YAML files and lists each semantic difference.
- `test.sh`: `bash tmp/pkl-hk-spike/test.sh`, six assertions in a throwaway copy, compared with `pantheon-adr` `origin/main` (`060531b`, fetched 06-11-2026). Set `ADR_REPO` for a different clone path.

Run on 06-11-2026 with Apple pkl 0.32.2 and bun 1.4.2. All six tests pass.

## Findings

| Finding | Detail |
|---|---|
| Builtin and custom workflows work | A builtin is a function, a consumer calls it with its values or amends the result. A custom workflow is a typed object written inline. |
| Rendered builtin is semantically equal to `pantheon-adr` except one key | Upstream `workflow_dispatch:` is YAML null. pkl's `YamlRenderer` with `omitNullProperties` cannot emit it, so the render has `workflow_dispatch: {}`. I expect GitHub Actions to treat both the same, but I have not run it. |
| Byte-identical output is not possible | Differences: key order of `name` and `on`, block lists instead of `[a, b]`, no blank lines, no trailing `# v7.0.1` comment on the pinned `uses:`, `'on'` quoted, and the cron unquoted. Plan decision 1 (verbatim text for byte-for-byte match) conflicts with typed workflows. |
| Override works | Changing `toolchain` in `pantheon.pkl` flows into the rendered `run` step. |
| Orphan deletion and seed once | Same runner, same behaviour as the first spike. |

## pklr (also tested, not part of the tests)

pklr 5.2.3 (`../pklr-spike`) evaluates this config, including `renderValue`. Its `docs-check.yml` is byte-identical to Apple pkl's. Its `cargo-deny.yml` is **not**: the job `cargo-deny` is missing `runs-on: ubuntu-latest`, a class default that Apple pkl keeps. It is silent, so a wrong workflow would be written with no error. Not root-caused: simple class-default probes pass in pklr (`../pklr-spike/probes/v_default.pkl`), so the trigger is something specific to the builtin function. Treat pklr as unsafe for rendering until this is understood.

## Not tested

`package://` publishing and `amends "package://..."`, comments preserved in YAML, and running the rendered workflows in GitHub.
