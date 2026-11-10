# Phase 4 wave A: mutation check

`mutate.ts` breaks twelve properties of the wave A workflows (a `run:` script, the zizmor ignore comment, the required check's job name, a schedule, the App client id option, job permissions, an explicit empty permissions block, a runner image, a pin, `continue-on-error`, a config file, and the directive check itself) one at a time and fails if the workflow and equality tests stay green for any of them. Run it from the repository root: `bun tmp/phase-4-wave-a/mutate.ts`. It takes about a minute.

Run on 08-11-2026: 12 of 12 caught, files restored.
