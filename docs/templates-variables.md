# Builtin templates: the variable table

Status: for owner approval. Nothing is converted until this is accepted. Date: 14-11-2026.

Rule used (accepted in the goal): a value is a **variable** if it can change per repository or over time. Fixed policy
text, comments and structure stay in the template. Every variable is defined once, in `pkl/Tools.pkl` (tool and image
versions, already there) or in a new `pkl/Values.pkl` (everything else), and reaches the template through its context.

## A. The seven `@@NAME@@` templates

| Template | Variable | Defined in today | Proposed home |
| --- | --- | --- | --- |
| `mise.toml.tmpl` | `toolchain` | `Options.toolchain` | unchanged |
| | `actionlint`, `zizmor`, `rumdl`, `agnix` | `Tools.pkl` | unchanged |
| | `runner` | computed in `mise/Builtin.pkl` | unchanged |
| | `minimumReleaseAge` | `Tools.pkl` | unchanged |
| | `lockfilePlatforms` (a list, joined in the template) | `Tools.pkl` | unchanged |
| `hk.pkl.tmpl`, `hk.shared.pkl.tmpl` | `hkVersion` (3 uses, two files) | `Tools.hkConfigVersion` | unchanged |
| | `allFeatures` (boolean, replaces `@@FEATURES@@`) | `Options.features` | unchanged |
| `rust-toolchain.toml.tmpl` | `toolchain` | `Options.toolchain` | unchanged |
| `tag-release.sh.tmpl` | `crate` | `Shared.needCrateName` | unchanged |
| `security-binary.md.tmpl` | `crate`, `skills` (boolean), `skill` | `Options` | unchanged |
| `coderabbit.yaml.tmpl` | `skills` (boolean), `skill` | `Options` | unchanged |

## B. Conditionals, not placeholders (need `{% if %}` and a test with `skills` on and off)

| File | Today | In Jinja |
| --- | --- | --- |
| `security-binary.md.tmpl` | line `@@SKILL_LINE@@` replaced by a sentence or by nothing | `{% if skills %}` the sentence `{% endif %}` |
| `coderabbit.yaml.tmpl` | a comment plus one list entry deleted when `skills` is off | `{% if skills %}` both lines `{% endif %}` |
| `agnix.toml` | rewritten in pkl with `replaceFirst` | `{% if skills %}` block `{% endif %}`, no pkl rewrite |

## C. The ten static assets: every value found

| Asset | Value | Variable? | Name and home | Reason |
| --- | --- | --- | --- | --- |
| `dist-workspace.toml` | `cargo-dist-version = "0.32.0"` | yes | `dist` (`Tools.dist`) | already defined twice today; a test keeps them equal |
| | `targets` list | yes | `distTargets` (`Values.pkl`) | grows when Windows is added |
| | `installers`, `ci`, `install-path`, `install-updater`, `pr-run-mode` | no | in the template | structure of the release design |
| | `global`/Linux runner `ubuntu-22.04` (3 uses) | yes | `linuxRunner` (`Tools.pkl`) | one image, used three times; retired images have broken releases before |
| | `macos-latest` (2 uses) | yes | `macosRunner` (`Tools.pkl`) | same |
| | three `github-action-commits` SHAs | yes | `distActionCommits` (`Pins.pkl`) | pins move; `Pins.pkl` is where they are bumped |
| `deny.toml` | licence allow-list | yes | `denyLicences` (`Values.pkl`) | grows with dependencies |
| | `ignore = ["RUSTSEC-2024-0449"]` | yes | `denyIgnore` (`Values.pkl`) | advisories come and go; the comment above it moves with it |
| | `allow-git` dev-kit URL | yes | `denyAllowGit` (`Values.pkl`) | removed when published; today's `own-deny` preset becomes a context change |
| | `yanked`, `multiple-versions`, `wildcards`, `unknown-*` | no | in the template | policy |
| `rumdl.toml` | `disable` rule list | yes | `rumdlDisabled` (`Values.pkl`) | per-repository taste |
| | `per-file-ignores` path | yes | `prTemplatePath` | couples to the `pull-request-template` builtin (presence part 2) |
| | `include`, `exclude`, `MD010` | no | in the template | structure |
| `plumber.yaml` | `trustedGithubActions` list | yes | `plumberTrustedActions` (`Values.pkl`) | tracks which workflows the repo uses |
| | `trustedUrls` list, `trustedOwners` list | yes | `plumberTrustedUrls`, `plumberTrustedOwners` | same |
| | `minimumStars`, `forbiddenVariables`, each control's `enabled` | no | in the template | policy switches, not values |
| `agnix.toml` | `target = "ClaudeCode"` | yes | `agnixTarget` | the linted agent |
| | `exclude` list | yes | `agnixExclude` | project layout |
| | `disabled_rules` list | yes | `agnixDisabledRules` | rule choices |
| | the `[rules]` family switches | no | in the template | structure |
| `actionlint.yaml` | the two ignore patterns (SC2086, SC2129) and the path `release.yml` | yes | `actionlintIgnores` (list of pattern) and `releaseWorkflow` | the shellcheck rules change with cargo-dist output |
| `zizmor.yml`, `zizmor-composite.yml` | `release.yml:18:3`, `:79:91`, `:110:40`, `:110:66`, `action.yml:35:7` | yes | `zizmorIgnores` (list of `file:line:col`) | **open question 1** |
| | the two files differ only by the extra `github-env` block | merge | one `zizmor.yml.j2` with `{% if composite %}` | removes a duplicate of the whole header comment |
| `claude-settings.json` | `$schema` URL, `attribution.sessionUrl` | yes | `claudeSchemaUrl`, `claudeSessionUrl` | a user's attribution choice |
| `mise-repo.toml` | none: comments and an empty `[tools]` | no | stays as it is | **open question 2**: nothing to render |

## Open questions for the owner

1. **zizmor line numbers.** `release.yml:18:3` points into a file cargo-dist regenerates, so the numbers drift. Options:
   (a) keep them as a variable list (`zizmorIgnores`), updated by hand when the finding reappears (recommended: the
   behaviour is unchanged and the baseline stays byte for byte); (b) drop to a file-wide ignore with no line, which also
   hides new findings in that file. I recommend (a).
2. **`mise-repo.toml`.** It has no value to pass. Options: (a) leave it static, a named exception to "every toml is a
   template" (recommended); (b) make it a `.j2` with an empty context so the rule has no exception.

## What this does not change

The default render of all 55 configurations stays byte for byte equal to the frozen baseline. Preset context overrides
replace whole keys.
