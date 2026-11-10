Feature: what the runner refuses to do
  The runner never writes over what a person edited, never deletes what git does not track, never works while another
  run holds the lock, and says why when it stops.

  Scenario: a render while the lock is held fails and writes nothing
    Given a new empty repository
    And the write lock is held by a running process
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "is held"
    And the file "rust-toolchain.toml" does not exist

  Scenario: an existing file that differs from the render is not adopted
    Given a new empty repository
    And I write "# mine\n" to the file "rust-toolchain.toml"
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 3
    And the error contains "already exists and differs from the render"
    And the file "rust-toolchain.toml" has the content "# mine\n"

  Scenario: a dry run writes nothing
    Given a new empty repository
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package> --dry-run"
    Then the exit code is 0
    And the output contains "dry run: would write 3 file(s), delete 0"
    And the file "rust-toolchain.toml" does not exist
    And the file ".pkl-gen" does not exist

  Scenario: a dry run exits 3 when the render would be refused
    Given a new empty repository
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    And I write "# mine\n" to the file "rust-toolchain.toml"
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package> --dry-run"
    Then the exit code is 3
    And the file "rust-toolchain.toml" has the content "# mine\n"

  Scenario: a manifest that is not JSON stops the render
    Given a new empty repository
    And I write "{ not json" to the file ".pkl-gen/manifest.json"
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "is not valid JSON"
    And the file "rust-toolchain.toml" does not exist

  Scenario: a manifest from a newer format stops the render
    Given a new empty repository
    And I write '{"manifestVersion":2,"files":{}}' to the file ".pkl-gen/manifest.json"
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "this runner supports 1"

  Scenario: two paths that differ only by case are refused
    Given a new empty repository
    When I run "render <fixture:smoke-case-collision> --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "differ only by case"

  Scenario: a case-only rename is refused and the old file is kept
    Given a new empty repository
    And I run "render <fixture:smoke-case-a> --root <repo> --allow-local-package <package>"
    When I run "render <fixture:smoke-case-b> --root <repo> --allow-local-package <package>"
    Then the exit code is 3
    And the error contains "differ only by case"
    And the file "readme.md" has the content "one\n"

  Scenario: an orphan is deleted in a clean git repository
    Given a new git repository
    And I run "render <fixture:smoke-orphan-a> --root <repo> --allow-local-package <package>"
    And I commit everything
    When I run "render <fixture:smoke-orphan-b> --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the output contains "deleted 1"
    And the file "b.txt" does not exist
    And the file "a.txt" exists

  Scenario: a dirty tracked tree refuses the deletion
    Given a new git repository
    And I run "render <fixture:smoke-orphan-a> --root <repo> --allow-local-package <package>"
    And I write "n\n" to the file "notes.md"
    And I commit everything
    And I write "changed\n" to the file "notes.md"
    When I run "render <fixture:smoke-orphan-b> --root <repo> --allow-local-package <package>"
    Then the exit code is 3
    And the error contains "not clean"
    And the file "b.txt" exists

  Scenario: outside a git repository nothing is deleted
    Given a new empty repository
    And I run "render <fixture:smoke-orphan-a> --root <repo> --allow-local-package <package>"
    When I run "render <fixture:smoke-orphan-b> --root <repo> --allow-local-package <package>"
    Then the exit code is 3
    And the error contains "not a git repository"
    And the file "b.txt" exists

  Scenario: a path that climbs out of the repository is refused
    Given a new empty repository
    When I run "render <fixture:smoke-path-escape> --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "escapes the repository"
    And the file ".pkl-gen/manifest.json" does not exist

  Scenario: an absolute path is refused
    Given a new empty repository
    When I run "render <fixture:smoke-path-absolute> --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "absolute paths are not allowed"

  Scenario: a symbolic link that points outside the repository is not followed
    Given a new empty repository
    And the file "link.txt" is a symbolic link to a file outside the repository
    When I run "render <fixture:smoke-path-link> --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "is a symlink that points outside the repository"
    And the file "target.txt" does not exist outside the repository

  Scenario: a lock held by a process that has gone is taken over
    Given a new empty repository
    And the write lock is held by a process that no longer exists
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file "rust-toolchain.toml" exists
    And the file ".pkl-gen/lock" does not exist

  Scenario: a lock older than ten minutes is taken over even when its process is running
    Given a new empty repository
    And the write lock is held by a running process and is 11 minutes old
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file "rust-toolchain.toml" exists

  Scenario: on CI a recent lock holds even when its process has gone
    Given a new empty repository
    And the environment variable "CI" is "true"
    And the write lock is held by a process that no longer exists
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains "is held"
    And the file "rust-toolchain.toml" does not exist
