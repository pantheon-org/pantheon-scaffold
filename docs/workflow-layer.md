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
