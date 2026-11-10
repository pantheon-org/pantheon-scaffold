# Phase 2b wave A: mutation check

`mutate.ts` breaks twelve behaviours of the runner one at a time (guard, lock, orphan deletion, exec bit, check) and fails if the suite stays green for any of them. Run it from `runner/`: `bun ../tmp/phase-2b-wave-a/mutate.ts`. It restores each file after every mutation and ends by re-running the suite.

Run on 07-11-2026 against the wave A branch: 12 of 12 caught, suite green after restore.
