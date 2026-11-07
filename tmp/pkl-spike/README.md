# pkl projen spike (06-11-2026)

Proves a pkl template plus a thin runner can do what projen does for a Pantheon Rust repo.

- `template.pkl`: open module with typed `Options` (toolchain, agnix toggle) and a `generated` mapping of files. Each file is `owned`, `seed` (write once) and/or `executable`. It renders the files and a `manifest.json`.
- `pantheon.pkl`: the consumer config, which amends the template.
- `runner.ts`: run with `bun runner.ts pantheon.pkl target`. It runs `pkl eval -m`, then seeds once, keeps `target/.pkl-gen/manifest.json`, deletes orphans and sets the executable bit.
- `test.sh`: run with `bash tmp/pkl-spike/test.sh`. It runs nine assertions in a throwaway copy, compares against `pantheon-adr` `origin/main` (set `ADR_REPO` for a different clone path) and exits with the failure count.
- `assets/`: verbatim files from `pantheon-adr` `origin/main` at `060531b` (fetched 06-11-2026). Toolchain `1.96.1` is replaced by `@@TOOLCHAIN@@` in the two `.tmpl` files.
- `target/` and `.stage/`: scratch output.

Run on 06-11-2026 against Apple pkl 0.32.2 and bun. Rendered `rust-toolchain.toml`, `deny.toml` and `cargo-deny.yml` are byte-identical to `pantheon-adr`. Not tested: pklr (the pure Rust evaluator, https://github.com/jdx/pklr), packaging as a `package://` module, real Cargo.toml seeding.

The test pins pantheon-adr at 060531b, the state this spike was written against (set UPSTREAM_REF=origin/main to compare with the current main, which is expected to fail now that upstream has moved).
