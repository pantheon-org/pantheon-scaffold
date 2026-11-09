# pantheon-scaffold

A pkl package and a small runner that render the shared scaffold files (toolchain, deny policy, CI workflows, hooks) for the Pantheon Rust repositories, in the style of hk's `hk.pkl`: one `pantheon.pkl` per repository, reusable builtins and custom workflows.

Status: early stage. The package and the runner are published together as GitHub releases (the newest is 0.1.5, a stable release since 09-11-2026: see [docs/release-process.md](docs/release-process.md)). No repository has adopted it yet. To adopt it in a repository, read [docs/adopting.md](docs/adopting.md). The repository was created on 07-11-2026 after feasibility spikes, whose evidence is under `tmp/`.

## Where to read

| To | Read |
|---|---|
| Adopt, upgrade or leave the scaffold | [docs/adopting.md](docs/adopting.md) |
| Cut or promote a release, and the accepted risks | [docs/release-process.md](docs/release-process.md) |
| Install the runner, and what `upgrade`, `render`, `check` and `detach` do | [docs/runner-install.md](docs/runner-install.md), [docs/runner-upgrade.md](docs/runner-upgrade.md), [docs/runner-lifecycle.md](docs/runner-lifecycle.md), [docs/runner-safety.md](docs/runner-safety.md) |
| Run `check` in CI | [docs/check-in-ci.md](docs/check-in-ci.md) |
| See what the package renders | [docs/tooling-layer.md](docs/tooling-layer.md), [docs/workflow-layer.md](docs/workflow-layer.md), [docs/feature-toggles.md](docs/feature-toggles.md) |
| Check the repository rulesets and merge settings (nothing applied yet) | [governance/README.md](governance/README.md) |
| See the evidence for a release | [docs/proof-report-0.1.3.md](docs/proof-report-0.1.3.md), [docs/security-dossier-0.1.5.md](docs/security-dossier-0.1.5.md) |

Licence: MIT.
