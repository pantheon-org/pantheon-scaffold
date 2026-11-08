# Real-repository render of the workflow layer

Wave D of Phase 4. `runner/scripts/real-repo-workflows.ts` clones each of the four Rust repositories (https, depth 1) into a temporary folder, renders the thirteen workflow builtins and their config files into an empty folder from `runner/test/fixtures/real-workflows-<repository>/pantheon.pkl`, and compares each rendered file with the repository's own copy. It writes only inside a temporary folder; nothing in the repositories is changed and nothing is pushed.

A YAML file is compared under the P.1 rule (parsed data, so job ids, job names and every `run:` script byte for byte) and its directive comments (such as `# zizmor: ignore[...]`) are checked. Any other file is compared byte for byte. The rendered workflows must also pass `actionlint`. Run from the repository root: `bun runner/scripts/real-repo-workflows.ts`. It needs network, git and `actionlint` installed through mise.

Run on 08-11-2026 against `main` of each repository: **PASS for all four.** Under P.1 a file is "equal" when only comments and formatting differ. The comment-only differences are the header and in-file comments that the typed model does not carry (the wording of a header, a comment between two keys). They never change what a workflow does.

Not built yet, and so not compared: `.github/actionlint.yaml`, `audit-skill.yml` and `release.yml` (three repositories; `audit-skill.yml` and `release.yml` belong to the phase 5 toggles), and the four files decision 74 settles that no task builds yet (see [workflow-layer.md](workflow-layer.md)).

## pantheon-adr at 1e2fd46

| File | Result | Detail |
|---|---|---|
| .agnix.toml | identical | none |
| .github/commit-types.txt | identical | none |
| .github/dependabot.yml | equal (P.1) | comments only: 3 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/labels.txt | identical | none |
| .github/pull_request_template.md | identical | none |
| .github/workflows/agnix.yml | equal (P.1) | comments only: 15 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/aislop.yml | equal (P.1) | comments only: 8 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/cargo-deny.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/commit-types.yml | equal (P.1) | comments only: 2 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/conventional-commits.yml | equal (P.1) | comments only: 6 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/inline-scripts.yml | equal (P.1) | formatting only |
| .github/workflows/plumber.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/pr-auto-rebase.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-automerge.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-please.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/rust-ci.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/sync-labels.yml | equal (P.1) | formatting only |
| .github/workflows/zizmor.yml | equal (P.1) | comments only: 12 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/zizmor.yml | identical | none |
| .plumber.yaml | identical | none |

- actionlint over the 13 rendered workflows: exit 0.
- Files the repository has that no builtin builds yet (3): .github/actionlint.yaml, .github/workflows/audit-skill.yml, .github/workflows/release.yml.

## pantheon-journal at 05c3d84

| File | Result | Detail |
|---|---|---|
| .agnix.toml | identical | none |
| .github/commit-types.txt | identical | none |
| .github/dependabot.yml | equal (P.1) | comments only: 3 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/labels.txt | identical | none |
| .github/pull_request_template.md | identical | none |
| .github/workflows/agnix.yml | equal (P.1) | comments only: 15 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/aislop.yml | equal (P.1) | comments only: 8 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/cargo-deny.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/commit-types.yml | equal (P.1) | comments only: 2 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/conventional-commits.yml | equal (P.1) | comments only: 6 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/inline-scripts.yml | equal (P.1) | formatting only |
| .github/workflows/plumber.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/pr-auto-rebase.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-automerge.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-please.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/rust-ci.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/sync-labels.yml | equal (P.1) | formatting only |
| .github/workflows/zizmor.yml | equal (P.1) | comments only: 12 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/zizmor.yml | identical | none |
| .plumber.yaml | identical | none |

- actionlint over the 13 rendered workflows: exit 0.
- Files the repository has that no builtin builds yet (3): .github/actionlint.yaml, .github/workflows/audit-skill.yml, .github/workflows/release.yml.

## pantheon-skill-auditor at b9e706e

| File | Result | Detail |
|---|---|---|
| .agnix.toml | identical | none |
| .github/commit-types.txt | identical | none |
| .github/dependabot.yml | equal (P.1) | comments only: 3 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/labels.txt | identical | none |
| .github/pull_request_template.md | identical | none |
| .github/workflows/agnix.yml | equal (P.1) | comments only: 15 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/aislop.yml | equal (P.1) | comments only: 8 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/cargo-deny.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/commit-types.yml | equal (P.1) | comments only: 2 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/conventional-commits.yml | equal (P.1) | comments only: 6 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/inline-scripts.yml | equal (P.1) | formatting only |
| .github/workflows/plumber.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/pr-auto-rebase.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-automerge.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-please.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/rust-ci.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/sync-labels.yml | equal (P.1) | formatting only |
| .github/workflows/zizmor.yml | equal (P.1) | comments only: 12 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/zizmor.yml | identical | none |
| .plumber.yaml | identical | none |

- actionlint over the 13 rendered workflows: exit 0.
- Files the repository has that no builtin builds yet (3): .github/actionlint.yaml, .github/workflows/audit-skill.yml, .github/workflows/release.yml.

## pantheon-agentic-harness-dev-kit at 42f48b4

| File | Result | Detail |
|---|---|---|
| .agnix.toml | identical | none |
| .github/commit-types.txt | identical | none |
| .github/dependabot.yml | equal (P.1) | comments only: 3 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/labels.txt | identical | none |
| .github/pull_request_template.md | identical | none |
| .github/workflows/agnix.yml | equal (P.1) | comments only: 15 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/aislop.yml | equal (P.1) | comments only: 8 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/cargo-deny.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/commit-types.yml | equal (P.1) | comments only: 2 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/conventional-commits.yml | equal (P.1) | comments only: 6 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/inline-scripts.yml | equal (P.1) | formatting only |
| .github/workflows/plumber.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/pr-auto-rebase.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-automerge.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-please.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/rust-ci.yml | equal (P.1) | comments only: 3 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/sync-labels.yml | equal (P.1) | formatting only |
| .github/workflows/zizmor.yml | equal (P.1) | comments only: 12 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .plumber.yaml | identical | none |

- actionlint over the 13 rendered workflows: exit 0.
- Files the repository has that no builtin builds yet (0): none.

## Summary of the run

| Repository | Commit | Files built | Identical | Equal under P.1 | Differ or absent | actionlint | Result |
|---|---|---|---|---|---|---|---|
| pantheon-adr | 1e2fd46 | 20 | 6 | 14 | 0 | clean | PASS |
| pantheon-journal | 05c3d84 | 20 | 6 | 14 | 0 | clean | PASS |
| pantheon-skill-auditor | b9e706e | 20 | 6 | 14 | 0 | clean | PASS |
| pantheon-agentic-harness-dev-kit | 42f48b4 | 19 | 5 | 14 | 0 | clean | PASS |

Verdict: PASS
