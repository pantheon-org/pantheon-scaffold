# Presets: design record

Written on 11-11-2026 for task 4.0 of the refocus plan. It decides how a repository customises a Builtin, before any scenario or code (tasks 4.1 to 4.4). Every claim marked **E<n>** was run with `pkl` 0.32.2 against the real `Config.pkl` and `Builtins.pkl`; the results are in the table at the end. Open question 4 was answered on 11-11-2026: Presets live in a fixed `presets/` directory.

## What a Preset is

A **Builtin** is a function in `Builtins.pkl` that returns a `Workflow` or a `GenFile` (`rustCi(options)`, `plumber()`). A **Preset** is a local pkl file in `presets/` that takes a Builtin's value and returns a changed one. Nothing new is needed in pkl to write one: a Preset is a module with functions, imported by `pantheon.pkl` (E6).

## Decisions

| # | Decision | Evidence |
|---|---|---|
| D1 | A Preset is a `.pkl` module under `presets/`. It imports `Config.pkl` (for the types) and exports functions from a Builtin's value to a changed value. `pantheon.pkl` imports it and applies it. | E6 |
| D2 | Four operations are supported: **override** a field (amend the object), **append** steps (amend `steps { ... }`), **replace** steps (`steps = new Listing<Config.Step> { new Config.Step { ... } }`), **add** a workflow (a new key). Replacing a whole Builtin is an entry assigned in an amended mapping. Removing one is a filter. | E1, E2a, E2b, E3, E9, E12 |
| D3 | **Precedence is the order of application.** Builtins come first; Presets are applied after them in the order `pantheon.pkl` lists. The last change to a field wins, silently, as pkl amending does. The order is in the config, and `check` prints it (D6). Two Presets that change the same field are therefore legal; this is a deliberate trade-off, not an oversight. | E14 |
| D4 | The API is **amend-based, never spread-plus-entry.** `workflows { ...standardWorkflows(o)  ["rust-ci"] = ... }` is a pkl error (`Duplicate definition of member`), so the package must offer `workflows = amendWorkflow(Builtins.standardWorkflows(options), "rust-ci", ...)`. | E4a, E4b, E7, E8 |
| D5 | **A Preset that names something the Builtins no longer have fails loudly.** Plain pkl does not: amending a missing workflow or job silently creates a new, empty one (E5a, E11). The package therefore provides `amendWorkflow` and `amendJob` helpers that throw, naming what was asked for and what exists (E13). A typo in a field name and a wrong type already fail in pkl with a clear message (E5b, E5c). | E5a, E11, E13 |
| D6 | **The manifest treats `presets/` as inputs.** The runner hashes every `presets/**/*.pkl` into the manifest under a new optional key (the manifest already carries optional keys, such as `packageVersion`). Presets are never written or deleted by the runner, and editing one by hand is the normal way to change them. The rendered outputs stay owned: a hand edit of an output is still refused. `check` prints every Preset in use, in order, and warns about a file in `presets/` that no config applies (otherwise each Preset is an invisible fork). | design; the manifest key is a 4.3 task |
| D7 | **Upgrade checks compatibility before it writes.** `upgrade` renders the new package with the Presets in memory; an error from a Preset (D5) stops it before any file or pin changes, and the message names the Preset file, the workflow or job, and the new package version. `upgrade` already rolls back on any failure; this adds a failure that happens earlier. | design; 4.3 |
| D8 | **The contract names the Presets applied.** The contract (the JSON the runner reads) has no way today to say which Preset changed which file, and its schema is closed (`additionalProperties: false`). Task 4.2 adds an optional `presets` list to the schema: the paths the config applies, because the pkl helpers are pure functions and cannot record what they changed. It is left out of the contract when empty, so a repository without Presets renders the same JSON as before and `requiresRunner` stays at 0.0.0; an older runner refuses only a contract that has the list. | design; 4.2 |

## What the experiments also showed (constraints for 4.2)

- Inside `new Listing<...> { }` an untyped `new { }` becomes a `Dynamic` and fails the type check; steps must be written `new Config.Step { }` (E2b).
- A function passed as a parameter is called with `.apply(x)`, not `f(x)` (E13).
- Amending a `Listing` appends. A Preset cannot replace one step in place with plain amending; replacing a step by name needs a helper (`amendStep`) that is not built or tested yet. It is a candidate for 4.2, not a decision.
- The `raw` escape hatch is counted as "uses" by the quality gate. A Preset that uses it counts too; this is unchanged.

