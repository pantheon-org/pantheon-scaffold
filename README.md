# pantheon-scaffold

A pkl package and a small runner that render the shared scaffold files (toolchain, deny policy, CI workflows, hooks) for the Pantheon Rust repositories, in the style of hk's `hk.pkl`: one `pantheon.pkl` per repository, reusable builtins and custom workflows.

Status: early stage. The package and the runner are published together as GitHub releases (the newest is 0.1.5, a pre-release until it is promoted: see [docs/release-process.md](docs/release-process.md)). No repository has adopted it yet. To adopt it in a repository, read [docs/adopting.md](docs/adopting.md). The repository was created on 07-11-2026 after feasibility spikes, whose evidence is under `tmp/`.

Licence: MIT.
