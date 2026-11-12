# Presets: changing what the package renders for your repository

Written for the owner of a repository that uses the scaffold. A **Preset** is a small pkl file in your repository that takes something the package renders (a workflow, a job, a step, a file) and returns a changed version. Use one when a toggle in `options` is not enough and you do not want to give up the rest of the Builtin.

Presets need package **0.1.7 or later** (released on 12-11-2026) and the Rust runner of the same release. The design and the experiments behind it are in [presets-design.md](presets-design.md).

**Do not edit a rendered file by hand.** The runner refuses a hand-edited owned file. A Preset is how a change survives the next render and the next upgrade.

## A worked example: more time for the Rust CI job

The Rust CI workflow gives its job 15 minutes. Yours needs 30.

1. **Write the Preset**, `presets/ci.pkl`. In the examples `PKG` stands for `package://github.com/pantheon-org/pantheon-scaffold/releases/download/vVERSION/pantheon-scaffold@VERSION::sha256:SUM`, the same address as in your `pantheon.pkl`.

   ```pkl
   import "PKG#/Config.pkl"
   import "PKG#/Presets.pkl"

   function apply(workflows: Mapping<String, Config.Workflow>): Mapping<String, Config.Workflow> =
     Presets.amendWorkflow(workflows, "rust-ci", (w) ->
       Presets.amendJob(w, "build-test", (j) -> (j) { `timeout-minutes` = 30 }))
   ```

2. **Apply it in `pantheon.pkl`**: import it, list it in `presets`, and pass the Builtins through it.

   ```pkl
   amends "PKG#/Config.pkl"

   import "PKG#/Builtins.pkl"
   import "presets/ci.pkl" as ci

   options { toolchain = "1.96.1"; skills = false; binary = false }

   presets { "presets/ci.pkl" }

   workflows = ci.apply(Builtins.standardWorkflows(options))
   ```

3. **Render.** `.github/workflows/rust-ci.yml` now has `timeout-minutes: 30` and nothing else about it changed.

4. **Check.** `check` lists the Presets it applied, in order, before its verdict:

   ```text
   preset presets/ci.pkl
   check ok: 14 owned file(s) match the render
   ```

5. **Commit** `pantheon.pkl`, `presets/` and the rendered files, including `.pkl-gen/manifest.json`, which now records a hash of every Preset.

Paths in `presets { }` are relative to the directory of `pantheon.pkl`, and must be a `.pkl` file under `presets/`.

## The helpers

`Presets.pkl` has five helpers. Each one fails, naming what exists, when what you ask for is not there. Plain pkl does not: amending a workflow or job the package no longer has silently creates an empty one, and your change would stop working without a word.

| Helper | Changes | Fails when |
|---|---|---|
| `amendWorkflow(workflows, key, change)` | one workflow of a mapping | there is no workflow `key`. The message lists the builtins. |
| `amendJob(workflow, job, change)` | one job of a workflow | there is no such job. The message lists the jobs. |
| `amendStep(job, name, change)` | the step with that `name` | no step has that name. The message lists the names. |
| `insertStepsBefore(job, name, steps)` | adds steps before the step with that `name` | no step has that name |
| `amendFile(files, path, change)` | one file of a mapping | there is no such file. The message lists the files. |

A step is found by its `name`. A step without one cannot be targeted.

## Recipes

**Append steps to a job.** Amending `steps { ... }` appends:

```pkl
(j) { steps { new Config.Step { name = "Extra"; run = "echo extra" } } }
```

**Replace all the steps of a job.** Assign a typed listing. An untyped `new { }` inside it is a `Dynamic` and fails the type check:

```pkl
(j) { steps = new Listing<Config.Step> { new Config.Step { name = "Only step"; run = "echo only" } } }
```

**Change one step, or add steps before it.**

```pkl
Presets.amendStep(job, "Test", (s) -> (s) { run = "cargo test --workspace --locked" })
Presets.insertStepsBefore(job, "Test", List(new Config.Step { name = "Set up"; run = "echo hello" }))
```

**Add a workflow of your own.** Write it as a function in a Preset, and add its key to the mapping:

```pkl
// presets/extra.pkl
import "PKG#/Config.pkl"

function nightly(): Config.Workflow = new {
  name = "Nightly"
  on = new Mapping { ["schedule"] = new Listing { new Mapping { ["cron"] = "0 3 * * *" } } }
  jobs { ["check"] { steps { new Config.Step { name = "Say hello"; run = "echo hello" } } } }
}
```

```pkl
// pantheon.pkl
workflows = (Builtins.standardWorkflows(options)) { ["nightly"] = extra.nightly() }
```

**Add to a rendered file**, for instance more lines in `.gitignore`:

