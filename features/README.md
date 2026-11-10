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

The step definitions are in `crates/scaffold-features/tests/features.rs`. They stay thin and say nothing about the language of the runner. A command line may use `<repo>` (the repository under test), `<package>` (this repository's `pkl` folder) and `<fixture:NAME>` (`runner/test/fixtures/NAME/pantheon.pkl`). In a file written with `I write "..." to the file "..."`, `\n` is a newline.

| File | What it covers |
|---|---|
| `render.feature` | `render`, `check` and `detach` on a fresh repository: the files written, the executable bit, a second render, drift, a hand edit refused, `--force`. |
