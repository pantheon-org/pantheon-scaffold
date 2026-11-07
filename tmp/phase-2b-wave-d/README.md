# Phase 2b wave D: mutation check

`mutate.ts` breaks 19 properties (the tag and package-file checks in `runner/src/release.ts`, and the safety properties of `.github/workflows/release.yml`) one at a time and fails if the release and workflow tests stay green for any of them. Run it from the repository root: `bun tmp/phase-2b-wave-d/mutate.ts`. It runs only `test/release.test.ts` and `test/release-workflow.test.ts`, so it takes under two minutes.

Run on 07-11-2026: 18 of 19 caught the first time. The miss was the `.zip.sha256` check, which had no test. The test was added and that mutation was re-checked on its own (caught).
