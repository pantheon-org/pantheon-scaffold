# Release process

Added in phase 2b, wave D. A release publishes the pkl package and the runner binaries together, under the version in `pkl/PklProject`, as a GitHub **pre-release**. The pre-release flag is the gate (decision 46): promoting a release to a stable one is a separate step, described under "Promoting a release" below.

## Cutting a release

1. In a normal pull request, set `version` in `pkl/PklProject` to the version (it already holds the next unreleased version, see the rule below), and raise `requiresRunner` in `pkl/Config.pkl` only if the package needs a newer runner than the last release. A pre-release ranks below its release, so never require a runner version a pre-release of the same version cannot meet.
2. Merge it, then run the release workflow by hand once (Actions, release, Run workflow, `dry_run` left on). It tests the package, builds the runner on each of the three platforms, smoke tests each binary on its own platform and assembles `checksums.txt`, and attests and publishes nothing. The assembled files are kept for 7 days as the `release-assets` artifact.
3. Push the tag: `git tag v<version> && git push origin v<version>` from the merged commit. Tags `v[0-9]*` start the real release. The tag ruleset forbids moving or deleting a pushed tag.

**After a release, the first pull request that changes anything under `pkl/` must bump `version` in `pkl/PklProject`.** `pkl project package` fetches the published package with the same version and refuses to build when the contents differ ("was already published with different contents ... bump the packages version"). The package build in CI and several runner tests fail until it is bumped. The version in `PklProject` is therefore always the next unreleased one, and the next tag is that version.

**A job that fails on a 5xx from GitHub is safe to re-run.** Downloads from GitHub release URLs (the mise download in `mise-action`, or the published-version check that `pkl project package` makes) can return 504 during an outage, which fails the job before any of our code runs. The release workflow publishes nothing until its last two jobs, and it refuses a version that already has a release, so re-running it is safe. This is decision 81 of the plan: no retry code.

A version is never re-published. If a release is bad, cut the next patch version and add the bad version to `known-bad.json` on `main`.

## What the workflow does

| Job | What it does | Permissions |
|---|---|---|
| prepare | The tag must be `v<version>` and match `pkl/PklProject`. A release for that tag must not exist yet. | read |
| package | The runner tests (`bun test`) and the package build (`pkl project package`). Uploads `package-files`. | read |
| build | One job per target, on a machine of that target (`ubuntu-latest`, `ubuntu-24.04-arm`, `macos-latest`): installs the pinned Rust toolchain, installs the tools through mise with `MISE_ENV=rust-build` (which adds `zig`, see `mise.rust-build.toml`), builds the Rust runner with `runner/scripts/build-rust.ts` (static musl on Linux, with `zig cc` compiling the C code) and uploads it as `binary-<target>`. | read |
| smoke | The 16 check smoke suite on linux-x64, linux-arm64 and darwin-arm64, each on the binary built for that target. | read |
| release | Downloads `package-files` and the three binaries into one folder, cross-checks the package files, writes `checksums.txt`, attests every asset, creates the pre-release. Attestation and publication run on a tag push only. | contents, id-token and attestations: write |
| verify-release | From the outside, on all three platforms: installs the runner through mise with `mise lock` and `mise install --locked` (checksums and provenance enforced), runs `gh attestation verify` on the downloaded `checksums.txt`, limits the lock to the supported platforms (`lockfile_platforms`) and checks that it names only the three runner binaries, renders and checks through the real package URI with the published checksum, and runs `upgrade --dry-run` against the real release. Phase 5: it also renders the whole standard set (`Builtins.standardWorkflows` and `standardFiles`, imported from the same package URI) into an empty folder, runs `check`, and renders again expecting "written 0 file(s)" | read |

`checksums.txt` lists the three binaries, the package metadata file (its checksum is the one pkl checks with `::sha256`) and the zip. The attestation covers the binaries, the package files and `checksums.txt`.

`verify-release` is the first place the real download, `gh attestation verify` and mise's provenance check meet the code that `upgrade` and `mise install` rely on. It runs after publication, so a failure leaves a published pre-release: say so, cut the next patch version, and list the failed one in `known-bad.json`.

## Safety properties

They are asserted by `runner/test/release-workflow.test.ts` and kept honest by `archive/spikes/phase-2b-wave-d/mutate.ts`:

- Only a `v[0-9]*` tag push or a manual run starts it, never `pull_request_target`.
- Every job asks for its own permissions, and only `release` can write.
- Every action is pinned by SHA, no secrets are used, checkouts keep no credentials.
- Tag names, refs and inputs reach scripts through `env`, never through `${{ }}` inside `run`.
- A manual run is a dry run unless told otherwise.

## Promoting a release

