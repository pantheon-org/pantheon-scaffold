# `migrate`: bringing an existing project under the scaffold

Written for the owner of a repository that already has its own workflows and files. `pantheon-scaffold migrate` compares what the standard set would render with what the repository has, and lists the difference. It is in the runner after 0.1.7 and is not in a release yet. Turning a difference into a Preset (task 5.4) is not built yet, so `--write` only runs when nothing has drifted.

```sh
pantheon-scaffold migrate --toolchain 1.96.1            # lists every file, writes nothing
pantheon-scaffold migrate --toolchain 1.96.1 --write    # writes pantheon.pkl and the new files
mise lock                                               # then commit
```

It takes the same options as [`init`](init.md) (`--toolchain` is required) and needs `mise` and the `gh` CLI for the same reason: it pins a verified release.

## What the report says

One line per file, the class padded to ten columns, then the path. A drifted file is followed by its differences.

| Class | Meaning | What `--write` does |
|---|---|---|
| `identical` | The file is the same as the Builtin. A `.yml` or `.yaml` file is compared as YAML (the rule `check` uses), so quoting, comments and layout do not count. Any other file must match byte for byte. | Nothing, or, for a YAML file whose text differs, replaces it with the render's text. Its comments and layout change; the report marks these files with `equal as YAML, different text`. |
| `drifted` | The path is a Builtin's and the content differs. | Refuses. Nothing is written. |
| `new` | The standard set has this file and the repository does not. | Creates it. |
| `seed-kept` | A seed file (such as `SECURITY.md`) that exists. The render never overwrites a seed. | Nothing. Not counted. |
| `external` | A file another tool writes (`release.yml` from cargo-dist). | Nothing. Not counted. |
| `per-repo` | A file the standard set does not render, under `.github/` or at the top of the repository. Files elsewhere (`src/`, `docs/`) are not listed. | Nothing. It is never read or changed. |

The last line gives the four counts: `migrate: 2 identical, 1 drifted, 38 new, 3 per-repo. Nothing was written.`

## Exit codes

| Code | When |
|---|---|
| 0 | A report was produced (even with drift), or `--write` finished. |
| 2 | `--toolchain` is missing or an option does not fit (for example `--binary` without `--crate-name`). |
| 3 | `pantheon.pkl` already exists, or `--write` was asked while a file has drifted, or the render would be refused. Nothing is written. |
| 1 | An error: the release could not be verified, a file could not be read. |

## What it never does

- It never deletes a file.
- It never overwrites a file that has drifted, and it overwrites an identical YAML file only to replace it with the same content in the render's text.
- A failure after `pantheon.pkl` is written takes it back, as `init` does.

## Checks

The scenarios are in `features/migrate.feature`, and they run against the package in this checkout. They were written first (task 5.2) and pass since task 5.3. The classification of the four `pantheon-*` repositories is task 5.5 and has not been run.
