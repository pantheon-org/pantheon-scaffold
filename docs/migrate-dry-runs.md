# `migrate` dry runs on the four `pantheon-*` repositories

Written for the owner, who has to choose the first repository to adopt (open question 3 of the refocus plan) and decide what to build next. It records what [`migrate`](migrate.md) says about each repository on 12-11-2026. Nothing was written to any of them: each run was a dry run on a copy of `origin/main` made with `git archive` in a new directory, and each clone's branch, HEAD and working tree were the same afterwards.

## How it was run

The runner was built from `main` of this repository at cf9d615 and used the package in the same checkout (`--allow-local-package pkl`), so the package is the unreleased 0.1.8, not the 0.1.7 release. Each repository was run with the options its test fixture in this repository sets. Checked against `origin/main` of each repository on 12-11-2026:

| Repository | Commit | Date of the commit | Options given to `migrate` |
|---|---|---|---|
| `pantheon-adr` | 1e2fd46 | 07-11-2026 | `--toolchain 1.96.1 --binary --crate-name adr --skills --skill-name adr-creator`, and `--git-dependency-ignore` for `pahdk-core` and `pahdk-install` |
| `pantheon-journal` | 05c3d84 | 07-11-2026 | the same, with crate `journal` and skill `journal-entry-creator` |
| `pantheon-skill-auditor` | 4e3e9ff | 09-11-2026 | the same, with crate `skill-auditor`, skill `skill-quality-auditor`, and `--composite-action` |
| `pantheon-agentic-harness-dev-kit` | 42f48b4 | 07-11-2026 | `--toolchain 1.96.1` only (no binary, no skill) |

Every run exited 0 and gave the same output when run twice.

## What each repository would get

| Class | adr | journal | skill-auditor | dev-kit |
|---|---|---|---|---|
| `identical` | 36 | 36 | 36 | 31 |
| of which equal as YAML but in other text | 13 | 13 | 13 | 11 |
| `drifted` | 5 | 5 | 5 | 5 |
| `new` | 4 | 4 | 4 | 3 |
| `per-repo` | 7 | 7 | 8 | 7 |
| `seed-kept` | 2 | 2 | 2 | 2 |
| `external` | 2 | 2 | 2 | 1 |

**`new`** is the same in the three binary repositories: `scaffold-check.yml`, `.mise/conf.d/repo.toml`, `hk.shared.pkl` and `dist-check.yml`. The dev-kit has no binary, so no `dist-check.yml`.

**`per-repo`** is what the standard set does not render: `release-please-config.json`, `.release-please-manifest.json`, `Cargo.toml`, `Cargo.lock`, `README.md`, `LICENSE`, plus `CLAUDE.md` (the three binary repositories), `action.yml` (skill-auditor) and `CHANGELOG.md` (dev-kit).

## The drifted files, by cause

Every drifted file has one of three causes: **stale** (the repository was rendered before a change in the package, so it only needs the package's current file), **option** (the repository sets an option that `migrate` was not given) or **divergence** (the repository chose a different file on purpose). The first and last columns say which repositories have it.

| File | Repositories | Cause | What differs |
|---|---|---|---|
| `.github/dependabot.yml` | all four | stale | The package now lists the workflows it owns under `exclude-paths` (0.1.8), and, with a binary, ignores the two artifact actions. The repositories have neither. |
| `.github/workflows/plumber.yml` | all four | stale | The pin of `getplumber/plumber`: the repositories have an older commit than the package. |
| `mise.toml` | all four | stale | The package's comments on zizmor and agnix (pinned in `Tools.pkl`), the runner pin (`github:pantheon-org/pantheon-scaffold`, shown as 0.0.0 here because the package was a folder, not a release), and the lock settings `lockfile`, `locked_verify_provenance` and `lockfile_platforms`, which the repositories lack. Compared line by line for `pantheon-adr`: the tool versions are the same. The other three show the same first differences. |
| `.gitignore` | all four | stale | The package adds `.pkl-gen/cache/`. The dev-kit also writes `/target` where the package writes `target/` (the same ignore). |
| `dist-workspace.toml` | adr, journal, skill-auditor | stale | The commits of `upload-artifact` and `download-artifact`: both are at the same major version (v7, v8), at different commits. |
| `.github/workflows/rust-ci.yml` | dev-kit | option | The dev-kit builds with `--all-features` and has an extra "Build with default features" step. That is what the package renders with `features = "all"`. Checked: with that option its `rust-ci.yml` jobs equal the repository's. `migrate` has no flag for it yet. |

No file is a divergence. Counting the cosmetic `/target`, nothing in the four repositories differs from the Builtins on purpose.

## What this means

- **Adopting is mostly accepting the package's files.** All the drift is the repositories being older than the package, plus one option. A Preset would freeze the old file, which is the opposite of what these repositories need.
- **The first adopter.** The three binary repositories have the same four drifted files and the same four new ones, so they are alike. The dev-kit has no binary and no skill, so it has fewer files and fewer ways to go wrong, but it needs the `features = "all"` option first. The record does not choose; that is the owner's call.

## Findings, not fixed here

1. **`migrate --write --as-presets` exits 1 on all four repositories.** The Preset for `plumber.yml` holds the repository's file, and the package's pin check (`versions.rs`) refuses a `uses:` line whose commit is not one of the package's pins: `no version known for getplumber/plumber@<commit>`. Any repository with an older pin than the package hits this. The check exists so a pin never loses its version comment; how a verbatim Preset should treat an older pin is a design choice.
2. **`migrate` and `init` have no `--features` option.** The package has `features = "all"`, and the dev-kit needs it.
3. **There is no way to accept the package's file for a drifted file.** `migrate --write` refuses while a file drifted, and `--as-presets` keeps the old file. For stale drift, the wanted outcome is to replace the file with the Builtin's and record the difference. That is a mode `migrate` does not have yet.

   *Note, 13-11-2026:* `migrate --accept-builtins` now exists (see [migrate.md](migrate.md#replacing-drifted-files---accept-builtins) and [migrate-accept-builtins-design.md](migrate-accept-builtins-design.md)). The dry runs above were not re-run. The one file that drifts because of a missing option (the dev-kit's `rust-ci.yml`) must wait for `--features`.

## Repeat it

Run `migrate` (without `--write`) in a copy of the repository with the options in the first table, and `--allow-local-package <package folder>` when working on the package. It is read-only. Each output is the list of files with their class, then a line with the four counts.
