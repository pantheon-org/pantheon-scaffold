Feature: leaving builtins out
  `options.without` lists builtins to leave out by name (docs/workflow-layer.md). The aggregators in Builtins.pkl skip a left-out builtin, so the
  runner sees its files as no longer rendered. The scenarios start from a small project in features/projects, copied into a git repository: first
  with every builtin, then with one left out.

  Scenario: leaving out a builtin deletes its files on the next render
    Given a new git repository
    And the project "without-none"
    And I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    And I commit everything
    And the project "without-plumber"
    And I commit everything
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/plumber.yml" does not exist
    And the file ".plumber.yaml" does not exist
    And the file ".github/workflows/rust-ci.yml" exists

  Scenario: a file of a left-out builtin that was edited by hand is not deleted
    Given a new git repository
    And the project "without-none"
    And I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    And I write "mine\n" to the file ".plumber.yaml"
    And I commit everything
    And the project "without-plumber"
    And I commit everything
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 3
    And the error contains "it is not deleted"
    And the file ".plumber.yaml" has the content "mine\n"

  Scenario: leaving out hk deletes hk.shared.pkl and keeps the seeded hk.pkl
    Given a new git repository
    And the project "without-none"
    And I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    And I commit everything
    And the project "without-hk"
    And I commit everything
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file "hk.shared.pkl" does not exist
    And the file "hk.pkl" exists

  Scenario: leaving out a builtin that another needs fails and names both
    Given a new empty repository
    And the project "without-conventional-commits"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains 'options.without leaves out "conventional-commits", which "commit-types" needs'
    And the error contains 'Leave "commit-types" out too'
    And the file ".github/workflows/commit-types.yml" does not exist

  Scenario: a name that is not a builtin fails and lists the builtins
    Given a new empty repository
    And the project "without-typo"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains '"plumbr", which is not a builtin of this package version'
    And the error contains "plumber"
    And the file ".github/workflows/plumber.yml" does not exist
