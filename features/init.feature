@rust-only
Feature: init
  `init` writes pantheon.pkl once for a new project and renders it. It writes nothing without --write, and nothing at all when
  the render would be refused. The scenarios use the package in this checkout (--allow-local-package), so they need no release;
  the release itself (the newest stable one, its checksum and its attestation) is tested in crates/scaffold-cli/tests/init.rs.
  The TypeScript runner has no init.

  Scenario: init is a dry run unless told to write
    Given a new empty repository
    When I run "init --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    Then the exit code is 0
    And the output contains "pantheon.pkl would be written:"
    And the output contains "create    mise.toml"
    And the output contains "Run again with --write to write them."
    And the file "pantheon.pkl" does not exist
    And the file "mise.toml" does not exist

  Scenario: init with --write creates the config and the files, and check then passes
    Given a new empty repository
    And I run "init --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write"
    Then the exit code is 0
    And the output contains "init: wrote pantheon.pkl and "
    And the output contains "Next: run mise lock, then commit."
    And the file "pantheon.pkl" contains 'toolchain = "1.96.1"'
    And the file "mise.toml" exists
    And the file ".github/workflows/rust-ci.yml" exists
    And the file ".pkl-gen/manifest.json" exists
    When I run "check <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the output contains "written 0 file(s), deleted 0"

  Scenario: init never overwrites a pantheon.pkl
    Given a new empty repository
    And I write "mine\n" to the file "pantheon.pkl"
    When I run "init --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write"
    Then the exit code is 3
    And the error contains "pantheon.pkl already exists"
    And the file "pantheon.pkl" has the content "mine\n"
    And the file "mise.toml" does not exist

  Scenario: init is refused, and writes nothing, when it would overwrite a file that differs
    Given a new empty repository
    And I write "target/\n" to the file ".gitignore"
    When I run "init --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write"
    Then the exit code is 3
    And the error contains ".gitignore: already exists and differs"
    And the file "pantheon.pkl" does not exist
    And the file "mise.toml" does not exist
    And the file ".gitignore" has the content "target/\n"

  Scenario: init needs a toolchain
    Given a new empty repository
    When I run "init --root <repo> --allow-local-package <package>"
    Then the exit code is 2
    And the error contains "init needs --toolchain, for example --toolchain 1.96.1"
    And the file "pantheon.pkl" does not exist

  Scenario: a binary needs a crate name, and with one the config and the binary files are written
    Given a new empty repository
    When I run "init --root <repo> --allow-local-package <package> --toolchain 1.96.1 --binary"
    Then the exit code is 2
    And the error contains "--binary needs --crate-name"
    When I run "init --root <repo> --allow-local-package <package> --toolchain 1.96.1 --binary --crate-name my-tool --write"
    Then the exit code is 0
    And the file "pantheon.pkl" contains 'crateName = "my-tool"'
    And the file "dist-workspace.toml" exists

  Scenario: skills need a skill name, and with one the config and the skill audit are written
    Given a new empty repository
    When I run "init --root <repo> --allow-local-package <package> --toolchain 1.96.1 --skills"
    Then the exit code is 2
    And the error contains "--skills needs --skill-name"
    When I run "init --root <repo> --allow-local-package <package> --toolchain 1.96.1 --skills --skill-name my-skill --write"
    Then the exit code is 0
    And the file "pantheon.pkl" contains 'skillName = "my-skill"'
    And the file ".github/workflows/audit-skill.yml" exists
