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

## Wave C: the toggles

The four toggles are options of `pantheon.pkl`: `skills`, `binary`, `features` and `compositeAction`. `Builtins.standardWorkflows(options)` and `Builtins.standardFiles(options)` give the whole workflow layer for a repository with those options, so a repository's `pantheon.pkl` is the options plus two spreads. Turning a toggle off removes its entries, and the runner then deletes the files it rendered before (the delete guard still applies: the file has to be tracked by git and unchanged).

| Toggle | Needs | On | What flipping it changes |
|---|---|---|---|
| `skills` | `skillName` | `true` | Removes `audit-skill.yml`. Changes `.coderabbit.yaml` (the skill lines), `SECURITY.md` (the skill line), `.agnix.toml` and `agnix.yml` (comment wording, decision 74) |
| `binary` | `crateName` | `true` | Removes `dist-workspace.toml`, `.github/actionlint.yaml`, `.github/scripts/tag-release.sh`, `.github/zizmor.yml` and the external `release.yml`. Changes `release-please.yml` (no tag step), `zizmor.yml` (no config) and `SECURITY.md` (the library text) |
| `features` | none | `default` | `all` changes `rust-ci.yml` only (and the hook steps, which are not part of the standard files) |
| `compositeAction` | none | `false` | `true` changes `audit-skill.yml` (the repository's own action), `zizmor.yml` and `.github/zizmor.yml` (which also scan `action.yml`) |

A toggle that needs a name and does not get one stops the render with an error that names the option and the way out: `options.skillName is required when options.skills is true ... Set skillName, or set skills = false.` Turning `skills` or `binary` off needs no name.

The external `release.yml` is never written, hashed or deleted, so turning `binary` off leaves a `release.yml` that `dist generate` wrote. Removing it is the repository's own change.

Checks: `runner/test/phase5-toggles.test.ts` flips each toggle from a base configuration and asserts the exact files removed and changed and none added, the two error messages, and, in a scratch git repository, the files deleted when `skills` and `binary` go off (and that the external `release.yml` stays) and that turning a toggle back on writes its files again. The mutation check is in `tmp/phase-5-wave-c`. The real-repository render, now through the two standard builtins, gives the same result as before.
