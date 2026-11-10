# Security dossier of the v0.1.5 release candidate

Phase 7, wave E (task 7.5). Generated on 09-11-2026 by `runner/scripts/proof-dossier.ts` from the **published** package and assets of v0.1.5 (the checksums file's attestation and every asset's digest are verified first). Nothing is written to the four repositories. The repository `pantheon-org/pantheon-scaffold` is **public**, so this file is public: it holds counts, file names and domains, never a matched secret, an e-mail address or a user name.

## Scope and what is not covered

- **Security approval is not obtained.** No Security reviewer is named and none was asked (decision 78, an accepted risk of the owner, 07-11-2026). This dossier is input for that review. It does not replace it.
- **The reviewer is the maintainer.** The author of the package, the owner of the four repositories and the person who reviews this dossier are the same person (decisions 69, 70 and 72), so the review and the sample check are not independent. A second reviewer is not named.
- **Covered:** the 16 workflow files that the package renders for the four repositories, and the 8 assets and the release notes of v0.1.5.
- **Not covered:** the content of `release.yml` (see below), anything in the four repositories that the package does not render, the GitHub organisation's settings, and the App `pantheon-ai-bot` itself.

## Every rendered workflow

Rendered with the published runner's package for each repository (options as in the wave A report), parsed with Bun.YAML. Columns: triggers; permissions (top level, then each job that sets its own); secrets read; tokens (`GITHUB_TOKEN` or an App token and its permissions); third-party and first-party actions and how many are pinned to a full commit; `${{ }}` expressions inside `run:` that read event data (untrusted input). A row repeated for several repositories is shown once.

| Workflow | Triggers | Permissions | Secrets | Tokens | Actions pinned | Untrusted input in run | Repositories |
|---|---|---|---|---|---|---|---|
| agnix.yml | pull_request, push | top contents:read issues:write pull-requests:write | none | GITHUB_TOKEN | 4 of 4 | inline: none; via env: `github.event_name == 'push' && 'main' || 'pr'`, `github.event.pull_request.number` | all four |
| aislop.yml | pull_request, push | top contents:read | none | none | 2 of 2 | inline: none; via env: none | all four |
| audit-skill.yml | pull_request, push | top contents:read | none | none | 2 of 2 | inline: none; via env: none | adr, journal |
| audit-skill.yml | pull_request, push | top contents:read | none | none | 1 of 1 | inline: none; via env: none | skill-auditor |
| cargo-deny.yml | pull_request, push, schedule, workflow_dispatch | top contents:read | none | none | 1 of 1 | inline: none; via env: none | all four |
| commit-types.yml | pull_request, push | top contents:read | none | none | 1 of 1 | inline: none; via env: none | all four |
| conventional-commits.yml | pull_request_target | top none | none | none | 0 of 0 | inline: none; via env: `github.event.pull_request.title` | all four |
| dist-check.yml | pull_request, push, workflow_dispatch | top contents:read | none | none | 2 of 2 | inline: none; via env: none | adr, journal, skill-auditor |
| inline-scripts.yml | pull_request, push | top contents:read | none | none | 1 of 1 | inline: none; via env: none | all four |
| plumber.yml | pull_request, push, schedule | top contents:read; jobs plumber=contents:read security-events:write | none | none | 2 of 2 | inline: none; via env: none | all four |
| pr-auto-rebase.yml | push, workflow_dispatch | top contents:read | GH_APP_PRIVATE_KEY | GITHUB_TOKEN; App token (contents:write pull-requests:write) | 2 of 2 | inline: none; via env: none | all four |
| release-automerge.yml | schedule, workflow_dispatch | top contents:read | GH_APP_PRIVATE_KEY | App token (contents:write pull-requests:write) | 2 of 2 | inline: none; via env: none | all four |
| release-please.yml | push, workflow_dispatch | top contents:read | GH_APP_PRIVATE_KEY | GITHUB_TOKEN; App token (contents:write pull-requests:write) | 3 of 3 | inline: none; via env: none | adr, journal, skill-auditor |
| release-please.yml | push, workflow_dispatch | top contents:read | GH_APP_PRIVATE_KEY | GITHUB_TOKEN; App token (contents:write pull-requests:write) | 2 of 2 | inline: none; via env: none | agentic-harness-dev-kit |
| rust-ci.yml | pull_request, push | top contents:read | none | none | 1 of 1 | inline: none; via env: none | all four |
| scaffold-check.yml | pull_request | top none; jobs check=contents:read | none | GITHUB_TOKEN | 3 of 3 | inline: none; via env: none | all four |
| sync-labels.yml | push, workflow_dispatch | top contents:read issues:write | none | GITHUB_TOKEN | 1 of 1 | inline: none; via env: none | all four |
| zizmor.yml | pull_request, push | top contents:read | none | none | 2 of 2 | inline: none; via env: none | all four |

Review items from the table:

