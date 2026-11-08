# Real-repository render of the workflow layer

Wave D of Phase 4, extended in Phase 5 wave B. `runner/scripts/real-repo-workflows.ts` clones each of the four Rust repositories (https, depth 1) into a temporary folder, renders the workflow builtins, their config files and the Phase 5 files into an empty folder from `runner/test/fixtures/real-workflows-<repository>/pantheon.pkl`, and compares each rendered file with the repository's own copy. It writes only inside a temporary folder; nothing in the repositories is changed and nothing is pushed.

A YAML file is compared under the P.1 rule (parsed data, so job ids, job names and every `run:` script byte for byte) and its directive comments (such as `# zizmor: ignore[...]`) are checked. Pinned action commits are masked in that comparison, because a pin that moved since the repository last updated it is a pin difference and not a difference in content: the report lists it as "pin moved". Any other file is compared byte for byte. The rendered workflows must also pass `actionlint`. Run from the repository root: `bun runner/scripts/real-repo-workflows.ts`. It needs network, git and `actionlint` installed through mise.

Run on 08-11-2026 against `main` of each repository: **PASS for all four.** Under P.1 a file is "equal" when only comments, formatting and pin commits differ. A "recorded" difference is one the plan accepts, listed in the script: the library repository's `.gitignore` (`/target`, while decision 74 picks `target/`) and the skill auditor's seeded `SECURITY.md` (it words its scope bullets differently, and a seeded file is written once and then belongs to the repository).

`release.yml` is listed by the package but never written (class `external`): `dist generate` writes it, and the drift check is Phase 5 wave D. No file the repositories have is left without a builtin.

## pantheon-adr at 1e2fd46

| File | Result | Detail |
|---|---|---|
| .agnix.toml | identical | none |
| .coderabbit.yaml | identical | none |
| .github/actionlint.yaml | identical | none |
| .github/commit-types.txt | identical | none |
| .github/dependabot.yml | equal (P.1) | comments only: 3 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/labels.txt | identical | none |
| .github/pull_request_template.md | identical | none |
| .github/scripts/tag-release.sh | identical | none |
| .github/workflows/agnix.yml | equal (P.1) | comments only: 15 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/aislop.yml | equal (P.1) | comments only: 8 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/audit-skill.yml | equal (P.1) | comments only: 3 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/cargo-deny.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/commit-types.yml | equal (P.1) | comments only: 2 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/conventional-commits.yml | equal (P.1) | comments only: 6 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/inline-scripts.yml | equal (P.1) | formatting only |
| .github/workflows/plumber.yml | equal (P.1) | pin moved since the repository's: getplumber/plumber@ec8cb3a; comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/pr-auto-rebase.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-automerge.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-please.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/rust-ci.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/sync-labels.yml | equal (P.1) | formatting only |
| .github/workflows/zizmor.yml | equal (P.1) | comments only: 12 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/zizmor.yml | identical | none |
| .gitignore | identical | none |
| .plumber.yaml | identical | none |
| SECURITY.md | identical | none |
| dist-workspace.toml | identical | none |

- actionlint over the 14 rendered workflows: exit 0.
- Files the repository has that no builtin builds yet (0): none.

## pantheon-journal at 05c3d84

| File | Result | Detail |
|---|---|---|
| .agnix.toml | identical | none |
| .coderabbit.yaml | identical | none |
| .github/actionlint.yaml | identical | none |
| .github/commit-types.txt | identical | none |
| .github/dependabot.yml | equal (P.1) | comments only: 3 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/labels.txt | identical | none |
| .github/pull_request_template.md | identical | none |
| .github/scripts/tag-release.sh | identical | none |
| .github/workflows/agnix.yml | equal (P.1) | comments only: 15 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/aislop.yml | equal (P.1) | comments only: 8 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/audit-skill.yml | equal (P.1) | comments only: 3 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/cargo-deny.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/commit-types.yml | equal (P.1) | comments only: 2 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/conventional-commits.yml | equal (P.1) | comments only: 6 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/inline-scripts.yml | equal (P.1) | formatting only |
| .github/workflows/plumber.yml | equal (P.1) | pin moved since the repository's: getplumber/plumber@ec8cb3a; comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/pr-auto-rebase.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-automerge.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-please.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/rust-ci.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/sync-labels.yml | equal (P.1) | formatting only |
| .github/workflows/zizmor.yml | equal (P.1) | comments only: 12 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/zizmor.yml | identical | none |
| .gitignore | identical | none |
| .plumber.yaml | identical | none |
| SECURITY.md | identical | none |
| dist-workspace.toml | identical | none |

