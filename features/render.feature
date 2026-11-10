Feature: render, check and detach
  The runner renders the files of a contract into a repository, keeps a manifest of what it wrote, and refuses to
  overwrite what a person edited.

  Scenario: render writes the files of the contract
    Given a new empty repository
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file "rust-toolchain.toml" exists
    And the file ".pkl-gen/manifest.json" exists

  Scenario: the executable bit is set from the contract
    Given a new empty repository
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the file ".github/scripts/hello.sh" has the mode "755"

  Scenario: a second render writes and deletes nothing
    Given a new empty repository
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the output contains "written 0 file(s), deleted 0"

  Scenario: check finds no drift after a render
    Given a new empty repository
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    When I run "check <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 0

  Scenario: check reports a hand edit as drift
    Given a new empty repository
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    And I write "# mine\n" to the file "rust-toolchain.toml"
    When I run "check <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 4

  Scenario: render refuses to overwrite a hand edit and leaves it
    Given a new empty repository
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    And I write "# mine\n" to the file "rust-toolchain.toml"
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 3
    And the file "rust-toolchain.toml" has the content "# mine\n"

  Scenario: --force overwrites a hand edit
    Given a new empty repository
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    And I write "# mine\n" to the file "rust-toolchain.toml"
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package> --force"
    Then the exit code is 0
    And the file "rust-toolchain.toml" does not have the content "# mine\n"

  Scenario: detach removes the manifest directory and leaves the files
    Given a new empty repository
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    When I run "detach --root <repo>"
    Then the exit code is 0
    And the file ".pkl-gen" does not exist
    And the file "rust-toolchain.toml" exists
