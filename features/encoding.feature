Feature: line endings and file names
  What the runner does with text it did not write in the form it expects. The CRLF scenarios record a gap, not a wish: the
  runner compares bytes, so a checkout that turned every line ending into CRLF looks like a hand edit everywhere. The owner
  decides whether the Rust build keeps that (and the scenarios stay) or fixes it (and the scenarios change). See "Gaps" in
  features/README.md.

  Scenario: an owned file whose line endings were changed to CRLF counts as a hand edit
    Given a new empty repository
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    And I write '[toolchain]\r\nchannel = "1.96.1"\r\n' to the file "rust-toolchain.toml"
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 3
    And the error contains "was edited by hand since the last render"
    And the file "rust-toolchain.toml" has the content '[toolchain]\r\nchannel = "1.96.1"\r\n'

  Scenario: check reports drift on a file whose line endings were changed to CRLF
    Given a new empty repository
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    And I write '[toolchain]\r\nchannel = "1.96.1"\r\n' to the file "rust-toolchain.toml"
    When I run "check <fixture:minimal> --root <repo> --allow-local-package <package>"
    Then the exit code is 4
    And the error contains "rust-toolchain.toml: different"

  Scenario: --force puts the line endings back
    Given a new empty repository
    And I run "render <fixture:minimal> --root <repo> --allow-local-package <package>"
    And I write '[toolchain]\r\nchannel = "1.96.1"\r\n' to the file "rust-toolchain.toml"
    When I run "render <fixture:minimal> --root <repo> --allow-local-package <package> --force"
    Then the exit code is 0
    And the file "rust-toolchain.toml" has the content '[toolchain]\nchannel = "1.96.1"\n'

  Scenario: a file name with an accented letter is written, and a second render leaves it alone
    Given a new empty repository
    When I run "render <fixture:smoke-unicode> --root <repo> --allow-local-package <package>"
    And I run "render <fixture:smoke-unicode> --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the output contains "written 0 file(s)"
    And the file "café.txt" has the content "café\n"
