# Spike evidence for the pkl generator plan

Local, re-runnable evidence behind the pkl plan and the gate A research notes. Each folder has its own README.
Needs bun, pkl, gh (logged in, read-only calls), and a clone of `pantheon-org/pantheon-adr` at `~/Projects/github/pantheon-org/pantheon-adr` for the two original tests (set `ADR_REPO` otherwise).

| Folder | What | Re-run |
|---|---|---|
| `pkl-spike` | First spike: pkl module plus runner, nine assertions, pinned to pantheon-adr 060531b | `bash pkl-spike/test.sh` |
| `pkl-hk-spike` | hk-style typed config, six assertions, pinned to 060531b | `bash pkl-hk-spike/test.sh` |
| `pklr-spike` | pklr (pure Rust evaluator) probes and the late-binding repro | Rust: `cargo build` then see the README (heavy) |
| `pkl-s1-spike` | S.1 package: local rewrite test and the GitHub release test | `bun pkl-s1-spike/check.ts`, `bash pkl-s1-spike/real-github.sh` (needs the spike repo releases) |
| `pkl-s2-spike` | S.2 runner binary build | `bash pkl-s2-spike/build.sh` (heavy: downloads bun targets) |
| `pkl-s3-spike` | S.3 version comments | `cd pkl-s3-spike && bun test`, `bun roundtrip.ts`, `bun verify-tags.ts` |
| `pkl-s4-spike` | S.4 YAML null, cron, `on` key | `cd pkl-s4-spike && bun check.ts` |
| `pkl-yaml-trial` | Bun.YAML against the YAML test suite | `bash pkl-yaml-trial/fetch-suite.sh <dir>`, then `bun pkl-yaml-trial/trial.ts <dir>` |
| `p6-projen-spike` | Projen evidence (P.6), both styles | `cd p6-projen-spike && npm install`, then `bash test.sh` |
| `p6-pkl-typed-refresh` | pkl typed builtin refreshed to current upstream | `bash p6-pkl-typed-refresh/test.sh` (compares with the current main) |
| `p6-drift-baseline` | Do-nothing baseline and the SP-0.1 snapshot | `cd p6-drift-baseline && bun drift.ts`, `bun baseline.ts baseline.json` |
| `phase-2b-wave-a` | Mutation check of the wave A runner behaviours | `cd runner && bun ../tmp/phase-2b-wave-a/mutate.ts` |
| `phase-2b-wave-b1` | Mutation check of the wave B1 lifecycle behaviours | `bun tmp/phase-2b-wave-b1/mutate.ts` (about 5 minutes) |
| `apply-fourth-review`, `edit-roles` | One-off plan editing helpers, kept for the record | not meant to be re-run |

Tests that read the current upstream (`p6-*`, `pkl-s3-spike`, `pkl-yaml-trial`'s real workflows) can change result when upstream changes. The two original tests are pinned.
