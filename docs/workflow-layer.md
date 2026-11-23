# The workflow layer

Workflows the four Rust repositories share, rendered from typed pkl by the builtins in `pkl/Builtins/`. Phase 4 of the plan. This page grows wave by wave: wave A is here.

## Where the builtins live

`pkl/Builtins.pkl` is the API: every function a repository's `scaffold.pkl` calls (`standardWorkflows`, `standardFiles`, `rustCi`, `plumber` and the rest) is defined there, and each is one line that calls into a folder.

| Path | What it holds |
|---|---|
| `pkl/Builtins/<name>/Builtin.pkl` | One builtin: its workflow function (`workflow`) and the slices of the file aggregators it feeds (`baseFiles`, `toolingFiles`, `workflowFiles`, `analysisFiles`, `verbatimFiles`, `parameterisedFiles`), or `files` for a builtin that renders only files (`mise`, `hk`). |
| `pkl/Builtins/<name>/assets/` | The verbatim files and templates that builtin reads, found with `read("assets/...")` from its own module. Each asset has exactly one copy. |
| `pkl/Builtins/Shared.pkl` | The steps and values several builtins use (`checkout`, `mainTriggers`, `mintToken` and others), Internal: not part of the API, and not for Presets. |
| `pkl/Presence.pkl` | The `Present` class, which says which builtins are present: the registry of builtin names, the table of hard requirements behind `options.without`, the tool owners, and the `present` and `tools` mappings a template reads. A class-as-a-function whose input is `without`. |
| `pkl/Steps.pkl`, `pkl/Jobs.pkl`, `pkl/Workflows.pkl`, `pkl/GenFiles.pkl` | One module each, named in the plural and holding the singular class (`Steps.Step`, `Jobs.Job`, `Workflows.Workflow`, `GenFiles.GenFile`), as a class-as-a-function: the typed keys are the inputs, `fixed` properties and methods are the outputs (`rendered`, `pins`, `amendStep`, `amendJob`, `entry`, `withContext`). `import "Steps.pkl"` and `new Steps.Step { }`. |

