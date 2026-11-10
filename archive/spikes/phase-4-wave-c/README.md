# Phase 4 wave C: mutation check

`mutate.ts` breaks twenty properties of the wave C pieces one at a time and fails if the matching tests stay green: the consumer upgrade workflow (detach removal, App token scopes, triggers, job permissions, checkout credentials, the freshness check, the "only when newer" guards, conventional commit, App token for the pull request), the package's own `update-pins.yml` (token scope, no pull request when nothing changed, the freshness branch, the push paths, the freshness output) and `update-pins.ts` (a no-write run, a problem stops everything, no summary when nothing changed, the record written when nothing moved). Run from the repository root: `bun tmp/phase-4-wave-c/mutate.ts`, or with a text to run only matching entries. It takes about a minute.

Run on 08-11-2026: 19 of 20 caught the first time. The miss was an equivalent mutation (rewriting identical text), so it was replaced with one that changes behaviour, and that was caught. Files restored.
