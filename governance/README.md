# Governance of the four Rust repositories

The rulesets that should hold on `pantheon-adr`, `pantheon-journal`, `pantheon-skill-auditor` and `pantheon-agentic-harness-dev-kit`, as files, with tools that compare them with the live repositories. Phase 6 of the shared plan.

**Nothing here has been applied.** `check` and `show` only read (GET). The settings commands read by default and send nothing: they change a repository only with `--apply`, the repository written out in full in `--confirm`, and after the settings before have been written to a record file. Applying to a real repository is the owner's decision, one repository at a time, and has not been done.

## What is in `desired/`

| File | What it is |
|---|---|
| `protect-main.json` | The ruleset "Protect main" in the create shape (what `POST /repos/{owner}/{repo}/rulesets` takes): no deletion, no force push, linear history, a pull request that may only be squash merged, and five required checks (strict). |
| `require-cargo-deny.json` | The ruleset "Require Cargo Deny": one required check. It repeats a check that "Protect main" already requires. |
| `settings.json` | The merge settings the settings task manages: squash only, and a branch is deleted when its pull request is merged. |
| `repos.json` | Which of these each repository should hold. `pantheon-adr` holds only "Protect main". The other three hold both. |

The first desired state **mirrors the live rulesets exactly**, so a first check shows no drift. A change to a rule is a separate decision: it is made here, in a pull request, and then applied (or not) by the owner.

## Checking

```sh
bun runner/scripts/governance.ts check                  # all four repositories
bun runner/scripts/governance.ts check pantheon-adr     # one
bun runner/scripts/governance.ts show pantheon-journal "Protect main"   # a live ruleset in the create shape
```

Exit 0 means no drift, 4 means drift (each difference is printed in words), 1 means a repository could not be read. A repository that cannot be read is never reported as "no drift". Any token that can read the repositories will do: the rulesets of a public repository are public, but their bypass actors are visible to admins only.

## The merge settings task

```sh
bun runner/scripts/governance.ts settings                      # a dry run for the four repositories: what would change, nothing is sent
bun runner/scripts/governance.ts settings pantheon-adr         # one repository
bun runner/scripts/governance.ts settings pantheon-adr --apply --confirm pantheon-org/pantheon-adr
bun runner/scripts/governance.ts settings-restore <record file> [--apply --confirm pantheon-org/pantheon-adr]
```

- **A dry run is the default.** It prints each change and the exact request it would send. A dry run on 09-11-2026 found 3 changes in each of the four repositories (merge commits off, rebase merges off, delete branch on merge on).
- **Applying works on one repository at a time,** only on a repository listed in `repos.json`, and only with `--confirm <owner>/<repo>` spelled out.
- **The settings before are written first** to a record in `.context/governance/` (untracked; `--record-dir` changes it), and the file is never overwritten. If the write fails, nothing is sent.
- **It sends only what differs,** so a second apply sends nothing and writes no record.
- **It reads again afterwards** and fails, naming the record, if a setting is not what it should be.
- **`settings-restore`** puts the recorded settings back from the record alone, as a dry run unless applied and confirmed, sending only what differs now.

## The required checks and the package

A required check is a job name. `runner/test/governance-checks.test.ts` renders the standard workflows for each repository's options and checks that every required check is the name of a job the package renders. It also tests what happens when a job is renamed: the required check is reported missing, and a renamed job would block every pull request until the ruleset follows. The jobs the package renders that nothing requires (for example `Scaffold check`) are reported as unrequired. Requiring them is a decision for each owner.

## What the tools do not do

- They do not apply rulesets yet (the ruleset task is the next change). Nothing has been applied to any repository.
- They do not compare bypass actors when the token cannot see them.
- They do not govern the package repository `pantheon-scaffold` itself.

## Merge methods

"Protect main" lets a pull request be merged by squash only. The repository settings (squash, merge commit and rebase) are a separate thing and are the subject of the settings task.