```pkl
Presets.amendFile(files, ".gitignore", (f) -> (f) { content = f.content + "\nmy-own-entry/\n" })
```

**Add a workflow of your own and keep Dependabot off it.** The rendered `.github/dependabot.yml` lists the workflow files the package owns under `exclude-paths`, so Dependabot does not bump pins in files the runner would then report as drift. A workflow you add in a Preset is not in that list. Add its path, the same way you change any rendered file:

```pkl
Presets.amendFile(files, ".github/dependabot.yml", (f) -> (f) { content = f.content.replaceFirst("  exclude-paths:\n", "  exclude-paths:\n  - .github/workflows/nightly.yml\n") })
```

Workflows you write by hand, outside `pantheon.pkl`, are not owned and Dependabot still updates them.

**Keep your own version of a rendered file, or drop a Builtin.** Filter it out of the mapping, and the runner renders nothing for it:

```pkl
files.toMap().filter((path, _) -> path != "deny.toml").toMapping()
```

**Do not write** `workflows { ...Builtins.standardWorkflows(options)  ["rust-ci"] = ... }`. Spreading a mapping and then assigning one of its keys is a pkl error (`Duplicate definition of member`). Amend the mapping, as the helpers do: `workflows = (standard) { ["rust-ci"] = ... }`.

## What the runner does with Presets

| Command | What it does |
|---|---|
| `render` | Records `presets` in `.pkl-gen/manifest.json` (path and hash of each listed Preset). Never writes or deletes a Preset. A listed file that is missing is an error, before anything is written. |
| `check` | Prints `preset <path>` for each Preset in order. Warns, without changing the exit code, about a file under `presets/` that your config does not list (`presets/x.pkl is not applied by pantheon.pkl`) and about a Preset that changed since the last render (`presets/x.pkl changed since the last render`). Drift in a rendered file is still exit 4. |
| `upgrade` | Moves the package imports in `presets/` with the config, then evaluates the config as it now reads, before `mise` runs. If a Preset no longer fits (a workflow or job it names was renamed or removed), it stops with exit 1 and puts every file back. `--dry-run` lists the Presets it would move and does not evaluate. |
| `detach` | Leaves `presets/` alone, like any file that is yours. |

Editing a Preset is how you change what is rendered: edit it and run `render`. If you edit a Preset and do not render, `check` reports the changed hash, or drift in the files whose content would change.

## Messages you may meet

| Message | Meaning |
|---|---|
| `preset: workflow "rust-cii" is not a builtin of this package version. Builtins: ...` | A typo, or a package that no longer has that workflow. |
| `preset: job "build" does not exist in workflow "Rust CI". Jobs: build-test` | The same, for a job. |
| `preset presets/x.pkl is listed by pantheon.pkl but the file does not exist` | `presets { }` lists a file that is not there. |
| `pantheon.pkl does not evaluate with 0.1.8, so nothing was changed:` followed by pkl's message | `upgrade` found that the new package breaks the config or a Preset. |
| `#/presets/0: does not match ...` | A path in `presets { }` is not a `.pkl` file under `presets/`. |

## Rules and limits

- **Order is precedence.** The Builtins come first, then your Presets in the order your config applies them. If two Presets change the same field, the later one wins, silently. The order is in `pantheon.pkl`, and `check` prints it.
- **Presets are inputs, not outputs.** They are yours. Only owned files that the runner writes are protected from hand edits.
- **The escape hatch still counts.** A Preset that uses `raw` counts in the quality gate like any other use.
- **The TypeScript runner does not know Presets.** It is removed in task 3.4 of the plan.
- **`upgrade` moves the package imports in your Presets.** Every `.pkl` file under `presets/` next to your config that imports the package gets the new version and checksum, as `pantheon.pkl` does, because a Preset left at the old version makes pkl stop with `Module version conflict`. Files outside `presets/` are not moved, so a helper module that imports the package from elsewhere must be updated by hand.

## This repository's own Presets

`pantheon-scaffold` is rendered by its own scaffold: `pantheon.pkl` at the root, with four Presets in `presets/`. They import the package in this checkout by a relative path, so `upgrade` has nothing to move in them.

| Preset | What it does |
|---|---|
| `presets/gitignore.pkl` | Adds this repository's own entries to the rendered `.gitignore` (`amendFile`). |
| `presets/own-deny.pkl` | Drops the Builtin's `deny.toml`, because this repository keeps its own licence list (a filter). |
| `presets/rust-ci.pkl` | Installs the pinned tools before the test step and leaves the features crate out of it (`amendJob`, `insertStepsBefore`, `amendStep`). |
| `presets/scaffold-check.pkl` | Builds the runner in this checkout and checks with it, instead of installing a released runner (`amendJob`, `insertStepsBefore`, `amendStep`). |
