# git-cliff as an alternative to release-please

Issue #158. Investigation only: nothing under `pkl/Builtins/release-please/` changes. Checked on 25-11-2026 against git-cliff 2.14.2 (18-09-2026), `orhun/git-cliff-action` v4.9.1 and the git-cliff docs in its repository (`website/docs/`), and by running git-cliff 2.14.2 on the history of this repository.

## Recommendation: no-go for now

Do not add git-cliff as a release tool yet. git-cliff is a good changelog generator and it can reproduce our changelog and our pre-1.0 bump rules. But it does only two of release-please's five jobs. The other three (the release pull request, the tag and the draft release) and the version-file edits would become new scripts that this package writes, tests and keeps up. That is most of what release-please does for us. Three other builtins (`release-automerge`, `release-bump`, `pr-auto-rebase`) are built around release-please's branch name and its pull request. No repository has adopted the scaffold yet and none has asked for another changelog layout, so the cost has nothing to pay it back (YAGNI).

Look at it again when a repository needs something release-please cannot give: a changelog layout its options cannot produce, or no manifest file. If that happens, the shape under "If it is revisited" below is the smallest one that fits, and the answers below still hold.

## Which jobs git-cliff covers

| release-please job | git-cliff | Needed besides |
|---|---|---|
| Changelog from conventional commits | Yes: `--unreleased --bump --prepend CHANGELOG.md`, or `--latest --strip all` for release notes | nothing |
| Next version | Yes: `--bumped-version`, with the `[bump]` rules | a check against the latest tag (see below) |
| Version in files (`Cargo.toml`, `Cargo.lock`, `versionFile`) | No: git-cliff edits no file but the changelog | `cargo set-version` and `cargo update -p`, or a `sed` of the version line |
| Rolling release pull request | No | a script that rebuilds a branch from `main`, commits and opens or updates the pull request under the App token |
| Tag and draft release on merge | No | a step that sees the release commit on `main`, tags it under the App token and makes the draft |

Two details matter for the scripts:

- With no releasable commits, `git cliff --bumped-version` prints the current version, warns "there is nothing to bump", and exits 0. A script must compare the output with the latest tag; the exit code says nothing.
- `--bump` dates the new section with today's date, so the release pull request has to be rebuilt from `main` on each run (as release-please does), not edited in place.

## The release shapes and `publish-release.sh`

