# `init`: putting a new project under the scaffold

Written for the owner of a new Rust repository. `pantheon-scaffold init` writes `pantheon.pkl` once and renders the standard set into the repository. For a project that already has its own workflows and files, use `migrate` (task 5.2 to 5.4 of the plan, not built yet); `init` refuses to overwrite anything.

```sh
pantheon-scaffold init --toolchain 1.96.1            # shows what it would do, writes nothing
pantheon-scaffold init --toolchain 1.96.1 --write    # writes pantheon.pkl and the files
mise lock                                            # records the checksums of the tools, then commit
```

`init` needs `mise` and the `gh` CLI, signed in to GitHub, like `upgrade`: it downloads the release's `checksums.txt` and checks its attestation before it pins anything.

## Options

| Option | Meaning |
|---|---|
| `--toolchain <x.y.z>` | **Required.** The Rust toolchain every file that names one uses. |
| `--package-version <x.y.z>` | The release to use. Without it, the newest stable release that is not a draft and not on the known-bad list (what `latest` prints). A known-bad release is refused even when named. |
| `--binary --crate-name <name>` | The repository releases a binary with cargo-dist. Both are needed together. |
| `--skills --skill-name <name>` | The repository carries a skill. Both are needed together. |
| `--write` | Write. Without it `init` is a dry run. |
| `--root <dir>` | The repository, default the current directory. |

Both toggles are written out in `pantheon.pkl` (`skills = false`, `binary = false` unless you turn them on), because the package turns them on by default.

## What it does

1. Picks the release and verifies it, as above. Any failure stops here with exit 1 and nothing is written.
2. Builds the text of `pantheon.pkl` and evaluates it against the package, in a scratch folder, so the package itself says what it would render.
3. Dry run: prints the config and the list of files the render would write, then `dry run: init would write pantheon.pkl and 41 file(s). Run again with --write to write them.`
4. With `--write`: writes `pantheon.pkl` (never over an existing one), then renders. A failure after that removes `pantheon.pkl` again.

A dry run may leave pkl's download cache in `.pkl-gen/cache/`, as `render --dry-run` does. It is ignored by the `.gitignore` the render writes.

## What `pantheon.pkl` holds

```pkl
amends "package://.../pantheon-scaffold@0.1.6::sha256:<checksum>#/Config.pkl"

import "package://.../pantheon-scaffold@0.1.6::sha256:<checksum>#/Builtins.pkl"

options { toolchain = "1.96.1"; skills = false; binary = false }

workflows { ...Builtins.standardWorkflows(options) }

files {
  ...Builtins.standardFiles(options)
  ...Builtins.baseFiles(options)
  ...Builtins.toolingFiles()
  ...Builtins.miseFiles(options)
  ...Builtins.hkFiles(options)
}
```

That is the whole standard set. From here on only `upgrade` edits the amends line and the imports of the package, and everything else is yours: to change what a Builtin renders, use a Preset ([presets.md](presets.md)).

## When it refuses (exit 3)

| Message | Why |
|---|---|
| `pantheon.pkl already exists in <dir>: init writes it once. Use upgrade to move it to another release.` | Nothing is overwritten. |
| `refused N file(s), nothing was written:` followed by the files | The render would overwrite a file that is there and differs, such as a `.gitignore` or `deny.toml` you wrote. Nothing is written, not even `pantheon.pkl`. These are the files `migrate` is for. |
| the known-bad message of `render` | The release you named is on the known-bad list. |

A usage mistake (no `--toolchain`, `--binary` without `--crate-name`, and so on) exits 2 with the usage text.

## For working on the package itself

`--allow-local-package <dir>` makes `init` amend that folder instead of a release, and skips the release and the attestation. It writes the folder's absolute path into `pantheon.pkl` and pins the runner version of this binary, so it is for testing the package, never for a repository you will commit.
