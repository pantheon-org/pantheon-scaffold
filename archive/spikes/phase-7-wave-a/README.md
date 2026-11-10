# Phase 7 wave A: mutation check

`mutate.ts` loosens or breaks twelve rules of the proof categoriser one at a time (the known new files, the one file that may change mode alone, the exact size of the `mise.toml` migration, what `.gitignore` may gain and lose, what `dist-workspace.toml` may change, what `dependabot.yml` may gain, who may differ in `SECURITY.md` and how, moved pins, lost directive comments, and the `hk.pkl` migration) and fails if `runner/test/proof-categories.test.ts` stays green for any. Run from the repository root: `bun tmp/phase-7-wave-a/mutate.ts`, or with a text to run only matching entries. It takes about a minute.

Run on 09-11-2026: 9 of 12 caught the first time. The three misses were gaps in the tests (losing a `.gitignore` line other than `/target`, a differing `SECURITY.md` bullet in another repository, a lost `zizmor` directive comment). A test was added for each and all three were then caught. File restored.
