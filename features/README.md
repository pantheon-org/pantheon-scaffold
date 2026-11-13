# Features

The behaviour of the runner, written as cucumber features. They are black-box: each scenario runs a binary and looks at its exit code, its output and the files it leaves, so the same files run against any build of the runner. There are two builds, and both pass every scenario: the TypeScript build, and the Rust build (`crates/scaffold-cli`), which Phase 2 of the refocus plan made pass the same files.

## Run them

Build a binary, name it in `SCAFFOLD_BIN`, and run the harness. It needs `pkl` and `git` on the PATH, and Rust 1.96.1. The version is named in three places that must agree: `rust-toolchain.toml`, `mise.toml` and the rustup steps in `.github/workflows/ci.yml`. mise cannot lock Rust, so the workflows set `MISE_DISABLE_TOOLS=rust` and `mise install --locked` skips it.

```bash
cargo build -p scaffold-cli
SCAFFOLD_BIN=target/debug/pantheon-scaffold cargo test -p scaffold-features
```

CI builds the runner and runs the features on `linux-x64`, `linux-arm64` and `darwin-arm64`; a release binary is built by `runner/scripts/build-rust.ts` and checked by `runner/scripts/smoke.ts`.

### The baseline

What a repository renders is checked by `runner/scripts/builtins-baseline/` (55 configurations compared with a frozen snapshot), run by the `baseline` job of `ci.yml`; see its README.

`SCAFFOLD_SKIP_TAGS` is a comma-separated list of tags, without the `@`. Scenarios carrying any of them are not run, and the harness prints the list at the start. It was used while the Rust build was missing commands: the scenarios for each carried its tag (`@detach`, `@freshness`, `@latest`, `@upgrade`), and the tag left the list in CI when the command was ported. All four are ported. The TypeScript job of CI skips the tag `rust-only`: the scenarios for Presets in the manifest and in `check`, which the TypeScript runner will never get because task 3.4 removes it. The tags stay on the scenarios, and the variable stays in the harness, for the next command that is added to one build before the other.

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

`--version` is checked against `SCAFFOLD_VERSION` (default `0.0.0`, which is what a build without a version prints). CI sets it to the version it built.

The harness removes `CI` from the environment the binary sees, because the runner judges a stale lock differently on CI. A scenario that needs it says `Given the environment variable "CI" is "true"`.

A scenario that needs a web server starts one with `Given the stub server answers "/path" with '...'` (or `with status 500`, or `with a freshness record from N days ago`) and uses `<stub>` for its address in a command line or an environment value. `Given the stub server serves the known-bad list '...'` points `PANTHEON_SCAFFOLD_KNOWN_BAD_URL` at it, so no scenario reaches the network.

## The pkl version

The runner reads pkl through the `pkl` on the PATH. The version is pinned exactly in `mise.toml` (the lock file holds its checksums), so every run of the features uses the same pkl.

## Behaviour map

What the runner does, and where it is specified. (It was first written as a TypeScript runner, retired in the pull request that added `docs/ts-runner-test-mapping.md`.) A behaviour is a feature when a black-box run shows it, and a Rust unit test (ported one for one from `runner/test`) when only the code shows it.

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
| The text of a network error | When a request cannot be made at all (nothing listens, no route), the message ends with what the HTTP library says. The TypeScript build prints Bun's `Unable to connect. Is the computer able to access the url?`, the Rust build prints the operating system's `io: Connection refused (os error 61)`. Everything before it matches (`could not fetch <url> after <n> attempts, failing closed:`), and so does the exit code (1). A status error (`HTTP 500`) is the same in both. Found by running both builds side by side on 11-11-2026 (`freshness`, and again for `latest`), not a scenario: the scenarios only check `failing closed`. | Open. Proposal: accept, because the text is the library's and the part that scripts rely on is the prefix and the exit code. |

### Presets

`presets.feature` starts from a small project in `features/projects/<name>` (a `pantheon.pkl` and its `presets/`), copied into an empty repository by the step `Given the project "<name>"`, which replaces `@PACKAGE@` in every `.pkl` file with the package under test. The scenarios for the pkl API pass since task 4.2 (the helpers in `Presets.pkl`, the `presets` list of the config and its validation); since task 4.3 the Rust runner records Presets in the manifest and `check` lists them, and the four scenarios for that are tagged `rust-only`. The two `upgrade` scenarios of `docs/presets-design.md` are integration tests in `crates/scaffold-cli/tests/upgrade.rs` (task 4.3), because they need a published package with a renamed job.

### init

`init.feature` is tagged `rust-only`: the TypeScript runner has no `init`. It runs against the package in this checkout (`--allow-local-package`), so it needs no release. The release part of `init` (the newest stable release that is not known bad, its checksum, the attestation) is tested in `crates/scaffold-cli/tests/init.rs` with a local server and fake `gh` and `pkl`.

### migrate

`migrate.feature` is tagged `rust-only`: the TypeScript runner has no `migrate`. It was written ahead of the command (task 5.2) and has passed since task 5.3. It fixes the contract of the command: the classes `identical`, `drifted`, `new` and `per-repo`, printed as the class padded to ten columns and then the path (an existing seed is printed as `seed-kept` and an external file as `external`, and neither is counted); the YAML equality rule for `.yml` and `.yaml` files and byte equality for the rest; `per-repo` limited to `.github/` and the top of the repository; the dry run as the default; exit 3 for an existing `pantheon.pkl`, and for `--write` while a file is drifted unless `--as-presets` keeps each one as a Preset (task 5.4a: a whole-file Preset per drifted file, proved by evaluating the config with the Presets before anything is written); exit 2 without `--toolchain`. `features/projects/migrate-mixed` has two identical files, one drifted, and files of the repository's own, with no `pantheon.pkl`; `migrate-adopt` has no drift, so `--write` runs on it.
