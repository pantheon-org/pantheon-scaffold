# Release process

Added in phase 2b, wave D. A release publishes the pkl package and the runner binaries together, under the version in `pkl/PklProject`, as a GitHub **pre-release**. The pre-release flag is the gate (decision 46): promoting a release to a stable one is a later step (task 5.5) and needs a reviewer other than the maintainer, which this repository does not have yet.

## Cutting a release

1. In a normal pull request, set `version` in `pkl/PklProject` to the version (it already holds the next unreleased version, see the rule below), and raise `requiresRunner` in `pkl/Config.pkl` only if the package needs a newer runner than the last release. A pre-release ranks below its release, so never require a runner version a pre-release of the same version cannot meet.
2. Merge it, then run the release workflow by hand once (Actions, release, Run workflow, `dry_run` left on). It tests, builds, smoke tests on three platforms and assembles `checksums.txt`, and attests and publishes nothing. The assembled files are kept for 7 days as the `release-assets` artifact.
3. Push the tag: `git tag v<version> && git push origin v<version>` from the merged commit. Tags `v[0-9]*` start the real release. The tag ruleset forbids moving or deleting a pushed tag.

**After a release, the first pull request that changes anything under `pkl/` must bump `version` in `pkl/PklProject`.** `pkl project package` fetches the published package with the same version and refuses to build when the contents differ ("was already published with different contents ... bump the packages version"). The package build in CI and several runner tests fail until it is bumped. The version in `PklProject` is therefore always the next unreleased one, and the next tag is that version.

**A job that fails on a 5xx from GitHub is safe to re-run.** Downloads from GitHub release URLs (the mise download in `mise-action`, or the published-version check that `pkl project package` makes) can return 504 during an outage, which fails the job before any of our code runs. The release workflow publishes nothing until its last two jobs, and it refuses a version that already has a release, so re-running it is safe. This is decision 81 of the plan: no retry code.

A version is never re-published. If a release is bad, cut the next patch version and add the bad version to `known-bad.json` on `main`.

## What the workflow does

| Job | What it does | Permissions |
|---|---|---|
| prepare | The tag must be `v<version>` and match `pkl/PklProject`. A release for that tag must not exist yet. | read |
| build | Runner tests, the package build, the three binaries. | read |
| smoke | The 16 check smoke suite on linux-x64, linux-arm64 and darwin-arm64. | read |
| release | Cross-checks the package files, writes `checksums.txt`, attests every asset, creates the pre-release. Attestation and publication run on a tag push only. | contents, id-token and attestations: write |
| verify-release | From the outside, on all three platforms: installs the runner through mise with `mise lock` and `mise install --locked` (checksums and provenance enforced), runs `gh attestation verify` on the downloaded `checksums.txt`, limits the lock to the supported platforms (`lockfile_platforms`) and checks that it names only the three runner binaries, renders and checks through the real package URI with the published checksum, and runs `upgrade --dry-run` against the real release. Phase 5: it also renders the whole standard set (`Builtins.standardWorkflows` and `standardFiles`, imported from the same package URI) into an empty folder, runs `check`, and renders again expecting "written 0 file(s)" | read |

`checksums.txt` lists the three binaries, the package metadata file (its checksum is the one pkl checks with `::sha256`) and the zip. The attestation covers the binaries, the package files and `checksums.txt`.

`verify-release` is the first place the real download, `gh attestation verify` and mise's provenance check meet the code that `upgrade` and `mise install` rely on. It runs after publication, so a failure leaves a published pre-release: say so, cut the next patch version, and list the failed one in `known-bad.json`.

## Safety properties

They are asserted by `runner/test/release-workflow.test.ts` and kept honest by `tmp/phase-2b-wave-d/mutate.ts`:

- Only a `v[0-9]*` tag push or a manual run starts it, never `pull_request_target`.
- Every job asks for its own permissions, and only `release` can write.
- Every action is pinned by SHA, no secrets are used, checkouts keep no credentials.
- Tag names, refs and inputs reach scripts through `env`, never through `${{ }}` inside `run`.
- A manual run is a dry run unless told otherwise.

## Accepted risks and open items

- **No Security review.** The owner recorded P.3 as an accepted risk on 07-11-2026 (decision 78). Security has not reviewed this pipeline or the pkl flags, and no Security reviewer is named. This is a recorded risk, not an approval.
- **Cross-compiling downloads the bun runtime** for each target at build time, and the pipeline downloads tools through mise. Both are supply-chain inputs for any later Security review.
- **No second reviewer.** The repository has one member, so promotion with an independent reviewer (task 5.5) cannot be set up yet.
- **The tag ruleset has no creation rule**, so anyone with write access can push a `v*` tag. Today that is the maintainer only.

## Releases so far

| Version | Published | Status |
|---|---|---|
| 0.1.0 | 07-11-2026, pre-release | Published and **known bad** (listed in `known-bad.json`). Its runner could not read the version of a package URI that carries the pinned checksum, which every real consumer has, so it fails on every real consumer. The release pipeline's `verify-release` job found this on all three platforms. The assets, the attestations, `mise lock` and `mise install --locked` with provenance, and the real `gh attestation verify` all worked. A published version is never re-published, so the fix is 0.1.1. |
| 0.1.1 | 07-11-2026, pre-release | Published and verified from the outside on ubuntu-latest, ubuntu-24.04-arm and macos-latest (release run 42427253206): `mise lock` and `mise install --locked` with provenance, `gh attestation verify` of `checksums.txt`, render and check through the real package URI with the published checksum, and `upgrade --dry-run` against the real release. Finding: without `lockfile_platforms`, `mise lock` names the package zip for platforms with no runner (see [runner-install.md](runner-install.md)). |
| 0.1.2 | 08-11-2026, pre-release | The Phase 5 release candidate: it holds the standard workflow and file sets, the four toggles, `dist-check.yml` and `scaffold-check.yml`. A manual dry run first (run 42533622055), then the tag on `f26da89` (release run 42533855429). Published and verified from the outside on ubuntu-latest, ubuntu-24.04-arm and macos-latest, as for 0.1.1, and with the consumer smoke test: the standard workflows and files render through the real package URI (29 files written), `check` passes (28 owned files match), and a second render writes 0. `gh attestation verify` of `checksums.txt` passes. |