- conventional-commits.yml uses `pull_request_target` (permissions top none; 1 step(s), 1 run step(s), checkout no, event data inline in a run: no; event data reaches the script through: inline: none; via env: `github.event.pull_request.title`). It runs with base-repository context on a pull request, so it must never check out or run pull-request code, and event data must reach a script only through an environment variable, never inline in the script text. The reviewer checks the line above against that rule.

## The `release.yml` that `dist generate` produces

The package does **not** render `release.yml`. `cargo-dist` generates it in each repository that ships a binary, and the package renders two things around it: `dist-workspace.toml` (with the two action commits that the generated file uses today, decision 85) and `dist-check.yml`, which runs `dist generate --check` and fails when the committed `release.yml` differs from what `dist` would generate. So this dossier covers the **check** and not the **content**: what `release.yml` is allowed to do (its permissions, its tokens, its tag trigger) is reviewed in each repository, and is outside this review. The library repository renders no `dist-check.yml` (`binary = false`).

## Token and App key custody

What the files show: the workflows `pr-auto-rebase.yml`, `release-automerge.yml` and `release-please.yml` read the secret `GH_APP_PRIVATE_KEY` and mint a short-lived installation token for the App `pantheon-ai-bot` (its client id is written in the file: a public identifier). Each asks for contents and pull-requests write on the repository it runs in. The token step runs before every other step, and a key that is not valid stops the job there (shown in the wave C run).

What is **not** in the files, and that the review token cannot read (organisation and App settings): the App's permission scope and installation list, who holds the key and where, rotation, who can read the secret, and whether the custody is written down. Six questions about these were put to the owner and answered on 09-11-2026.

The answers are **not recorded in this repository**, which is public: they describe how a secret is held and who can reach it. They are held in the owner's private record, together with an analysis of what the key could do if it were misused and the gaps that analysis found. This dossier therefore does not claim that the custody is verified. It records that the questions were asked and answered, that the answers are held privately, and that the private record lists open items.

## What the public release reveals

Scanned: `pantheon-scaffold-darwin-arm64`, `pantheon-scaffold-linux-arm64`, `pantheon-scaffold-linux-x64`, `pantheon-scaffold@0.1.5`, `pantheon-scaffold@0.1.5.zip`, the release notes, and the files inside `pantheon-scaffold@0.1.5.zip`. A match is counted and located, never printed.

| Pattern | Matches | Where |
|---|---|---|
| GitHub token prefixes (ghp_, gho_, ghu_, ghs_, ghr_, github_pat_) | 0 | none |
| AWS access key ids | 0 | none |
| Slack tokens | 0 | none |
| Private key blocks | 0 | none |
| sk- style keys | 0 | none |
| E-mail addresses (domains only) | 28 | pantheon-scaffold@0.1.5 (1), pantheon-scaffold-darwin-arm64 (8), pantheon-scaffold-linux-x64 (10), pantheon-scaffold-linux-arm64 (9) |
| Home-directory paths (/Users/, /home/) | 0 | none |
| Internal host names (.internal, .corp, .lan) | 3 | pantheon-scaffold-darwin-arm64 (1), pantheon-scaffold-linux-x64 (1), pantheon-scaffold-linux-arm64 (1) |
| Organisation or company words (postcodelottery, novamedia) | 0 | none |

Notes:

- E-mail domains seen: openssl.org (24), izenpe.com (3), users.noreply.github.com (1). Most of the matches sit in the three compiled runners, which bundle the bun runtime and third-party code (certificate and package metadata), so they are not authored here. Whether an address belongs to a person is for the owner to check in the build inputs.
- The `authors` field of the package metadata (set in `pkl/PklProject`) holds the maintainer's GitHub noreply address, which names a GitHub account. The owner confirms that this public attribution is intended.
- Internal-style host names seen (not a secret, listed so the owner can judge them): `property.internal`.

Token and key patterns are the GitHub token prefixes, `github_pat_`, AWS access key ids, Slack tokens, `-----BEGIN ... PRIVATE KEY-----` and `sk-` keys. A non-zero count there is an incident for the owner's internal route and not a line in this file.

## Findings carried from the earlier waves

1. **Organisation secrets are visible to a new repository** (wave C). The names are deliberately not recorded in this repository, which is public. The owner reviews the organisation setting.
2. **`[skip ci]` does not stop Dependabot** (wave C): the rendered `dependabot.yml` starts Dependabot updates on the first push of a new repository.
3. **`rumdl` fails in all four repositories today** (decision 80): fix or exempt each before `hk check --all` is enforced in CI.
4. **`sync-labels.sh` becomes executable** in each adopting repository (decision 79).
5. **`upgrade` bugs found and fixed in Phase 7** (decisions 88 and the `mise.toml` seal): the published 0.1.3 and 0.1.4 runners still have them. 0.1.5 is the candidate.

## Differences by repository

- pantheon-adr: 23 identical, 24 differences
- pantheon-journal: 23 identical, 24 differences
- pantheon-skill-auditor: 22 identical, 25 differences
- pantheon-agentic-harness-dev-kit: 20 identical, 21 differences
