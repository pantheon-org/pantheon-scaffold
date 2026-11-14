Feature: pantheon.pkl was renamed to scaffold.pkl
  A repository's config file is scaffold.pkl. The old name gets one error that names the new file, and nothing is written.

  Scenario: a config path named pantheon.pkl is refused by contract
    Given a new empty repository
    And I write "x\n" to the file "pantheon.pkl"
    When I run "contract <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "pantheon.pkl was renamed to scaffold.pkl: rename the file"
    And the file ".pkl-gen/manifest.json" does not exist

  Scenario: a config path named pantheon.pkl is refused by render
    Given a new empty repository
    And I write "x\n" to the file "pantheon.pkl"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "pantheon.pkl was renamed to scaffold.pkl: rename the file"
    And the file ".pkl-gen/manifest.json" does not exist

  Scenario: a config path named pantheon.pkl is refused by check
    Given a new empty repository
    And I write "x\n" to the file "pantheon.pkl"
    When I run "check <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "pantheon.pkl was renamed to scaffold.pkl: rename the file"
    And the file ".pkl-gen/manifest.json" does not exist

  Scenario: a nested path with the old file name is refused too
    Given a new empty repository
    And I write "x\n" to the file "conf/pantheon.pkl"
    When I run "render <repo>/conf/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "pantheon.pkl was renamed to scaffold.pkl"

  Scenario: init in a root that holds only pantheon.pkl names the new file and writes nothing
    Given a new empty repository
    And I write "x\n" to the file "pantheon.pkl"
    When I run "init --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write"
    Then the exit code is 3
    And the error contains "pantheon.pkl was renamed to scaffold.pkl"
    And the file "scaffold.pkl" does not exist
    And the file "mise.toml" does not exist

  Scenario: migrate in a root that holds only pantheon.pkl names the new file and writes nothing
    Given a new empty repository
    And I write "x\n" to the file "pantheon.pkl"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write"
    Then the exit code is 3
    And the error contains "pantheon.pkl was renamed to scaffold.pkl"
    And the file "scaffold.pkl" does not exist

  Scenario: upgrade in a root that holds only pantheon.pkl names the new file
    Given a new empty repository
    And I write "x\n" to the file "pantheon.pkl"
    When I run "upgrade 0.1.9 --root <repo>"
    Then the exit code is 1
    And the error contains "pantheon.pkl was renamed to scaffold.pkl"

  Scenario: upgrade with --config pantheon.pkl names the new file
    Given a new empty repository
    And I write "x\n" to the file "pantheon.pkl"
    When I run "upgrade 0.1.9 --root <repo> --config pantheon.pkl"
    Then the exit code is 1
    And the error contains "pantheon.pkl was renamed to scaffold.pkl"

  Scenario: a root that holds both files is not told to rename, and init keeps its own refusal
    Given a new empty repository
    And I write "x\n" to the file "pantheon.pkl"
    And I write "mine\n" to the file "scaffold.pkl"
    When I run "init --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write"
    Then the exit code is 3
    And the error contains "scaffold.pkl already exists"
