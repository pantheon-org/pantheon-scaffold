# pkl-s2-spike

Spike S.2 of the pkl plan: do `bun build --compile` binaries for linux-x64, linux-arm64 and darwin-arm64 install through mise from a
GitHub release with a checksum and a provenance attestation, and run on the CI runner images?
Run on 07-11-2026 with mise 2026.10.25, bun 1.4.2 and pkl 0.32.2. Local files untracked; the published side is
`pantheon-org/pantheon-scaffold-spike` (branch `s2-runner`, pre-release `runner-v0.0.1`, verify tag `runner-verify-v0.0.2`).
Local: `bash build.sh` builds the three targets. Source of the runner and the workflows is in the spike repo, not here.

| Question | Answer |
|---|---|
| Do the three targets build and run? | Yes. Built natively on `ubuntu-latest`, `ubuntu-24.04-arm` and `macos-latest` (all free public-repo runners), and also cross-compiled locally with `--target=bun-<os>-<arch>`. Sizes: darwin-arm64 62 MB, linux 81 MB. The smoke test (`--version`, render a sample through pkl) passed on all three runner images. |
| Release assets | Three raw binaries `pantheon-runner-<os>-<arch>` and `checksums.txt`, created as a pre-release by the workflow, with a build provenance attestation per binary. `gh attestation verify` passes for the real binaries and fails for a copy with one byte added. |
| Does mise install it? | Yes, through the `github:` backend with `version = "0.0.1"` and `version_prefix = "runner-v"`. mise picks the right asset per platform from the file names alone (no `asset_pattern`), and installs the binary under the name `pantheon-runner`. |
| Checksum and provenance | `mise lock` writes a checksum and `provenance = "github-attestations"` for each platform into `mise.lock`, and verifies the attestations at lock time. `mise install --locked` then enforces the checksums. Settings used: `lockfile = true`, `locked_verify_provenance = true`. A lock file with the checksums zeroed is rejected (exit 1) locally and on all three runners. |
| On CI | `runner-verify` passed on all three runners: `mise install --locked` from the committed lock file, run the installed runner, render the sample, and reject a tampered lock. |
| Decision 29 (second mise config file) | A `.mise/conf.d/*.toml` file is loaded beside `mise.toml`, and tools, env and tasks merge. On a conflict `mise.toml` wins, so a per-repo file cannot override the generator's pins. |
| macOS signing | The darwin binary is ad-hoc, linker-signed and valid, with no quarantine attribute when fetched by mise or curl. Not tested: an unsigned or ad-hoc binary on a managed Mac with a Gatekeeper policy. |

Pitfalls found, each relevant to the design:
- `mise install --locked` fails if any tool in the config hierarchy is missing from the lock file. A parent `mise.toml` (the repo root pinned `bun`) was picked up from a subfolder. The CI job needs `MISE_CEILING_PATHS` or a lock file that covers every tool. In a consumer repo this means `mise.lock` must cover all tools of both mise config files.
- A tag glob `runner-v*` also matches `runner-verify-v...`. Release workflows should use `runner-v[0-9]*`.
- The runner shells out to `pkl`, so pkl has to be in the same mise config as the runner.
- mise-action v5.2.7 was one day old at use, so v5.2.0 (about 3 weeks old) was pinned by SHA.

Not covered: Windows, an unsigned binary on a managed Mac, and a bun security rebuild policy.