Added in phase 8. A release stays a pre-release until it is promoted. The promotion is the workflow `promote.yml`, run by hand from `main`, and it is the only thing that edits a release. It never changes the assets: it removes the pre-release flag, drops "(pre-release)" from the title and "Pre-" from the first words of the notes.

1. **Dry run.** Run `promote` with the tag (for example `v0.1.5`) and `dry_run` left true. One job runs every guard and edits nothing. It never touches the environment.
2. **Real run.** Run it again with `dry_run` false. The first job repeats the guards, then the `promote` job runs in the environment `promote`. That environment has no protection rules today, so the job starts at once (see "Who approves" below). It runs every guard again, edits the release, and checks the result.

The guards, all before the edit (`runner/src/promote.ts`, with tests and a mutation check):

| Guard | Refuses when |
|---|---|
| Run from main | the workflow runs on any ref other than `refs/heads/main` |
| Tag shape | the tag is not `v<major>.<minor>.<patch>[-pre-release]` |
| Release state | the release is a draft, or is already stable (a published version is never re-published) |
| Assets | the release does not carry exactly the files a release carries |
| Attestation and digests | `checksums.txt` has no valid attestation, or any file's digest differs from it |
| Known bad | the version is in `known-bad.json` |
| Release run | the release workflow did not succeed for the tag, or its verify job is missing or not successful |

