Feature: the merged analysis workflow
  `options.workflowLayout["analysis"] = "merged"` renders zizmor, aislop, agnix and plumber as the jobs of one workflow, `analysis.yml`, instead of
  four workflows. `options.without` still switches each tool off, by the tool's name. The default renders the four separate workflows, as before.
  The scenarios start from small projects in features/projects, copied into a git repository (docs/workflow-layer.md).

  Scenario: by default the four tools are separate workflows
    Given a new git repository
    And the project "without-none"
    When I run "render <repo>/scaffold.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/zizmor.yml" exists
    And the file ".github/workflows/agnix.yml" exists
    And the file ".github/workflows/aislop.yml" exists
    And the file ".github/workflows/plumber.yml" exists
    And the file ".github/workflows/analysis.yml" does not exist

  Scenario: merged, the four tools are the jobs of one workflow
    Given a new git repository
    And the project "analysis-merged"
    When I run "render <repo>/scaffold.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/analysis.yml" exists
    And the file ".github/workflows/zizmor.yml" does not exist
    And the file ".github/workflows/agnix.yml" does not exist
    And the file ".github/workflows/aislop.yml" does not exist
    And the file ".github/workflows/plumber.yml" does not exist
    And the file ".github/workflows/analysis.yml" contains "name: zizmor\n"
    And the file ".github/workflows/analysis.yml" contains "name: agnix\n"
    And the file ".github/workflows/analysis.yml" contains "name: aislop quality gate\n"
    And the file ".github/workflows/analysis.yml" contains "name: Plumber Score\n"

  Scenario: merged, the workflow holds no write permission and the jobs that need one hold their own
    Given a new git repository
    And the project "analysis-merged"
    When I run "render <repo>/scaffold.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/analysis.yml" contains "permissions:\n  contents: read\nconcurrency:"
    And the file ".github/workflows/analysis.yml" contains "      issues: write\n"
    And the file ".github/workflows/analysis.yml" contains "      pull-requests: write\n"
    And the file ".github/workflows/analysis.yml" contains "      security-events: write\n"

  Scenario: merged, plumber's schedule runs plumber alone
    Given a new git repository
    And the project "analysis-merged"
    When I run "render <repo>/scaffold.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/analysis.yml" contains "cron: 41 3 * * 1"
    And the file ".github/workflows/analysis.yml" contains "if: github.event_name != 'schedule'\n"
    And the file ".github/workflows/analysis.yml" contains "if: (github.event_name != 'schedule') && (github.event_name == 'push' || github.event.pull_request.head.repo.fork == false)\n"

  Scenario: merged, a pull request cancels its own older run and no other run is cancelled
    Given a new git repository
    And the project "analysis-merged"
    When I run "render <repo>/scaffold.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/analysis.yml" contains "group: ${{ github.workflow }}-${{ github.event_name == 'pull_request' && github.ref || github.run_id }}"
    And the file ".github/workflows/analysis.yml" contains "cancel-in-progress: ${{ github.event_name == 'pull_request' }}"

  Scenario: merged, leaving a tool out removes its job and keeps the others
    Given a new git repository
    And the project "analysis-merged-without-zizmor"
    When I run "render <repo>/scaffold.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/analysis.yml" exists
    And the file ".github/workflows/analysis.yml" does not contain "name: zizmor\n"
    And the file ".github/workflows/analysis.yml" contains "name: agnix\n"
    And the file ".github/workflows/zizmor.yml" does not exist

  Scenario: merged without plumber has no schedule and no guard
    Given a new git repository
    And the project "analysis-merged-without-plumber"
    When I run "render <repo>/scaffold.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/analysis.yml" exists
    And the file ".github/workflows/analysis.yml" does not contain "schedule"
    And the file ".github/workflows/analysis.yml" does not contain "!= 'schedule'"

  Scenario: merged with every tool left out renders no analysis workflow
    Given a new git repository
    And the project "analysis-merged-without-all"
    When I run "render <repo>/scaffold.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/analysis.yml" does not exist

  Scenario: merged, leaving a tool out keeps the Dependabot entry that scans the repository's own workflow
    Given a new git repository
    And the project "analysis-merged-without-zizmor"
    When I run "render <repo>/scaffold.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/dependabot.yml" contains "package-ecosystem: github-actions"
    And the file ".github/dependabot.yml" contains "- .github/workflows/analysis.yml"

  Scenario: switching from separate to merged deletes the four workflows and writes the new one
    Given a new git repository
    And the project "without-none"
    And I run "render <repo>/scaffold.pkl --root <repo> --allow-local-package <package>"
    And I commit everything
    And the project "analysis-merged"
    And I commit everything
    When I run "render <repo>/scaffold.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 0
    And the file ".github/workflows/analysis.yml" exists
    And the file ".github/workflows/zizmor.yml" does not exist
    And the file ".github/workflows/plumber.yml" does not exist

  Scenario: a layout key that is not a recipe fails and lists the recipes
    Given a new empty repository
    And the project "analysis-bad-recipe"
    When I run "render <repo>/scaffold.pkl --root <repo> --allow-local-package <package>"
    Then the exit code is 1
    And the error contains '"analyses", which is not a workflow recipe of this package version'
    And the error contains "analysis"
    And the file ".github/workflows/analysis.yml" does not exist
