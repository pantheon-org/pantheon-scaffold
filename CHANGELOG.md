# Changelog

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
