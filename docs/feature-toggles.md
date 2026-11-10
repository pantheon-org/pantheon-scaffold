# Feature toggles and the files they own

Phase 5. The toggles are the options `skills`, `binary`, `features` and `compositeAction` (decision 74). This page lists what each one owns, wave by wave as it is built.

## Wave A: the verbatim files

`Builtins.verbatimFiles(options)` returns:

| File | When | Source |
|---|---|---|
| `.gitignore` | always | `pkl/assets/gitignore`: `target/` (the form three repositories use, decision 74), `.context/` and `.pkl-gen/cache/` (the runner's package cache, which must not be committed, decision 87). The three binary repositories gain one line, and the library repository changes `/target` to `target/` and gains the same line |
| `.github/actionlint.yaml` | `binary` | byte for byte from `pantheon-adr` at 1e2fd46 (identical in `pantheon-journal` and `pantheon-skill-auditor`) |
| `dist-workspace.toml` | `binary` | the pantheon-adr file with two action commits changed to the ones the repositories' `release.yml` uses (wave D, decision 85) |
| `.github/workflows/release.yml` | `binary` | class `external`: listed with no content, so the runner never writes, hashes or deletes it. `dist generate` writes it. The drift check against `dist generate --check` is wave D |

`actionlint.yaml` belongs to `binary` because the three repositories that have it all build a binary and the library repository, which has none, has no such file (Q1 of the refined plan).

Known limit (see wave D): `dist-workspace.toml` pins `actions/checkout`, `upload-artifact` and `download-artifact` by commit in its `[dist.github-action-commits]` table. Those commits are not in `pkl/Pins.pkl`, so the pin update workflow does not move them and they are not checked against a release tag. They change with the file.

Checks: `runner/test/phase5-verbatim.test.ts` (git blobs of the pinned files, both kinds of repository, `release.yml` never written) and the mutation check in `archive/spikes/phase-5-wave-a`.

## Wave B: the parameterised files

The new option `skillName` is the folder under `skills/` that the files below name. `crateName` already existed. Both are needed by the toggles that use them (a clear error for a missing one is wave C).

| File | When | What varies |
|---|---|---|
| `.github/workflows/audit-skill.yml` (`Builtins.auditSkill(options)`) | `skills` | `skillName`. With `compositeAction` the repository runs its own action (`uses: ./`, the tekhne v0.3.0 bootstrap binary, and `action.yml` in the watched paths). Otherwise it installs the released `pantheon-skill-auditor` action |
| `.github/scripts/tag-release.sh` | `binary` | `crateName`. Executable |
| `.coderabbit.yaml` | always | `skillName`. The two lines that exclude the skill are left out without `skills` |
| `SECURITY.md` | always, seeded once | binary text (names `pantheon-<crate>`, `crates/<crate>/`, and the skill line with `skills`) or the library text |

The skill auditor action is a typed pin built from `pkl/Tools.pkl` (commit, version and the binary's SHA-256 together), and is not in `pkl/Pins.pkl`: the pin update workflow would otherwise move the action without its binary. The runner still writes its `# v0.3.1` comment from the contract's pins.

`SECURITY.md` is a seed, so the skill auditor repository, which words its scope bullets differently, keeps its own text. The report in [real-repo-workflows.md](../archive/proof/docs/real-repo-workflows.md) lists it and the library `.gitignore` as recorded differences. Checks: `runner/test/phase5-parameterised.test.ts`, the mutation check in `archive/spikes/phase-5-wave-b`, and the real-repository render.

## Wave C: the toggles

The four toggles are options of `pantheon.pkl`: `skills`, `binary`, `features` and `compositeAction`. `Builtins.standardWorkflows(options)` and `Builtins.standardFiles(options)` give the whole workflow layer for a repository with those options, so a repository's `pantheon.pkl` is the options plus two spreads. Turning a toggle off removes its entries, and the runner then deletes the files it rendered before (the delete guard still applies: the file has to be tracked by git and unchanged).

| Toggle | Needs | On | What flipping it changes |
|---|---|---|---|
| `skills` | `skillName` | `true` | Removes `audit-skill.yml`. Changes `.coderabbit.yaml` (the skill lines), `SECURITY.md` (the skill line), `.agnix.toml` and `agnix.yml` (comment wording, decision 74) |
| `binary` | `crateName` | `true` | Removes `dist-workspace.toml`, `.github/actionlint.yaml`, `.github/scripts/tag-release.sh`, `.github/workflows/dist-check.yml`, `.github/zizmor.yml` and the external `release.yml`. Changes `dependabot.yml` (no ignore of the artifact actions), `release-please.yml` (no tag step), `zizmor.yml` (no config) and `SECURITY.md` (the library text) |
| `features` | none | `default` | `all` changes `rust-ci.yml` only (and the hook steps, which are not part of the standard files) |
| `compositeAction` | none | `false` | `true` changes `audit-skill.yml` (the repository's own action), `zizmor.yml` and `.github/zizmor.yml` (which also scan `action.yml`) |

A toggle that needs a name and does not get one stops the render with an error that names the option and the way out: `options.skillName is required when options.skills is true ... Set skillName, or set skills = false.` Turning `skills` or `binary` off needs no name.

The external `release.yml` is never written, hashed or deleted, so turning `binary` off leaves a `release.yml` that `dist generate` wrote. Removing it is the repository's own change.

Checks: `runner/test/phase5-toggles.test.ts` flips each toggle from a base configuration and asserts the exact files removed and changed and none added, the two error messages, and, in a scratch git repository, the files deleted when `skills` and `binary` go off (and that the external `release.yml` stays) and that turning a toggle back on writes its files again. The mutation check is in `archive/spikes/phase-5-wave-c`. The real-repository render, now through the two standard builtins, gives the same result as before.

## Wave D: the drift check and `check` in CI

| Workflow | When | What it does |
|---|---|---|
| `dist-check.yml` (`Builtins.distCheck()`) | `binary` | Runs `dist generate --check` with the cargo-dist version from `Tools.pkl` (`0.32.0`, which a test ties to the `cargo-dist-version` in `dist-workspace.toml`). Fails when `release.yml` is not what cargo-dist would write from `dist-workspace.toml`. Runs on a pull request or push that changes `dist-workspace.toml`, `release.yml` or `Cargo.toml`, and by hand. Reads contents only, no secrets |
| `scaffold-check.yml` (`Builtins.scaffoldCheck()`) | always | `pantheon-scaffold check pantheon.pkl` on every pull request, the sample job of [check-in-ci.md](check-in-ci.md). A test compares it with that sample under P.1 |

Both are new files: no repository has them yet, so the real-repository report lists them as "new". `actions/cache` is now a pin in `pkl/Pins.pkl` (v6.1.0), so the pin update workflow keeps it current.

### Finding that shaped wave D (decision 85)

`dist generate --check` fails on today's `main` of pantheon-adr, pantheon-journal and pantheon-skill-auditor. On 04-11-2026 Dependabot bumped `actions/upload-artifact` (v7.0.1 to v7.0.2) and `actions/download-artifact` (v8.0.1 to v8.0.2) inside the generated `release.yml`, and `dist-workspace.toml`, which tells cargo-dist which commits to write, was not changed. So `dist-workspace.toml` is rendered with the two commits `release.yml` uses. Checked locally with cargo-dist 0.32.0 on a clone of each of the three repositories: the check passes with the new file and fails when `release.yml` is edited by hand.

Known limit: Dependabot will keep bumping actions inside `release.yml` and cannot edit `dist-workspace.toml`, so the next such bump fails `dist-check.yml` until `dist-workspace.toml` and `release.yml` are regenerated together. The failure is the point of the check. To stop a failing Dependabot pull request each time, the rendered `dependabot.yml` of a repository with a binary now ignores those two actions (decision 86, below).

Checks: `runner/test/phase5-dist.test.ts`, the updated `phase5-verbatim.test.ts`, the mutation check in `archive/spikes/phase-5-wave-d` (11 of 11 caught), actionlint and zizmor 1.32.0 over both workflows, and the real-repository render.

## Wave E: the consumer smoke test

The consumer smoke test is a step of `verify-release` in `release.yml` (moved here from wave D, because only the release candidate contains the Phase 5 builtins). After the release is published, on all three platforms, a scratch consumer loads the package by its package URI with the published checksum, both for `Config.pkl` and for `Builtins.pkl`, renders `standardWorkflows` and `standardFiles` with `skillName` and `crateName` set into an empty folder, runs `check`, and renders again. It fails unless `check` passes, the second render writes nothing, the files of every toggle are there, and the external `release.yml` was not written. It runs with the runner installed through mise with checksums and provenance enforced, so it is the consumer's whole path.

Checks: the two new assertions in `runner/test/release-workflow.test.ts`, the mutation check in `archive/spikes/phase-5-wave-e` (6 of 6 caught), actionlint, and a local rehearsal of the same sequence with the local package (29 files written, `check` ok, second render 0 written).

## Decision 86: Dependabot leaves the two artifact actions of release.yml alone

With `binary`, the `github-actions` entry of the rendered `dependabot.yml` has an `ignore` list for `actions/upload-artifact` and `actions/download-artifact`, and the header says why. cargo-dist is the only writer of `release.yml`: those two commits change in `dist-workspace.toml`, followed by `dist generate`. Without `binary` there is no such entry. In the three binary repositories this is a recorded difference from their own `dependabot.yml`.

Known limit, not verified: whether a Dependabot `ignore` entry also stops Dependabot's security updates for these two actions. If it does, a security fix for either action has to be applied through `dist-workspace.toml` by hand. Moving the two commits into `Pins.pkl` (so the pin update workflow keeps them current) was the rejected alternative: it needs `dist` in that workflow to regenerate `release.yml` in the same pull request.

Checks: `runner/test/phase5-dependabot.test.ts`, the adjusted equality test in `workflows-b.test.ts`, the mutation check in `archive/spikes/phase-5-dependabot` (5 of 5 caught) and the real-repository render.
