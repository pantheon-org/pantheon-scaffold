# `upgrade`: moving a repository to a new package and runner version

`pantheon-scaffold upgrade <version> [--root <dir>] [--config <file>] [--dry-run] [--force] [--no-render]`

Added in phase 2b, wave B2. It was built and tested against a faked release (a local server, plus fake `mise` and `gh` programs). The release pipeline's `verify-release` job runs `upgrade --dry-run` against the real release, which is the first time the real download and the real attestation check are used. See [release-process.md](release-process.md).

## What it does, in order

1. Reads the current pins and refuses a version that is not a version, the version already pinned, or an older one (unless `--force`).
2. Downloads `checksums.txt` from the release (`https://github.com/pantheon-org/pantheon-scaffold/releases/download/v<version>/checksums.txt`, in sha256sum format).
3. Runs `gh attestation verify <file> --repo pantheon-org/pantheon-scaffold` on the exact file it downloaded. If that fails, nothing has been written.
4. Reads the checksum of `pantheon-scaffold@<version>` (the package metadata file, which is what pkl's `::sha256:` checks). A missing, repeated or malformed entry stops the upgrade.
5. Rewrites, in one step: the `amends` line in `pantheon.pkl` (the tag, the file name and the checksum) and the runner pin for `github:pantheon-org/pantheon-scaffold` in `mise.toml`. If either rewrite is not possible (no such line, or more than one), neither file changes.
6. Runs `mise install` and then `mise lock`, so the lock file holds the checksums of the new runner for every platform. If either fails, `pantheon.pkl`, `mise.toml` and `mise.lock` are put back exactly as they were (a lock file that did not exist is removed again).
7. Runs `mise x -- pantheon-scaffold render <config> --root <dir>`, so the new runner does the render, and that runner checks the version rules itself. The write lock is released first so that render can take it.

`--dry-run` does steps 1 to 4 and prints what would change. `--no-render` stops after step 6.

## Exit codes

0 done, 1 error (nothing fetched, attestation failed, mise failed and was rolled back, no pin found), 2 usage. When the final render fails, the pins stay updated and `upgrade` exits with the render's own code (3 for a refused file, 4 for drift). Fix what the render reports and run `render` again.

## Limits

- The pin locations are in `runner/src/pins.ts` and nowhere else, so a consumer that keeps the pin in a `PklProject` is a change to that one file (decision 45). The `mise.toml` entry has to be on one line, as `"github:pantheon-org/pantheon-scaffold" = "x.y.z"` or with `version = "x.y.z"` inside an inline table. Multi-line tables are not read, and the runner says so rather than guessing.
- `PANTHEON_SCAFFOLD_RELEASE_BASE` replaces the download address, for tests and mirrors. The attestation is still checked against the repository, so a mirror cannot change what is trusted.
- Needs the `gh` CLI. Whether the consumer CI image has it, and what token it needs, is a question for the Security review of the release pipeline (task 7.5).
- The tool key `github:pantheon-org/pantheon-scaffold` is a working name, confirmed when the binary is published (wave C).
