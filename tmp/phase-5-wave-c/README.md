# Phase 5 wave C

`diff-toggles.ts` prints what flipping each toggle changes in the rendered contract, from a base configuration (`bun tmp/phase-5-wave-c/diff-toggles.ts`). The expected sets in `runner/test/phase5-toggles.test.ts` were read from it and checked against what each toggle owns.

`mutate.ts` breaks eleven properties of the toggles one at a time (what `skills` and `binary` own, the standard sets, the two name errors and their messages, the external `release.yml`) and fails if `phase5-toggles.test.ts` stays green for any of them (`bun tmp/phase-5-wave-c/mutate.ts`, about two minutes).

Run on 08-11-2026: 9 of 11 caught the first time. The two misses were flawed mutations (`"x" + throw(...)` still throws), replaced with ones that bypass the error, and both were caught. Files restored.
