Feature: Presets
  A Preset is a pkl file in the repository's presets/ directory that takes a Builtin and returns a changed one
  (docs/presets-design.md). The scenarios start from a small project in features/projects, copied into an empty
  repository. The two `upgrade` scenarios of the design record (a compatible Preset, an incompatible one) cannot be
  written black-box, because they need a published package with a renamed job; they are integration tests of task 4.3, in
  crates/scaffold-cli/tests/upgrade.rs, which has the fake gh and mise.

  Scenario: a Preset overrides one field of a Builtin and nothing else changes
    Given a new empty repository
    And the project "preset-override"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/rust-ci.yml" contains "timeout-minutes: 30"
    And the file ".github/workflows/rust-ci.yml" contains "name: Rust CI"
    And the file ".github/workflows/rust-ci.yml" contains "cargo clippy"

  Scenario: a Preset replaces the steps of a Builtin's job
    Given a new empty repository
    And the project "preset-replace-steps"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/rust-ci.yml" contains "Only step"
    And the file ".github/workflows/rust-ci.yml" contains "echo only"
    And the file ".github/workflows/rust-ci.yml" does not contain "cargo clippy"

  Scenario: a Preset adds a workflow that is not a Builtin
    Given a new empty repository
    And the project "preset-add"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/nightly.yml" contains "name: Nightly"
    And the file ".github/workflows/nightly.yml" contains "echo hello"

  Scenario: a Preset file edited by hand changes the render, so check reports drift until the next render
    Given a new empty repository
    And the project "preset-override"
    And I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    And I write "function rustCi(base) = base\n" to the file "presets/ci.pkl"
    When I run "check <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 4

  Scenario: detach after a Preset keeps the rendered files and leaves presets/ alone
    Given a new git repository
    And the project "preset-add"
    And I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    And I commit everything
    When I run "detach --root <repo>"
    Then the exit code is 0
    And the file ".github/workflows/nightly.yml" exists
    And the file "presets/extra.pkl" exists
    And the file ".pkl-gen/manifest.json" does not exist

  Scenario: the helpers amend a workflow and one of its jobs, and the config names the Preset it applies
    Given a new empty repository
    And the project "preset-helpers"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/rust-ci.yml" contains "timeout-minutes: 30"
    And the file ".github/workflows/cargo-deny.yml" exists

  Scenario: a Preset that names a job the workflow does not have fails and says which jobs exist
    Given a new empty repository
    And the project "preset-missing-job"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains 'preset: job "build" does not exist in workflow "Rust CI". Jobs: build-test'
    And the file ".github/workflows/rust-ci.yml" does not exist

  Scenario: a config that lists a Preset outside presets/ is refused by the contract
    Given a new empty repository
    And the project "preset-bad-path"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "#/presets/0: does not match"
    And the file ".pkl-gen/manifest.json" does not exist

  Scenario: a Preset that names a workflow the Builtins do not have fails and writes nothing
    Given a new empty repository
    And the project "preset-missing"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains 'preset: workflow "rust-cii" is not a builtin of this package version'
    And the error contains "rust-ci"
    And the file ".github/workflows/rust-ci.yml" does not exist
    And the file ".pkl-gen/manifest.json" does not exist

  Scenario: the file helper adds to a rendered file
    Given a new empty repository
    And the project "preset-file"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".gitignore" contains "my-own-entry/"
    And the file ".gitignore" contains "target/"

  Scenario: a Preset that names a file the Builtins do not render fails and says which files exist
    Given a new empty repository
    And the project "preset-missing-file"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains 'preset: file ".gitignorr" is not rendered by the builtins of this package version. Files: '
    And the error contains ".gitignore"

  Scenario: the step helpers add a step before a named step and change a named step
    Given a new empty repository
    And the project "preset-steps"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/rust-ci.yml" contains "name: Set up pkl"
    And the file ".github/workflows/rust-ci.yml" contains "run: echo replaced"
    And the file ".github/workflows/rust-ci.yml" does not contain "cargo test --workspace"
    And the file ".github/workflows/rust-ci.yml" contains "cargo clippy"

  Scenario: a Preset that names a step the job does not have fails and says which steps exist
    Given a new empty repository
    And the project "preset-missing-step"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains 'preset: step "Tset" does not exist in job "Build, test, lint". Steps: '
    And the error contains "Test"

  @rust-only
  Scenario: the manifest records every Preset as an input
    Given a new empty repository
    And the project "preset-override"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".pkl-gen/manifest.json" contains '"presets"'
    And the file ".pkl-gen/manifest.json" contains "presets/ci.pkl"

  @rust-only
  Scenario: check lists the Presets in use and warns about one that nothing applies
    Given a new empty repository
    And the project "preset-unused"
    And I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    When I run "check <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the output contains "preset presets/ci.pkl"
    And the error contains "presets/forgotten.pkl is not applied by pantheon.pkl"

  @rust-only
  Scenario: a Preset that the config lists but that is not there stops the render before anything is written
    Given a new empty repository
    And the project "preset-listed-missing"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "preset presets/ghost.pkl is listed by pantheon.pkl but the file does not exist"
    And the file ".pkl-gen/manifest.json" does not exist

  @rust-only
  Scenario: check warns about a Preset that changed since the last render, even when the files it renders are the same
    Given a new empty repository
    And the project "preset-unused"
    And I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    And I write "// edited\nfunction rustCi(base) = base\n" to the file "presets/ci.pkl"
    When I run "check <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the error contains "presets/ci.pkl changed since the last render"