- **`draft`** fits best. `publish-release.sh` already creates the pre-release itself when no draft exists for the tag, so a git-cliff flow only needs to push the tag and pass `git cliff --latest --strip all` as `RELEASE_NOTES`. The draft step could be dropped. The `release` workflow contract in [adopting.md](adopting.md#releasing-with-release-please) stays the same.
- **`published`**: the tag step also runs `gh release create` with the notes. One short step.
- **`cargo-dist`**: cargo-dist makes the release, and `tag-release.sh` tags the crate version. git-cliff then only writes the changelog, but the release pull request must bump `crates/<crate>/Cargo.toml` and `Cargo.lock` itself. For Rust crates, [release-plz](https://github.com/release-plz/release-plz) already does this: it uses git-cliff for the changelog and opens the release pull request itself. It tags `<package>-v<version>` by default and publishes to crates.io, so it needs its own evaluation.

## Option or separate builtin

If it is ever built, it should be an option, `releaseTool: ("release-please"|"git-cliff") = "release-please"`, picked the same way as `reviewBot`. It should not be a second builtin the user must remember to leave out with `without`. The two tools must not both run in one repository: both would write `CHANGELOG.md` and tag `v*` on each push to `main`. With an option, `Presence.pkl` keeps one release workflow, and `releaseShape`, `versionFile` and `seedReleaseConfig` keep their meaning. A seeded `release-please-config.json` left behind after a switch does nothing, but `migrate` should report it.

## `cliff.toml` as a rendered file

`cliff.toml` depends only on the options (the repository URL, the shape, and the commit types), so it renders the same each time and `check` stays stable. Two points:

- It must be a `content` file, not a `template` file. The runner renders templates with minijinja, and git-cliff's body is a Tera template with the same `{{ }}` and `{% %}` syntax. A minijinja pass would consume it.
- It should be `owned` (rendered and drift-checked), not a seed, so that its sections cannot drift from `.github/commit-types.txt`. A repository that wants its own layout overrides it with a Preset, as for any owned file.

The output of git-cliff is deterministic for released versions: two runs on this repository gave identical output. Dates come from tag timestamps. Only the unreleased section uses the current date. Leave out `[remote.github]`. It adds PR authors through the GitHub API, which needs a token and network access. Our PR numbers are already in the squash-merge titles.

## Staying in step with `.github/commit-types.txt`

release-please uses its built-in default sections: `feat`, `fix`, `perf` and `revert` are shown, and `chore`, `docs`, `style`, `refactor`, `test`, `build` and `ci` are hidden (`DEFAULT_CHANGELOG_SECTIONS` in release-please's `src/util/filter-commits.ts`). In git-cliff the same rules are an explicit `commit_parsers` list: one group for each shown type, `skip = true` for each hidden one, and `filter_commits = true` to drop anything else. A skipped commit does not bump the version either. The pkl could build that list from one mapping of the eleven types. `check-commit-types.sh` would then also compare the parsers with the file. This works better than release-please, where the sections are implicit.

Checked on this repository's history with that config and `[bump] features_always_bump_minor = false`, `breaking_always_bump_major = false`:

- `--latest` reproduced the 0.4.2 entry of `CHANGELOG.md`: the same sections, entries, compare link and date. Only the blank lines and the PR-number link differ, and the template can match those.
- `--bumped-version` on `main` gave `v0.4.3`, which is what release-please would give. A `feat!:` commit gave `v0.5.0`, a `perf:` commit gave `v0.4.3`, and `refactor:` and `chore:` commits gave no bump. These are the same rules as `bump-minor-pre-major` plus `bump-patch-for-minor-pre-major` in the draft config. With `bump-patch-for-minor-pre-major = false` (the cargo-dist and published seeds), set `features_always_bump_minor = true`.

## Pinning and supply chain

- `orhun/git-cliff-action` v4.9.1 is commit `a9a95522b26fe6403f7bb24031f21fb573d0f5ff`. It could be a `Pins.pkl` pin, and `update-pins` would move it. But it is a composite action that downloads the git-cliff release named by its `version` input (default `v2.14.2`, or `latest`) with `curl` and does not check the `.sha512` file published next to each archive. A pinned action commit therefore does not pin a checked binary.
- The better route is mise: its registry maps `git-cliff` to `aqua:orhun/git-cliff`, and the aqua registry checks the release's `.sha512` checksum. The version goes in `pkl/Tools.pkl` and the shared `mise.toml`, `mise.lock` pins it, and `minimumReleaseAge` applies, like the other tools. The workflow then installs it with the pinned `jdx/mise-action`. No new action is pinned.
- The archives are also PGP-signed (`.sig`). They have no GitHub build attestation, so the agnix-style "digest from an attested archive" check is not available.

## Migrating a repository, and its `CHANGELOG.md`

The history carries over if the first git-cliff run uses `--prepend CHANGELOG.md` and never `--output`. This was checked on a copy of this repository's `CHANGELOG.md`: the new 0.4.3 section went under the existing `# Changelog` header, the header was not repeated, and every older entry stayed byte for byte. `--output` would regenerate every past release in git-cliff's format and rewrite the history.

A migration would be one pull request. It sets the option, renders (which adds `cliff.toml` and the new workflow, and removes `release-please.yml`), and deletes `release-please-config.json` and `.release-please-manifest.json` by hand, because they are seeds. git-cliff reads the latest version from the `v*` tags, so the manifest needs no hand-over. An open release-please pull request must be closed first.

## If it is revisited

The smallest version that fits is git-cliff for the `draft` and `published` shapes only. It has three parts:

1. A release pull request script based on `release-bump.sh`, which already opens and merges a pull request under the App token. It compares `--bumped-version` with the latest tag and stops when they are equal. Otherwise it rebuilds a fixed branch from `main`, runs `--prepend`, sets `versionFile`, and opens or updates the pull request.
2. A tag step that runs on a push to `main` whose commit is that pull request.
3. Changes to `release-automerge.sh` and `auto-rebase.sh` for the new branch name.

`cargo-dist` would stay on release-please or go to release-plz.
