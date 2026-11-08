# Lock platform guard: mutation check

`mutate.ts` breaks seven behaviours (the lock check for platforms the runner is not built for, wrong assets, and missing urls, the rollback in `upgrade`, and the two safeguards in the release workflow's `verify-release` job) one at a time and fails if the lock, upgrade and workflow tests stay green for any of them. Run it from the repository root: `bun tmp/phase-2b-lock-platforms/mutate.ts`. It takes under a minute.

Run on 07-11-2026: 7 of 7 caught, files restored.

The two lock files in `runner/test/fixtures/mise-lock-*.lock` are real `mise lock` output for the published v0.1.1, with and without `lockfile_platforms`. The workflow's shell check was also run by hand against both: the three-platform lock passes (exit 0), the all-platform lock fails (exit 1), and a three-entry lock with a zip in it fails with the asset message.
