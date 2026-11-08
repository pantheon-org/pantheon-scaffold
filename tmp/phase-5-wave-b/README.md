# Phase 5 wave B: mutation check

`mutate.ts` breaks fifteen properties of the parameterised files one at a time (the composite variant's paths, the executable bit and crate name of `tag-release.sh`, `SECURITY.md` seeded and its skill line and library text, `.coderabbit.yaml`'s skill name and skill-less form, the auditor digest, commit and bootstrap URL, the grade gate, the job timeout, and two asset bytes) and fails if `runner/test/phase5-parameterised.test.ts` stays green for any of them. Run from the repository root: `bun tmp/phase-5-wave-b/mutate.ts`, or with a text to run only matching entries. It takes about two minutes.

Run on 08-11-2026: 15 of 15 caught the first time. Files restored.
