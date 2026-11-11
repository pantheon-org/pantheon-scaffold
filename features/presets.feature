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

  @pending-presets-api
  Scenario: a Preset that names a workflow the Builtins do not have fails and writes nothing
    Given a new empty repository
    And the project "preset-missing"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "preset: workflow \"rust-cii\" is not a builtin of this package version"
    And the error contains "rust-ci"
    And the file ".github/workflows/rust-ci.yml" does not exist
    And the file ".pkl-gen/manifest.json" does not exist

  @pending-presets-runner
  Scenario: the manifest records every Preset as an input
    Given a new empty repository
    And the project "preset-override"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".pkl-gen/manifest.json" contains "\"presets\""
    And the file ".pkl-gen/manifest.json" contains "presets/ci.pkl"

  @pending-presets-runner
  Scenario: check lists the Presets in use and warns about one that nothing applies
    Given a new empty repository
    And the project "preset-unused"
    And I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    When I run "check <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the output contains "preset presets/ci.pkl"
    And the error contains "presets/forgotten.pkl is not applied by pantheon.pkl"