After the edit it checks that the release is stable and the latest, that `checksums.txt` is byte for byte the one attested before, that every digest still matches, and that the attestation still verifies (decision 46's revisit trigger: promotion must not change the asset bytes or the checksums).

The tag is not compared with `pkl/PklProject`: on `main` that file always holds the next unreleased version, so the comparison would refuse every older candidate.

**Who approves.** Nobody, today. The environment `promote` has **no protection rules** (decision 91, 09-11-2026): the owner decided that a required reviewer would block the work while the repository has one contributor. The job still names the environment, and GitHub creates a missing environment with no rules on its first use, so the job runs as soon as it is dispatched. The environment then records each promotion as a deployment, and it does not gate it.

What controls a promotion until a second contributor exists:

- it is dispatched by hand, from `main`, by someone with write access to the repository (the owner);
- every guard in the table above, run twice (once in each job), and the post-edit checks;
- the run itself, in the Actions tab, with its log.

This is a recorded accepted risk and not an approval of any kind: nothing is checked by a second person (decisions 78, 90 and 91).

### Before onboarding a contributor

Do this **before** the new contributor, collaborator, organisation member or bot with write access can dispatch a workflow here, and at the latest before the next promotion. It takes about five minutes and changes no code, because the workflow already names the environment.

1. Open the repository's Settings, then Environments, then `promote` (the environment exists after the first real promotion run. If it does not, click "New environment", name it `promote`, and click "Configure environment").
2. Tick "Required reviewers" and pick **a person who is not the maintainer**. Leave "Prevent self-review" ticked, so that whoever dispatches a promotion cannot approve it.
3. Under "Deployment branches and tags", choose "Selected branches and tags", add the rule `main`, and click "Add rule".
4. Click "Save protection rules".
5. Check it: `gh api repos/pantheon-org/pantheon-scaffold/environments/promote --jq '[.protection_rules[]|.type]'` must list `required_reviewers` and `branch_policy`.
6. Run `promote` once as a dry run: the first job passes and the second is skipped, so the check does not need an approval. The first real promotion afterwards waits for the reviewer.
7. Record it: add a dated line under "Releases so far" and in the plan, with who the reviewer is, and say the independent approval is now in place (it ends decision 91's accepted risk).

Also settle, in the same change, the other places where the maintainer is the only check: a required approving review on `main` and the creation of release tags (see "Accepted risks and open items").

## Accepted risks and open items

- **No Security review.** The owner recorded P.3 as an accepted risk on 07-11-2026 (decision 78). Security has not reviewed this pipeline or the pkl flags, and no Security reviewer is named. This is a recorded risk, not an approval.
- **The Rust build downloads crates and a C compiler.** `cargo build --locked` takes the versions in `Cargo.lock`, which `cargo-deny` checks for advisories, licences and sources in CI. The C code of `ring` is compiled by `zig cc`, a version of Zig that mise pins and checksums in `mise.rust-build.lock` and that only the release-binary jobs install (the Ubuntu archive is not used: when its DNS failed on the runners on 11-11-2026, every Rust binary job failed with it). The pipeline downloads its other tools through mise too. All of these are supply-chain inputs for any later Security review.
- **No second reviewer, and no approval step.** The repository has one contributor. The owner recorded on 09-11-2026 that no second person approves the promotion (decision 90) and that the environment `promote` has no protection rules (decision 91). The steps to add them are under "Before onboarding a contributor" and are due before a second contributor can dispatch a workflow.
- **The tag ruleset has no creation rule**, so anyone with write access can push a `v*` tag. Today that is the maintainer only.

## Releases so far

| Version | Published | Status |
|---|---|---|
| 0.1.0 | 07-11-2026, pre-release | Published and **known bad** (listed in `known-bad.json`). Its runner could not read the version of a package URI that carries the pinned checksum, which every real consumer has, so it fails on every real consumer. The release pipeline's `verify-release` job found this on all three platforms. The assets, the attestations, `mise lock` and `mise install --locked` with provenance, and the real `gh attestation verify` all worked. A published version is never re-published, so the fix is 0.1.1. |
| 0.1.1 | 07-11-2026, pre-release | **Known bad** (listed in `known-bad.json`, with the reason). Published and verified from the outside on ubuntu-latest, ubuntu-24.04-arm and macos-latest (release run 42427253206): `mise lock` and `mise install --locked` with provenance, `gh attestation verify` of `checksums.txt`, render and check through the real package URI with the published checksum, and `upgrade --dry-run` against the real release. Finding: without `lockfile_platforms`, `mise lock` names the package zip for platforms with no runner (see [runner-install.md](runner-install.md)). |
| 0.1.2 | 08-11-2026, pre-release | **Known bad** (listed in `known-bad.json`, with the reason). The Phase 5 release candidate: it holds the standard workflow and file sets, the four toggles, `dist-check.yml` and `scaffold-check.yml`. A manual dry run first (run 42533622055), then the tag on `f26da89` (release run 42533855429). Published and verified from the outside on ubuntu-latest, ubuntu-24.04-arm and macos-latest, as for 0.1.1, and with the consumer smoke test: the standard workflows and files render through the real package URI (29 files written), `check` passes (28 owned files match), and a second render writes 0. `gh attestation verify` of `checksums.txt` passes. Superseded by 0.1.3, which also holds the Dependabot ignore for `release.yml` (decision 86) and the `.pkl-gen/cache/` line in `.gitignore` (decision 87). |
| 0.1.3 | 09-11-2026, pre-release | **Known bad** (listed in `known-bad.json`, with the reason). The Phase 5 release candidate that Phase 7 proves. It is 0.1.2 plus decisions 86 and 87. A manual dry run first (run 42561726542), then the tag on `2630bb2` (release run 42561933310). Published with 8 assets and verified from the outside on ubuntu-latest, ubuntu-24.04-arm and macos-latest, with the consumer smoke test: the standard workflows and files render through the real package URI (29 files written), `check` passes (28 owned files match), and a second render writes 0. `gh attestation verify` of `checksums.txt` passes. The next change under `pkl/` must bump the version to 0.1.4. |
| 0.1.4 | 09-11-2026, pre-release | **Known bad** (listed in `known-bad.json`, with the reason). 0.1.3 plus the `upgrade` fix of decision 88 and the generated-file header of `plumber.yml` (decision 89). A manual dry run first (run 42574935271), then the tag on `f87d607` (release run 42576757594). Published with 8 assets and verified on the three platforms; `gh attestation verify` of `checksums.txt` passes. The Phase 7 benefit test found that `upgrade` still exited 3 for a consumer that renders `mise.toml`, so 0.1.4 is superseded by 0.1.5. |
| 0.1.5 | 09-11-2026, pre-release, **promoted to a stable release on 09-11-2026** | 0.1.4 plus the `upgrade` fix that records the runner pin in the manifest (PR 38). The Phase 7 release candidate, signed off by the owner for the four repositories. Promoted by the `promote` workflow (run 42636955156, dispatched by the owner): every guard passed twice, the release is stable and the latest, the title and notes lost the pre-release wording, and all 8 assets and `checksums.txt` are byte for byte the ones attested before, checked from outside against a snapshot taken before the run. The approval was the owner's own dispatch: the environment has no protection rules (decision 91). A manual dry run first (run 42587832833), then the tag on `e7cf2e6` (release run 42588132278). Published with 8 assets and verified on the three platforms; `gh attestation verify` of `checksums.txt` passes. The four repositories render with zero uncategorised differences and both benefit-test clones upgrade with exit 0. The next change under `pkl/` must bump the version to 0.1.6 (done). |
| 0.1.6 | 11-11-2026, pre-release, **promoted to a stable release on 11-11-2026** | The first release with the Rust runner (refocus plan, Phase 2 and 3): the same commands, the 65 features and the 34 Builtin fixtures identical to the TypeScript runner, binaries of 5.2 to 6.5 MB (static musl on Linux) against about 59 MB. A manual dry run first (run 43026743774), then the tag on `75317cd` (release run 43031568484: all 12 jobs passed, including `verify-release` on the three platforms). Promoted by the `promote` workflow after a dry run (run 43033307531). v0.1.5 stays installable as the rollback; nothing is known bad. The next change under `pkl/` must bump the version to 0.1.7. |
