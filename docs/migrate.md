# `migrate`: bringing an existing project under the scaffold

Written for the owner of a repository that already has its own workflows and files. `pantheon-scaffold migrate` compares what the standard set would render with what the repository has, and lists the difference. It is in release 0.1.8 and later (not in 0.1.7). `--write` is refused while a file has drifted, unless you add `--as-presets`, which keeps each drifted file as a Preset, or `--accept-builtins`, which replaces each one with the Builtin's file (both below). Turning a difference into a smaller Preset, such as a changed `timeout-minutes`, is task 5.4b and is not built yet.

```sh
pantheon-scaffold migrate --toolchain 1.96.1            # lists every file, writes nothing
pantheon-scaffold migrate --toolchain 1.96.1 --write    # writes pantheon.pkl and the new files
mise lock                                               # then commit
```

It takes the same options as [`init`](init.md) (`--toolchain` is required; set `--composite-action`, `--git-dependency-ignore` and `--features all` too when the repository does, or `dependabot.yml`, `audit-skill.yml`, `zizmor.yml` and `rust-ci.yml` will show as drifted for that reason) and needs `mise` and the `gh` CLI for the same reason: it pins a verified release.

## What the report says

One line per file, the class padded to ten columns, then the path. A drifted file is followed by its differences.

| Class | Meaning | What `--write` does |
|---|---|---|
| `identical` | The file is the same as the Builtin. A `.yml` or `.yaml` file is compared as YAML (the rule `check` uses), so quoting, comments and layout do not count. Any other file must match byte for byte. | Nothing, or, for a YAML file whose text differs, replaces it with the render's text. Its comments and layout change; the report marks these files with `equal as YAML, different text`. |
| `drifted` | The path is a Builtin's and the content differs. | Refuses. Nothing is written. With `--as-presets`, keeps the file as a Preset. With `--accept-builtins`, replaces it with the render. |
| `new` | The standard set has this file and the repository does not. | Creates it. |
| `seed-kept` | A seed file (such as `SECURITY.md`) that exists. The render never overwrites a seed. | Nothing. Not counted. |
| `external` | A file another tool writes (`release.yml` from cargo-dist). | Nothing. Not counted. |
| `per-repo` | A file the standard set does not render, under `.github/` or at the top of the repository. Files elsewhere (`src/`, `docs/`) are not listed. | Nothing. It is never read or changed. |

The last line gives the four counts: `migrate: 2 identical, 1 drifted, 38 new, 3 per-repo. Nothing was written.`

## Keeping drifted files: `--as-presets`

```sh
pantheon-scaffold migrate --toolchain 1.96.1 --as-presets           # lists the Presets it would write
pantheon-scaffold migrate --toolchain 1.96.1 --write --as-presets   # writes them, pantheon.pkl and the new files
```

Each drifted file becomes one Preset under `presets/`, named after the file (`presets/aislop.pkl` for `.github/workflows/aislop.yml`; a name already taken gets `-2`, `-3`). The Preset holds the whole file as it was and replaces the Builtin's file with it. Its header lists what differed when it was written. `pantheon.pkl` imports it, lists it under `presets { }` and applies it, as in [presets.md](presets.md), so `check` lists it, the manifest records its hash, `upgrade` moves its package import, and `detach` leaves it alone.

A Preset made this way is a **fork**: the file no longer follows the Builtin when the package changes. That is the price of keeping it as it was. To follow the Builtin again, delete the Preset, take it out of `pantheon.pkl` and render.

Before it writes anything, `migrate` evaluates the new config with the Presets and compares again. If any file still differs it stops with exit 1 and writes nothing. A file that is not UTF-8 text cannot be kept this way and stops the run. It never overwrites a file under `presets/`.

## Replacing drifted files: `--accept-builtins`

Use it when the drift is only age: the repository has an older `dependabot.yml`, an older pin or an older `mise.toml`, and you want the package's file.

```sh
pantheon-scaffold migrate --toolchain 1.96.1 --accept-builtins           # lists the drift, checks git, writes nothing
pantheon-scaffold migrate --toolchain 1.96.1 --accept-builtins --write   # replaces every drifted file, writes pantheon.pkl and the new files
git diff                                                                  # read what was replaced, then commit
```

It replaces **every** drifted file or none. There is no way to pick some and keep another. A mixed repository is not supported yet.

**Git is the record and the way back.** `--write` is refused (exit 3, nothing written) unless the folder is a git work tree, is not a sparse or partial checkout, has no uncommitted changes to tracked files, and tracks every drifted file. The lines `migrate` prints for a non-YAML file are only the first differences, so read the `git diff` of the adoption commit, which is the full record. If a write fails part-way, the files it already replaced stay replaced, and git restores them. After a successful write, `migrate` names the replaced files on its last lines. A dry run runs the same git check and says whether a write would be refused.

**Read the difference first.** `migrate` cannot tell a stale file from a change someone made on purpose. The dry run lists each drifted file with its differences, and nothing is replaced until you add `--write`.

**Set every option first.** A file that drifts because an option is missing (`--composite-action`, `--git-dependency-ignore` and `--features all`) is not stale. `--accept-builtins` would replace it with the wrong render. The dev-kit's `rust-ci.yml` is the known case: run `migrate --features all` first, and check that `rust-ci.yml` is `identical` before you accept the rest.

It is for first adoption. It does not change files a manifest already records, and it cannot be combined with `--as-presets` or used with `init`. The decisions behind it are in [migrate-accept-builtins-design.md](migrate-accept-builtins-design.md).

## Exit codes

| Code | When |
|---|---|
| 0 | A report was produced (even with drift), or `--write` finished. |
| 2 | `--toolchain` is missing or an option does not fit (for example `--binary` without `--crate-name`, or `--accept-builtins` with `--as-presets`). |
| 3 | `pantheon.pkl` already exists, or `--write` was asked while a file has drifted (and neither `--as-presets` nor `--accept-builtins`), or `--accept-builtins --write` found that git cannot hold what it would replace, or the render would be refused. Nothing is written. |
| 1 | An error: the release could not be verified, a file could not be read, or the Presets would not reproduce a file. |

## What it never does

- It never deletes a file.
- It never overwrites a file that has drifted, unless you add `--accept-builtins` and `--write` in a clean git tree (with `--as-presets` the file is kept by a Preset and is not changed). It overwrites an identical YAML file only to replace it with the same content in the render's text.
- A failure after `pantheon.pkl` is written takes it back, as `init` does.

## Checks

The scenarios are in `features/migrate.feature`, and they run against the package in this checkout. They were written first (task 5.2) and pass since task 5.3. The classification of the four `pantheon-*` repositories is task 5.5 and has not been run.
