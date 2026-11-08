# Phase 5 wave D: mutation check

`mutate.ts` breaks eleven properties of `dist-check.yml`, `scaffold-check.yml` and `dist-workspace.toml` one at a time (the dist version, the check command, `binary` ownership, the standard set, the retry count, permissions, the trigger paths, the lock file install, and the three bytes of `dist-workspace.toml` that matter) and fails if `phase5-dist.test.ts` and `phase5-verbatim.test.ts` stay green for any of them. Run from the repository root: `bun tmp/phase-5-wave-d/mutate.ts`, or with a text to run only matching entries. It takes about two minutes.

Run on 08-11-2026: 11 of 11 caught the first time. Files restored.
