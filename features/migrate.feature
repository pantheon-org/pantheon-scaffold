@rust-only
Feature: migrate
  `migrate` brings an existing repository under the scaffold. It renders the Builtins in memory for the options it is given
  (the flags of `init`) and compares each rendered file with the file already in the repository. It is a dry run unless given
  --write, and it never reads a file it does not render.

  Each file gets one class, printed as the class padded to ten columns, then the path:

    identical   same as the Builtin. A YAML file (.yml, .yaml) is compared by the YAML equality rule of `check`, so quoting,
                comments and layout do not count. Any other file must match byte for byte.
    drifted     the path is a Builtin's and the content differs. The differences follow, one per line.
    new         a Builtin file the repository does not have.
    per-repo    a file the Builtins do not render, in .github/ or at the top of the repository. Files elsewhere (src/, docs/)
                are not listed.

  They were written ahead of the command (plan task 5.2) and pass since task 5.3. The TypeScript runner has no migrate.

  Scenario: the dry run classifies every file and writes nothing
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    Then the exit code is 0
    And the output contains "identical  rust-toolchain.toml"
    And the output contains "identical  .github/workflows/sync-labels.yml"
    And the output contains "drifted    .github/workflows/aislop.yml"
    And the output contains "new        mise.toml"
    And the output contains "per-repo   README.md"
    And the output contains "per-repo   .github/workflows/ci.yml"
    And the output contains "2 identical, 1 drifted, "
    And the output contains "Nothing was written."
    And the file "pantheon.pkl" does not exist
    And the file "mise.toml" does not exist
    And the file ".pkl-gen/manifest.json" does not exist

  Scenario: a workflow that differs only in quoting, comments and layout is identical
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    Then the output contains "identical  .github/workflows/sync-labels.yml"
    And the output does not contain "drifted    .github/workflows/sync-labels.yml"
    And the output contains "equal as YAML, different text: --write replaces it with the render's text"

  Scenario: a workflow with one changed value is drifted, and the difference is shown
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    Then the output contains "drifted    .github/workflows/aislop.yml"
    And the output contains "timeout-minutes: 10 vs 20"

  Scenario: a file that is not YAML is identical only when it matches byte for byte
    Given a new empty repository
    And I write '[toolchain]\nchannel = "1.96.1"\ncomponents = ["rustfmt", "clippy"]\n' to the file "rust-toolchain.toml"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    Then the output contains "identical  rust-toolchain.toml"
    When I write '[toolchain]\nchannel   = "1.96.1"\ncomponents = ["rustfmt", "clippy"]\n' to the file "rust-toolchain.toml"
    And I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    Then the output contains "drifted    rust-toolchain.toml"

  Scenario: a file the Builtins do not render is per-repo, and is never touched
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    Then the output contains "per-repo   README.md"
    And the file "README.md" has the content "# Example\n"
    And the file "src/main.rs" has the content "fn main() {}\n"

  Scenario: a file outside .github and the top of the repository is not listed
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    Then the output does not contain "src/main.rs"

  Scenario: a Builtin file the repository lacks is new
    Given a new empty repository
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    Then the exit code is 0
    And the output contains "new        mise.toml"
    And the output contains "0 identical, 0 drifted, "

  Scenario: an existing pantheon.pkl is refused and nothing is written
    Given a new empty repository
    And I write "mine\n" to the file "pantheon.pkl"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    Then the exit code is 3
    And the error contains "pantheon.pkl already exists"
    And the file "pantheon.pkl" has the content "mine\n"

  Scenario: migrate needs a toolchain
    Given a new empty repository
    When I run "migrate --root <repo> --allow-local-package <package>"
    Then the exit code is 2
    And the error contains "migrate needs --toolchain, for example --toolchain 1.96.1"

  Scenario: the report is the same when it is run twice
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    And I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    Then the exit code is 0
    And the output contains "2 identical, 1 drifted, "

  Scenario: --write is refused while any file is drifted, and nothing is written
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write"
    Then the exit code is 3
    And the error contains "drifted"
    And the error contains ".github/workflows/aislop.yml"
    And the file "pantheon.pkl" does not exist
    And the file "mise.toml" does not exist

  Scenario: after a migrate with --write, detach leaves every file in place and removes the manifest
    Given a new empty repository
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write"
    Then the exit code is 0
    And the file "pantheon.pkl" exists
    And the file ".pkl-gen/manifest.json" exists
    When I run "detach --root <repo>"
    Then the exit code is 0
    And the file ".pkl-gen/manifest.json" does not exist
    And the file "pantheon.pkl" exists
    And the file "mise.toml" exists

  Scenario: --write adopts an identical YAML file with the render's text, and leaves the repository's own files alone
    Given the project "migrate-adopt"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write"
    Then the exit code is 0
    And the output contains "Wrote pantheon.pkl and "
    And the output contains "Next: run mise lock, then commit."
    And the file "pantheon.pkl" exists
    And the file ".github/workflows/sync-labels.yml" contains "# Creates the labels Dependabot applies"
    And the file "README.md" has the content "# Example\n"
    When I run "check <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the output contains "written 0 file(s), deleted 0"
