# Phase 3 wave A: mutation check

`mutate.ts` breaks seven properties of the verbatim tooling files (the exec bit of all nine scripts, the configs' mode, a missing file, a script rendering another script's asset, two assets changed by one byte, and `check`'s mode comparison) one at a time and fails if the tooling, mode-index and check tests stay green for any of them. Run it from the repository root: `bun tmp/phase-3-wave-a/mutate.ts`. It takes under a minute.

Run on 08-11-2026: 7 of 7 caught, files restored.
