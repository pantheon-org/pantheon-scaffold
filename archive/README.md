# Archive

Material kept for the record after the refocus of 10-11-2026 (see [docs/refocus-2026-11-10.md](../docs/refocus-2026-11-10.md)). Nothing here is part of the product, and nothing here is run by CI.

The state before the refocus is the tag `pre-refocus`, on commit `6cc70313d247b3bea21d39ab27b9785f071354b8` (`main` before any refocus change). Files moved with `git mv`, so `git log --follow` shows their history.

## What is here

| Folder | What it is | Why it is archived |
|---|---|---|
| `governance/` | The desired rulesets and merge settings for the four Rust repositories, the checker and settings tools (`src/`, `scripts/`), their 6 test files and fixtures, and the owner file. | Governance is dropped for now. Nothing was ever applied to a real repository. The weekly drift workflow was deleted, so ruleset drift is no longer checked. |
| `proof/docs/` | The proof report of 0.1.3, the security dossier and sample check of 0.1.5, the real-repository render reports and the bypass log. | Evidence for releases that were published, not documentation for adopters. |
| `proof/scripts/` and `proof/test/` | The scripts that produced that evidence (`proof-*.ts`, `real-repo-*.ts`) and the test of `proof-categories`. | Same. |
| `spikes/` | The feasibility spikes, the phase evidence folders and the mutation checks that were in `tmp/`, with their READMEs. | Evidence, not product. |

## How to re-run anything here

Files were moved, so their relative paths and imports (`../src/...`, `tmp/...`, `runner/...`) no longer resolve. To run one, check out the tag and use it from there:

```bash
git worktree add ../pantheon-scaffold-pre-refocus pre-refocus
```

## Known leftovers

- Comments in `pkl/Builtins.pkl` still name `tmp/phase-4-generators`. That folder is now `archive/spikes/phase-4-generators`. The comments were left alone so this change does not touch the package.
- Fixtures under `runner/test/fixtures/` keep a few comments that name the old script paths.
