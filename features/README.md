# Features

The behaviour of the runner, written as cucumber features. They are black-box: each scenario runs a binary and looks at its exit code, its output and the files it leaves, so the same files run against any build of the runner. There are two builds: the TypeScript build, which passes every scenario, and the Rust build (`crates/scaffold-cli`), which has to pass the same files (Phase 2 of the refocus plan). The Rust build does not have `latest` or `upgrade` yet, so CI skips the scenarios tagged for them (see below).

## Run them

Build a binary, name it in `SCAFFOLD_BIN`, and run the harness. It needs `pkl` and `git` on the PATH, and Rust 1.96.1. The version is named in three places that must agree: `rust-toolchain.toml`, `mise.toml` and the rustup steps in `.github/workflows/ci.yml`. mise cannot lock Rust, so the workflows set `MISE_DISABLE_TOOLS=rust` and `mise install --locked` skips it.

```bash
mise x -- bun runner/scripts/build.ts --out dist --targets darwin-arm64
SCAFFOLD_BIN=dist/pantheon-scaffold-darwin-arm64 cargo test -p scaffold-features
```

Use the target that matches your machine: `linux-x64`, `linux-arm64` or `darwin-arm64`. CI runs the features on all three after the build job.

### Against the Rust build

```bash
cargo build -p scaffold-cli
SCAFFOLD_SKIP_TAGS=latest,upgrade SCAFFOLD_BIN=target/debug/pantheon-scaffold cargo test -p scaffold-features
```

`SCAFFOLD_SKIP_TAGS` is a comma-separated list of tags, without the `@`. Scenarios carrying any of them are not run, and the harness prints the list at the start. The scenarios for a command the Rust build has not ported carry its tag (`@latest`, `@upgrade`; `@detach` and `@freshness` were removed from the list when those commands were ported). When a command is ported, remove its tag from the list in the `features-rust` job of `.github/workflows/ci.yml` and the scenarios start to count. Without the variable every scenario runs, which is how the TypeScript build is tested.

## Write a scenario

