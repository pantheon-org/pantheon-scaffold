# Scaffold Agent Collaboration Guide

This file defines how LLM agents work in this repository.

## Project Purpose

- Define the GitHub CI/CD of the `pantheon-*` repositories in pkl, as Builtins, and render it from one `scaffold.pkl` per repository.
- Keep a small Rust runner (`pantheon-scaffold`) that renders, checks, upgrades and detaches those files, and sets up new projects (`init`, `migrate`).
- Prefer small, targeted edits over broad rewrites. Read `README.md` for the current status.

## Engineering Principles

Respect these, in this order of use:

- TDD
- BDD
- KISS, where it applies
- YAGNI, where it applies
- DRY, where it applies
- SOLID, where it applies

## Language Preference

- The project's code is Rust. Write all new code in Rust.
- GitHub Actions scripts are Shell scripts under `.github/scripts/`.
- `runner/` holds the TypeScript release tooling and tests that remain from the earlier runner (run with bun, see `docs/runner-file-sort.md`). Do not add to it. Port new work to Rust.

## Repository Map

- `crates/scaffold-cli`: the runner binary.
- `crates/scaffold-core`: shared library code.
- `crates/scaffold-release`: release tooling (build, smoke, promote, pins).
- `crates/scaffold-features`: the cucumber harness for `features/`.
- `features/`: black-box behaviour of the runner, written as `.feature` files.
- `pkl/`: the package. `pkl/Builtins/<name>/` is one Builtin. `pkl/PklProject` holds the package version.
- `presets/`: this repository's own Presets, applied by `scaffold.pkl`.
- `scaffold.pkl`: this repository, rendered by its own scaffold.
- `runner/`: remaining bun tooling and tests (see Language Preference).
- `docs/`: design and process documents. `README.md` has the reading table.
- `archive/`: retired work, with its own `README.md`. Do not extend it.
- `.context/`: gitignored working notes. Keep it out of every commit.

## Agent Workflow

1. Read the relevant doc under `docs/` before changing the code or Builtin it describes.
2. Write the test or feature first (TDD, BDD), then the change.
3. Keep changes scoped to the request. Report unrelated findings once, at the end.
4. Run the validation commands below before finishing.
5. Update `README.md` or the matching `docs/*.md` when behaviour, commands or layout change.

## Rendered Files

Most workflow, hook and tooling files in this repository are rendered from the Builtins by its own `scaffold.pkl`. Change the Builtin under `pkl/Builtins/`, not the rendered file, then render and commit both:

```bash
cargo run -p scaffold-cli -- render scaffold.pkl --allow-local-package pkl
cargo run -q -p scaffold-cli --locked -- check scaffold.pkl --allow-local-package pkl
```

CI runs `check` and fails when a rendered file drifts. `release.yml` and `release-please-config.json` are hand-written (see `docs/release-process.md`).

## CI Scripts

A `run:` block in a workflow holds at most 15 lines of code, blank and comment lines not counted. Longer logic goes in a Shell script under `.github/scripts/`, and the step calls it. A block that must stay long opts out with a `# inline-ok: <reason>` line inside it. `.github/scripts/check-inline-scripts.sh` enforces this.

## Validation Commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo test --workspace --exclude scaffold-features --locked
cargo build -p scaffold-cli && SCAFFOLD_BIN=target/debug/pantheon-scaffold cargo test -p scaffold-features
mise x -- bun test
rumdl check .
```

`rumdl` (config in `.rumdl.toml`) is not pinned in `mise.toml`: install it yourself. Rust is 1.96.1. The version is named in `rust-toolchain.toml`, `mise.toml` and the rustup steps in `.github/workflows/ci.yml`, and the three must agree. Other tools are pinned in `mise.toml` (`mise install --locked`).

## Pull Requests

Open pull requests from `.github/pull_request_template.md` and keep its three headings, `## What`, `## Why` and `## Notes`, in that order. Use a conventional-commit title (`type(scope): lowercase summary`). Put verification evidence and anything left for later under Notes, and end with `Refs #N` for partial work or `Closes #N` when the pull request finishes the issue.

Pull requests are squash-merged, so the title becomes the commit message on `main` and is what release-please reads to pick the version bump and write the changelog. Allowed types are in `.github/commit-types.txt`, which the title workflow copies into a regex and `.github/scripts/check-commit-types.sh` keeps in step. Releases are described in `docs/release-process.md`.

Write commit messages, pull request descriptions and comments without an agent session link, session ID or session trailer.

## Git Hooks

Pre-commit runs through `hk` (`hk.pkl`): `actionlint` on workflows and `pkl eval` on every package module. Hooks are installed with `hk install`. Let every hook run, and bypass one only when explicitly requested.

## Safety Constraints

- Leave a pushed tag where it is, and publish each version once (see `docs/release-process.md`).
- Keep secrets, tokens and the names of organisation secrets out of every commit.
- If you detect unrelated dirty changes, do not revert them. Continue safely.
- If repository conventions conflict, prefer explicit user instructions.
