# The runner binary: build and smoke suite

Added in phase 2b, wave C; the build changed in phase 2, task 2.5. The runner is a Rust program (`crates/scaffold-cli`), built to one standalone binary per target: `linux-x64`, `linux-arm64` and `darwin-arm64` (decisions 15 and 34). Releases up to 0.1.5 carry the earlier TypeScript build, which is still in `runner/` and still built by `runner/scripts/build.ts` for the pull request checks that compare the two; the first release cut after task 2.5 is the first to carry the Rust one.

## Build

`bun runner/scripts/build-rust.ts --target <linux-x64|linux-arm64|darwin-arm64> [--version <x.y.z>] [--out <dir>]`

- It builds on a machine of the target and nowhere else: Rust does not cross-compile to macOS from Linux, and a native build can be run at once. The release workflow and `ci.yml` run it on `ubuntu-latest`, `ubuntu-24.04-arm` and `macos-latest`.
- One binary per target, named `pantheon-scaffold-<target>`, as before, so `mise` picks the same files. 4.9 MB for macOS and 5.8 to 6.2 MB for the static Linux builds (measured on the release dry run of 11-11-2026, which musl makes larger than the macOS one because libc is inside), against about 60 MB for the TypeScript build.
- The Linux binaries are **static musl builds** (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`): libc is inside the binary, so they run on any Linux distribution whatever its glibc. A normal glibc build would only run on distributions at least as new as the build machine. The macOS binary is a normal build. The C code of `ring` (used by `rustls`) is compiled against musl by `zig cc`: `build-rust.ts` writes a small wrapper script per target (`target/zig-cc/cc-<triple>`) that runs `zig cc -target <arch>-linux-musl` and drops the `--target` flag the `cc` crate adds. Zig comes from mise, pinned in `mise.rust-build.toml` and checksummed in `mise.rust-build.lock`; only the jobs that build a release binary set `MISE_ENV=rust-build`, so no other job downloads it. Run the script locally with `MISE_ENV=rust-build mise x -- bun runner/scripts/build-rust.ts ...`.
- `contract.schema.json` and the repository's `known-bad.json` are compiled in, so a binary reads nothing from the source tree.
- `--version` sets the version the binary reports, at compile time (`SCAFFOLD_VERSION`). Built without it, a binary reports `0.0.0`. The runner and the package are released together under one version.
- After building, the script checks its own output: the binary reports the version it was built for, and on Linux `ldd` says the binary is static. It fails the build if either is wrong.
- `checksums.txt` is written by the release workflow (`runner/scripts/release-assets.ts checksums`), in sha256sum format, after it has the package files and the three binaries in one folder. The release pipeline attests the file, and `upgrade` reads it (see [runner-upgrade.md](runner-upgrade.md)).

The Rust build downloads crates, whose versions `Cargo.lock` fixes (`--locked`) and `cargo-deny` checks in CI, and downloads Zig through mise (a checksummed tarball from ziglang.org) in the release-binary jobs. Those are supply-chain inputs for any Security review of the release pipeline (task 7.5).

## Smoke suite

`bun runner/scripts/smoke.ts <binary> [--version <x.y.z>] [--repo <checkout>]`

It runs the compiled binary from a directory unrelated to the source tree, against the fixtures and the pkl package of a checkout, and needs `pkl` and `git` on the PATH. 16 checks:

- the version, a usage error, and `contract` from an unrelated directory (the schema is inside the binary)
- `render` with the executable bit, a second render that changes nothing, `check`, drift (exit 4), a refused hand edit (exit 3), `--force`, `detach`
- a held lock (exit 1)
- two paths that differ only by case (exit 1), and a case-only rename (exit 3, old file kept)
- orphan deletion in a clean git repository, and the refusal in a dirty one (git works inside the binary)

It prints the filesystem it ran on. CI runs it on `ubuntu-latest` (linux-x64, case-sensitive), `ubuntu-24.04-arm` (linux-arm64, case-sensitive) and `macos-latest` (darwin-arm64, case-insensitive by default), on the TypeScript build, built once on Linux and passed over as an artifact, so the tested file is the built file. A second set of jobs, `Rust binary <target>`, builds the Rust release binary natively on each of the three machines and runs the same suite against it. The release workflow does the same: each target is built on its own machine and uploaded, and the smoke job downloads the binary of its own target.

## Case-only renames

If the contract renames an owned file only by case (`readme.md` to `README.md`), the runner refuses, whatever the filesystem, and `--force` does not override it. On a case-insensitive filesystem both names are one file, so deleting the orphan would delete the file just rendered. Rename it by hand and render again.

## Not in this wave

Installing the binary through mise, the checksums in the consumer's lock file and the build provenance attestation need a published release. They are done by the release pipeline and verified after publication (see [release-process.md](release-process.md)).
