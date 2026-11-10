Feature: which release to use, and whether the pins are fresh
  `latest` names the newest release worth upgrading to. `freshness` says whether the pin check still runs. Both fail closed:
  a list or a record that cannot be read is an error, never an answer. The release list, the freshness record and the
  known-bad list come from a stub server, so nothing here reaches the network.

  Background:
    Given a new empty repository
    And the stub server serves the known-bad list '{"versions":[]}'

  Scenario: latest names the newest stable release
    Given the stub server answers "/releases" with '[{"tag_name":"v0.1.4"},{"tag_name":"v0.1.5"},{"tag_name":"v0.2.0","prerelease":true},{"tag_name":"v0.3.0","draft":true}]'
    When I run "latest --url <stub>/releases --root <repo>"
    Then the exit code is 0
    And the output is "0.1.5"

  Scenario: latest offers a pre-release only when asked
    Given the stub server answers "/releases" with '[{"tag_name":"v0.1.5"},{"tag_name":"v0.2.0","prerelease":true}]'
    When I run "latest --url <stub>/releases --root <repo> --include-prereleases"
    Then the exit code is 0
    And the output is "0.2.0"

  Scenario: latest skips a version on the known-bad list
    Given the stub server serves the known-bad list '{"versions":[{"version":"0.1.5","reason":"broken"}]}'
    And the stub server answers "/releases" with '[{"tag_name":"v0.1.4"},{"tag_name":"v0.1.5"}]'
    When I run "latest --url <stub>/releases --root <repo>"
    Then the exit code is 0
    And the output is "0.1.4"

  Scenario: latest ignores a tag that is not a version
    Given the stub server answers "/releases" with '[{"tag_name":"nightly"},{"tag_name":"v0.1.5"}]'
    When I run "latest --url <stub>/releases --root <repo>"
    Then the exit code is 0
    And the output is "0.1.5"

  Scenario: latest with nothing that qualifies is an error
    Given the stub server answers "/releases" with '[{"tag_name":"v0.2.0","prerelease":true}]'
    When I run "latest --url <stub>/releases --root <repo>"
    Then the exit code is 1
    And the error contains "no release found that qualifies"

  Scenario: latest fails closed when the release list cannot be fetched
    Given the stub server answers "/releases" with status 500
    When I run "latest --url <stub>/releases --root <repo> --attempts 1"
    Then the exit code is 1
    And the error contains "failing closed"

  Scenario: latest fails when the answer is not a list of releases
    Given the stub server answers "/releases" with '{"message":"rate limited"}'
    When I run "latest --url <stub>/releases --root <repo>"
    Then the exit code is 1
    And the error contains "did not return a list of releases"

  Scenario: a recent record is fresh
    Given the stub server answers "/freshness.json" with a freshness record from 3 days ago
    When I run "freshness --url <stub>/freshness.json"
    Then the exit code is 0
    And the output contains "fresh: the last successful pin check was 3 day(s) ago"

  Scenario: a record older than thirty days is stale
    Given the stub server answers "/freshness.json" with a freshness record from 40 days ago
    When I run "freshness --url <stub>/freshness.json"
    Then the exit code is 3
    And the error contains "stale: the last successful pin check was 40 day(s) ago"

  Scenario: the age limit can be changed
    Given the stub server answers "/freshness.json" with a freshness record from 10 days ago
    When I run "freshness --url <stub>/freshness.json --max-age-days 5"
    Then the exit code is 3
    And the error contains "more than 5"

  Scenario: a record that is not JSON is an error
    Given the stub server answers "/freshness.json" with 'not json'
    When I run "freshness --url <stub>/freshness.json"
    Then the exit code is 1
    And the error contains "is not valid JSON"

  Scenario: a record from a newer format is an error
    Given the stub server answers "/freshness.json" with '{"schemaVersion":2,"lastSuccessfulPinCheck":"2026-11-08T12:00:00Z"}'
    When I run "freshness --url <stub>/freshness.json"
    Then the exit code is 1
    And the error contains "this runner reads 1"

  Scenario: a time that is not UTC is an error
    Given the stub server answers "/freshness.json" with '{"schemaVersion":1,"lastSuccessfulPinCheck":"2026-11-08T12:00:00+01:00"}'
    When I run "freshness --url <stub>/freshness.json"
    Then the exit code is 1
    And the error contains "must be a UTC time"

  Scenario: a record dated well in the future is an error
    Given the stub server answers "/freshness.json" with a freshness record dated 3 days ahead
    When I run "freshness --url <stub>/freshness.json"
    Then the exit code is 1
    And the error contains "dated in the future"

  Scenario: freshness fails closed when the record cannot be fetched
    Given the stub server answers "/freshness.json" with status 500
    When I run "freshness --url <stub>/freshness.json --attempts 1"
    Then the exit code is 1
    And the error contains "failing closed"

  Scenario: upgrade refuses a version that is not a version
    Given I write 'amends "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v0.1.0/pantheon-scaffold@0.1.0#/Config.pkl"\n' to the file "pantheon.pkl"
    And I write '[tools]\n"github:pantheon-org/pantheon-scaffold" = "0.1.0"\n' to the file "mise.toml"
    When I run "upgrade banana --root <repo>"
    Then the exit code is 1
    And the error contains "is not a version"

  Scenario: upgrade to the version already pinned is an error
    Given I write 'amends "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v0.1.0/pantheon-scaffold@0.1.0#/Config.pkl"\n' to the file "pantheon.pkl"
    And I write '[tools]\n"github:pantheon-org/pantheon-scaffold" = "0.1.0"\n' to the file "mise.toml"
    When I run "upgrade 0.1.0 --root <repo>"
    Then the exit code is 1
    And the error contains "already at 0.1.0"

  Scenario: upgrade to an older version needs --force
    Given I write 'amends "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v0.1.0/pantheon-scaffold@0.1.0#/Config.pkl"\n' to the file "pantheon.pkl"
    And I write '[tools]\n"github:pantheon-org/pantheon-scaffold" = "0.1.0"\n' to the file "mise.toml"
    When I run "upgrade 0.0.9 --root <repo>"
    Then the exit code is 1
    And the error contains "is older than 0.1.0"
    And the error contains "use --force to go back"
