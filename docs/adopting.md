# Adopting pantheon-scaffold in a repository

How to put one Rust repository under the scaffold, keep it there, and take it out again. Written for the owner of a repository. Everything below was run in the Phase 7 proof, against scratch clones of the real repositories, with the published release ([proof-report-0.1.3.md](../archive/proof/docs/proof-report-0.1.3.md)).

**Status.** No repository has adopted the package yet. Adopting stays the decision of each repository's owner. Check the release you pick: some early releases are on the known-bad list (see "Choosing a version").

## What you get

A `pantheon.pkl` in your repository says which options you use. The runner renders a set of shared files from the package into your repository and keeps them in step.

| Kind | Meaning | Examples |
|---|---|---|
| **Owned** | The runner writes it, and refuses to overwrite a hand edit | the standard workflows, `mise.toml`, `.rumdl.toml`, the nine shared scripts, `hk.shared.pkl` |
| **Seeded** | Written once if it does not exist, then yours | `hk.pkl`, `SECURITY.md`, `.mise/conf.d/repo.toml` |
| **External** | Listed so it is known, never written by the runner | `mise.lock`, and `release.yml` written by `cargo-dist` |

What the package does **not** touch: your source, your `Cargo.toml`, and anything it does not list. It never deletes a file it did not render, and it deletes an orphan only from a clean tracked tree.

## Before you start

- A git repository with a **clean tracked tree**, on a branch of its own (see "Adopt on a clean branch").
- `mise` and the `gh` CLI, signed in to GitHub. The runner uses `gh` to check attestations.
- A runner exists for `linux-x64`, `linux-arm64` and `macos-arm64` only. There is none for Intel Macs, Windows or musl Linux.

## Choosing a version

Pick the newest release that is not on the known-bad list:

```sh
gh release list --repo pantheon-org/pantheon-scaffold
curl -s https://raw.githubusercontent.com/pantheon-org/pantheon-scaffold/main/known-bad.json
```

Below, `VERSION` stands for the version you picked, for example `0.1.5`. Versions are never re-published and never deleted: a bad release is superseded by the next patch version and listed in `known-bad.json`, and the runner warns when your package is on that list.

## First-time setup

### 1. Install the runner with mise

Add this to your `mise.toml` (create it if you have none). The first render replaces `mise.toml` with the package's, which already carries these lines. Your own tools move to `.mise/conf.d/repo.toml`, which mise reads together with `mise.toml`.

```toml
[settings]
lockfile = true
locked_verify_provenance = true
lockfile_platforms = ["linux-x64", "linux-arm64", "macos-arm64"]

[tools]
pkl = "0.32.2"
"github:pantheon-org/pantheon-scaffold" = "VERSION"
```

Always set `lockfile_platforms`. Without it mise locks every platform it knows, and for a platform with no runner it records some other file of the release as if it were one.

```sh
mise lock
mise install --locked
mise x -- pantheon-scaffold --version     # prints VERSION
```

`mise install --locked` fails on a checksum that does not match or an attestation that does not verify. It also installs hk's git hooks into the repository (through `git config`), so expect the hook commands to appear.

### 2. Get the package checksum, from a release you have verified

```sh
gh release download vVERSION --repo pantheon-org/pantheon-scaffold -p checksums.txt
gh attestation verify checksums.txt --repo pantheon-org/pantheon-scaffold
grep ' pantheon-scaffold@VERSION$' checksums.txt      # the checksum to use below
```

### 3. Write `pantheon.pkl`

Replace `SUM` with the checksum from the last command. This example is the one used for the Rust tools with a binary and a skill; the options are explained in the next section.

```pkl
amends "package://github.com/pantheon-org/pantheon-scaffold/releases/download/vVERSION/pantheon-scaffold@VERSION::sha256:SUM#/Config.pkl"
import "package://github.com/pantheon-org/pantheon-scaffold/releases/download/vVERSION/pantheon-scaffold@VERSION::sha256:SUM#/Builtins.pkl"

options {
  toolchain = "1.96.1"
  crateName = "my-crate"
  skillName = "my-skill"
}

workflows { ...Builtins.standardWorkflows(options) }
files {
  ...Builtins.standardFiles(options)
  ...Builtins.baseFiles(options)
  ...Builtins.toolingFiles()
  ...Builtins.miseFiles(options)
  ...Builtins.hkFiles(options)
}
```