## Rejected

- **Presets anywhere the config imports.** Rejected by the owner on 11-11-2026 (open question 4): the runner would have to follow imports to list them.
- **Treating a conflict between two Presets as an error.** pkl amending cannot detect it without a helper that tracks who set a field; the cost is high for a rare case. Revisit if a real repository hits it.
- **Having the runner parse Preset files.** Presets are pkl; only pkl evaluates them.
- **A new pkl construct for Presets.** Not needed (D1).

## Scenarios for task 4.1

1. Override one field of a Builtin; render; the field changed and nothing else.
2. Replace a Builtin's steps; the file has exactly the new steps.
3. Add a workflow through a Preset; it renders and `detach` handles it like any owned file.
4. A Preset amends a workflow or job the Builtins no longer have: exit non-zero, nothing written, the message names the Preset and what exists.
5. A Preset file edited by hand: `check` sees the changed input and reports drift in the rendered files until they are rendered again; an output hand edit is still refused.
6. `upgrade` with a Preset present, compatible: pin moves, render succeeds. Incompatible: nothing changes and the message names the Preset.
7. `detach` after a Preset: rendered files stay, `presets/` is left alone.
8. `check` lists the Presets in order and warns about an unused file in `presets/`.

## Experiments

Files in `tmp/presets-spike/` (untracked working files). Config: `toolchain = "1.96.1"`, `skills = false`, `binary = false` unless the standard set was not used.

| E | What was tried | Result |
|---|---|---|
| E1 | `(Builtins.rustCi(options)) { jobs { ["build-test"] { timeout-minutes = 30 } } }` | Works: timeout 30, 7 steps unchanged. |
| E2a | Amend `steps { new { ... } }` | Appends: 8 steps. |
| E2b | `steps = new Listing { new { ... } }` | pkl error: got `Dynamic`, expected `Step`. |
| E2b (typed) | `steps = new Listing<Config.Step> { new Config.Step { ... } }` | Works: exactly 1 step. |
| E3 | A new key in `workflows` | Works: `my-extra.yml` rendered. |
| E4a | `...standardWorkflows(o)` then `["rust-ci"] = ...` | pkl error: `Duplicate definition of member "rust-ci"`. |
| E4b | The same key twice | The same error. |
| E5a | Amend a job name that is not in the Builtin | **No error**: a second, empty job is rendered. |
| E5b | A mistyped field name (`timeout-minute`) | pkl error, with `Did you mean timeout-minutes`. |
| E5c | A wrong type (`"30"` for an Int) | pkl error naming the expected type. |
| E6 | A Preset module in `presets/` exporting a function, applied from `pantheon.pkl` | Works: timeout 45. |
| E7 | Spread, then amend the same key from the spread value | pkl error: duplicate. |
| E8 | `workflows = (standardWorkflows(o)) { ["rust-ci"] { name = "Mine" } }` | Works: name changed, rest unchanged. |
| E9 | The same with `=` and a new value | Works: replaced. |
| E10 | Amend one entry and add another in the same amended mapping | Works: timeout 30, `my-extra.yml` added. |
| E11 | Amend a workflow key that does not exist | **No error**: a `no-such-workflow.yml` is rendered. |
| E12 | Filter a key out of the standard set | Works: `rust-ci.yml` absent. |
| E13 | `amendWorkflow` and `amendJob` helpers | Works on a real key (timeout 30). A missing workflow gives: `preset: workflow "rust-cii" is not a builtin of this package version. Builtins: agnix, aislop, ...`. A missing job gives: `preset: job "build" does not exist in workflow "Rust CI". Jobs: build-test`. |
| E14 | Two Presets change the same field (20, then 40) | The last wins silently: 40. |

## Open points for the owner (none block 4.1)

1. D3 allows two Presets to change the same field with the last winning. Say so if you would rather have an error.
2. `amendStep` (replace one step by name) is not decided; add it in 4.2 only if a real repository needs it.
