# Real-repo render of the phase 2a slice (task 4.6)

`bun runner/scripts/real-repo-render.ts [owner/repo] [commit]` clones a real repository into a temporary folder and renders the slice (`runner/test/fixtures/slice/pantheon.pkl`: cargo-deny, rust-ci and inline-scripts workflows, three scripts, `deny.toml`, `rust-toolchain.toml`) into it. It needs network, `git`, `bun`, `pkl` and `actionlint`, and writes only inside the temporary folder.

It checks, in order:

1. A render without `--force`: identical files are adopted, and files that exist and differ are refused. The typed workflows differ in bytes from the hand-written ones, so the three workflows are refused.
2. A render with `--force` replaces them.
3. Each workflow against its original by the P.1 rule (parsed equality, `run:` byte for byte, null equal to `{}` inside `on:`, no duplicate keys). The scripts and TOML files are compared byte for byte through git.
4. `actionlint` on the whole repository, `check` (expects no drift) and a second render (expects no writes).

Run on 07-11-2026:

| Repository | Commit | Result |
|---|---|---|
| pantheon-adr | 1e2fd46 (the commit the tests pin) | pass |
| pantheon-skill-auditor | b9e706e | pass |
| pantheon-journal | 05c3d84 | pass |
| pantheon-agentic-harness-dev-kit | 42f48b4 | `rust-ci.yml` is not equal: this repository builds with `--all-features` and an extra default-features build. That is the `features` option of decision 74, which the slice does not have yet (phase 7 renders this repository as a scratch clone to cover it) |

In every repository the five verbatim files were adopted unchanged (byte for byte), the three workflows differ in bytes from the originals by 44, 30 and 62 diff lines (formatting, key order and the comments the typed model cannot carry), `actionlint` passed on the whole repository, `check` found no drift and a second render wrote nothing.

## Coverage estimate for the builtins not yet built

`bun runner/scripts/coverage-estimate.ts [owner/repo]` reads the workflows of a repository (read-only, `gh api`) and counts how many constructs (workflows, jobs and steps) carry a key the typed classes do not have yet, so they would need the raw escape hatch. Run on 07-11-2026 against `pantheon-adr`:

- 15 workflows, 114 constructs, 12 constructs need the escape hatch (10.5 percent). The keys are `continue-on-error` (5 steps in `agnix.yml` and 1 in `plumber.yml`), and `outputs`, `shell` and `container` in `release.yml`.
- `release.yml` is written by `dist generate` and is out of the generator's ownership, so without it the figure is 5 of 78 constructs (6.4 percent).
- The eight workflow builtins of decision 43: the slice (cargo-deny, rust-ci, inline-scripts) uses no raw constructs, and of the five others (plumber, sync-labels, pr-auto-rebase, release-please, release-automerge) only plumber needs a key the model lacks (`continue-on-error` on a step), which can become a typed field.

This counts keys only. Whether each workflow then renders equal under P.1 is shown by building the builtin, which is what phases 2b and 4 do.
