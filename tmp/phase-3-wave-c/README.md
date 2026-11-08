# Phase 3 wave C: mutation check

`mutate.ts` breaks ten properties of the hook files (the pre-commit steps, the `features` option in both pre-push steps and in the builtin, the seeded file's class, text and imports, the hk config package version, and the shared module's header) one at a time and fails if the hk tests stay green for any of them. Run it from the repository root: `bun tmp/phase-3-wave-c/mutate.ts`. It takes about two minutes and needs hk.

Run on 08-11-2026: 10 of 10 caught, files restored.