`upgrade` moves the version and the checksum in both places, so you edit them by hand only this once.

### 4. Look before you write, then render

```sh
mise x -- pantheon-scaffold render pantheon.pkl --dry-run
```

The first render refuses every file that already exists and differs from the package's (exit 3), and lists them. That is the safe default. Look at each. When you accept the package's version of them, render with `--force`:

```sh
mise x -- pantheon-scaffold render pantheon.pkl --force
mise x -- pantheon-scaffold check pantheon.pkl      # exit 0: every owned file matches
mise x -- pantheon-scaffold render pantheon.pkl     # writes 0 files
```

Commit the result, including `.pkl-gen/manifest.json` (what the runner knows it wrote) and `mise.lock`. Do **not** commit `.pkl-gen/cache/`: the rendered `.gitignore` already ignores it.

## The options

| Option | Default | What it changes |
|---|---|---|
| `skills` | `true`, needs `skillName` | adds `audit-skill.yml`, and the skill lines of `.coderabbit.yaml`, `SECURITY.md` and the `agnix` files |
| `binary` | `true`, needs `crateName` | adds `dist-workspace.toml`, `dist-check.yml`, `tag-release.sh`, the dist lint config, and the artifact-action ignore in `dependabot.yml` |
| `features` | `"default"` | `"all"` builds and tests with `--all-features` |
| `compositeAction` | `false` | the repository's own `action.yml` is audited and scanned |

A toggle that needs a name and has none stops the render with an error that names the option. Details and what each toggle removes: [feature-toggles.md](feature-toggles.md).

## What the render changes in a real repository

Expect these, all of which were checked in the proof against the four repositories:

- **Workflows are rewritten from data.** The triggers, permissions, steps and every `run:` script are equal. Layout changes, the key `on` is quoted, and explanatory comments inside the file are dropped. A warning comment you rely on (for example "do not rename this job, the ruleset matches it") is lost, so keep that note somewhere else.
- **`mise.toml`** changes by about ten lines added and four removed: the lock settings, the runner pin, and the tool comments.
- **`hk.pkl`** is the one file with a one-off migration (below).
- **`.gitignore`**, **`dist-workspace.toml`** and **`dependabot.yml`** change in the ways the plan decided (decisions 74, 85, 86 and 87).
- **`sync-labels.sh`** becomes executable (a mode change only).
- **New files:** `hk.shared.pkl`, `.mise/conf.d/repo.toml`, `scaffold-check.yml` and, with `binary`, `dist-check.yml`.

## One-off: `hk.pkl`

Your repository has an `hk.pkl` with the pre-commit and pre-push steps written out. The package's seed imports the same steps from `hk.shared.pkl`, which the runner owns. A seed is never written over an existing file, so you migrate it once:

1. Render, then compare your `hk.pkl` with the seeded one the package would write. The `hooks` block is the same except that the inline steps become `shared.preCommit()` and `shared.prePush()`.
2. Move any step of your own into the hook that needs it. A module can define `hooks` only once.
3. Replace your `hk.pkl` with the seeded one plus those steps, delete the inline `local preCommit` and `local prePush`, and run `hk check --all`.

Details: [tooling-layer.md](tooling-layer.md).

## One-off: Markdown findings

`rumdl` reports Markdown findings in all four existing repositories today, with or without the package. Fix or exempt yours before you enforce `hk check --all` in CI. The render does not change them.

## Adopt on a clean branch

Work on a branch cut from an up-to-date main, with nothing uncommitted. The runner refuses to delete any file unless the tracked tree is clean, is not a sparse or partial checkout, and the file is tracked by git. After `upgrade`, commit the changed pins before you render again if the new package drops a file.

## Keeping a repository in step

| Command | What it does | Exit |
|---|---|---|
| `check pantheon.pkl` | renders in memory, compares with the files on disk, writes nothing | 0 match, 4 drift |
| `render pantheon.pkl` | writes what changed, refuses a hand edit | 3 refused |
| `upgrade VERSION` | moves the pins, installs and locks the new runner, renders | see below |
| `detach` | ends management, keeps every file | 0 |

`scaffold-check.yml` runs `check` on every pull request, so drift shows up in CI. A repository that never runs it, or edits the workflow, is not checked.

## Upgrading

