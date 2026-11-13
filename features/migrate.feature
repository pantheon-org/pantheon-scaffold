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

  Scenario: --as-presets in a dry run lists the Preset each drifted file would become, and writes nothing
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --as-presets"
    Then the exit code is 0
    And the output contains "preset     presets/aislop.pkl  keeps .github/workflows/aislop.yml"
    And the output contains "Nothing was written. Run again with --write to write pantheon.pkl, 1 Preset(s) and the new files."
    And the file "presets/aislop.pkl" does not exist
    And the file "pantheon.pkl" does not exist

  Scenario: a drifted workflow is kept as a Preset, and check, render and detach then behave as for any Preset
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write --as-presets"
    Then the exit code is 0
    And the output contains "Wrote pantheon.pkl, 1 Preset(s) and "
    And the file "pantheon.pkl" contains 'presets/aislop.pkl'
    And the file "presets/aislop.pkl" contains "// What differed when this was written:"
    And the file "presets/aislop.pkl" contains "timeout-minutes: 20"
    And the file ".github/workflows/aislop.yml" contains "timeout-minutes: 20"
    And the file ".github/workflows/sync-labels.yml" contains "# Creates the labels Dependabot applies"
    And the file "README.md" has the content "# Example\n"
    When I run "check <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the output contains "preset presets/aislop.pkl"
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the output contains "written 0 file(s), deleted 0"
    When I run "detach --root <repo>"
    Then the exit code is 0
    And the file ".pkl-gen/manifest.json" does not exist
    And the file "presets/aislop.pkl" exists
    And the file ".github/workflows/aislop.yml" contains "timeout-minutes: 20"

  Scenario: a drifted file that is not YAML, and has no final newline, is kept exactly
    Given a new empty repository
    And I write '[toolchain]\nchannel   = "1.96.1"' to the file "rust-toolchain.toml"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write --as-presets"
    Then the exit code is 0
    And the file "rust-toolchain.toml" has the content '[toolchain]\nchannel   = "1.96.1"'
    And the file "presets/rust-toolchain.pkl" exists
    When I run "check <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0

  Scenario: a Preset never takes the name of a file already under presets/
    Given the project "migrate-mixed"
    And I write "mine\n" to the file "presets/aislop.pkl"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write --as-presets"
    Then the exit code is 0
    And the file "presets/aislop.pkl" has the content "mine\n"
    And the file "presets/aislop-2.pkl" exists
    And the file "pantheon.pkl" contains 'presets/aislop-2.pkl'

  Scenario: --as-presets with no drift writes no Preset
    Given the project "migrate-adopt"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write --as-presets"
    Then the exit code is 0
    And the output does not contain "Preset"
    And the file "pantheon.pkl" does not contain "presets"

  Scenario: the plain --write refusal names --as-presets
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write"
    Then the exit code is 3
    And the error contains "Run with --as-presets to keep each one as a Preset."

  Scenario: --as-presets belongs to migrate, not to init
    Given a new empty repository
    When I run "init --root <repo> --allow-local-package <package> --toolchain 1.96.1 --as-presets"
    Then the exit code is 2
    And the error contains "--as-presets belongs to migrate"

  Scenario: migrate takes the same options as init, so the dry run renders what the repository sets
    Given a new empty repository
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --composite-action --git-dependency-ignore pahdk-core --write"
    Then the exit code is 0
    And the file "pantheon.pkl" contains "compositeAction = true"
    And the file ".github/dependabot.yml" contains "dependency-name: pahdk-core"

  # --accept-builtins (follow-up 1, docs/migrate-accept-builtins-design.md). It replaces every drifted file with the render, and
  # only in a git work tree whose tracked tree is clean, so the git diff of the adoption commit is the full record.

  Scenario: --accept-builtins in a dry run lists what it would replace, reports the git check and writes nothing
    Given the project "migrate-mixed"
    And a new git repository
    And I commit everything
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --accept-builtins"
    Then the exit code is 0
    And the output contains "drifted    .github/workflows/aislop.yml"
    And the output contains "git guard: ok, 1 file(s) to replace are tracked and the tree is clean"
    And the output contains "would replace 1 file(s)"
    And the output contains "Nothing was written."
    And the file "pantheon.pkl" does not exist
    And the file "mise.toml" does not exist

  Scenario: --accept-builtins in a dry run outside git says a write would be refused
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --accept-builtins"
    Then the exit code is 0
    And the output contains "git guard: a write would be refused"
    And the output contains "not a git repository"
    And the file "pantheon.pkl" does not exist

  Scenario: --accept-builtins --write replaces the drifted file in a clean git tree, and the repository is then in sync
    Given the project "migrate-mixed"
    And a new git repository
    And I commit everything
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --accept-builtins --write"
    Then the exit code is 0
    And the output contains "replaced by --accept-builtins (the git diff of the adoption commit is the full record):"
    And the output contains "  .github/workflows/aislop.yml"
    And the file "pantheon.pkl" exists
    And the file ".pkl-gen/manifest.json" exists
    When I run "check <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    When I run "render <repo>/pantheon.pkl --root <repo> --allow-local-package <package>"
    Then the output contains "written 0 file(s), deleted 0"

  Scenario: a second migrate on an adopted repository is still refused
    Given the project "migrate-mixed"
    And a new git repository
    And I commit everything
    And I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --accept-builtins --write"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --accept-builtins --write"
    Then the exit code is 3
    And the error contains "pantheon.pkl already exists"

  Scenario: --accept-builtins --write is refused while the tracked tree has uncommitted changes, and nothing is written
    Given the project "migrate-mixed"
    And a new git repository
    And I commit everything
    And I write "# edited\n" to the file "README.md"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --accept-builtins --write"
    Then the exit code is 3
    And the error contains "not clean"
    And the file "pantheon.pkl" does not exist
    And the file "mise.toml" does not exist
    And the file ".pkl-gen/manifest.json" does not exist

  Scenario: --accept-builtins --write is refused for a drifted file git does not track, and nothing is written
    Given the project "migrate-mixed"
    And a new git repository
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --accept-builtins --write"
    Then the exit code is 3
    And the error contains ".github/workflows/aislop.yml: not tracked by git"
    And the file "pantheon.pkl" does not exist

  Scenario: --accept-builtins --write is refused outside a git repository, and nothing is written
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --accept-builtins --write"
    Then the exit code is 3
    And the error contains "not a git repository"
    And the file "pantheon.pkl" does not exist
    And the file "mise.toml" does not exist

  Scenario: --accept-builtins cannot be combined with --as-presets
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --accept-builtins --as-presets"
    Then the exit code is 2
    And the error contains "--accept-builtins cannot be used with --as-presets"

  Scenario: --accept-builtins belongs to migrate, not to init
    Given a new empty repository
    When I run "init --root <repo> --allow-local-package <package> --toolchain 1.96.1 --accept-builtins"
    Then the exit code is 2
    And the error contains "--accept-builtins belongs to migrate"

  Scenario: --accept-builtins with nothing drifted replaces nothing
    Given a new empty repository
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --accept-builtins"
    Then the exit code is 0
    And the output contains "no file has drifted, so --accept-builtins replaces nothing"

  Scenario: the --write refusal and the dry-run summary both name --accept-builtins
    Given the project "migrate-mixed"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1"
    Then the output contains "--accept-builtins"
    When I run "migrate --root <repo> --allow-local-package <package> --toolchain 1.96.1 --write"
    Then the exit code is 3
    And the error contains "--accept-builtins"
    And the error contains "Run with --as-presets to keep each one as a Preset."
