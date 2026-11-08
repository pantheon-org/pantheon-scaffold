# Phase 3 wave B: mutation check

`mutate.ts` breaks twelve properties of the mise files (the tool pins, the supported platforms, the lock and provenance settings, `latest` staying `latest`, the two changed comments, the runner pin and where it comes from, the file classes, the version read from a checksum-pinned package URI, and the empty per-repo config) one at a time and fails if the mise and package URI tests stay green for any of them. Run it from the repository root: `bun tmp/phase-3-wave-b/mutate.ts`. It takes about two minutes.

Run on 08-11-2026: 12 of 12 caught, files restored.
