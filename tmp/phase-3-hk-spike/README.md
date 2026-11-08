# Phase 3 wave C spike: hk imports a rendered local module

Question (plan task 3.1, decision 31): does hk 2.10.0 evaluate a seeded `hk.pkl` that imports a committed local file rendered by the runner, and still merge the repository's own steps?

Run `./spike.sh` (needs hk, pkl and, for the real run, rumdl).

Result on 08-11-2026 (macOS, hk 2.10.0, hk config package v1.49.0): yes.

- `hk validate`: "hk.pkl is valid".
- `hk check --all --plan` lists `rustfmt` and `rumdl` from the shared module and `my-local-step` from `hk.pkl`.
- A real `hk check --all` runs the `rumdl` step and passes.

The shared module imports the hk config package for the `Step` type and exposes steps as functions, so the seeded file calls `shared.preCommit()`. Decision 31 stands and the fallback (render the whole `hk.pkl` as owned) is not needed.