```sh
mise x -- pantheon-scaffold upgrade NEWVERSION --dry-run   # downloads and verifies, changes nothing
mise x -- pantheon-scaffold upgrade NEWVERSION
```

It verifies the new release's attestation first, then rewrites the `amends` line, **every import of the package**, and the runner pin in one step, runs `mise install` and `mise lock`, and renders with the new runner. If `mise` fails, everything is put back. If the final render is refused, the pins stay updated: fix what it reports and run `render` again.

Releases before 0.1.5 had two `upgrade` bugs, and they are on the known-bad list. If you are on 0.1.3 or earlier, the old runner moves only the `amends` line: edit the `Builtins.pkl` import to the new version by hand before the first `upgrade`. If you are on 0.1.2, 0.1.3 or 0.1.4, the render at the end of `upgrade` is refused for `mise.toml` (exit 3), because the old runner did not record its pin edit: read the refusal, check that the only difference is the runner pin, and run `render --force`.

## Rolling back

```sh
mise x -- pantheon-scaffold upgrade OLDVERSION --force   # moves the pins; its render is refused, exit 3
mise x -- pantheon-scaffold render pantheon.pkl --force  # renders the older package
mise x -- pantheon-scaffold check pantheon.pkl
```

Rolling back is two steps on purpose: the runner refuses to render an older package than the one that last rendered, and `upgrade` does not pass `--force` on, because that would also overwrite hand edits. A release that turns out to be bad is superseded by the next patch version and listed in `known-bad.json`. It is never deleted or re-published.

## Leaving: `detach`

```sh
mise x -- pantheon-scaffold detach --dry-run
mise x -- pantheon-scaffold detach
git add -A && git commit -m "chore: detach from pantheon-scaffold"
```

It deletes the manifest and the runner's folders under `.pkl-gen/`, deletes the owned files the package marked for removal on detach (the consumer upgrade workflow), and keeps every other file as a plain file. A later render adopts files that still match. To undo a detach, `git revert` the commit: the manifest comes back, `check` passes and a render writes nothing.

## Tokens and secrets

Most rendered workflows use no secret: they use the built-in `GITHUB_TOKEN` with the permissions they declare. Three need more. `release-please.yml`, `pr-auto-rebase.yml` and `release-automerge.yml` mint a short-lived token from a GitHub App, so that the tag, the pull requests and the merges they make start other workflows (a tag pushed with `GITHUB_TOKEN` would not start `release.yml`).

| For these three workflows | You need |
|---|---|
| A GitHub App installed on the repository, with contents and pull-requests write | the App's private key, available to the repository as the secret `GH_APP_PRIVATE_KEY` |

**The App is the organisation's.** The three workflows carry the client id of the `pantheon-org` App in the file. A repository outside that organisation cannot use them unchanged: the client id is not an option of the package today. Without the key, each of these jobs stops at the token step before it does anything.

`release.yml` itself is written by `cargo-dist`, not by the package, and has its own permissions. `dist-check.yml` only checks that it is current.

## What the package does not do

- It does not write `release.yml` (it checks it, with `binary`).
- It does not manage your source, your `Cargo.toml` or your branch rules.
- It does not give you a second reviewer. Security approval of the release workflow is not obtained, and the Phase 7 sign-off is not independent: [proof-report-0.1.3.md](../archive/proof/docs/proof-report-0.1.3.md), [security-dossier-0.1.5.md](../archive/proof/docs/security-dossier-0.1.5.md).
- It has no runner for Intel Macs, Windows or musl Linux.

## Every claim, and where it is shown

A claim in this guide, and the test or the proof line that shows it. A path is a file in this repository. "Report" is [proof-report-0.1.3.md](../archive/proof/docs/proof-report-0.1.3.md), with the wave named.

