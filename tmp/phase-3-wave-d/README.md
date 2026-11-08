# Phase 3 wave D: mutation check of the evidence script

`mutate.ts` breaks the render in four ways (a changed verbatim script, a renamed hook step, an extra line in `mise.toml`, a renamed seeded file) and fails if `runner/scripts/real-repo-tooling.ts` still exits 0 for any of them. Run it from the repository root: `bun tmp/phase-3-wave-d/mutate.ts`. It needs the same tools and network as the script and takes about a minute per entry.

Run on 08-11-2026: the first run caught 3 of 4. The miss was a change to `mise.toml`, because the script only compared the original with the pinned blob and left the exact change to the unit test. The script now also requires the exact line counts of the `mise.toml` change (10 added, 4 removed). Second run: 4 of 4 caught, files restored.
