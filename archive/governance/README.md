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

## The ruleset task

```sh
bun runner/scripts/governance.ts rulesets                      # a dry run for the four repositories
bun runner/scripts/governance.ts rulesets pantheon-adr --show-body
bun runner/scripts/governance.ts rulesets pantheon-adr --desired-dir <folder>    # a dry run of a change before it is committed
bun runner/scripts/governance.ts rulesets pantheon-adr --apply --confirm pantheon-org/pantheon-adr
bun runner/scripts/governance.ts rulesets-restore <record file> [--apply --confirm pantheon-org/pantheon-adr]
```

- **A dry run is the default.** For each repository it says which rulesets are unchanged, which would be created, which would be replaced (with each difference in words), and which are not in the desired state and are left alone.
- **Applying works on one repository at a time,** with `--confirm <owner>/<repo>` spelled out, like the settings task. Every ruleset as it is now is written to a record first (untracked `.context/governance/`, never overwritten). The requests are sent in order and stop at the first failure, which names the record. Afterwards the rulesets are read again and must equal the desired ones. A second apply sends nothing.
- **It never deletes a ruleset it was not asked about.** A live ruleset that the desired state does not mention is reported as untouched.
- **`rulesets-restore`** is the rollback, from the record alone: a recorded ruleset that differs is replaced by the recorded one, one that is gone is created again, and a ruleset the apply created (and only that) is deleted. A ruleset someone adds later is left alone. It is a dry run unless applied and confirmed, and it reads again afterwards.

**Not yet proven against the real API.** The tests use a fake GitHub. The rulesets read from GitHub carry parameters (for example `dismissal_restriction`) that the create and update calls have not been tried with. A dry run prints the request it would send (`--show-body`), and the first real apply, on a throwaway repository, is the way to find out. Nothing has been applied to any repository.

## The weekly drift check

`.github/workflows/governance-drift.yml` runs every Monday morning (and by hand) and compares the live rulesets of the four repositories with `desired/`. It uses only this run's own `GITHUB_TOKEN`: no App, no secret.

- **It opens, updates or closes one issue** in this repository, labelled `governance-drift` and assigned to the owner in `drift-owner.json`. The issue lists each difference in words. It is updated (with a comment) when the drift changes, and closes itself, with a comment, when the drift is gone. It only ever touches an open issue with exactly its own title.
- **By hand it is a dry run unless told otherwise:** it prints the issue it would open and opens nothing. Run it with `dry_run` off to let it write.
- **What it cannot see.** A token that is not an admin of a repository is never shown the bypass actors of a ruleset or the repository's merge settings (checked on 09-11-2026: both are absent from the answers). So the scheduled run compares everything else about the rulesets and says in the issue and in its log that it did not compare the bypass actors. The merge settings are not compared by it at all. Both are compared when an admin runs `check` (bypass actors) and `settings` (merge settings) by hand.
- **Locally:** `bun runner/scripts/governance.ts drift` prints the same findings and, with drift, the issue text, and exits 4. With `--issue` it writes the issue (it needs a token that can write issues in this repository). This was run for real on 09-11-2026 against a scratch desired state: create, unchanged, update and close all worked (issue 61, now closed). It has not yet run from the workflow itself, with the workflow's own token.

## The required checks and the package

A required check is a job name. `runner/test/governance-checks.test.ts` renders the standard workflows for each repository's options and checks that every required check is the name of a job the package renders. It also tests what happens when a job is renamed: the required check is reported missing, and a renamed job would block every pull request until the ruleset follows. The jobs the package renders that nothing requires (for example `Scaffold check`) are reported as unrequired. Requiring them is a decision for each owner.

## What the tools do not do

- They have never been run against a real repository to change it. Nothing has been applied.
- They do not compare bypass actors when the token cannot see them.
- They do not govern the package repository `pantheon-scaffold` itself.

## Merge methods

"Protect main" lets a pull request be merged by squash only. The repository settings (squash, merge commit and rebase) are a separate thing and are the subject of the settings task.
