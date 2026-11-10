# Mutation check of the promotion (Phase 8, wave A)

`mutate.ts` removes one guard of `runner/src/promote.ts` at a time and runs `runner/test/promote.test.ts`. Every mutation must make the tests fail. The source file is restored afterwards.

Run on 09-11-2026 against the promotion as first written. Rerun with `bun tmp/phase-8-promote/mutate.ts` after any change to `promote.ts`.
