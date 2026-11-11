# Runner lifecycle: version rules, the cache, the known-bad list and `detach`

How the runner treats the package it renders from, over time. Added in phase 2b, wave B1. `upgrade` is in [runner-upgrade.md](runner-upgrade.md).

## The package version

A released package is loaded from a `package://` URI that carries its version. The package reports that URI in the contract (`packageUri`, set by `Config.pkl` from the module's own location), and the runner reads the version from it. A package loaded from a local folder has no `packageUri`, so none of the rules below apply to it.

A package URI with no readable version is an error, never ignored. After every render the runner records the version in `.pkl-gen/manifest.json` as `packageVersion`.

## Rules checked before anything is rendered

| Rule | Result |
|---|---|
| The package requires a newer runner than this one (`requires.runner`, raised by hand in the release that needs it) | Exit 1, "upgrade the runner". |
| The package is older than the version that last rendered this repository | `render` refuses with exit 3 unless `--force`. `check` only warns, because it writes nothing. A pre-release of the recorded version counts as older. |
| The package version is on the known-bad list | Warning with the reason. With `--strict`, exit 3 before anything is written. |
| A required Security builtin is disabled (`securityBuiltins` in the contract) | Warning for each one. `--strict` does not change it. |

Versions are compared as semantic versions, not as text. A pre-release ranks below its release, so a runner `0.1.0-rc.1` is older than a requirement of `0.1.0`. Keep `requires.runner` at a version that pre-releases of the next release can meet: CI builds `0.0.1-ci.N` against the current requirement of `0.0.0`, and the smoke suite fails if it does not. `--repair` skips reading the manifest first, because it exists to fix one that cannot be read.

## The package cache

The runner passes `--cache-dir` to pkl, by default `.pkl-gen/cache` under the repository root (override with `--cache-dir <dir>`). A warm cache lets pkl render with no network, which is what spike S.1 showed. In consumer CI, keep the folder with `actions/cache` so an outage does not block the job. Add `.pkl-gen/cache/` to `.gitignore`.

## The known-bad list

A JSON document of `{ "versions": [{ "version": "x.y.z", "reason": "..." }] }` kept on the package repository's `main` branch (`known-bad.json`). The runner only fetches it when the package has a version.

1. It fetches the list (5 second limit) and writes it to `.pkl-gen/cache/known-bad.json`.
2. If the fetch fails or the text is not a valid list, it uses the cached copy and prints a note.
3. With no cache it uses the snapshot bundled in the runner (the repository's `known-bad.json`, compiled in when the binary is built, so it can be behind the list on `main`) and prints a note.

A failed fetch never stops a run. The list can only produce a warning, or a refusal with `--strict`. `PANTHEON_SCAFFOLD_KNOWN_BAD_URL` overrides the address, for tests and mirrors. The package repository publishes `known-bad.json` on `main`. If the fetch fails the runner falls back with a note.

## `detach`

`pantheon-scaffold detach [--root <dir>] [--dry-run] [--force]` ends management of a repository. It needs no `pantheon.pkl` and no network.

- It deletes the manifest, and the cache, stage and lock folders under `.pkl-gen/`.
- It deletes the owned files the package marked `removeOnDetach` (the consumer upgrade workflow, which would fail on every run once detached). A file edited by hand is refused unless `--force`.
- Deleting those files goes through the same delete guard as a render (clean tracked tree, tracked files only, no sparse or partial checkout). With no such file, git is not needed.
- Every other file stays as a plain file. A later render adopts files that still match.
- Exit 0 done, 1 error (no manifest, lock held), 3 refused.
