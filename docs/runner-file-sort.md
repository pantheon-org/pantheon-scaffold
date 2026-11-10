# Runner files: what happens to each in the Rust rewrite

Written on 10-11-2026 for task 3.4 of the refocus plan (remove the TypeScript runner after one release cycle of Rust use). It sorts every file under `runner/src`, `runner/scripts` and `runner/test` into one group. The sort follows the imports found on `main` after the governance and evidence move; where two groups apply, the file is marked shared.

Groups:

- **Port**: rewritten in Rust. Its tests become Rust unit tests, or features for what only a black-box run shows.
- **Keep**: stays as bun tooling, because CI or the release flow runs it.
- **Re-point**: a test of the pkl package that renders through the TypeScript runner. After removal it must render through the Rust binary instead.
- **Archived**: already moved under `archive/` (governance and the proof evidence).

## runner/src

| File | Group | Note |
|---|---|---|
| `apply.ts`, `check.ts`, `cli.ts`, `contract.ts`, `detach.ts`, `equality.ts`, `freshness.ts`, `guard.ts`, `latest.ts`, `lock.ts`, `lockcheck.ts`, `manifest.ts`, `paths.ts`, `pins.ts`, `pkl.ts`, `schema.ts`, `upgrade.ts`, `version.ts`, `versions.ts` | Port | The runner itself. |
| `knownbad.ts`, `known-bad.bundled.json` | Port, shared | Also imported by `scripts/promote.ts` through `promote.ts`. |
| `checksums.ts`, `semver.ts`, `preflight.ts` | Port, shared | Imported by `build.ts`, `release.ts`, `promote.ts`. They stay while any kept script imports them. |
| `release.ts`, `promote.ts`, `pinupdate.ts` | Keep | Used by `release-assets.ts`, `promote.ts` and `update-pins.ts`. |
| `governance/` | Archived | |

## runner/scripts

| File | Group | Note |
|---|---|---|
| `build.ts`, `release-assets.ts`, `promote.ts`, `update-pins.ts` | Keep | Run by `ci.yml`, `release.yml`, `promote.yml` and `update-pins.yml`. |
| `smoke.ts` | Keep | The black-box suite run against the built binary. The Phase 1 features replace it when they cover the same behaviour. |
| `coverage-estimate.ts` | Keep, to confirm | An estimate for builtins that were not yet built when it was written. The owner decides whether to archive it. |
| `governance.ts`, `proof-*.ts`, `real-repo-*.ts` | Archived | |

## runner/test

| Files | Group | Note |
|---|---|---|
| `apply`, `check`, `contract`, `detach`, `equality`, `freshness`, `guard`, `knownbad`, `latest`, `lock`, `lockcheck`, `manifest`, `paths`, `pins`, `pkl`, `preflight`, `schema`, `semver`, `upgrade`, `versions`, `checksums` | Port | Unit tests of one module each. |
| `cli`, `cli-freshness`, `cli-latest`, `cli-lifecycle`, `cli-upgrade`, `mode-index`, `package-uri`, `smoke-fixtures`, `consumer-check` | Port as features | They run the binary or its fixtures, so they become Phase 1 scenarios. |
| `builtins`, `hk`, `mise`, `tooling`, `phase5-dependabot`, `phase5-dist`, `phase5-parameterised`, `phase5-toggles`, `phase5-verbatim`, `workflows-a`, `workflows-b`, `workflows-c` | Re-point | They check what the package renders. They keep their value after the rewrite. |
| `promote`, `promote-workflow`, `release`, `release-workflow`, `update-pins-script`, `update-pins-workflow`, `pinupdate` | Keep | Tests of the kept tooling. |
| `helpers.ts`, `fixtures/`, `upstream/` | Keep | Shared by the tests above. |
| `governance-*`, `proof-categories` | Archived | |
