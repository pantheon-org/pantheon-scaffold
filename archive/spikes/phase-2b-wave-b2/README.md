# Phase 2b wave B2: mutation check

`mutate.ts` breaks 15 behaviours of `upgrade` (checksums parsing, pin rewriting, the attestation, rollback, `--dry-run`, version rules, the lock, exit codes) one at a time and fails if the suite stays green for any of them. Run it from the repository root: `bun tmp/phase-2b-wave-b2/mutate.ts`. It takes about five minutes.

Do not edit files under `runner/` or `pkl/` while it runs.
