# Phase 2b wave B1: mutation check

`mutate.ts` breaks 19 behaviours (semver, version rules, known-bad list, cache directory, `detach`, the contract fields) one at a time and fails if the suite stays green for any of them. Run it from the repository root: `bun tmp/phase-2b-wave-b1/mutate.ts`. It restores each file after every mutation and ends by re-running the suite. It takes about five minutes because it runs the whole runner suite 20 times.

Do not edit files under `runner/` or `pkl/` while it runs.

Run on 07-11-2026: the first run caught 18 of 19. The miss was the HTTP status check of the known-bad fetch, because the 404 test returned an invalid body and the fallback happened anyway. The test now returns a valid list with status 404, and that mutation is caught (re-checked on its own, the other 18 were not re-run).