| Claim | Shown by |
|---|---|
| mise installs the runner with checksums and provenance enforced, and a mismatch fails | `release.yml` job "Verify the published release" (three platforms); Report, wave B (real releases installed with checksums and provenance enforced) |
| Without `lockfile_platforms` the lock names a wrong file for a platform with no runner, and `upgrade` refuses it | `runner/test/lockcheck.test.ts`, `runner/test/upgrade.test.ts` (lock cases), [runner-install.md](runner-install.md) |
| The package is loaded through its URI and checksum | `runner/test/package-uri.test.ts`; Report, wave A |
| The first render refuses existing files that differ, and `--force` takes the package's | `runner/test/apply.test.ts`; Report, wave A ("the first render without `--force`") |
| Owned, seeded and external files behave as the table says | `runner/test/apply.test.ts`, [runner-safety.md](runner-safety.md) |
| A repeat render writes 0 files, and `check` passes after a render | `runner/test/check.test.ts`; Report, wave D (benefit test: "check", "second render") |
| `check` writes nothing and exits 4 on drift; it runs on pull requests in CI | `runner/test/check.test.ts`, `runner/test/consumer-check.test.ts`, [check-in-ci.md](check-in-ci.md) |
| Workflows keep their triggers, permissions and `run:` scripts when re-rendered | `runner/test/equality.test.ts`, `runner/test/workflows-a.test.ts`, `workflows-b.test.ts`, `workflows-c.test.ts`; Report, wave A (zero uncategorised differences) |
| Each option adds and removes exactly the files listed | `runner/test/phase5-toggles.test.ts`, [feature-toggles.md](feature-toggles.md); Report, wave A (the four repositories cover the variants) |
| A toggle that needs a name stops with an error naming the option | `runner/test/phase5-toggles.test.ts` |
| The render changes a real repository only in the ways listed | Report, wave A (every difference categorised, none uncategorised) and the sample check ([sample-check-0.1.5.md](../archive/proof/docs/sample-check-0.1.5.md), 19 of 19 confirmed) |
| The first-time setup works as written: install, checksum, `pantheon.pkl`, dry run, render, `check`, a second render that writes 0 files, and the rendered tree builds and passes its tests | Report, Phase 8 wave E (every step of this guide run on a scratch clone of `pantheon-adr`: 188 tests passed, 0 failed), `archive/proof/scripts/proof-adoption.ts` |
| `mise.toml` changes by about ten lines added and four removed | `archive/proof/scripts/proof-categories.ts` (the rule), Report, wave A |
| The `hk.pkl` migration keeps the hook results | `runner/test/hk.test.ts`, `runner/test/tooling.test.ts`; Report, wave A (`hk check --all` the same before and after) |
| `rumdl` findings exist before and after, unchanged by the render | Report, wave A (findings per repository) |
| The runner deletes only from a clean tracked tree, and only tracked files | `runner/test/guard.test.ts`, [runner-safety.md](runner-safety.md) |
| `upgrade` verifies the attestation, moves the amends line, every import and the pin, and puts everything back if mise fails | `runner/test/upgrade.test.ts`, `runner/test/cli-upgrade.test.ts`; Report, wave B and wave D |
| `upgrade` of a repository that renders `mise.toml` succeeds without `--force` | `runner/test/upgrade.test.ts` ("recorded in the manifest"); Report, wave D (0.1.3 to 0.1.5, exit 0, both clones) |
| Rolling back is two steps and ends byte for byte at the old render | Report, wave B ("pin-back"), [runner-upgrade.md](runner-upgrade.md) |
| `detach` keeps every file, and a `git revert` restores the management | `runner/test/detach.test.ts`; Report, wave B ("detach, then a git revert") |
| A known-bad version produces a warning, and a refusal with `--strict` | `runner/test/knownbad.test.ts`, `runner/test/preflight.test.ts`, [runner-lifecycle.md](runner-lifecycle.md) |
| Releases 0.1.0 to 0.1.4 are on the known-bad list | `known-bad.json`, `runner/test/knownbad.test.ts` (the bundled list is a subset of it) |
| Three workflows need the App key secret; the others use only `GITHUB_TOKEN` | [security-dossier-0.1.5.md](../archive/proof/docs/security-dossier-0.1.5.md) (the workflow table); Report, wave C (the three fail closed with a bad key) |
| `release.yml` is not rendered; `dist-check.yml` checks it | `runner/test/phase5-dist.test.ts`, [feature-toggles.md](feature-toggles.md) (wave D) |
| A release is built, attested and verified on three platforms before it is published | `runner/test/release-workflow.test.ts`, [release-process.md](release-process.md) |
| A pre-release is promoted only after its guards pass, and the guards are tested | `runner/test/promote.test.ts`, `runner/test/promote-workflow.test.ts`, the dry run of the promote workflow on v0.1.5 (Actions run 42625442636) |
