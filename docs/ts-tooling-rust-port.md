# Plan: port the remaining TypeScript tooling to Rust

**Status (15-11-2026): draft 2, revised after a three-lens review (Technical, Strategic, Risk). The owner chose to port everything (D0 and D1) and named themselves for D5 (see "Decisions for the owner"). Phases 0 to 6 are done and merged (Phase 6b, the update-pins TypeScript, `freshness.ts` and `semver.ts`, was done after the first real Rust update-pins run on 16-11-2026); the gate after Phase 2 was decided go on 15-11-2026 (see "The gate after Phase 2"). Phase 7 is done, in the parts 7a to 7e below. Phase 8 (the bun pin and `runner/package.json`) is what is left.** Written against `origin/main` at `8eae6ad`.

## Why, and the case against

The rendering runner is already Rust (refocus plan, decision N1). What stays in TypeScript on bun is release and test tooling: 11 modules and 9 scripts under `runner/src` and `runner/scripts` (1,401 lines), and 41 test files plus helpers under `runner/test` (about 4,250 lines; counts as of `main` at `3a02910`, 15-11-2026, and still growing: `publish-release.test.ts` arrived with #125, and `release-bump.test.ts` and `release-automerge.test.ts` with #126). It uses only `node:fs`, `node:path`, `node:os` and `node:crypto`, with no third-party packages.

**This plan reverses a decision of two days ago.** [runner-file-sort.md](runner-file-sort.md) (13-11-2026) sorted `release.ts`, `promote.ts`, `pinupdate.ts`, the scripts that CI runs, and their tests as **Keep**. Nothing in the repository has changed since to make that wrong. The only new input is the owner's question of 15-11-2026, whether a better language exists for them.

What the port would give:

- The release logic in the same language, toolchain and test setup as the runner.
- bun leaves the release job. bun is a pinned supply-chain input there, and [release-process.md](release-process.md) already lists such inputs for a later Security review.
- No more workarounds for bun behaviour. bun consumes `--dry-run` itself, which forced the `--no-write` flag ([workflow-layer.md](workflow-layer.md)).

**What it would not give:** no new capability, and no size gain (the runner's 60 MB to 5 MB gain is already taken). It adds risk to the release path, which is public and cannot be taken back (tags are never moved, see [release-process.md](release-process.md)).

### Options

| Option | End state | Verdict |
|---|---|---|
| A. Do nothing | The tooling stays as sorted on 13-11-2026. The three release scripts changed in three commits in total, so maintenance looks cheap (`git log` on the three scripts) | A real option. Choose it if the gate below fails or D1 is "keep bun" and the owner does not value the rest |
| B. Port the release tooling and the package tests | bun is gone from the repository | The only option that delivers the headline benefit. This plan |
| C. Port the release tooling only, keep bun for the 21 package tests | Two toolchains for contributors, bun pin stays, new crate added | **Dropped by the owner on 15-11-2026 (D1).** It was worse than A or B for the contributor, and better than A for the release job only |
| D. Delete the TypeScript duplicates only | Not possible: the live TypeScript release code imports them (`promote.ts` imports checksums, semver and preflight; `knownbad.ts` imports semver, manifest and preflight; `pinupdate.ts` imports freshness) | Rejected |

## What is already in Rust, and what is not

The Rust crates have a module with the same name as most of the TypeScript modules. Phase 0 compares them; until then parity is not known. The Technical review already found differences (see Phase 0).

| TypeScript file | Lines | Rust counterpart today | Action |
|---|---|---|---|
| `src/checksums.ts` | 34 | `scaffold-core/checksums.rs` | Phase 0 parity check; **deleted in Phase 6** |
| `src/semver.ts` | 31 | `semver.rs` | Phase 0; delete in Phase 6 |
| `src/contract.ts`, `src/schema.ts` | 40, 45 | `contract.rs`, `schema.rs` | Phase 0; delete in Phase 7 (the package tests import them) |
| `src/manifest.ts` | 43 | `manifest.rs` | Phase 0; delete in Phase 6. Rust hashes text only, and release and promote hash files, so a bytes variant is needed |
| `src/preflight.ts` | 58 | `preflight.rs` | Phase 0; delete in Phase 6 |
| `src/knownbad.ts`, `src/known-bad.bundled.json` | 62 | `knownbad.rs` (embeds `known-bad.json` at the repository root) | Phase 0; delete in Phase 6. Replace the test that checks the bundled file as a subset |
| `src/freshness.ts` | 59 | `freshness.rs` reads the record only | Phase 0 for the reader; the writer is new (Phase 3) |
| `src/release.ts` | 60 | none (`assetNames`, `checkTag`, `assembleChecksums`, `pklProjectVersion`, `TARGETS`, `binaryName`) | Port in Phase 1. **Stays until Phase 5**: `build-rust.ts` imports `TARGETS` and `binaryName` from it |
| `scripts/release-assets.ts` | 30 | none | Port in Phase 1 |
| `src/promote.ts` + `scripts/promote.ts` | 106 + 82 | none | Port in Phase 2 |
| `src/pinupdate.ts` + `scripts/update-pins.ts` | 135 + 47 | **none that can be reused.** `pinupdate.ts` rewrites `Name: Pin = new { action; sha; version }` lines in this repository's `pkl/Pins.pkl`, calls five GitHub endpoints, treats a 404 as an answer and retries on a 5xx, and writes `freshness.json`. `pins.rs` and `versions.rs` handle other formats, and `latest.rs` fetches a different endpoint. Only `semver.rs` and `http.rs` carry over (and `http.rs` turns any non-2xx into the text `HTTP <n>`, so the 404 and 5xx cases need a typed error) | Fresh port in Phase 3: a `Pins.pkl` parser and rewriter, a GitHub client, a freshness writer, a fake-API test server |
| `scripts/smoke.ts` | 149 | none | Port in Phase 4 |
| `scripts/builtins-baseline/{snapshot,compare,freeze,packaged}.ts`, `check.sh`, `baseline.json`, `README.md` | 122, 74, 24, 83, plus the shell script and data | none | Port the four scripts in Phase 4. `check.sh` calls `bun snapshot.ts` and `compare.ts`: port or delete it with them. `baseline.json` stays as data; on 16-11-2026 it and its `README.md` moved to `crates/scaffold-release/baseline/`, so that Phase 8 can delete `runner/` |
| `scripts/build-rust.ts` | 117 | none | Port in Phase 5, last |
| `test/helpers.ts`, `test/support/{contract,equality}.ts` | 78, 61, 117 | `equality.rs` for `equality.ts` | Ported or deleted in Phase 7 (D1) |
| `test/fixtures/**`, `test/upstream/**` | data | Rust tests read them: `scaffold-core/src/pkl.rs:282`, `scaffold-cli/src/lib.rs:1601,2186`, `scaffold-features/tests/features.rs:84` | Moved on 16-11-2026 (7e) to `crates/scaffold-package-tests/fixtures` and `upstream`, with the Rust call sites that read them. The baseline tool reads the old folder in a tree from before the move |

## The tests fall into three groups

| Group | Files | Lines | What happens |
|---|---|---|---|
| T1: module tests with a Rust counterpart | 8: checksums, contract, freshness, knownbad, manifest, preflight, schema, semver. Plus `equality` (it tests `support/equality.ts`, which uses `Bun.YAML`, and `equality.rs` is its counterpart) | about 720 | Deleted in Phase 6, each only when a named Rust test covers the same case. Evidence in a mapping file |
| T2: release tooling tests | 11: release, promote, pinupdate, build-rust, release-workflow, promote-workflow, update-pins-script, update-pins-workflow, publish-release, release-bump, release-automerge | about 1,350 | Ported with the module they test (see "Order of work" for the files that span phases). `publish-release.test.ts` and `release-bump.test.ts` run the shell scripts `.github/scripts/publish-release.sh` and `release-bump.sh` (release-please, #125 and #126) through bun, and `release-automerge.test.ts` reads a workflow: the scripts stay Bash, and each test becomes a Rust test that spawns the script or reads the workflow, in Phase 1 with the `release.yml` change |
| T3: package tests | 21: builtins, config-name, consumer-check, hk, mise, mode-index, package-render, package-uri, phase5-* (5), presence, presence-templates, smoke-fixtures, templates, tooling, workflows-a, -b, -c | about 2,100 | **D1.** These test what the pkl package renders, read through `pantheon-scaffold contract --json`. `package-uri` also imports `pklProjectVersion` from `release.ts`. `smoke-fixtures` uses the harness `support/contract.ts` |

Three more facts that shape the work:

- **`config-name.test.ts`** is a repository-wide hygiene test. Its allow-list names `runner/test/config-name.test.ts`, `runner/scripts/builtins-baseline/README.md` and `docs/ts-runner-test-mapping.md` and reads each. Moving or deleting any of them throws ENOENT, so the test is edited in the same pull request as the move.
- **The workflow tests use `Bun.YAML` and `Bun.TOML`.** `scaffold-core` has a YAML parser but no TOML crate. A TOML crate is a new dependency to check with `cargo-deny` (Phase 0). Porting T3 also needs it.
- **The workflow tests match literal bun command strings** (`promote-workflow.test.ts:48-49`, `release-workflow.test.ts:71,98,159,165-166`, `update-pins-workflow.test.ts:47,56`). They are the only controls on permissions and secrets in these workflows, so see "Verification" for how their assertions are protected.

## Decisions for the owner

**Recorded on 15-11-2026.** The owner said "I want Rust instead of TypeScript" and "proceed", and accepted the Phase 0 goal that listed these settings. D0 follows from that. The owner then confirmed D1 ("I want typescript gone") and D5 ("I am the owner"). D2 to D4 are the plan's recommended defaults, set by the person writing the plan, and the owner may change any of them.

| Decision | Setting |
|---|---|
| D0 | Port (option B): bun leaves the repository |
| D1 | Port the package tests too: no TypeScript and no bun left (owner, 15-11-2026) |
| D2 | New crate `scaffold-release`, built in an unprivileged job and passed as an artifact; reuse `scaffold-core` |
| D3 | Add `scaffold-release` to `cargo-mutants`; `main` is not excluded |
| D4 | Yes, this repository wants the tooling in Rust |
| D5 | Delete the TypeScript version within 14 days of one real release and one real promotion on the Rust tools. The repository owner is responsible (owner, 15-11-2026) |

The options and reasons behind each decision follow.

- **D0, whether to do it at all.** The options table above. This plan assumes B, and the gate lets the owner stop at A.
- **D1, the package tests (T3). Decided by the owner on 15-11-2026: port them, so no TypeScript and no bun is left in the repository.** They become a cargo integration test that parses `contract --json` with `scaffold-core::contract`. Option C (keep bun as their harness) is dropped. They test pkl output, not runner code, and porting them is about 2 to 3 of the days in the total. This was decided before the gate, as the plan asked, so gate criterion 5 is met for D1.
- **D2, where the code lives, and what runs with a token.** Recommended: one new crate `crates/scaffold-release` (`publish = false`, binary `scaffold-release`, not part of the release binaries), arguments parsed by hand as `scaffold-cli` does. It joins the workspace automatically (`members = ["crates/*"]`). **It is built in a separate job with `permissions: contents: read` and no token, uploaded as an artifact, and the privileged jobs only download and run it** (see Risks). Sub-choice: reuse `scaffold-core` (shares checksums and semver, pulls the `ureq`, `ring` and `rustls` tree into the build) or a leaf crate with only `sha2` and `serde_json` (less reuse, much smaller build). Recommended: reuse `scaffold-core`, because the build job is unprivileged; revisit if Phase 0 finds a `build.rs` the owner does not want.
- **D3, mutation testing.** Add `scaffold-release` to the `cargo-mutants` set. Say up front which functions are excluded (code that shells out or calls the network), and **do not exclude `main`**: a `main` that exits 0 on a `PromoteError` must be caught.
- **D4, does the repository want this tooling in Rust.** The owner's own convention for throwaway helper scripts is TypeScript on bun, but this is project tooling, so that convention does not decide it. The owner decides for the repository.
- **D5, how long two implementations may coexist, and who owns removing the old one.** Decided by the owner on 15-11-2026: one real tagged release and one real promotion on the Rust tools, then delete within 14 days. **The owner of the repository is the person responsible**, and has asked for it to be done without further sign-off at that point, so deleting is not blocked on a review. The conditions (a real release and a real promotion made by the Rust tools) still hold.
- **Opportunity cost.** The charter's first pass (four repositories) is not finished. The owner says whether this plan competes with that work or follows it.

## Order of work

One pull request per phase, each from a branch cut from fast-forwarded `main`. In every phase the TypeScript version stays until the Rust version has passed in CI **and** a real release or promotion has used it. Phase 6 is gated on that, so a revert of an earlier phase never restores a workflow that calls a deleted file.

| Phase | Work | Done when | Estimate (guess from line counts) |
|---|---|---|---|
| 0 | Compare each module in the first block of the table with its Rust twin. Write `docs/ts-tooling-test-mapping.md`. A gap becomes a Rust test first. **Where the TypeScript side is the wrong one, record a deliberate difference** (known: `semver` orders `1.0.0-01` and `1.0.0-1` inconsistently in TypeScript and accepts a 20-digit major, Rust rejects it; `preflight` and `manifest` have more checks in Rust). Also the cheap probes: `cargo deny check` with a candidate HTTP crate and a TOML crate; the crates in the `scaffold-core` tree that have a `build.rs`; the time of a cold `cargo run` in a job; `gh search code 'release-assets.ts' --owner pantheon-org` for other consumers of these scripts | Every T1 test has a named Rust test, a new one, or a recorded deliberate difference; the probes are answered in the file. **Done 15-11-2026**: [ts-tooling-test-mapping.md](ts-tooling-test-mapping.md) maps all 114 tests (109 already covered, 3 Rust tests added, 2 not applicable) and answers the probes | about 4 hours |
| 1 | Create `scaffold-release` with `release-assets` (`version`, `check-tag`, `checksums`). Port `release.ts`, keep it until Phase 5. Switch the three call sites in `release.yml` | See "Verification": differential run on one fixed `dist/` directory, plus the dry-run steps below | about 1 day |
| 2 | Port `promote` (module, script, `promote.test.ts`, `promote-workflow.test.ts`). Switch `promote.yml`. `release-workflow.test.ts` imports `stableWording`, so its Phase 2 part moves here | See "Verification": the negative dry runs and the argv test | about 1.5 days |
| Gate | **Go or no-go.** Criteria and artefacts below | Recorded in this file by the owner | |
| 3 | Fresh port of `update-pins` and `pinupdate` (see the table). Add a `dry_run` input to `update-pins.yml` in the same pull request. Update the `paths:` filter of that workflow, which lists the TypeScript files | A run with `dry_run` before merge; the merge itself does not trigger a writing run (drop the `push` trigger for that commit) | about 1.5 days |
| 4 | Port `smoke` and the four baseline scripts with `check.sh`. Choose an HTTP server for `packaged` using the Phase 0 `cargo-deny` answer. `smoke-fixtures.test.ts` needs the harness that Phase 7 replaces, so the Phase 7 design (a cargo test on `contract --json`) must exist before this phase ports it | `smoke` and the 55-configuration baseline pass on all three platforms | about 1 day |
| 5 | Port `build-rust` (zig wrapper scripts, build, static-link check), and give `TARGETS` and `binaryName` a Rust home. **Decided by the owner on 16-11-2026: `release.ts` and `build-rust.ts` stay as the reference until Phase 6**, as in Phases 1 to 4, because `release.ts` still has users that belong to other phases (`release-assets.ts` and `release-assets-diff.sh`, `package-uri.test.ts`, `promote.test.ts`, `release.test.ts`). The D5 deadline still holds | Byte-identity: a binary built by the Rust tool equals one built by the TypeScript tool from the same commit, on all three targets (release-process.md says builds are reproducible from one commit). The `ci.yml` rust-binary job already builds and smoke-tests it on every pull request | about 1 day |
| 6 | Delete the duplicated TypeScript modules, `known-bad.bundled.json`, and the T1 tests, with the mapping file as evidence. **Gated on one real tagged release and one real promotion made by the Rust tools** (met by v0.4.0 on 15-11-2026). **Decided by the owner on 16-11-2026: Phase 6 deletes `release-assets.ts`, `promote.ts` (script and module), `smoke.ts`, `build-rust.ts`, the baseline scripts and `check.sh`, `knownbad.ts` with `known-bad.bundled.json`, `checksums.ts`, their tests (`build-rust`, `checksums`, `knownbad`, `promote`, and `release.test.ts` except `pklProjectVersion`), the five differential scripts and the `implementation` input of `promote.yml`; `release.ts` is cut down to `pklProjectVersion`. Phase 6b, after the first real writing run of the Rust update-pins, deletes `pinupdate.ts`, `update-pins.ts`, `freshness.ts` (`pinupdate.test.ts` imports it), their tests, `update-pins-diff.sh`, the fake API and the `implementation` input of `update-pins.yml`. `contract`, `schema`, `manifest`, `preflight`, `semver` and `equality` stay until Phase 7: the package tests import them, and `package-uri.test.ts` also imports `preflight` and `pklProjectVersion`** | `grep` finds no import of a deleted module | about half a day |
| 7 | Port T3 (D1: no TypeScript is left). The tests become a cargo integration test on `contract --json`. Move fixtures only together with the three Rust call sites that read them | No `.ts` file is left under `runner/` (the update-pins pieces of Phase 6b excepted until it is done) | about 4 to 6 working days in about 6 pull requests, not 2 to 3: on 16-11-2026 there were 377 tests in 38 files under `runner/test`, not 21 files and 2,100 lines, because tests were added after this plan (`dependabot-entry`, `release-builtins`, the release, promote and update-pins workflow tests, `publish-release`, `release-bump`). **Decided by the owner on 16-11-2026:** the tests live in a new dev-only crate `scaffold-package-tests` and run in process through the core library; the fixtures move into it in the last part (7e) together with their Rust call sites, (done in 7e part 2; until then the Rust tests read `runner/test/fixtures`); all the workflow and script tests are in scope (bun cannot go in Phase 8 otherwise); 7a is first and Phase 6b is needed only before the update-pins part of 7d. **7a** (first eight files, 41 tests), **7b** (`builtins`, `workflows-a/b/c`, `tooling`; then `hk`, `mise`, `phase5-*`), **7c** (`presence`, `presence-templates`, `release-builtins`), **7d** (the workflow and script tests), **7e** (delete the module tests with a Rust twin and their modules, `helpers.ts` and `support/`; move the fixtures; docs). Each ported file has rows in `docs/ts-tooling-test-mapping.md`, and each pull request breaks the package in several places to show that the Rust tests fail where the TypeScript ones did |
| 8 | If bun is gone (the baseline already moved out of `runner/`, see Phase 7): remove the `bun` pin from `mise.toml`, **regenerate `mise.lock`** (it carries `[[tools.bun]]` entries), delete `runner/package.json` and the `bun test` steps. Update the docs below | `grep -rn bun` finds only history | about 3 hours |

**Files that span phases.** `release-workflow.test.ts` asserts on `release-assets.ts` (Phase 1), `stableWording` (Phase 2), `smoke.ts` (Phase 4) and `build-rust.ts` (Phase 5), and on `ci.yml`. It is edited in four phases, each time in the same pull request as the workflow change. `build-rust.test.ts` imports `release` and `build-rust`, so it moves in Phase 5.

**Total, as a guess:** about 7 to 10 days (D1 is decided: the package tests are ported too).

## The gate after Phase 2

Continue only if **all** of these hold, each with the artefact that proves it:

1. In Phases 1 and 2 the Rust and TypeScript tools gave byte-identical output on the same inputs. Artefact: the differential test run in CI (URL recorded here).
2. Every dry run listed under "Verification" ran, with the run URLs recorded here.
3. Time of the release `prepare` job, before and after (artifact download against a cold build). Artefact: the two job times. No-go if the `prepare` job gets more than 2 minutes slower.
4. The count of behaviours the Rust version had to add that had no TypeScript test (the Phase 0 differences plus any new ones) is written down. It is information for the owner, not a pass mark.
5. D1 and D5 are decided (both done on 15-11-2026).

**On no-go:** revert Phases 1 and 2 (their pull requests, plus the external undo in "Rollback"), and the end state is option A. Keeping them is allowed only if the owner says so in this file.

### The decision (15-11-2026): go

The owner decided **go** on 15-11-2026, after Phases 1 and 2 were merged (#130 and #132) and this evidence was reported:

| # | Result |
|---|---|
| 1 | Met. The release differential (142 comparisons) and the promote differential (36) found 0 differences. In every real dry run of `promote.yml` the guards job ran both tools and printed no "disagree" |
| 2 | Met with one substitution. `v0.3.0` was meant to be the pass case, but the owner had promoted it at 21:20 with the TypeScript tool, before #132 merged. `v0.1.4`, a real pre-release, was run instead: the Rust tool and the TypeScript tool both passed the assets, attestation and digest guards with real downloads and `gh attestation verify`, and both refused at the known-bad guard. Also run: a dispatch from a branch (refused), `v0.2.0` (already stable, refused), `v9.9.9` (no such release, refused). The three refusals and the pass up to the known-bad guard were identical from both tools |
| 3 | Met. The first report said it was not met (+167 s), because that run waited about 104 s behind the real `v0.3.0` release run (the workflow's concurrency group). Re-measured from the job times: the new `tool` job takes 70 s (toolchain 8 s, build 54 s) and `prepare` 6 s, against 12 s before, so +64 s, under the 2 minute limit. The owner chose not to spend time on a faster build, and not to cache it (a cache the build job restores could swap the binary in, and the checksum check cannot catch that because the same job produces both) |
| 4 | Information. Phase 0 added 3 Rust tests and pinned 2 semver differences and 1 wording difference. Phase 2 mutation testing found 3 survivors (an unused function and the exit code of a killed `gh`), which were fixed |
| 5 | Met on 15-11-2026 |

**Still unproven when the decision was made:** the Rust tool had never made a real edit of a release. That changed the same evening, see below.

### Real use of the Rust release tools (15-11-2026)

After the gate, `v0.4.0` went through the Rust tools for real, so the conditions of Phase 6 (and the clock of D5, 14 days, to 29-11-2026) are met for the release and the promotion:

| What | Evidence |
|---|---|
| `v0.4.0` released by the Rust tool | Release run 43767262162 on the tag commit (it contains Phase 1): `Build the release tool`, `Check the version` (the real `check-tag`), `Attest and publish` (the real `checksums`) and the three `Verify the published release` jobs all succeeded |
| `v0.4.0` promoted by the Rust tool | Promote run 43768748500 at 22:20, run by the owner with the default `implementation`: the Rust step ran, the TypeScript steps were skipped, every guard passed, and it printed `after the edit: stable and latest, the assets and checksums.txt are unchanged and still attested` and `v0.4.0 is now a stable release.` The dry run before it (43768540185) ran both tools and they agreed |
| `update-pins` | Ran for real on Monday 16-11-2026 at 04:23 UTC (pull request 160, merged the same day). It needed a hand step the bot does not do: the pull request stages only `pkl/Pins.pkl`, so `Scaffold check` and `Baseline render` failed until a person rendered again and refreshed the baseline. Since 17-11-2026 the baseline masks pins and `mise run refresh-pins` is that step (decision 92 in [workflow-layer.md](workflow-layer.md)). Before that, two dry runs (branch and main) ran both tools against the real GitHub API and agreed |

Phase 6 still waits for Phases 4 and 5, because the TypeScript release code is imported by what they port.

## CI call sites that change

| File | What calls bun today | Phase |
|---|---|---|
| `release.yml` | none: the `bun test` step of the `package` job went with Phase 6b. The release assets (Phase 1), the smoke suite (Phase 4) and the runner build (Phase 5) run `scaffold-release` | 8 |
| `promote.yml` | none: the Rust tool only since Phase 2, and the TypeScript way back was deleted in Phase 6 | |
| `update-pins.yml` | none: the Rust tool only since Phase 6b (before, `update-pins.ts` and a `push` trigger with `paths` for it) | 3 |
| `ci.yml` | actionlint on `crates/scaffold-package-tests/fixtures/consumer-check.yml`. The baseline, smoke and runner build steps run `scaffold-release` since Phases 4 and 5, and the TypeScript twins and their differential scripts were deleted in Phase 6. `update-pins-diff.sh` and the `bun test` step went with Phase 6b | 8, 7, 6b |

`release.yml` (the `prepare` and `Attest and publish` jobs), `promote.yml` and `update-pins.yml` install no Rust today. Each needs the artifact download from D2, not a toolchain step.

## Docs to update

`docs/runner-binary.md`, `docs/release-process.md`, `docs/workflow-layer.md`, `docs/runner-file-sort.md` (say it was superseded), `docs/ts-runner-test-mapping.md`, `docs/tooling-layer.md`, `docs/adopting.md`, `docs/check-in-ci.md`, `docs/feature-toggles.md`, `docs/templates-design.md`, `features/README.md`, the baseline `README.md`, comments in `pkl/Builtins.pkl` and `pkl/Builtins/{mise,hk}/Builtin.pkl` that name `mise.test.ts` and `hk.test.ts`, and the comment in `crates/scaffold-core/tests/yaml_corpus.rs` that names a script that no longer exists.

## Verification

Every phase: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --exclude scaffold-features --locked`, and the CI jobs that call the switched tool. Success means zero failures in every test binary, not a sum of passes. A pass on macOS is not a pass in CI: smoke and baseline run on `linux-x64`, `linux-arm64` and `darwin-arm64`.

**What the dry runs cannot show, and what replaces it.** The earlier draft said a dry run proves the switch. It does not.

| Phase | Gap in the dry run | Added control |
|---|---|---|
| 1 | A dry run takes the `version` branch (`release.yml:54-58`), so `check-tag` first runs on a real tag. A last-release comparison of `checksums.txt` is impossible (different source gives different hashes) | Run the TypeScript and Rust tools on the **same** dry-run `release-assets` artifact and diff their output in CI. Add `check-tag v$(version)` as a dry-run step. Negative cases: wrong version, no `v`, two version lines |
| 2 | The `promote` job (the edit) is skipped on a dry run (`promote.yml`, `if: inputs.dry_run == false`), so `gh release edit`, `stableWording`, the post-edit check and the latest-tag check never run. A guard that always passes also reports `ok`. Guard 1 requires `refs/heads/main`, so the Rust version cannot be dry-run from the pull request branch. The only pre-releases left are v0.1.0 to v0.1.4, so a pass case needs a real release | Negative dry runs as an exit condition: a pre-release, an already-stable tag, a bogus tag, a branch dispatch (refused). Diff the TypeScript and Rust `check` reports for the same tag in one run. Put the `gh` calls behind a function and test the exact arguments. `promote` reads `known-bad.json` from the checkout and **fails closed**, unlike `knownbad.rs`, which fails open for consumers: the release tool must not reuse `load_known_bad` for this. The first real promotion after the switch keeps the TypeScript job available for that release |
| 3 | `update-pins.yml` has no inputs, and the push trigger runs a writing job on merge with the workflow's token, force-pushing `chore/update-pins` and `freshness` | The `dry_run` input and the dropped push trigger in Phase 3. Exit non-zero on any problem is a mutation-tested property. The freshness record is accepted only as `...Z` with no fractional seconds, so a test pins the written form |
| 5 | A transcription slip in the zig wrapper, the `--target` handling or a feature-unifying flag can change the shipped binary without failing smoke | The byte-identity check in the Phase 5 row |

**Workflow-invariant tests.** `release-workflow.test.ts` and `promote-workflow.test.ts` are the only checks on permissions, secrets and environment. When a phase edits their literal command strings, the pull request description shows the old and new assertion side by side, and no assertion is loosened. `archive/spikes/phase-2b-wave-d/mutate.ts` is the harness that proved these tests catch 19 regressions; it targets files this plan deletes, so the plan adds an equivalent that mutates the workflow files, or states that the protection is dropped. Add a workflow test that no `cargo` step shares a job with a write permission.

**Checksum format.** The writer (release tool) and the reader (`scaffold-core` `checksums.rs`) would share code after the port, so a round-trip test passes even if the format drifts from what `mise` and `sha256sum -c` expect. Pin the format with a golden file that this code does not generate.

## Risks

| Risk | Likelihood, impact | What reduces it |
|---|---|---|
| The first run of the Rust `check-tag`, the promotion edit and the pin writer happens on a real tag, a real release or `main`, because no dry run exercises them. **The highest risk.** | medium, high | The added controls in the table above. A burnt tag cannot be reused |
| Supply chain: the `scaffold-core` dependency tree (`ureq`, `ring`, `rustls`, `regex`, `minijinja` and others) would compile, with its `build.rs` files, inside jobs that hold write tokens and attest. Today the bun scripts run no third-party code. `cargo-deny` and `--locked` do not review build scripts | low, high | D2: build in an unprivileged job, download the artifact in the privileged jobs. `cargo build --locked` and the run in separate steps if a build in the job is unavoidable. Phase 0 lists the crates with a `build.rs`. A workflow test that no `cargo` step shares a job with a write permission |
| Rust and TypeScript twins differ in a corner | medium, medium | Phase 0 with recorded deliberate differences |
| Coexistence drift: a workflow calls one implementation, the tests cover the other, and a change to `scaffold-core` for a runner reason silently changes release assembly | medium, medium | Golden files; a path filter so that a change to those `scaffold-core` modules also runs the release tool's tests; D5 time limit |
| `build-rust` is a bootstrap: Rust builds the Rust binary | low, high | Phase 5 last, with the byte-identity check |
| The HTTP-server and TOML crates may fail `cargo-deny` | medium, low | Phase 0 probe. A standard-library server is the fallback |
| Mutation testing time grows | medium, low | Pull requests mutate only the diff, as today |
| No user-visible gain | high, medium | The gate, and option A |
| A moving target: three release pull requests landed on `main` in one day (#124 to #126), and two added bun tests. Every new release test written in TypeScript is more to port | high, medium | Each phase starts with a fresh inventory of `runner/test` and `runner/scripts`. Recommended: write new release tests in Rust from now on, so the port does not grow while it is under way. The owner decides |

## Rollback, by phase

A revert pull request restores the repository. It does not undo what a workflow already did outside it. A tag-triggered release runs `release.yml` from the **tagged** commit, so a bad workflow at a tag is fixed by a new patch tag and a `known-bad.json` entry, not by reverting `main`.

| Phase | Revert the pull request, and also |
|---|---|
| 1 | A burnt tag if `check-tag` falsely rejected; a published pre-release and attestations if a bug passed `verify-release`: new patch tag, `known-bad.json` entry |
| 2 | A release already flipped to stable and latest: `gh release edit --prerelease` by hand |
| 3 | The force-pushed `freshness` branch and the `chore/update-pins` branch and pull request. A stale `freshness.json` (older than 30 days) makes consumer upgrade workflows fail, so check it after the first run |
| 5 | A published binary built differently: new patch tag, `known-bad.json` entry |
| 6 | After Phase 6, reverting Phases 1 to 3 alone is not enough: restore the deleted files too. That is why Phase 6 is gated on real use |

## Out of scope

- Any change to the pkl package, the contract or the rendering runner.
- Changes to the release process itself (tags, pre-release, promotion, rollback).
- The archive under `archive/`, which stays as history.

## Not checked

- The release flow changed on `main` while this plan was written: release-please (#125) now owns the version, the tag and a draft release, and `release.yml` publishes through `.github/scripts/publish-release.sh` (from 0.4.0 rendered from the `release-please` Builtin for the `draft` shape, with `RELEASE_NOTES` and `DIST_DIR` in the environment). The review of the release path (dry-run limits, rollback, the order of Phase 1 and 2) was done on the flow before #125 and must be re-read against `docs/release-process.md` before Phase 1 starts.
- Behavioural parity of the Rust twins beyond what the review found (Phase 0).
- Whether `cargo-deny` accepts an HTTP or a TOML crate, and which crates have a `build.rs` (Phase 0).
- Whether other `pantheon-org` repositories call these scripts. A search of the local checkouts found nothing; the organisation-wide search is Phase 0.
- Whether two successive real releases of the same source have identical `checksums.txt` (reproducibility), and the real cost of a cold `cargo` build in a job. Measured on 16-11-2026 on `darwin-arm64` (Phase 5): a clean rebuild of `scaffold-cli` in the same target directory gives an identical binary, and a build with `CARGO_TARGET_DIR` set to a different directory gave different bytes (the cause was not looked into). The Phase 5 check therefore builds with both tools the same way: each from nothing, in its own copy of the checkout at the same path, with no `CARGO_TARGET_DIR`. Three runs of it in three different temporary copies gave the same binary every time (sha256 `99e6306247207c89…`, `darwin-arm64`), so where the copy lives does not matter. The Linux musl builds are not measured yet.
- The effort estimates are guesses from line counts.
