# The runner binary: build and smoke suite

Added in phase 2b, wave C. The runner is TypeScript on bun, compiled to one standalone binary per target: `linux-x64`, `linux-arm64` and `darwin-arm64` (decisions 15 and 34).

## Build

`bun runner/scripts/build.ts [--version <x.y.z>] [--out <dir>] [--targets linux-x64,linux-arm64,darwin-arm64]`

- One binary per target, named `pantheon-scaffold-<target>`, cross-compiled by `bun build --compile` (about 60 to 80 MB each, built in under two seconds).
- `contract.schema.json` and the bundled known-bad snapshot are embedded, so a binary reads nothing from the source tree. The schema used to be found through `import.meta.dir`, which does not exist inside a compiled binary.
- `--version` sets the version the binary reports (`bun build --define BUILD_VERSION`). From source it reports `0.0.0`. The runner and the package are released together under one version.
- `checksums.txt` lists the binaries in sha256sum format. The release pipeline adds the package's checksum and attests the file (wave D), and `upgrade` reads it (see [runner-upgrade.md](runner-upgrade.md)).

Cross-compiling downloads the bun runtime for each target at build time. That is a supply-chain input for the Security review of the release pipeline (task 7.5).

## Smoke suite

`bun runner/scripts/smoke.ts <binary> [--version <x.y.z>] [--repo <checkout>]`

It runs the compiled binary from a directory unrelated to the source tree, against the fixtures and the pkl package of a checkout, and needs `pkl` and `git` on the PATH. 16 checks:

- the version, a usage error, and `contract` from an unrelated directory (the schema is inside the binary)
- `render` with the executable bit, a second render that changes nothing, `check`, drift (exit 4), a refused hand edit (exit 3), `--force`, `detach`
- a held lock (exit 1)
- two paths that differ only by case (exit 1), and a case-only rename (exit 3, old file kept)
- orphan deletion in a clean git repository, and the refusal in a dirty one (git works inside the binary)

It prints the filesystem it ran on. CI runs it on `ubuntu-latest` (linux-x64, case-sensitive), `ubuntu-24.04-arm` (linux-arm64, case-sensitive) and `macos-latest` (darwin-arm64, case-insensitive by default), after building the binaries once on Linux and passing them over as an artifact, so the tested file is the built file.

## Case-only renames

If the contract renames an owned file only by case (`readme.md` to `README.md`), the runner refuses, whatever the filesystem, and `--force` does not override it. On a case-insensitive filesystem both names are one file, so deleting the orphan would delete the file just rendered. Rename it by hand and render again.

## Not in this wave

Installing the binary through mise, the checksums in the consumer's lock file and the build provenance attestation need a published release. They are done by the release pipeline and verified after publication (see [release-process.md](release-process.md)).
