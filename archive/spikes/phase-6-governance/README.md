# Mutation check of the governance code (Phase 6, wave A)

`mutate.ts` removes one behaviour of `runner/src/governance/*.ts` at a time and runs the two governance test files. Every mutation must make a test fail. The sources are restored afterwards.

Written 09-11-2026. Rerun with `bun tmp/phase-6-governance/mutate.ts` after any change to the governance code.
