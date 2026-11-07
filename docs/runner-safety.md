# Runner safety: deletion, exec bit and locking

How `render` behaves when it deletes files, sets the executable bit and takes the write lock. Added in phase 2b, wave A.

## Orphan deletion and the delete guard

An orphan is an owned file listed in `.pkl-gen/manifest.json` that the contract no longer renders.

| Situation | Result |
|---|---|
| The file is gone already | Dropped from the manifest, action `forget`. |
| The file is unchanged since the last render | Deleted, action `delete`. |
| The file was edited by hand | Refused, unless `--force`. |
| The class is seed or external | Never deleted, only dropped from the manifest. |

Before anything is written, the delete guard runs once. It refuses all deletion, and so the whole run, when:

- the directory is not a git work tree (git is a required tool of the runner)
- the tracked tree is not clean (`git status --porcelain --untracked-files=no` prints anything, staged changes included)
- the checkout is sparse (`core.sparseCheckout`) or partial (`extensions.partialClone`)
- an orphan is not tracked by git (`git ls-files --error-unmatch`), because untracked files are never deleted

A shallow checkout is allowed. `--force` overrides a hand-edit refusal only, never the guard. The guard removes every `GIT_*` environment variable before it runs git, so a hook or a linked worktree cannot point it at another repository. A render with no orphans does not need git. `--check` has no guard because it writes nothing.

`--dry-run` prints the same plan, deletions and guard refusals included, and changes nothing.

## Executable bit

The contract's `mode` for an owned file is applied on every render. If the content is right but the bit differs, the action is `chmod`, and it also covers a bit removed by hand. `check` reports a bit that differs as drift (`mode`). Seed files get their mode at creation only.

## Locking

`render` takes `.pkl-gen/lock` (process id and start time, created exclusively) before it reads the manifest and removes it when it finishes, also after a refusal or an error. A second `render` fails with exit 1 and names the holder.

A lock is stale when it is older than 10 minutes. Outside CI it is also stale when its process no longer exists. On CI (`CI` set) only the age counts, because a process id from another job means nothing. `check` and `--dry-run` never take the lock, so they run while a render is writing.

Add `.pkl-gen/lock` and `.pkl-gen/stage/` to the consumer's `.gitignore` if the repository is not otherwise ignoring `.pkl-gen` temporary files.

## Exit codes

0 ok, 1 error (including a held lock), 2 usage, 3 refused (hand edit, existing file or delete guard), 4 drift. `--dry-run` exits 3 when the real run would be refused, otherwise 0.

## Not in this wave

The manifest migration test waits for a manifest v2. The version-mismatch and downgrade rules are in [runner-lifecycle.md](runner-lifecycle.md).
