# Builtins baseline: render a tree and compare it with origin/main at 0b466dc

These scripts render a tree's own `pantheon.pkl`, every `runner/test/fixtures/*/pantheon.pkl` and an option matrix with one binary, and compare the
result with a baseline render field by field. They are how the Builtins split (pull request 116) and the presence work show that the default render
did not change: same binary, same inputs, only `pkl/` differs.

| Script | What it does |
|---|---|
| `snapshot.ts` | Renders a tree (`snapshot.ts <tree> <binary> <out dir> [--api old\|new]`) into one JSON file per configuration: exit codes, stdout, stderr, files (path, mode, sha256) and the manifest. `--api` is how the matrix calls the aggregators that gained a parameter. |
| `compare.ts` | Compares two snapshot folders. pkl stack frames, line numbers and source snippets may differ; nothing else may. Exit 1 on any difference. |
| `check.sh` | `snapshot.ts` on the current directory, then `compare.ts` against a baseline. |
| `packaged.ts` | Builds the package, serves it locally in place of GitHub, renders through the package URI and compares it with the folder render. |

Run them with `command bun` (`bun` is an alias in some shells).

## Making the baseline

The baseline is the render of `origin/main` at `0b466dc`, the last commit before the Builtins split, made with the binary built from that commit.

```bash
git archive 0b466dc | tar -x -C /tmp/scaffold-0b466dc
(cd /tmp/scaffold-0b466dc && cargo build -p scaffold-cli)
command bun runner/scripts/builtins-baseline/snapshot.ts /tmp/scaffold-0b466dc /tmp/scaffold-0b466dc/target/debug/pantheon-scaffold /tmp/baseline --api old
```

The 55 JSON files in `/tmp/baseline`, concatenated in name order, have sha256 `3dcaa4ca3915f0dd06ae4495e995b39348e6bf4f9d4c759f7bc7dea521d00b97`
(`cd /tmp/baseline && cat $(ls *.json | sort) | shasum -a 256`). That hash is the reference: if it differs, the baseline was not made from the same inputs.

## Checking a tree

```bash
runner/scripts/builtins-baseline/check.sh /tmp/baseline /tmp/scaffold-0b466dc/target/debug/pantheon-scaffold "" --api old
```

Use `--api old` on a tree where `toolingFiles()` takes no argument and `--api new` after the presence work. Expect `55 configurations compared, 0 differences`.
The binary is the same for both sides on purpose: a change to the Rust runner would otherwise hide inside the comparison.
