# `upgrade`: moving a repository to a new package and runner version

`pantheon-scaffold upgrade <version> [--root <dir>] [--config <file>] [--dry-run] [--force] [--no-render]`

Added in phase 2b, wave B2. It was built and tested against a faked release (a local server, plus fake `mise` and `gh` programs). The release pipeline's `verify-release` job runs `upgrade --dry-run` against the real release, which is the first time the real download and the real attestation check are used. See [release-process.md](release-process.md). Since 13-11-2026 it has also been run for real, not as a dry run, from one published release to the next with a Preset present: see [Checked on the real releases](#checked-on-the-real-releases) below.

## What it does, in order

1. Reads the current pins and refuses a version that is not a version, the version already pinned, or an older one (unless `--force`).
2. Downloads `checksums.txt` from the release (`https://github.com/pantheon-org/pantheon-scaffold/releases/download/v<version>/checksums.txt`, in sha256sum format).
3. Runs `gh attestation verify <file> --repo pantheon-org/pantheon-scaffold` on the exact file it downloaded. If that fails, nothing has been written.
4. Reads the checksum of `pantheon-scaffold@<version>` (the package metadata file, which is what pkl's `::sha256:` checks). A missing, repeated or malformed entry stops the upgrade.
4a. Lists the Presets that move with the config: every `.pkl` file under `presets/` next to it that imports the package gets the same version and checksum as the config's imports (a Preset left behind makes pkl stop with `Module version conflict`). A Preset with no import of the package, or already at the new version, is left alone. The report has one line per Preset (`presets/ci.pkl: 2 imports of the package`). See [presets.md](presets.md).
5. Rewrites, in one step: the `amends` line in `pantheon.pkl` (the tag, the file name and the checksum), **every `import` and `import*` of the same package in that file** (`Builtins.pkl` and the like, with the same version and checksum, so the render never mixes two versions of the package; an import of any other package or a local file is left alone), and the runner pin for `github:pantheon-org/pantheon-scaffold` in `mise.toml`. If either rewrite is not possible (no such `amends` line, or more than one), neither file changes. The report says how many imports moved. Before 0.1.4 only the `amends` line moved, which broke the render of any consumer that imports the builtins (found in Phase 7). When `mise.toml` is a file the package owns (`Builtins.miseFiles`) and still matches the manifest, `upgrade` records the text after the pin edit in `.pkl-gen/manifest.json`, so the render that follows does not refuse it as a hand edit. A `mise.toml` edited by hand keeps its old hash and is still refused. Before 0.1.5 the render in `upgrade` exited 3 for every such consumer (found by the Phase 7 benefit test).
5a. Evaluates the config as it now reads, with its Presets, before `mise` runs. If it cannot be evaluated, for instance because a Preset names a workflow or job that the new package no longer has, the upgrade stops with exit 1 and `pantheon.pkl does not evaluate with <version>, so nothing was changed:` followed by pkl's message, and `pantheon.pkl`, `mise.toml`, the Presets and the manifest are put back as they were. It checks that the config evaluates, not that its contract is one this runner understands. `--dry-run` does not evaluate, because it writes nothing.
6. Runs `mise install` and then `mise lock`, so the lock file holds the checksums of the new runner for every platform. If either fails, `pantheon.pkl`, `mise.toml` and `mise.lock` are put back exactly as they were (a lock file that did not exist is removed again). After `mise lock` the runner's entries in the lock are checked: a platform the runner is not built for, or the wrong asset for one it is, fails the upgrade and puts everything back too, with the `lockfile_platforms` setting to add (see [runner-install.md](runner-install.md)).
7. Runs `mise x -- pantheon-scaffold render <config> --root <dir>`, so the new runner does the render, and that runner checks the version rules itself. The write lock is released first so that render can take it.

`--dry-run` does steps 1 to 4 and prints what would change. `--no-render` stops after step 6.

## Exit codes

0 done, 1 error (nothing fetched, attestation failed, mise failed and was rolled back, no pin found), 2 usage. When the final render fails, the pins stay updated and `upgrade` exits with the render's own code (3 for a refused file, 4 for drift). Fix what the render reports and run `render` again.

## Limits

- The pin locations are in `crates/scaffold-core/src/pins.rs` and nowhere else, so a consumer that keeps the pin in a `PklProject` is a change to that one file (decision 45). The `mise.toml` entry has to be on one line, as `"github:pantheon-org/pantheon-scaffold" = "x.y.z"` or with `version = "x.y.z"` inside an inline table. Multi-line tables are not read, and the runner says so rather than guessing.
- `PANTHEON_SCAFFOLD_RELEASE_BASE` replaces the download address, for tests and mirrors. The attestation is still checked against the repository, so a mirror cannot change what is trusted.
- Needs the `gh` CLI. Whether the consumer CI image has it, and what token it needs, was meant for a Security review of the release pipeline. That review did not happen: it is a recorded accepted risk (decision 78, see [release-process.md](release-process.md)).
- The tool key is `github:pantheon-org/pantheon-scaffold` (`RUNNER_TOOL` in `crates/scaffold-core/src/pins.rs`). It is the key of the published runner, and the release's `verify-release` job installs the runner through it.

## Going back to an older version

`upgrade <older> --force` moves the pins back, but the render that follows is refused with exit 3, because the package is older than the one that last rendered this repository (the version rule in [runner-lifecycle.md](runner-lifecycle.md)). `upgrade` does not pass `--force` on to the render: it would also overwrite hand edits. Run `pantheon-scaffold render pantheon.pkl --force` once the pins are back, and `check` then passes. This was run end to end for 0.1.3 back to 0.1.1 (Phase 7, [proof-report-0.1.3.md](../archive/proof/docs/proof-report-0.1.3.md)).

If the new package drops or renames an owned file, the render after an upgrade is also refused until the pin change is committed, because the runner deletes an orphan only from a clean tracked tree. Commit the pins, then run `render`.

## Checked on the real releases

On 13-11-2026, at the owner's request, an agent upgraded a throwaway git repository from 0.1.7 to 0.1.8 with the published 0.1.7 runner, against the real 0.1.8 release (a pre-release at the time), in an isolated tool environment on macOS arm64. The repository was made by `init --package-version 0.1.7` and had a real Preset, `presets/ci.pkl`, that imports the package by its published address and raises `timeout-minutes` of the `build-test` job of `rust-ci` to 30 (the example in [presets.md](presets.md)). Tool versions: `mise` 2026.11.1, `gh` 2.103.3, and `pkl` 0.32.2 for `init` and 0.33.0 for the rest (the version the rendered `mise.toml` resolves).

What held:

- `upgrade 0.1.8` exited 0. Its report named `presets/ci.pkl: 2 imports of the package`, the move of `pantheon.pkl` and of the runner pin in `mise.toml` from 0.1.7 to 0.1.8, and one re-rendered file.
- Both imports of the Preset, the `amends` line and the Builtins import carried 0.1.8 and one checksum, equal to the `pantheon-scaffold@0.1.8` line of the attested `checksums.txt`.
- `mise.lock` named only the three runner binaries, and the runner `mise` installed printed 0.1.8.
- `check` exited 0, listed `preset presets/ci.pkl`, and printed no "changed since the last render" warning. The manifest held a new hash of the Preset, `rust-ci.yml` still had `timeout-minutes: 30`, and a second `render` wrote 0 files.
- Only six files differed from the committed state: `pantheon.pkl`, `presets/ci.pkl`, `mise.toml`, `mise.lock`, `.pkl-gen/manifest.json` and `.github/dependabot.yml`. The last one changed because the only package change between the two releases is the Dependabot exclusion list.
- The one-week `minimum_release_age` in the rendered `mise.toml` did not stop `mise install` of the exact 0.1.7 and 0.1.8 pins, although both releases were less than a week old.

What it did not exercise:

- The path in step 5a that stops with exit 1 when the new package breaks a Preset: `rust-ci` and `build-test` exist in both releases.
- Downgrade, `detach` and `migrate`.
- Running the runner on a platform other than macOS arm64. The lock covered linux-x64, linux-arm64 and macos-arm64.
