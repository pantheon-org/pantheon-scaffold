# Feature toggles and the files they own

Phase 5. The toggles are the options `skills`, `binary`, `features` and `compositeAction` (decision 74). This page lists what each one owns, wave by wave as it is built.

## Wave A: the verbatim files

`Builtins.verbatimFiles(options)` returns:

| File | When | Source |
|---|---|---|
| `.gitignore` | always | `pkl/assets/gitignore`, `target/` (the form three repositories use, decision 74). The library repository has `/target`, so it changes by one line when it adopts |
| `.github/actionlint.yaml` | `binary` | byte for byte from `pantheon-adr` at 1e2fd46 (identical in `pantheon-journal` and `pantheon-skill-auditor`) |
| `dist-workspace.toml` | `binary` | the same three repositories, byte for byte |
| `.github/workflows/release.yml` | `binary` | class `external`: listed with no content, so the runner never writes, hashes or deletes it. `dist generate` writes it. The drift check against `dist generate --check` is wave D |

`actionlint.yaml` belongs to `binary` because the three repositories that have it all build a binary and the library repository, which has none, has no such file (Q1 of the refined plan).

Known limit: `dist-workspace.toml` pins `actions/checkout`, `upload-artifact` and `download-artifact` by commit in its `[dist.github-action-commits]` table. Those commits are not in `pkl/Pins.pkl`, so the pin update workflow does not move them and they are not checked against a release tag. They change with the file.

Checks: `runner/test/phase5-verbatim.test.ts` (git blobs of the pinned files, both kinds of repository, `release.yml` never written) and the mutation check in `tmp/phase-5-wave-a`.
