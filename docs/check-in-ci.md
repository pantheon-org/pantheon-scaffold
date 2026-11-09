# Running `check` in a consumer repository's CI

`pantheon-scaffold check pantheon.pkl` renders in memory and compares the result with the owned files on disk. It writes nothing and exits 4 when a file differs or is missing. A repository enforces its scaffold by running it on pull requests.

The sample job is `runner/test/fixtures/consumer-check.yml`. A test asserts the properties that make it safe to run on code from forks:

- It runs on `pull_request`, never `pull_request_target`, because it evaluates a `pantheon.pkl` that a fork can change.
- It uses no repository or organisation secrets. The only token is the built-in `GITHUB_TOKEN` with `contents: read`, which mise uses to avoid download rate limits.
- Every action is pinned by SHA, and the tools come from the committed lock file with `mise install --locked`.
- pkl runs with restricted modules and resources, so the evaluated file cannot read arbitrary files.

When the package or a pin cannot be fetched, `check` retries (3 attempts by default, `--attempts`) and then fails closed with exit 1. It never reports a pass it could not verify. A newly created release can answer 404 for several minutes, which is why a fetch failure is retried.

The sample job keeps `.pkl-gen/cache` with `actions/cache` (pinned by SHA), so a warm cache lets `check` render when GitHub is unreachable. The `.gitignore` that the scaffold renders already ignores `.pkl-gen/cache/`; a repository that does not use it adds that line itself. The runner also warns when the package version is on the known-bad list, and `--strict` turns that warning into exit 3. See [runner-lifecycle.md](runner-lifecycle.md).

Detection only covers repositories that run this job. A repository that never adds it, or edits the workflow, is not checked (decision 62 of the plan).
