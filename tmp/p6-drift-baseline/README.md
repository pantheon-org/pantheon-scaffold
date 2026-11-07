# p6-drift-baseline

The do-nothing baseline for gate A. `bun drift.ts` (or `--json`) compares git blob hashes of the default branch of the four Rust repos with
8 read-only `gh api` calls (about 4.5 s) and prints identical and drifted files, excluding files that differ by design.
Run on 07-11-2026: 29 identical, 14 drifted of 43 shared files. See `.context/research/2026-11-07-gate-a-evidence-projen-pkl-and-do-nothing.md`.
`tmp/p6-pkl-typed-refresh` is a copy of the hk-style pkl spike with its cargo-deny builtin updated to current upstream (`timeout-minutes`, `persist-credentials`).

## baseline.ts (SP-0.1, 07-11-2026)

`bun baseline.ts baseline.json` classifies every file (identical, drifted with the repos per version, per-repo by design, single repo), records the commit SHAs, the toggles read from file presence, the repo settings, the rulesets (names, rules, required checks, bypass roles) and the organisation facts, all with GET requests. `bun show.ts` and `bun show2.ts` print readable summaries of `baseline.json`. Record: `.context/research/2026-11-07-baseline-sp-0-1.md`. Re-run when the snapshot is more than 7 days old at gate A.
