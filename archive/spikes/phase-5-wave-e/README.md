# Phase 5 wave E: mutation check

`mutate.ts` breaks six properties of the consumer smoke step in `verify-release` (the second render writes nothing, `check` runs, `Builtins` is imported from the same package URI, the standard files and workflows are rendered, the package is loaded with its published checksum) and fails if `runner/test/release-workflow.test.ts` stays green for any of them. It only mutates inside the smoke step. Run from the repository root: `bun tmp/phase-5-wave-e/mutate.ts`, or with a text to run only matching entries. It takes under a minute.

Run on 08-11-2026: 6 of 6 caught the first time. Files restored.
