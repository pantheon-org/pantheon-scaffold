# The workflow layer

Workflows the four Rust repositories share, rendered from typed pkl by the builtins in `pkl/Builtins.pkl`. Phase 4 of the plan. This page grows wave by wave: wave A is here.

## Wave A: the workflows that are identical in all four repositories

| Builtin | Renders | Config file it uses |
|---|---|---|
| `Builtins.plumber()` | `.github/workflows/plumber.yml` | `.plumber.yaml` |
| `Builtins.syncLabels()` | `.github/workflows/sync-labels.yml` | `.github/labels.txt` |
| `Builtins.prAutoRebase(options)` | `.github/workflows/pr-auto-rebase.yml` | none |
| `Builtins.releaseAutomerge(options)` | `.github/workflows/release-automerge.yml` | none |
| `Builtins.commitTypes()` | `.github/workflows/commit-types.yml` | `.github/commit-types.txt` |
| `Builtins.conventionalCommits()` | `.github/workflows/conventional-commits.yml` | none |

`Builtins.workflowFiles()` renders the four config files and `.github/pull_request_template.md`, byte for byte from `pantheon-adr` at `1e2fd46`. The scripts these workflows call stay in `Builtins.toolingFiles()` (phase 3).

Each workflow is compared with the pinned `pantheon-adr` copy under the P.1 rule, job ids and job names are compared (the required status checks depend on them), and `actionlint` runs over the render.

### What the typed model gained

- `Step.continue-on-error`, used by the Plumber action step.
- `Workflow.onComment`, a comment on the `on:` line. `conventional-commits.yml` needs it: `on: # zizmor: ignore[dangerous-triggers] ...` is a comment that zizmor itself reads. The renderer fails if the comment is set and there is no `on:` line to put it on.
- `Options.appClientId`, the client id of the GitHub App that `pr-auto-rebase` and `release-automerge` mint their token from. A public identifier, not a secret. The default is the org's `pantheon-ai-bot` App, as the four repositories use it today.
- Two pins in `Pins.pkl`: `actions/create-github-app-token` v3.3.0 and `getplumber/plumber` v0.7.16, each the commit of a release tag (checked against GitHub).

### Comments that a tool acts on

P.1 ignores comments, so a render that dropped `# zizmor: ignore[...]` would pass P.1 and then fail zizmor in the real repository. `compareDirectives` in `runner/src/equality.ts` compares those comments as a second check next to P.1, and every wave A workflow passes both. A `zizmor` comment inside a `run:` script is part of the script and is covered by the byte comparison. Other comments (explanations, section labels) are lost in the render, which P.1 allows. The top comment block of each workflow is kept as the `header`.

### Rendered text differs from the files in two harmless ways

`YamlRenderer` writes the key `on` quoted (`'on':`), because `on` is a YAML 1.1 boolean, and it drops the comments that sit between keys. Both parse to the same data, and GitHub accepts the quoted key.

### The GitHub App already exists

`pr-auto-rebase.yml` and `release-automerge.yml` already mint a token from a GitHub App, `pantheon-ai-bot` (client id in the workflow, private key in the org secret `GH_APP_PRIVATE_KEY`, contents and pull-requests write). So an App exists in the organisation. Whether it is the App the plan's P.4 asks for, and whether the secret is available to `pantheon-scaffold`, is for the owner to decide.

### Not in wave A

`.github/actionlint.yaml` is in three of the four repositories but not the library repository. It only ignores findings in the `cargo-dist`-generated `release.yml`, so it belongs with the `binary` toggle (phase 5).

## Wave B: the workflows whose differences are decided

Decision 74 settled how the four repositories differ, so these builtins take options and render each repository's own file.

| Builtin | Renders | What varies |
|---|---|---|
| `Builtins.releasePlease(options)` | `release-please.yml` | `binary`: with a binary it also checks out and runs `tag-release.sh`. `crateName` appears in the header comment only. |
| `Builtins.zizmor(options)` | `zizmor.yml` | `binary`: reads `.github/zizmor.yml`. `compositeAction`: also scans `action.yml`. The command in the header follows both. |
| `Builtins.agnix(options)` | `agnix.yml` | `skills`: the first sentence of the header. |
| `Builtins.aislop()` | `aislop.yml` | nothing |
| `Builtins.rustCi(options)` | `rust-ci.yml` | `features = "all"`: `--all-features` in clippy, build and test, and an extra build with the default features. |
| `Builtins.analysisFiles(options)` | `.agnix.toml`, `.github/dependabot.yml`, `.github/zizmor.yml` | `skills` (first line of `.agnix.toml`), `gitDependencyIgnore` (the `ignore` list), `binary` and `compositeAction` (which zizmor config, or none). |

### Options added to the typed model

`binary`, `skills`, `compositeAction`, `crateName` and `gitDependencyIgnore`. They are only what these builtins need to render the four repositories. Phase 5 turns the first three into the documented toggles and tests each one on and off.

### One place for the versions

`agnix.yml` pins agnix's version and SHA-256, `zizmor.yml` pins zizmor's version and `aislop.yml` pins aislop's, and `mise.toml` pins agnix and zizmor too. They now all come from `pkl/Tools.pkl`, which ends the "keep in sync with the workflow" comments the repositories carried. The SHA-256 belongs to the agnix version, so they change together.

### How it is checked

Three renders stand for the three kinds of repository, each compared with that repository's own pinned file under P.1, with the job ids and names and the comments a tool acts on checked too:

- like `pantheon-adr`: a binary, a skill, the crate `adr`, git dependencies to ignore
- like the library repository (`pantheon-agentic-harness-dev-kit`): no binary, no skill, all features
- like `pantheon-skill-auditor`: a composite action (its `zizmor.yml` and `.github/zizmor.yml`)

The same `adr`-pinned file is also the file of `pantheon-journal` and `pantheon-skill-auditor` for `agnix`, `aislop` and `rust-ci`. `release-please.yml` differs between the three repositories with a binary only in a comment that names the crate.

`actionlint` is run over the rendered workflows of the `adr`-like and the library-like render. Five new pins are each the commit of a release tag.

### Comment differences to expect

P.1 ignores comments. The header of each workflow is kept, and comments between keys are lost. The headers that depend on an option are built so that they never describe a file the repository does not have (the zizmor command, the agnix lead sentence). The wording of a header can still differ from a repository's own, and the real-repository render lists those.

### Not built by any task yet

Decision 74 settles how four more files are handled, but no task in the plan builds them: `.gitignore` (the `target/` pick), `.coderabbit.yaml` (the skill folder is a parameter), `SECURITY.md` (seeded once) and `tag-release.sh` (the crate name is a parameter, and the `binary` variant of `release-please.yml` calls it). `audit-skill.yml` and `release.yml` belong to the phase 5 toggles. The owner is asked where these go.
