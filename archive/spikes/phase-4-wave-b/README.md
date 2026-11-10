# Phase 4 wave B: mutation check

`mutate.ts` breaks fourteen properties of the wave B builtins (zizmor's code scanning upload and config, the all-features build, Dependabot's ignore list, release-please's tag steps, agnix's concurrency and fork rule, the agnix digest, the aislop version, a pin, the zizmor and agnix config files, zizmor's inputs, and the agnix config bytes) one at a time and fails if the wave B tests stay green for any of them. Run it from the repository root: `bun tmp/phase-4-wave-b/mutate.ts`, or with a text to run only the entries whose description contains it. It takes about two minutes.

Run on 08-11-2026: 13 of 14 caught the first time. The miss was a flaw in the mutation itself (it inserted a no-op), not a gap in the tests. The mutation was replaced with one that really breaks the match, and it was caught. Files restored.
