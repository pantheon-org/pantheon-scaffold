# The tooling layer

The files the four Rust repositories share for local tooling, rendered by `Builtins.toolingFiles()` and `Builtins.baseFiles()`. Phase 3 of the plan. This page grows wave by wave: wave A (verbatim files) is here, `mise.toml` and the hook steps follow.

## Wave A: verbatim files

Every file is copied byte for byte from `pantheon-adr` at commit `1e2fd46`, and a test compares the git blob hash of what pkl renders with the pinned upstream blob (`runner/test/upstream/tooling-blobs.json`). All nine files were identical in the four repositories when they were read (baseline SP-0.1, re-checked on 08-11-2026).

| File | Mode | Comes from |
|---|---|---|
| `.github/scripts/agnix-assemble-comment.sh`, `agnix-file-issues.sh`, `agnix-introduced-errors.sh` | 0755 | `toolingFiles()` |
| `.github/scripts/auto-rebase.sh`, `check-commit-types.sh`, `release-automerge.sh`, `sync-labels.sh` | 0755 | `toolingFiles()` |
| `.github/scripts/cargo-deny.sh`, `check-inline-scripts.sh` | 0755 | `baseFiles()` (phase 2a) |
| `.rumdl.toml`, `.claude/settings.json` | 0644 | `toolingFiles()` |

Those are the nine shared scripts: seven from `toolingFiles()` and two from `baseFiles()`. `check-toolchain-version.sh` (also from `baseFiles()`) and `tag-release.sh` (which differs between repositories) are not part of the nine.

### One named difference: `sync-labels.sh` is executable

In all four repositories `sync-labels.sh` is committed as `100644`, and its workflow runs it as `bash .github/scripts/sync-labels.sh`, so the bit has never mattered. The plan says the nine scripts are executable, so it is rendered `0755` like the other eight. In a repository that adopts the package, `check` reports one mode difference for it until the bit is committed (`git update-index --chmod=+x`). `tooling.test.ts` asserts it is the only script that differs.

## The executable bit as git records it

The runner never reads the git index (it calls git only for the delete guard). The plan's "CI check on `git ls-files -s`" is met a different way: in CI, `check` runs on a checkout, where the mode on disk is the mode in the index, so a committed mode that is wrong is reported as mode drift (exit 4). `runner/test/mode-index.test.ts` shows this with real git: after a render and a commit `git ls-files -s` lists the nine scripts as `100755`, a fresh clone passes `check`, and a script committed as `100644` fails it with `mode`.
