# Phase 2b wave C: mutation check

`mutate.ts` breaks five behaviours of the runner (the embedded schema, the build-time version, the case-only rename refusal, the dirty-tree delete guard, the executable bit), rebuilds the `darwin-arm64` binary each time and runs the smoke suite, which must fail. Run it from the repository root on an Apple silicon Mac: `bun tmp/phase-2b-wave-c/mutate.ts`. It ends by checking every mutated source file is as it was.

Run on 07-11-2026: 5 of 5 caught, files restored.

Also run by hand on 07-11-2026: the smoke suite against the `linux-arm64` binary in an Ubuntu 24.04 container (pkl 0.32.2 verified against the checksum in `mise.lock`, the smoke suite compiled to a Linux binary), 16 of 16 passed on a case-sensitive filesystem.
