# Phase 5 wave A: mutation check

`mutate.ts` breaks eight properties of `Builtins.verbatimFiles` one at a time (the files only with a binary, `release.yml` external, the pinned `dist-workspace.toml`, no executable bit, the decided `target/` pick, `.gitignore` for every repository, the pinned `actionlint.yaml`) and fails if `runner/test/phase5-verbatim.test.ts` stays green for any of them. Run from the repository root: `bun tmp/phase-5-wave-a/mutate.ts`, or with a text to run only matching entries. It takes under a minute.

Run on 08-11-2026: 7 of 7 caught the first time. Files restored. Decision 87 added an eighth entry on 09-11-2026 (the `.pkl-gen/cache/` line), caught the first time.
