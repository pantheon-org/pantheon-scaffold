# The tooling layer

The files the four Rust repositories share for local tooling, rendered by `Builtins.toolingFiles()` and `Builtins.baseFiles()`. Phase 3 of the plan. This page grows wave by wave: wave A (verbatim files) is here, `mise.toml` and the hook steps follow.

## Wave A: verbatim files

Every file is copied byte for byte from `pantheon-adr` at commit `1e2fd46`, and a test compares the git blob hash of what pkl renders with the pinned upstream blob (`runner/test/upstream/tooling-blobs.json`). All nine files were identical in the four repositories when they were read (baseline SP-0.1, re-checked on 08-11-2026).

| File | Mode | Comes from |
|---|---|---|
| `.github/scripts/agnix-assemble-comment.sh`, `agnix-file-issues.sh`, `agnix-introduced-errors.sh` | 0755 | `toolingFiles()` |
| `.github/scripts/auto-rebase.sh`, `check-commit-types.sh`, `release-automerge.sh`, `sync-labels.sh` | 0755 | `toolingFiles()` |
| `.github/scripts/cargo-deny.sh`, `check-inline-scripts.sh` | 0755 | `baseFiles()` (phase 2a) |
| `.rumdl.toml`, `.claude/settings.json` | 0644 | `toolingFiles()` |

Those are the nine shared scripts: seven from `toolingFiles()` and two from `baseFiles()`. `check-toolchain-version.sh` (also from `baseFiles()`) and `tag-release.sh` (which differs between repositories) are not part of the nine.

### One named difference: `sync-labels.sh` is executable

In all four repositories `sync-labels.sh` is committed as `100644`, and its workflow runs it as `bash .github/scripts/sync-labels.sh`, so the bit has never mattered. The plan says the nine scripts are executable, so it is rendered `0755` like the other eight. In a repository that adopts the package, `check` reports one mode difference for it until the bit is committed (`git update-index --chmod=+x`). `tooling.test.ts` asserts it is the only script that differs.

## The executable bit as git records it

The runner never reads the git index (it calls git only for the delete guard). The plan's "CI check on `git ls-files -s`" is met a different way: in CI, `check` runs on a checkout, where the mode on disk is the mode in the index, so a committed mode that is wrong is reported as mode drift (exit 4). `runner/test/mode-index.test.ts` shows this with real git: after a render and a commit `git ls-files -s` lists the nine scripts as `100755`, a fresh clone passes `check`, and a script committed as `100644` fails it with `mode`.

## Wave B: `mise.toml`

`Builtins.miseFiles(options)` renders three files.

| File | Class | What it is |
|---|---|---|
| `mise.toml` | owned | The shared tools, tasks, hooks and settings, from `pkl/assets/mise.toml.tmpl` with the tool versions from `Tools.pkl`. |
| `mise.lock` | external | Written by mise, listed in the manifest and never written, hashed or deleted by the runner. |
| `.mise/conf.d/repo.toml` | seed | Written once, empty `[tools]`, for the tools one repository needs on top. |

The per-repo file is read by mise together with `mise.toml`. For a tool listed in both, `mise.toml` wins, so a repository can add tools but cannot override a shared pin. `mise.test.ts` checks this with the real mise, using an empty data directory so the result does not depend on what is installed.

### Pins (decision q14, option A)

The specifiers are the ones the four repositories use today. `actionlint`, `zizmor`, `rumdl` and `agnix` are exact and come from `pkl/Tools.pkl` (one place, so a workflow in phase 4 can read the same value). `hk`, `pkl`, `yq`, `jq` and `shellcheck` stay `latest`, and the committed `mise.lock` pins what they resolve to. `rust` comes from `options.toolchain`.

The runner is pinned at the version of the package that rendered the file, because the two are released together. The version is read from the package URI (with or without the pinned `::sha256:` part). A package loaded from a folder has no version, so set `options.runnerVersion` there, otherwise rendering fails with a message that says so.

### Differences from the file the four repositories share today

The rendered file equals the current one except for these lines (asserted exactly by `mise.test.ts`):

- Two comments no longer say "keep in sync with the workflow": the version is pinned in the package now (`zizmor` and `agnix`).
- `"github:pantheon-org/pantheon-scaffold" = "<package version>"`, with its comment.
- In `[settings]`: `lockfile = true`, `locked_verify_provenance = true` and `lockfile_platforms = ["linux-x64", "linux-arm64", "macos-arm64"]` (see [runner-install.md](runner-install.md) for why the platform list is required).

### Adopting it

None of the four repositories has a `mise.lock` or a `.mise/` folder today, so adopting this file turns the lock on for them. After the first render, run `mise lock` once and commit `mise.lock`, and CI installs with `mise install --locked`. That is applied in phase 7, not here.

## Wave C: the hook steps

`Builtins.hkFiles(options)` renders two files (decision 31).

| File | Class | What it is |
|---|---|---|
| `hk.shared.pkl` | owned | The steps every repository shares, as two functions, `preCommit()` and `prePush()`. Rendered by the runner. Do not edit it. |
| `hk.pkl` | seed | Written once. Amends the hk config package, imports `hk.shared.pkl` and holds only the hooks. Yours to edit. |

The steps are copied by script from the `hk.pkl` of `pantheon-adr` at `1e2fd46`, and tests compare them byte for byte with it. A spike (`tmp/phase-3-hk-spike/spike.sh`) showed first that hk 2.10.0 accepts a seeded `hk.pkl` that imports a rendered local file and still merges the repository's own steps.

### Adding a step of your own

Edit a hook in `hk.pkl`. A module can define `hooks` only once, so a second `hooks { ... }` block fails to load:

```pkl
["check"] {
    steps { ...shared.preCommit(); ["my-step"] { glob = List("*.txt"); check = "my-check {{files}}" } }
}
```

### The `features` option

`options.features = "all"` renders `--all-features` in the pre-push clippy and test steps, which is the only difference between the library repository's `hk.pkl` and the other three (apart from one comment). The choice lives in `pantheon.pkl` and the owned file, not in the seeded `hk.pkl`, so changing it updates the shared file. A test checks that the `all` render is byte for byte the library repository's pre-push steps. The pre-commit steps differ from the library repository's by one comment about `.github/actionlint.yaml`, which the other three repositories have and the library repository does not.

### Fixers and generated files

No fixer matches a generated file today. The fixers are `rustfmt` (`*.rs`) and `rumdl` (`*.md`), and the render holds no Rust or Markdown. `yamllint`, `shellcheck` and `actionlint` only check. A test runs `hk fix --all` and `hk check --all` over a fresh render and asserts that both pass and that nothing changed. It needs `shellcheck` on the PATH.

### Migrating an existing `hk.pkl` (one-off, in phase 7)

Each repository already has an `hk.pkl` with the steps inline, so the seed is not written over it. For each repository:

1. Render, then diff the existing `hk.pkl` against the seeded one. The `hooks` block should be identical except that `steps = preCommit` and `steps = prePush` became `steps = shared.preCommit()` and `steps = shared.prePush()`.
2. Keep any step of your own: move it into the hook that needs it, as shown above.
3. Replace the repository's `hk.pkl` with the seeded one plus those local steps, delete the inline `local preCommit` and `local prePush`, and run `hk check --all`.
