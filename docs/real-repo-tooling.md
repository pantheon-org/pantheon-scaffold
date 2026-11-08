# Real-repository render of the Phase 3 files

Wave D of Phase 3. `runner/scripts/real-repo-tooling.ts` clones each of the four Rust repositories (https, depth 1) into a temporary folder, renders everything Phase 3 owns into it, and judges the result. It writes only in the temporary folder, and nothing is pushed. It needs network, git, and installed copies of hk 2.10.0, pkl 0.32.2, rumdl 0.2.77, shellcheck, yq, actionlint and the Rust toolchain the repositories pin, found without mise shims. Run it with `bun runner/scripts/real-repo-tooling.ts`.

Run on 08-11-2026 against `main` of each repository: **PASS for all four.**

| Repository | Commit | `features` | Result |
|---|---|---|---|
| `pantheon-adr` | `1e2fd46` | default | PASS |
| `pantheon-journal` | `05c3d84` | default | PASS |
| `pantheon-skill-auditor` | `b9e706e` | default | PASS |
| `pantheon-agentic-harness-dev-kit` | `42f48b4` | all | PASS |

## What the script checks, per repository

1. The repository's `mise.toml` is the pinned `pantheon-adr` copy (blob `16c264c`), and its `hk.pkl` is one of the two pinned copies (the `pantheon-adr` one, or the library repository's). Anything else would be an uncategorised difference.
2. A first render without `--force` adopts the files that are identical and refuses only `mise.toml`. In all four repositories it adopted 14 files unchanged and refused 1.
3. `check` before any render reports exactly three differences: `.github/scripts/sync-labels.sh: mode`, `hk.shared.pkl: missing` and `mise.toml: different`.
4. After `render --force`, and after the one-off migration that replaces the repository's `hk.pkl` with the seeded one, `git status` lists exactly six paths and nothing else: `.github/scripts/sync-labels.sh` (mode only, 0 lines changed), `.mise/conf.d/repo.toml`, `.pkl-gen/manifest.json`, `hk.pkl`, `hk.shared.pkl` and `mise.toml`. `mise.toml` changes by exactly 10 lines added and 4 removed, the set `mise.test.ts` lists line by line.
5. `hk check --all --no-fail-fast` gives the same exit code and the same result for every step before and after.
6. `check` after the render exits 0, and a second render writes nothing.

## What differs from today, in all four repositories alike

| Path | Difference | Category |
|---|---|---|
| `mise.toml` | Two comments reworded, the runner pin, `lockfile`, `locked_verify_provenance`, `lockfile_platforms` | Intended, listed line by line in `docs/tooling-layer.md` |
| `.github/scripts/sync-labels.sh` | Mode `100644` to `100755`, no content change | Named difference, owner can reverse it |
| `hk.shared.pkl` | New: the shared steps | Intended (decision 31) |
| `hk.pkl` | Replaced by the seeded file that imports `hk.shared.pkl` | The one-off migration, phase 7 |
| `.mise/conf.d/repo.toml`, `.pkl-gen/manifest.json` | New | Intended |

The other 13 verbatim files, the nine shared scripts included (apart from the mode above), `.rumdl.toml`, `.claude/settings.json`, `deny.toml` and `rust-toolchain.toml`, are byte for byte what the repositories have.

## Not caused by the render: `rumdl` fails in all four repositories today

`hk check --all` exits 1 in every repository before the render, because `rumdl` reports findings in their own Markdown. The render changes none of this (same result before and after). Counts with the pinned `rumdl 0.2.77` and each repository's `.rumdl.toml`, over every tracked Markdown file:

| Repository | Findings | Files with findings |
|---|---|---|
| `pantheon-adr` | 5 | 2 of 16 |
| `pantheon-journal` | 7 | 5 of 30 |
| `pantheon-skill-auditor` | 48 | 32 of 131 |
| `pantheon-agentic-harness-dev-kit` | 3 | 2 of 4 |

The pre-commit hook only checks staged files, so this does not show in daily use. It would show in CI wherever `hk check --all` is run. This is outside Phase 3 and nothing here fixes it.

## Limits

- Only the default branch is read, with no open pull requests, and only what `hk check --all` covers. `clippy` and `test` (the pre-push steps) are not run.
- The tools are the ones installed on the machine that ran it. The Rust toolchain is the repositories' pinned 1.96.1.
- The migration is simulated by replacing `hk.pkl` in the scratch clone. It is not applied anywhere.
