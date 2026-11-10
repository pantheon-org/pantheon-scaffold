# Features

The behaviour of the runner, written as cucumber features. They are black-box: each scenario runs a binary and looks at its exit code, its output and the files it leaves, so the same files run against any build of the runner. Today that is the TypeScript build. The Rust build, when it exists, has to pass the same files (Phase 1 and 2 of the refocus plan).

## Run them

Build a binary, name it in `SCAFFOLD_BIN`, and run the harness. It needs `pkl` and `git` on the PATH, and Rust 1.96.1 (see `rust-toolchain.toml`).

```bash
mise x -- bun runner/scripts/build.ts --out dist --targets darwin-arm64
SCAFFOLD_BIN=dist/pantheon-scaffold-darwin-arm64 cargo test -p scaffold-features
```

Use the target that matches your machine: `linux-x64`, `linux-arm64` or `darwin-arm64`. CI runs the features on all three after the build job.

## Write a scenario

The step definitions are in `crates/scaffold-features/tests/features.rs`. They stay thin and say nothing about the language of the runner. A command line may use `<repo>` (the repository under test), `<package>` (this repository's `pkl` folder) and `<fixture:NAME>` (the `pantheon.pkl` in the fixture folder of that name under `runner/test/fixtures`). In a file written with `I write "..." to the file "..."`, `\n` is a newline.

| File | What it covers |
|---|---|
| `render.feature` | `render`, `check` and `detach` on a fresh repository: the files written, the executable bit, a second render, drift, a hand edit refused, `--force`. |
| `cli.feature` | `--version`, a usage error, an unknown option, `contract`. |
| `safety.feature` | The lock, an existing file that is not adopted, `--dry-run` and its exit code, a manifest that is not JSON or from a newer format, case collisions and a case-only rename, deleting an orphan (clean git tree, dirty tree, no git). |

`--version` is checked against `SCAFFOLD_VERSION` (default `0.0.0`, which is what `runner/scripts/build.ts` builds unless told otherwise). CI sets it to the version it built.

## The pkl version

Both runners read pkl through the `pkl` on the PATH. The version is pinned exactly in `mise.toml` (the lock file holds its checksums), so a run of the features against the TypeScript build and one against the Rust build use the same pkl.

## Behaviour map

What the TypeScript runner does, and where it is specified or will be. A behaviour is a feature when a black-box run shows it, and a Rust unit test (ported one for one from `runner/test`) when only the code shows it.

| Behaviour | Where |
|---|---|
| Render, executable bit, second render, `check`, drift, a hand edit refused, `--force`, `detach` | `render.feature` |
| Version, usage, `contract` | `cli.feature` |
| Lock held, an existing file not adopted, dry run, bad manifest, case rules, orphan deletion and its guard | `safety.feature` |
| A seeded file written once and kept when edited; a seed edited then turned into an owned file | Feature to write, needs a fixture with a seed |
| `--repair` of a manifest that cannot be read | Feature to write |
| A path that escapes the repository, an absolute path, a symlink that points outside | Feature to write, needs fixtures |
| A hand-edited orphan is not deleted, and `--force` deletes it | Feature to write |
| `detach` with `--dry-run`, and refusing a hand-edited file | Feature to write |
| A lock that is stale (dead process, or older than 10 minutes), and on CI only the age counts | Feature to write |
| Package version rules: an older package is refused, `--strict`, a known-bad version | Feature to write, needs a stubbed release list |
| `upgrade`, `latest`, `freshness` | Feature to write, needs a stubbed release server |
| CRLF checkouts, Unicode path normalisation, an interrupted write | Feature to write |
| Freshness, latest, preflight, lock check, pins, checksums, semver, contract and schema validation, package-URI handling, versions, the known-bad list, the equality rule's edge cases | Rust unit tests in Phase 2 |