The folder is named after the builtin's key in `standardWorkflows` (`rust-ci`, `conventional-commits`), or after the tool for a builtin that renders only files (`rumdl`, `claude-settings`, `gitignore`). The aggregators in `Builtins.pkl` spread each folder's slice in a fixed order, which is the order the files and keys render in. `options.without` leaves builtins out: each aggregator builds a `Present` value from it and skips a builtin that is left out ([adopting.md](adopting.md#leaving-builtins-out-optionswithout)). A module never calls back into `Builtins.pkl`: when a builtin needs something from another (`dependabot` needs the workflow names), the facade passes it in as an argument.

## Wave A: the workflows that are identical in all four repositories

| Builtin | Renders | Config file it uses |
|---|---|---|
| `Builtins.plumber()` | `.github/workflows/plumber.yml` | `.plumber.yaml` |
| `Builtins.syncLabels()` | `.github/workflows/sync-labels.yml` | `.github/labels.txt` |
| `Builtins.prAutoRebase(options)` | `.github/workflows/pr-auto-rebase.yml` | none |
| `Builtins.releaseAutomerge(options)` | `.github/workflows/release-automerge.yml` | none |
| `Builtins.commitTypes()` | `.github/workflows/commit-types.yml` | `.github/commit-types.txt` |
| `Builtins.conventionalCommits(options)` | `.github/workflows/conventional-commits.yml` | none |

`Builtins.workflowFiles(options)` renders the four config files and `.github/pull_request_template.md`, byte for byte from `pantheon-adr` at `1e2fd46`. The scripts these workflows call stay in `Builtins.toolingFiles()` (phase 3).

Each workflow is compared with the pinned `pantheon-adr` copy under the P.1 rule, job ids and job names are compared (the required status checks depend on them), and `actionlint` runs over the render.

### What the typed model gained

- `Step.continue-on-error`, used by the Plumber action step.
- `Workflow.onComment`, a comment on the `on:` line. `conventional-commits.yml` needs it when `options.conventionalCommitsEvent` is `pull_request_target`: `on: # zizmor: ignore[dangerous-triggers] ...` is a comment that zizmor itself reads. With the default, `pull_request`, there is no comment. The renderer fails if the comment is set and there is no `on:` line to put it on.
- `Options.appClientId`, the client id of the GitHub App that `pr-auto-rebase` and `release-automerge` mint their token from. A public identifier, not a secret. The default is the org's `pantheon-ai-bot` App, as the four repositories use it today.
- Two pins in `Pins.pkl`: `actions/create-github-app-token` v3.3.0 and `getplumber/plumber` v0.7.16, each the commit of a release tag (checked against GitHub).

### Comments that a tool acts on

P.1 ignores comments, so a render that dropped `# zizmor: ignore[...]` would pass P.1 and then fail zizmor in the real repository. `compare_directives` in `crates/scaffold-core/src/equality.rs` compares those comments as a second check next to P.1, and every wave A workflow passes both. A `zizmor` comment inside a `run:` script is part of the script and is covered by the byte comparison. Other comments (explanations, section labels) are lost in the render, which P.1 allows. The top comment block of each workflow is kept as the `header`.

### Rendered text differs from the files in two harmless ways

`YamlRenderer` writes the key `on` quoted (`'on':`), because `on` is a YAML 1.1 boolean, and it drops the comments that sit between keys. Both parse to the same data, and GitHub accepts the quoted key.

### The GitHub App already exists

`pr-auto-rebase.yml` and `release-automerge.yml` already mint a token from a GitHub App, `pantheon-ai-bot` (the client id is in the workflow). So an App exists in the organisation. Whether it is the App the plan's P.4 asks for is for the owner to decide.

### Not in wave A

`.github/actionlint.yaml` is in three of the four repositories but not the library repository. It only ignores findings in the `cargo-dist`-generated `release.yml`, so it belongs with the `binary` toggle (phase 5).

A note in the `plumber.yml` header: since 0.1.4 it ends with "Generated by pantheon-scaffold from pantheon.pkl: change it there, not in this file." It is the one canonical change of the Phase 7 benefit test (task 7.6): it is in no repository's copy, reaches all four as a comment-only difference, and is the only workflow with it.

## Wave B: the workflows whose differences are decided

Decision 74 settled how the four repositories differ, so these builtins take options and render each repository's own file.

| Builtin | Renders | What varies |
|---|---|---|
| `Builtins.releasePlease(options)` | `release-please.yml` | `binary`: with a binary it also checks out and runs `tag-release.sh`. `releaseShape` selects the header; `draft` also renders `publish-release.sh`; the seeds `release-please-config.json` and `.release-please-manifest.json` are rendered once (`seedReleaseConfig`). `crateName` appears in the header and in the cargo-dist seed. |
| `Builtins.releaseBump(options)` | `release-bump.yml` | only with `versionFile` (and `releaseShape = "draft"`): sets the version in that file to the next patch after a verified release, through a pull request the App token opens and merges. Its script `release-bump.sh` is rendered from a template. |
| `Builtins.zizmor(options)` | `zizmor.yml` | `binary`: reads `.github/zizmor.yml`. `compositeAction`: also scans `action.yml`. The command in the header follows both. |
| `Builtins.agnix(options)` | `agnix.yml` | `skills`: the first sentence of the header. |
| `Builtins.aislop()` | `aislop.yml` | nothing |
| `Builtins.rustCi(options)` | `rust-ci.yml` | `features = "all"`: `--all-features` in clippy, build and test, and an extra build with the default features. |
| `Builtins.analysisFiles(options)` | `.agnix.toml`, `.github/dependabot.yml`, `.github/zizmor.yml` | `skills` (first line of `.agnix.toml`), `gitDependencyIgnore` (the `ignore` list), `binary` and `compositeAction` (which zizmor config, or none). |

### Options added to the typed model

`binary`, `skills`, `compositeAction`, `crateName` and `gitDependencyIgnore`. They are only what these builtins need to render the four repositories. Phase 5 turns the first three into the documented toggles and tests each one on and off.

### One place for the versions

`agnix.yml` pins agnix's version and SHA-256, `zizmor.yml` pins zizmor's version and `aislop.yml` pins aislop's, and `mise.toml` pins agnix and zizmor too, while their builtins are present. They now all come from `pkl/Tools.pkl`, which ends the "keep in sync with the workflow" comments the repositories carried. The SHA-256 belongs to the agnix version, so they change together.

### How it is checked

Three renders stand for the three kinds of repository, each compared with that repository's own pinned file under P.1, with the job ids and names and the comments a tool acts on checked too:

- like `pantheon-adr`: a binary, a skill, the crate `adr`, git dependencies to ignore
- like the library repository (`pantheon-agentic-harness-dev-kit`): no binary, no skill, all features
- like `pantheon-skill-auditor`: a composite action (its `zizmor.yml` and `.github/zizmor.yml`)

The same `adr`-pinned file is also the file of `pantheon-journal` and `pantheon-skill-auditor` for `agnix`, `aislop` and `rust-ci`. `release-please.yml` differs between the three repositories with a binary only in a comment that names the crate.

`actionlint` is run over the rendered workflows of the `adr`-like and the library-like render. Five new pins are each the commit of a release tag.

### Comment differences to expect

P.1 ignores comments. The header of each workflow is kept, and comments between keys are lost. The headers that depend on an option are built so that they never describe a file the repository does not have (the zizmor command, the agnix lead sentence). The wording of a header can still differ from a repository's own, and the real-repository render lists those.

### Built in Phase 5

Decision 74 settles how four more files are handled: `.gitignore` (the `target/` pick), `.coderabbit.yaml` (the skill folder is a parameter), `SECURITY.md` (seeded once) and `tag-release.sh` (the crate name is a parameter, and the `binary` variant of `release-please.yml` calls it). Phase 5 built them, with `audit-skill.yml` and `release.yml`: see [feature-toggles.md](feature-toggles.md).

The `pr-agent` Builtin is the review bot of a repository that does not choose CodeRabbit (`options.reviewBot`, `options.prAgentProviders`): `pr-agent.yml`, a docker step pinned by digest, and `.pr_agent.toml`. It has no upstream copy to compare with, so it is checked by structure tests (`crates/scaffold-package-tests/tests/review_bot.rs`). Its live behaviour is **unverified** until a pull request has run it with a key set; see [feature-toggles.md](feature-toggles.md#the-review-bot).

## The merged analysis workflow

By default every builtin is one workflow file. `options.workflowLayout` changes that for a group of builtins, by recipe name. The only recipe is `analysis`: with `workflowLayout { ["analysis"] = "merged" }` the package renders `analysis.yml`, one workflow with a job for each of zizmor, agnix, aislop and plumber, in place of `zizmor.yml`, `agnix.yml`, `aislop.yml` and `plumber.yml`. Nothing changes for a repository that does not set it. The code is `pkl/Builtins/Recipes.pkl`, an internal module: a recipe is not a builtin, so it has no name in `options.without`.

| What | In the merged workflow |
|---|---|
| Switching a tool off | `options.without`, by the tool's name (`zizmor`, `agnix`, `aislop`, `plumber`). The job goes, and the tool's config and script files follow the tool's own presence, as before. With all four left out no `analysis.yml` is rendered. |
| Job ids and names | The same as the separate workflows (`zizmor`, `agnix`, `quality-gate` named `aislop quality gate`, `plumber` named `Plumber Score`), so the names of the required status checks do not change. The workflow is named `analysis`. |
| Permissions | The workflow holds `contents: read` only. The write scopes stay on the jobs that need them: agnix (`issues`, `pull-requests`) and plumber (`security-events`). `env` that agnix set on its workflow is on its job. |
| Schedule | Plumber's weekly schedule is on the workflow. Every other job has `github.event_name != 'schedule'`, joined to its own condition in brackets (agnix: `(github.event_name != 'schedule') && (<its fork guard>)`), so only plumber runs on the schedule. Without plumber there is no schedule and no guard. |
| Concurrency | A pull request cancels its own older run. Every other run has its own group (`github.run_id`), so a push to main or a schedule run is never replaced by a newer one. This differs from the separate workflows, where aislop, zizmor and plumber cancel an older run on a push too. |
| Dependabot | `dependabot.yml` lists `analysis.yml` in `exclude-paths`. A tool left out with `without` counts as a workflow the repository may keep itself, as in the separate layout. |
| Presets | The four workflow keys are not in the mapping; amend a job of `analysis` ([presets.md](presets.md#rules-and-limits)). |
| Switching | Changing the layout and rendering deletes the files that drop out and writes the new one, under the same rules as `without` (a file edited by hand is not deleted). |

How it is checked: `features/analysis.feature` (the behaviour through the runner) and `crates/scaffold-package-tests/tests/analysis_workflow.rs` (the merged jobs against the four separate workflows: ids, names, steps, permissions, `if` strings; `actionlint`; Dependabot; `Shared.compose`). Merged mode has no row in the baseline matrix, so the 61-configuration baseline shows that the default is unchanged and nothing about the merged render.

## Wave C: keeping pins current, and proposing upgrades

Wave C is new work with no upstream copy, so it is checked by structure tests (`crates/scaffold-package-tests/tests/workflows_c.rs` and `update_pins_workflow.rs`, and the `update_pins` tests of `scaffold-release`) rather than by P.1 equality. The App token step of `update-pins.yml` has run successfully (the weekly run on 09-11-2026 included). Opening a pin pull request was exercised by the real run of 16-11-2026 (#160, which needed a hand-made render commit and a baseline refresh; #162 removes both), and `upgrade-scaffold.yml` has never run in a consumer repository: that one stays **unverified**.

### In the package repository

`.github/workflows/update-pins.yml` runs weekly (Monday 04:23 UTC) and by hand. It has no push trigger: it used to run when the workflow or its tool changed on `main`, and was removed when the tool moved to Rust (Phase 3 of [ts-tooling-rust-port.md](ts-tooling-rust-port.md)) so that merging that change could not start a real run with the App token. The first real run of the Rust tool was the schedule of 16-11-2026. Restoring the trigger is a later change, and it must also change the test that asserts there is none.

The workflow has three jobs, so that nothing that runs a tool or pkl sits next to the write token (#162):

1. `tool` builds `scaffold-release` and the runner (`pantheon-scaffold`) with no token, and uploads them with their SHA-256.
2. `pins` holds the read-only token only. It checks both binaries, runs `scaffold-release update-pins` (`crates/scaffold-release`) which checks every pin in `pkl/Pins.pkl` against GitHub (the tag is a published release, the commit belongs to the repository) and rewrites the ones that moved to the latest release (any failed check exits 1 and nothing crosses to the next job, the freshness record included), and, when `pkl/Pins.pkl` changed, runs `pantheon-scaffold render` and `check` so that the rendered workflows carry the new pins. `.github/scripts/collect-pin-update.sh` packs the changed files, `freshness.json` and `summary.md` into one tar, whose SHA-256 is a job output.
3. `update` holds the write token, minted from the GitHub App with `contents`, `pull-requests` and `workflows` write (the rendered files are workflow files, which GitHub refuses to push without `workflows`; no other workflow asks for it, and a test checks that every mint step names its permissions). The App secret is in the Environment `update-pins`, which only `main` can use, so a run from another branch cannot read it. The job runs no tool, no pkl and no cargo. It takes `.github/scripts/apply-pin-update.sh` from its own checkout of `main`, never from the tar, and the script treats the tar as untrusted: it checks the digest, the member types and names, that the files are those listed, that each path is `pkl/Pins.pkl`, the manifest or a workflow that `main`'s manifest lists as owned, and the content against `main` (workflows equal `main`'s with the pins masked, every commit is in the new `Pins.pkl` or was in `main`'s file, `Pins.pkl` changes only shas and versions, the manifest only hashes, the record has its shape). A refusal stops the run: no pull request and no freshness record. Then `.github/scripts/open-pin-pull-request.sh` commits the applied files to `chore/update-pins` and opens one pull request, or leaves the open one in place (a branch with a commit by someone else is left as it is, with a notice), and the job writes `freshness.json` to the `freshness` branch as a single orphan commit (decision 83). The record is written even when no pin moved, because a clean check is a successful check.

A manual run is a dry run unless told otherwise: `pins` runs in full and a `check` job runs the apply script in check-only mode with no token, so the whole path runs on a real runner and nothing is pushed. The pull request passes its checks with no commit from a person: the rendered files come with it, and the frozen baseline ignores pinned commits (`baseline snapshot` masks them, #166).

The tests do not assert the exact pinned versions, so a pin pull request can pass CI on its own: P.1 comparisons with the pinned upstream copies replace each action commit with a marker (`unpin` in `crates/scaffold-package-tests/src/pkg.rs`), and the pin lists are checked by shape (`expect_pins`: the right actions, each a 40 character commit with a version tag). That each commit belongs to a release tag is checked by `update-pins.ts`, not by the tests.

A pin pull request changes `pkl/`, so while the version in `pkl/PklProject` is already published it also needs a version bump (`pkl project package` refuses a changed package at a published version). The workflow does not bump it.

### In a consumer repository

`Builtins.upgradeWorkflow(options)` renders `.github/workflows/upgrade-scaffold.yml`, weekly (Monday 05:17 UTC) and by hand:

1. `pantheon-scaffold freshness` fails the run when the record is older than 30 days or cannot be read.
2. `pantheon-scaffold latest` finds the newest release the known-bad list allows. If the repository is already on it, nothing else runs.
3. `pantheon-scaffold upgrade <version> --config scaffold.pkl` verifies the release, rewrites the pins and renders.
4. The workflow pushes `chore/upgrade-pantheon-scaffold-<version>` and opens a pull request with the App token.

The App token asks for `contents`, `pull-requests` and `workflows` write. `workflows: write` is needed because an upgrade can change files under `.github/workflows`; whether the `pantheon-ai-bot` App has it is not confirmed (plan P.4). The file is marked `removeOnDetach`, so `pantheon-scaffold detach` deletes it: once the repository leaves the scaffold the workflow would fail on every run. To support that, `Workflow` has a new `removeOnDetach` option. Each `run:` block is 15 lines or fewer, and every value that comes from an expression goes through `env`.

### How it is checked

Mutation check: `archive/spikes/phase-4-wave-c/mutate.ts` (20 properties, all caught). Both workflows pass `actionlint`.

## Wave D: the real repositories

All thirteen workflow builtins render for each of the four real repositories, and each rendered file is equal to the repository's own under P.1, with the same job ids and names and the same directive comments, and passes `actionlint`. The per-file result, including the comment-only differences, is in [real-repo-workflows.md](../archive/proof/docs/real-repo-workflows.md).
