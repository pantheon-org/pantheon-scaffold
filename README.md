# pantheon-scaffold

Defines the GitHub CI/CD of the `pantheon-*` repositories in pkl, and renders it from one `pantheon.pkl` per repository, in the style of hk's `hk.pkl`. The workflows, hooks and tooling files a repository needs are ported into this package as **Builtins**, whatever language the repository uses. A repository can add its own **Presets**: local custom versions of a Builtin that add to it or override it. A small runner renders the files and keeps them in sync.

## Status

Early stage, being refocused (see [docs/refocus-2026-11-10.md](docs/refocus-2026-11-10.md)). The package and the runner, a Rust program, are published together as GitHub releases. The newest is 0.1.6, the first with the Rust runner, a stable release since 11-11-2026 (see [docs/release-process.md](docs/release-process.md)). No repository has adopted it yet. The repository was created on 07-11-2026 after feasibility spikes.

Where it is heading:

- The runner becomes a Rust project, built with TDD, BDD (cucumber-rs) and mutation testing (cargo-mutants).
- Presets are defined and supported by `render`, `check`, `upgrade` and `detach`.
- `init` sets up a new project, and `migrate` brings an existing one under the scaffold.
- The first pass covers the four Rust repositories: `pantheon-adr`, `pantheon-journal`, `pantheon-skill-auditor` and `pantheon-agentic-harness-dev-kit`.

Dropped for now: repository governance (rulesets, merge settings and drift checks), pklr, Windows, GitLab and `self-update`.

## Where to read

| To | Read |
|---|---|
| Adopt, upgrade or leave the scaffold | [docs/adopting.md](docs/adopting.md) |
| Start a new project under the scaffold (`init`) | [docs/init.md](docs/init.md) |
| Cut or promote a release, and the accepted risks | [docs/release-process.md](docs/release-process.md) |
| Install the runner, and what `upgrade`, `render`, `check` and `detach` do | [docs/runner-install.md](docs/runner-install.md), [docs/runner-upgrade.md](docs/runner-upgrade.md), [docs/runner-lifecycle.md](docs/runner-lifecycle.md), [docs/runner-safety.md](docs/runner-safety.md) |
| Run `check` in CI | [docs/check-in-ci.md](docs/check-in-ci.md) |
| Change what the package renders for your repository (Presets) | [docs/presets.md](docs/presets.md), the design in [docs/presets-design.md](docs/presets-design.md) |
| See what the package renders | [docs/tooling-layer.md](docs/tooling-layer.md), [docs/workflow-layer.md](docs/workflow-layer.md), [docs/feature-toggles.md](docs/feature-toggles.md) |
| See the evidence for a release | [archive/proof/docs/proof-report-0.1.3.md](archive/proof/docs/proof-report-0.1.3.md), [archive/proof/docs/security-dossier-0.1.5.md](archive/proof/docs/security-dossier-0.1.5.md) |
| See what changed in the refocus, and what was dropped | [docs/refocus-2026-11-10.md](docs/refocus-2026-11-10.md) |
| See what was archived, and how to re-run it | [archive/README.md](archive/README.md) |
| See which runner files move to Rust, stay, or are re-pointed | [docs/runner-file-sort.md](docs/runner-file-sort.md) |

Licence: MIT.
