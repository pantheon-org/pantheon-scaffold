Feature: command line
  The version, the usage rules and the contract command work from a directory unrelated to the source tree, so
  everything the binary needs is inside it.

  Scenario: --version prints the version the binary was built for
    When I run "--version"
    Then the exit code is 0
    And the output is the expected version

  Scenario: an unknown command is a usage error
    When I run "nope"
    Then the exit code is 2
    And the error contains "usage"

  Scenario: an unknown option is a usage error
    When I run "detach --nope"
    Then the exit code is 2
    And the error contains "unknown option --nope"

  Scenario: contract evaluates a config and reports what it holds
    When I run "contract <fixture:minimal> --allow-local-package <package>"
    Then the exit code is 0
    And the output contains "contract ok: 4 files"