- actionlint over the 14 rendered workflows: exit 0.
- Files the repository has that no builtin builds yet (0): none.

## pantheon-skill-auditor at b9e706e

| File | Result | Detail |
|---|---|---|
| .agnix.toml | identical | none |
| .coderabbit.yaml | identical | none |
| .github/actionlint.yaml | identical | none |
| .github/commit-types.txt | identical | none |
| .github/dependabot.yml | equal (P.1) | comments only: 3 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/labels.txt | identical | none |
| .github/pull_request_template.md | identical | none |
| .github/scripts/tag-release.sh | identical | none |
| .github/workflows/agnix.yml | equal (P.1) | comments only: 15 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/aislop.yml | equal (P.1) | comments only: 8 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/audit-skill.yml | equal (P.1) | comments only: 3 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/cargo-deny.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/commit-types.yml | equal (P.1) | comments only: 2 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/conventional-commits.yml | equal (P.1) | comments only: 6 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/inline-scripts.yml | equal (P.1) | formatting only |
| .github/workflows/plumber.yml | equal (P.1) | pin moved since the repository's: getplumber/plumber@ec8cb3a; comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/pr-auto-rebase.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-automerge.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-please.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/rust-ci.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/sync-labels.yml | equal (P.1) | formatting only |
| .github/workflows/zizmor.yml | equal (P.1) | comments only: 12 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/zizmor.yml | identical | none |
| .gitignore | identical | none |
| .plumber.yaml | identical | none |
| SECURITY.md | recorded | a seeded file: the repository words its scope bullets differently |
| dist-workspace.toml | identical | none |

- actionlint over the 14 rendered workflows: exit 0.
- Files the repository has that no builtin builds yet (0): none.

## pantheon-agentic-harness-dev-kit at 42f48b4

| File | Result | Detail |
|---|---|---|
| .agnix.toml | identical | none |
| .coderabbit.yaml | identical | none |
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
| .github/workflows/plumber.yml | equal (P.1) | pin moved since the repository's: getplumber/plumber@ec8cb3a; comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/pr-auto-rebase.yml | equal (P.1) | comments only: 5 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-automerge.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/release-please.yml | equal (P.1) | comments only: 1 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/rust-ci.yml | equal (P.1) | comments only: 3 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .github/workflows/sync-labels.yml | equal (P.1) | formatting only |
| .github/workflows/zizmor.yml | equal (P.1) | comments only: 12 line(s) of the repository's not rendered, 0 rendered that it lacks |
| .gitignore | recorded | decision 74 picks target/, the repository has /target |
| .plumber.yaml | identical | none |
| SECURITY.md | identical | none |

- actionlint over the 13 rendered workflows: exit 0.
- Files the repository has that no builtin builds yet (0): none.

## Summary of the run

| Repository | Commit | Files built | Identical | Equal under P.1 | Recorded | Differ or absent | actionlint | Result |
|---|---|---|---|---|---|---|---|---|
| pantheon-adr | 1e2fd46 | 27 | 12 | 15 | 0 | 0 | clean | PASS |
| pantheon-journal | 05c3d84 | 27 | 12 | 15 | 0 | 0 | clean | PASS |
| pantheon-skill-auditor | b9e706e | 27 | 11 | 15 | 1 | 0 | clean | PASS |
| pantheon-agentic-harness-dev-kit | 42f48b4 | 22 | 7 | 14 | 1 | 0 | clean | PASS |

Verdict: PASS
