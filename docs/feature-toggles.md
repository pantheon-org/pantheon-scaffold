# Feature toggles and the files they own

Phase 5. The toggles are the options `skills`, `binary`, `features` and `compositeAction` (decision 74). This page lists what each one owns, wave by wave as it is built.

## Wave A: the verbatim files

`Builtins.verbatimFiles(options)` returns:

| File | When | Source |
|---|---|---|
| `.gitignore` | always | `pkl/assets/gitignore`, `target/` (the form three repositories use, decision 74). The library repository has `/target`, so it changes by one line when it adopts |
| `.github/actionlint.yaml` | `binary` | byte for byte from `pantheon-adr` at 1e2fd46 (identical in `pantheon-journal` and `pantheon-skill-auditor`) |
| `dist-workspace.toml` | `binary` | the same three repositories, byte for byte |
| `.github/workflows/release.yml` | `binary` | class `external`: listed with no content, so the runner never writes, hashes or deletes it. `dist generate` writes it. The drift check against `dist generate --check` is wave D |

`actionlint.yaml` belongs to `binary` because the three repositories that have it all build a binary and the library repository, which has none, has no such file (Q1 of the refined plan).

Known limit: `dist-workspace.toml` pins `actions/checkout`, `upload-artifact` and `download-artifact` by commit in its `[dist.github-action-commits]` table. Those commits are not in `pkl/Pins.pkl`, so the pin update workflow does not move them and they are not checked against a release tag. They change with the file.

Checks: `runner/test/phase5-verbatim.test.ts` (git blobs of the pinned files, both kinds of repository, `release.yml` never written) and the mutation check in `tmp/phase-5-wave-a`.

## Wave B: the parameterised files

The new option `skillName` is the folder under `skills/` that the files below name. `crateName` already existed. Both are needed by the toggles that use them (a clear error for a missing one is wave C).

| File | When | What varies |
|---|---|---|
| `.github/workflows/audit-skill.yml` (`Builtins.auditSkill(options)`) | `skills` | `skillName`. With `compositeAction` the repository runs its own action (`uses: ./`, the tekhne v0.3.0 bootstrap binary, and `action.yml` in the watched paths). Otherwise it installs the released `pantheon-skill-auditor` action |
| `.github/scripts/tag-release.sh` | `binary` | `crateName`. Executable |
| `.coderabbit.yaml` | always | `skillName`. The two lines that exclude the skill are left out without `skills` |
| `SECURITY.md` | always, seeded once | binary text (names `pantheon-<crate>`, `crates/<crate>/`, and the skill line with `skills`) or the library text |

The skill auditor action is a typed pin built from `pkl/Tools.pkl` (commit, version and the binary's SHA-256 together), and is not in `pkl/Pins.pkl`: the pin update workflow would otherwise move the action without its binary. The runner still writes its `# v0.3.1` comment from the contract's pins.

`SECURITY.md` is a seed, so the skill auditor repository, which words its scope bullets differently, keeps its own text. The report in [real-repo-workflows.md](real-repo-workflows.md) lists it and the library `.gitignore` as recorded differences. Checks: `runner/test/phase5-parameterised.test.ts`, the mutation check in `tmp/phase-5-wave-b`, and the real-repository render.
