# Builtins baseline: render a tree and compare it with origin/main at 0b466dc

These scripts render a tree's own `scaffold.pkl`, every `runner/test/fixtures/*/scaffold.pkl` and an option matrix with one binary, and compare the
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
runner/scripts/builtins-baseline/check.sh /tmp/baseline /tmp/scaffold-0b466dc/target/debug/pantheon-scaffold
```

The default is `--api new` (`toolingFiles(options)`); pass `--api old` for the baseline tree, where `toolingFiles()` takes no argument. Expect `55 configurations compared, 0 differences`.
The binary is the same for both sides on purpose: a change to the Rust runner would otherwise hide inside the comparison.

## The frozen snapshot, and CI

`baseline.json` is the baseline in a form that needs no old tree and no old binary: for each of the 61 configurations (the 55 of the baseline above and six with builtins left out, `w-*`), the exit codes of `contract`, `render` and `check`, every
file written (path, mode, sha256) and the sha256 of the manifest. Messages are left out on purpose: a message may change when the runner changes, what a repository gets must not.
It was made from main at 1259a90, where it equalled the baseline above (0 differences), was extended with the six `w-*` configurations for the presence work, and was refreshed once for the rename of `pantheon.pkl` to `scaffold.pkl`, which changes the text of four rendered files (the plumber and hk headers, scaffold-check and upgrade), and once more for the plural class modules, where only the root manifest hash moved because it records the hashes of this repository's own `presets/*.pkl`, which were edited (no rendered file changed). It was refreshed again for the release Builtins (`releaseShape`, `releaseAutomerge`, `versionFile`, the seeded `release-please-config.json` and `.release-please-manifest.json`, the schedule gate of `release-automerge.yml`): 29 of the 61 configurations changed, only in `release-automerge.yml`, the two seeds (the configurations that render `release-please`), the manifest hash, and, for this repository's own render, `publish-release.sh`, `release-bump.yml`, `release-bump.sh`, `release-please.yml` and `dependabot.yml`. The fixture that is compared with the pantheon-adr copy sets `releaseAutomerge = "always"`, so its `release-automerge.yml` did not change. The per-configuration list was kept as evidence of the refresh. The rename was proved first by rendering `origin/main` with the same substitution applied to its sources and comparing the two snapshot folders: 55 configurations on both sides, 0 differences. To refresh it:

```bash
command bun runner/scripts/builtins-baseline/freeze.ts /tmp/baseline > runner/scripts/builtins-baseline/baseline.json
```

The `baseline` job of `.github/workflows/ci.yml` builds the Rust runner, renders the 61 configurations with `snapshot.ts` and compares them with `baseline.json` using `compare.ts`.
Run the same two commands locally to check a change:

```bash
command bun runner/scripts/builtins-baseline/snapshot.ts . target/debug/pantheon-scaffold /tmp/candidate
command bun runner/scripts/builtins-baseline/compare.ts runner/scripts/builtins-baseline/baseline.json /tmp/candidate
```

Refresh `baseline.json` only when a change is meant to alter what a repository renders, in the same commit as that change so no commit has a red baseline job, and say why in the pull request.
