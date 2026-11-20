# Changelog

## [0.4.3](https://github.com/pantheon-org/pantheon-scaffold/compare/v0.4.2...v0.4.3) (2026-11-20)


### Features

* **conventional-commits:** start the title check on pull_request, with an option for pull_request_target ([#164](https://github.com/pantheon-org/pantheon-scaffold/issues/164)) ([e0b1b1e](https://github.com/pantheon-org/pantheon-scaffold/commit/e0b1b1e561971a4c6c908323b38fe1aedf12e60f))
* **pkl:** pr-agent as the default review bot, with reviewBot and prAgentProviders options ([#171](https://github.com/pantheon-org/pantheon-scaffold/issues/171)) ([407fb1e](https://github.com/pantheon-org/pantheon-scaffold/commit/407fb1e09cd27fd60dc9cc562ab4df8afbf28205))


### Bug Fixes

* **baseline:** mask pinned actions in what a snapshot hashes, so a pin move needs no refresh ([#166](https://github.com/pantheon-org/pantheon-scaffold/issues/166)) ([1c85d20](https://github.com/pantheon-org/pantheon-scaffold/commit/1c85d204b09bfd98fad166edc12d6bb45bdb641b))

## [0.4.2](https://github.com/pantheon-org/pantheon-scaffold/compare/v0.4.1...v0.4.2) (2026-11-16)


### Features

* **release:** build the runner with scaffold-release build-rust ([#142](https://github.com/pantheon-org/pantheon-scaffold/issues/142)) ([0d499f3](https://github.com/pantheon-org/pantheon-scaffold/commit/0d499f3472c0cd52d038effa0b419b20536d1927))


### Bug Fixes

* **pkl:** trust jdx/mise-action in the Plumber configuration ([#153](https://github.com/pantheon-org/pantheon-scaffold/issues/153)) ([d77a8ee](https://github.com/pantheon-org/pantheon-scaffold/commit/d77a8eeaf27cc84fcc82ddf6903904997e3f8041))

## [0.4.1](https://github.com/pantheon-org/pantheon-scaffold/compare/v0.4.0...v0.4.1) (2026-11-16)


### Features

* **release:** run smoke and the baseline scripts from scaffold-release ([#138](https://github.com/pantheon-org/pantheon-scaffold/issues/138)) ([ade19f4](https://github.com/pantheon-org/pantheon-scaffold/commit/ade19f434375722d06c272f9af49254af6bc6327))


### Bug Fixes

* **pkl:** leave out the dependabot github-actions entry when nothing is left for it to scan ([#139](https://github.com/pantheon-org/pantheon-scaffold/issues/139)) ([cde80ff](https://github.com/pantheon-org/pantheon-scaffold/commit/cde80ff962c079b77ce4d20bb3fe900e98819ba0))

## [0.4.0](https://github.com/pantheon-org/pantheon-scaffold/compare/v0.3.0...v0.4.0) (2026-11-15)


### ⚠ BREAKING CHANGES

* **pkl:** release-automerge.yml is manual by default. The Monday merge only runs when the repository variable RELEASE_AUTOMERGE_SCHEDULE is true; set options.releaseAutomerge = "always" to keep the unconditional schedule.

### Features

* **pkl:** move the release behaviour into the Builtins ([#133](https://github.com/pantheon-org/pantheon-scaffold/issues/133)) ([46e4d47](https://github.com/pantheon-org/pantheon-scaffold/commit/46e4d47487cbbcd43f653216ed9294a1f5eecfc4))

## [0.3.0](https://github.com/pantheon-org/pantheon-scaffold/compare/v0.2.0...v0.3.0) (2026-11-15)


### ⚠ BREAKING CHANGES

* **pkl:** import the classes from the plural modules (Steps.Step, Jobs.Job, Workflows.Workflow, GenFiles.GenFile) instead of Config.Step and the like. Presets.amendJob, amendStep and insertStepsBefore are now methods on Workflow and Job. The package version is 0.3.0.

### Code Refactoring

* **pkl:** move step, job, workflow, file and presence into plural class modules ([#123](https://github.com/pantheon-org/pantheon-scaffold/issues/123)) ([8eae6ad](https://github.com/pantheon-org/pantheon-scaffold/commit/8eae6ad620251961a97350b73f9f6044ea0ab035))
