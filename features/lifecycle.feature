Feature: files over time
  A seeded file is written once and then belongs to the repository. An orphan is deleted only when nobody edited it. A
  manifest that cannot be read is rebuilt from a clean render, never from guesses. Detach hands every file back.

  Scenario: a seeded file is written once and kept when it is edited
    Given a new empty repository
    And I run "render <fixture:smoke-seed-a> --root <repo> --allow-local-package <package>"
    And I write "mine\n" to the file "seed.txt"
    When I run "render <fixture:smoke-seed-a> --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the output contains "seed-kept"
    And the file "seed.txt" has the content "mine\n"

  Scenario: a seeded file that is dropped from the config is not deleted
    Given a new git repository
    And I run "render <fixture:smoke-seed-a> --root <repo> --allow-local-package <package>"
    And I commit everything
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file "seed.txt" has the content "first\n"

  Scenario: an untouched seed can become an owned file
    Given a new empty repository
    And I run "render <fixture:smoke-seed-a> --root <repo> --allow-local-package <package>"
    When I run "render <fixture:smoke-seed-owned> --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file "seed.txt" has the content "second\n"

  Scenario: an edited seed is not turned into an owned file
    Given a new empty repository
    And I run "render <fixture:smoke-seed-a> --root <repo> --allow-local-package <package>"
    And I write "mine\n" to the file "seed.txt"
    When I run "render <fixture:smoke-seed-owned> --root <repo> --allow-local-package <package>"
    Then the exit code is 3
    And the error contains "was seeded and has been edited"
    And the file "seed.txt" has the content "mine\n"

  Scenario: --force turns an edited seed into an owned file
    Given a new empty repository
    And I run "render <fixture:smoke-seed-a> --root <repo> --allow-local-package <package>"
    And I write "mine\n" to the file "seed.txt"
    When I run "render <fixture:smoke-seed-owned> --root <repo> --allow-local-package <package> --force"
    Then the exit code is 0
    And the file "seed.txt" has the content "second\n"

  Scenario: an orphan that was edited by hand is not deleted
    Given a new git repository
    And I run "render <fixture:smoke-orphan-a> --root <repo> --allow-local-package <package>"
    And I write "mine\n" to the file "b.txt"
    And I commit everything
    When I run "render <fixture:smoke-orphan-b> --root <repo> --allow-local-package <package>"
    Then the exit code is 3
    And the error contains "it is not deleted"
    And the file "b.txt" has the content "mine\n"

  Scenario: --force deletes an orphan that was edited by hand
    Given a new git repository
    And I run "render <fixture:smoke-orphan-a> --root <repo> --allow-local-package <package>"
    And I write "mine\n" to the file "b.txt"
    And I commit everything
    When I run "render <fixture:smoke-orphan-b> --root <repo> --allow-local-package <package> --force"
    Then the exit code is 0
    And the file "b.txt" does not exist

  Scenario: --repair rebuilds a manifest that is not JSON
    Given a new empty repository
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    And I write "{ not json" to the file ".pkl-gen/manifest.json"
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package> --repair"
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the output contains "written 0 file(s)"

  Scenario: --repair leaves a file that differs from the render out of the manifest
    Given a new empty repository
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    And I write "# mine\n" to the file "rust-toolchain.toml"
    And I write "{ not json" to the file ".pkl-gen/manifest.json"
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package> --repair"
    Then the exit code is 0
    And the error contains "rust-toolchain.toml: differs from the render, left out of the repaired manifest"
    And the file "rust-toolchain.toml" has the content "# mine\n"

  Scenario: detach deletes the files marked for it and keeps the others
    Given a new git repository
    And I run "render <fixture:smoke-detach> --root <repo> --allow-local-package <package>"
    And I commit everything
    When I run "detach --root <repo>"
    Then the exit code is 0
    And the output contains "detached: deleted 1 file(s)"
    And the file "tool.txt" does not exist
    And the file "keep.txt" has the content "keep\n"
    And the file ".pkl-gen/manifest.json" does not exist

  Scenario: a dry run of detach changes nothing
    Given a new git repository
    And I run "render <fixture:smoke-detach> --root <repo> --allow-local-package <package>"
    And I commit everything
    When I run "detach --root <repo> --dry-run"
    Then the exit code is 0
    And the output contains "dry run: would delete 1 file(s) and remove the manifest"
    And the file "tool.txt" exists
    And the file ".pkl-gen/manifest.json" exists

  Scenario: detach refuses a marked file that was edited by hand
    Given a new git repository
    And I run "render <fixture:smoke-detach> --root <repo> --allow-local-package <package>"
    And I write "mine\n" to the file "tool.txt"
    And I commit everything
    When I run "detach --root <repo>"
    Then the exit code is 3
    And the error contains "edited by hand since the last render"
    And the file "tool.txt" has the content "mine\n"
    And the file ".pkl-gen/manifest.json" exists

  Scenario: detach --force deletes a marked file that was edited by hand
    Given a new git repository
    And I run "render <fixture:smoke-detach> --root <repo> --allow-local-package <package>"
    And I write "mine\n" to the file "tool.txt"
    And I commit everything
    When I run "detach --root <repo> --force"
    Then the exit code is 0
    And the file "tool.txt" does not exist

  Scenario: detach without a manifest has nothing to do
    Given a new empty repository
    When I run "detach --root <repo>"
    Then the exit code is 1
    And the error contains "there is nothing to detach"