The step definitions are in `crates/scaffold-features/tests/features.rs`. They stay thin and say nothing about the language of the runner. A command line may use `<repo>` (the repository under test), `<package>` (this repository's `pkl` folder) and `<fixture:NAME>` (the `pantheon.pkl` in the fixture folder of that name under `runner/test/fixtures`). In a file written with `I write "..." to the file "..."`, `\n` is a newline.

| File | What it covers |
|---|---|
| `render.feature` | `render`, `check` and `detach` on a fresh repository: the files written, the executable bit, a second render, drift, a hand edit refused, `--force`. |
| `cli.feature` | `--version`, a usage error, an unknown option, `contract`. |
| `safety.feature` | The lock (held, a process that has gone, older than ten minutes, and on CI), an existing file that is not adopted, `--dry-run` and its exit code, a manifest that is not JSON or from a newer format, case collisions and a case-only rename, deleting an orphan (clean git tree, dirty tree, no git), a path that escapes the repository, an absolute path, a symlink that points outside. |
| `lifecycle.feature` | Seeds (written once, kept, dropped from the config, turned into an owned file), an orphan edited by hand with and without `--force`, `--repair`, and `detach` (dry run, an edited file, `--force`, no manifest). |
| `releases.feature` | `latest` (newest stable, pre-releases, known-bad, a tag that is not a version, nothing qualifies, a failed or malformed list), `freshness` (fresh, stale, `--max-age-days`, a malformed record, a record from the future, a failed fetch), and the checks `upgrade` makes before it reaches the network. |
| `encoding.feature` | An owned file whose line endings became CRLF (a recorded gap, see below), and a file name with an accented letter. |

`--version` is checked against `SCAFFOLD_VERSION` (default `0.0.0`, which is what `runner/scripts/build.ts` builds unless told otherwise). CI sets it to the version it built.

The harness removes `CI` from the environment the binary sees, because the runner judges a stale lock differently on CI. A scenario that needs it says `Given the environment variable "CI" is "true"`.

A scenario that needs a web server starts one with `Given the stub server answers "/path" with '...'` (or `with status 500`, or `with a freshness record from N days ago`) and uses `<stub>` for its address in a command line or an environment value. `Given the stub server serves the known-bad list '...'` points `PANTHEON_SCAFFOLD_KNOWN_BAD_URL` at it, so no scenario reaches the network.

## The pkl version

Both runners read pkl through the `pkl` on the PATH. The version is pinned exactly in `mise.toml` (the lock file holds its checksums), so a run of the features against the TypeScript build and one against the Rust build use the same pkl.

## Behaviour map

What the TypeScript runner does, and where it is specified or will be. A behaviour is a feature when a black-box run shows it, and a Rust unit test (ported one for one from `runner/test`) when only the code shows it.

| Behaviour | Where |
|---|---|
| Render, executable bit, second render, `check`, drift, a hand edit refused, `--force`, `detach` | `render.feature` |
| Version, usage, `contract` | `cli.feature` |
| Lock held, stale (dead process, older than ten minutes, on CI only the age), an existing file not adopted, dry run, bad manifest, case rules, orphan deletion and its guard, unsafe paths | `safety.feature` |
| Seeds, a hand-edited orphan and `--force`, `--repair`, `detach` and its options | `lifecycle.feature` |
| Package version rules: an older package is refused, `--strict`, a known-bad version | Rust unit tests in Phase 2 (`preflight`). A black-box run needs a `package://` download, and a local package has no version, so no scenario can reach it without a real release |
| `latest`, `freshness`, and what `upgrade` checks before the network | `releases.feature` |
| `upgrade` past the checksums download (attestation through `gh`, `mise install`, `mise lock`, rollback) | Rust unit tests in Phase 2, with the three outside calls replaced. A black-box run needs a real release and `gh attestation verify` |
| CRLF checkouts, an accented file name | `encoding.feature` |
| Unicode normalisation (a name written two ways) and an interrupted write | Not a feature, see "Gaps" |
| Freshness, latest, preflight, lock check, pins, checksums, semver, contract and schema validation, package-URI handling, versions, the known-bad list, the equality rule's edge cases | Rust unit tests in Phase 2 |

## Gaps

Where a scenario and the runner disagree, or a behaviour cannot be a black-box scenario, it is recorded here. The owner decides each one: the Rust build reproduces it (the scenario stays) or fixes it (the scenario changes with the fix).

| Gap | What happens today | Decision |
|---|---|---|
| CRLF line endings | The runner compares bytes. A checkout that turned every line ending of an owned file into CRLF (for example git `autocrlf` on Windows) is refused as a hand edit by `render` (exit 3) and reported as drift by `check` (exit 4), and the printed diff shows identical lines. `--force` rewrites the file with LF. Scenarios in `encoding.feature` record this. | Reproduce (owner, 10-11-2026): the Rust build keeps today's behaviour and the `encoding.feature` scenarios stay as they are. Revisit if a consumer hits it. |
| Unicode normalisation of paths | Two paths that differ only in how an accented letter is written (one character, or a letter and a combining mark) are not detected as a collision: the runner lowercases names and nothing else. On a filesystem that treats them as one file (macOS) they would overwrite each other. Not a scenario, because the outcome depends on the filesystem. Not tried on a real filesystem. | Open |
| An interrupted write | Each file is written to a staging folder inside `.pkl-gen` and renamed into place, so a half-written file should never appear at its path. A black-box run cannot stop the runner part-way, so no scenario shows it. To be a Rust unit test in Phase 2. | Open |
| The text of a network error | When a request cannot be made at all (nothing listens, no route), the message ends with what the HTTP library says. The TypeScript build prints Bun's `Unable to connect. Is the computer able to access the url?`, the Rust build prints the operating system's `io: Connection refused (os error 61)`. Everything before it matches (`could not fetch <url> after <n> attempts, failing closed:`), and so does the exit code (1). A status error (`HTTP 500`) is the same in both. Found by running both builds side by side on 11-11-2026 (`freshness`), not a scenario: the scenarios only check `failing closed`. | Open. Proposal: accept, because the text is the library's and the part that scripts rely on is the prefix and the exit code. |
