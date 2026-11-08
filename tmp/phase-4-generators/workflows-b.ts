// Writes the wave B builtins (release-please, zizmor, agnix, aislop, dependabot and analysisFiles) into pkl/Builtins.pkl, between the BEGIN and
// END markers, from the pinned copies in runner/test/upstream. The structure of each file is written here by hand. Headers and every run:
// script are copied from the upstream files. The few header lines that depend on an option are built with interpolation, so a comment never
// describes a config file the repository does not have. Run from the repository root: bun tmp/phase-4-generators/workflows-b.ts
// It replaces the block between the markers, so running it again is safe.
import { readFileSync, writeFileSync } from "node:fs";

const upText = (rel: string) => readFileSync(`runner/test/upstream/${rel}`, "utf8");
const BEGIN = "// BEGIN generated: wave B workflows (tmp/phase-4-generators/workflows-b.ts)";
const END = "// END generated: wave B workflows";

/** The leading comment block of a file: the comment lines between the name:/version: line and the first non-comment line, without `# `. */
function commentLines(text: string, afterName: boolean): string[] {
  const lines = text.split("\n");
  let i = afterName ? lines.findIndex((l) => l.startsWith("name:") || l.startsWith("version:")) + 1 : 0;
  while (i < lines.length && (lines[i] === "" || lines[i].startsWith("#"))) i++;
  const end = i;
  const start = afterName ? lines.findIndex((l) => l.startsWith("name:") || l.startsWith("version:")) + 1 : 0;
  return lines.slice(start, end).filter((l) => l.startsWith("#")).map((l) => l.replace(/^# ?/, ""));
}

/** A multi-line raw string (backslashes stay literal, \#( ) interpolates) whose closing delimiter sits at `indent` spaces. */
function raw(lines: string[], indent: number, newline = false): string {
  const pad = " ".repeat(indent);
  const body = lines.map((l) => (l === "" ? "" : pad + l)).join("\n");
  return `#"""\n${body}${newline ? "\\#n" : ""}\n${pad}"""#`;
}

function run(rel: string, job: string, step: number, indent: number): string {
  const doc = Bun.YAML.parse(upText(rel)) as { jobs: Record<string, { steps: { run?: string }[] }> };
  const text = doc.jobs[job].steps[step].run!;
  if (!text.endsWith("\n")) throw new Error(`${rel} ${job} step ${step}: run does not end with a newline`);
  return raw(text.slice(0, -1).split("\n"), indent, true);
}

function replaceLines(lines: string[], from: string, to: string[], count = 1): string[] {
  const i = lines.indexOf(from);
  if (i === -1) throw new Error(`header line not found: ${from}`);
  return [...lines.slice(0, i), ...to, ...lines.slice(i + count)];
}

// release-please: the header names the crate (binary repositories) or explains the tag-is-the-release setup (library repository).
const rpAdr = replaceLines(commentLines(upText("workflows/release-please.yml"), true), "crates/adr/Cargo.toml and Cargo.lock, plus CHANGELOG.md, from conventional", ["crates/\\#(crate)/Cargo.toml and Cargo.lock, plus CHANGELOG.md, from conventional"]);
const rpKit = commentLines(upText("workflows-kit/release-please.yml"), true);

// zizmor: the command in the header and the scope paragraph depend on the config file and on action.yml.
let zz = commentLines(upText("workflows/zizmor.yml"), true);
zz = replaceLines(zz, "Scans .github only: the workflows and dependabot.yml. The action's default", ["\\#(scope)"], 2);
zz = replaceLines(zz, "  GH_TOKEN=$(gh auth token) mise exec -- zizmor --min-severity high \\", ["\\#(cmd)"], 2);

// agnix: the first sentence depends on whether the repository has a skill.
const agHeader = replaceLines(commentLines(upText("workflows/agnix.yml"), true), "Lints the bundled skill with agnix (https://github.com/agent-sh/agnix) against", ["\\#(lead)"]);

// dependabot: the cargo caveat names the git dependencies, or says the dev-kit crates are git dependencies.
const dbAdr = commentLines(upText("workflows/dependabot.yml"), true);
const dbKit = commentLines(upText("workflows-kit/dependabot.yml"), true);

const block = `${BEGIN}

local function bareCheckout(stepName: String): Config.Step = new {
  name = stepName
  uses = Pins.checkout
  \`with\` = new Mapping { ["persist-credentials"] = false }
}

/// Keeps one rolling release pull request up to date and tags releases, under the App token so the tag starts the release workflow. With a
/// binary (\`options.binary\`) it also tags the crate version itself when it has no tag yet (the script tag-release.sh).
function releasePlease(opts: Config.Options): Config.Workflow =
  let (crate = opts.crateName ?? "crate")
  new {
    header =
      if (opts.binary) ${raw(rpAdr, 8)}
      else ${raw(rpKit, 8)}
    name = "Release Please"
    on = new Mapping<String, Any?> {
      ["push"] = new Mapping { ["branches"] = new Listing { "main" } }
      ["workflow_dispatch"] = new Mapping {}
    }
    permissions = readOnly
    concurrency = new Mapping {
      ["group"] = "release-please"
      ["cancel-in-progress"] = false
    }
    jobs {
      ["release-please"] {
        name = "Update release PR and tag releases"
        \`timeout-minutes\` = 5
        steps {
          mintToken(opts)
          new {
            uses = Pins.releasePleaseAction
            \`with\` = new Mapping {
              ["token"] = "\${{ steps.app-token.outputs.token }}"
              ["config-file"] = "release-please-config.json"
              ["manifest-file"] = ".release-please-manifest.json"
            }
          }
          when (opts.binary) {
            bareCheckout("Checkout")
            new {
              name = "Tag the crate version if it has no tag yet"
              env = new Mapping {
                ["GH_TOKEN"] = "\${{ steps.app-token.outputs.token }}"
                ["GH_REPO"] = "\${{ github.repository }}"
              }
              run = "bash .github/scripts/tag-release.sh"
            }
          }
        }
      }
    }
  }

/// The zizmor security scan of the workflows. It reads .github/zizmor.yml when the repository has a binary (the config silences findings in the
/// cargo-dist generated release.yml), and also scans action.yml when the repository has its own composite action.
function zizmor(opts: Config.Options): Config.Workflow =
  let (inputs = if (opts.compositeAction) ".github action.yml" else ".github")
  let (cmd = if (opts.binary) "  GH_TOKEN=$(gh auth token) mise exec -- zizmor --min-severity high \\\\\\n    --config .github/zizmor.yml \\(inputs)" else "  GH_TOKEN=$(gh auth token) mise exec -- zizmor --min-severity high \\(inputs)")
  let (scope = if (opts.compositeAction) "Scans .github (the workflows and dependabot.yml) and the repository's own\\ncomposite action, action.yml. Not the whole repository: the action's default\\ninput is \\".\\", which also reaches the skill fixtures under crates/ and skills/." else "Scans .github only: the workflows and dependabot.yml. The action's default\\ninput is the whole repository, which this gate does not need to cover.")
  new {
    header = ${raw(zz, 6)}
    name = "zizmor"
    on = mainTriggers()
    permissions = readOnly
    concurrency = concurrencyPerRef
    jobs {
      ["zizmor"] {
        name = "zizmor"
        \`timeout-minutes\` = 10
        steps {
          checkout
          new {
            uses = Pins.zizmorAction
            \`with\` = new Mapping {
              ["version"] = Tools.zizmor
              ["min-severity"] = "high"
              ["inputs"] = inputs
              when (opts.binary) { ["config"] = ".github/zizmor.yml" }
              ["advanced-security"] = false
            }
          }
        }
      }
    }
  }

/// Lints the skill (or the agent files) with agnix. The pinned release is downloaded from its public URL and checked against a pinned SHA-256.
/// A pull request fails only on an error it introduces.
function agnix(opts: Config.Options): Config.Workflow =
  let (lead = if (opts.skills) "Lints the bundled skill with agnix (https://github.com/agent-sh/agnix) against" else "Lints agent skills and agent config files with agnix (https://github.com/agent-sh/agnix) against")
  new {
    header = ${raw(agHeader, 6)}
    name = "agnix"
    on = mainTriggers()
    permissions = new Mapping {
      ["contents"] = "read"
      ["issues"] = "write"
      ["pull-requests"] = "write"
    }
    concurrency = new Mapping {
      ["group"] = "\${{ github.workflow }}-\${{ github.ref }}"
      ["cancel-in-progress"] = "\${{ github.event_name == 'pull_request' }}"
    }
    env = new Mapping {
      ["AGNIX_VERSION"] = Tools.agnix
      ["AGNIX_SHA256"] = Tools.agnixSha256
    }
    jobs {
      ["agnix"] {
        name = "agnix"
        \`if\` = "github.event_name == 'push' || github.event.pull_request.head.repo.fork == false"
        \`timeout-minutes\` = 10
        steps {
          checkout
          new {
            name = "Check out the base branch"
            \`if\` = "github.event_name == 'pull_request'"
            uses = Pins.checkout
            \`with\` = new Mapping {
              ["ref"] = "\${{ github.event.pull_request.base.sha }}"
              ["path"] = "base"
              ["persist-credentials"] = false
            }
          }
          new {
            name = "Install agnix (pinned digest)"
            run = ${run("workflows/agnix.yml", "agnix", 2, 14)}
          }
          new {
            name = "Lint"
            run = ${run("workflows/agnix.yml", "agnix", 3, 14)}
          }
          new {
            name = "Lint the base branch"
            \`if\` = "github.event_name == 'pull_request'"
            run = ${run("workflows/agnix.yml", "agnix", 4, 14)}
          }
          new {
            name = "File issues for findings"
            \`continue-on-error\` = true
            env = new Mapping {
              ["GH_TOKEN"] = "\${{ github.token }}"
              ["GITHUB_REPOSITORY"] = "\${{ github.repository }}"
              ["MODE"] = "\${{ github.event_name == 'push' && 'main' || 'pr' }}"
              ["PR_NUMBER"] = "\${{ github.event.pull_request.number }}"
              ["RUN_URL"] = "\${{ github.server_url }}/\${{ github.repository }}/actions/runs/\${{ github.run_id }}"
              ["TRACKED_FILE"] = "tracked-issues.md"
            }
            run = "bash .github/scripts/agnix-file-issues.sh agnix.json"
          }
          new {
            name = "Find errors this pull request introduces"
            id = "gate"
            \`if\` = "github.event_name == 'pull_request'"
            \`continue-on-error\` = true
            env = new Mapping { ["INTRODUCED_FILE"] = "introduced-errors.txt" }
            run = "bash .github/scripts/agnix-introduced-errors.sh agnix.json agnix-base.json"
          }
          new {
            name = "Assemble comment"
            \`if\` = "github.event_name == 'pull_request'"
            run = "bash .github/scripts/agnix-assemble-comment.sh"
          }
          new {
            name = "Find existing agnix comment"
            id = "find-comment"
            \`if\` = "github.event_name == 'pull_request'"
            \`continue-on-error\` = true
            uses = Pins.findComment
            \`with\` = new Mapping {
              ["issue-number"] = "\${{ github.event.pull_request.number }}"
              ["comment-author"] = "github-actions[bot]"
              ["body-includes"] = "## agnix"
            }
          }
          new {
            name = "Post or update agnix comment"
            \`if\` = "github.event_name == 'pull_request'"
            \`continue-on-error\` = true
            uses = Pins.createOrUpdateComment
            \`with\` = new Mapping {
              ["comment-id"] = "\${{ steps.find-comment.outputs.comment-id }}"
              ["issue-number"] = "\${{ github.event.pull_request.number }}"
              ["edit-mode"] = "replace"
              ["body-path"] = "comment.md"
            }
          }
          new {
            name = "Fail on introduced errors"
            \`if\` = "github.event_name == 'pull_request' && steps.gate.outcome == 'failure'"
            run = ${run("workflows/agnix.yml", "agnix", 10, 14)}
          }
        }
      }
    }
  }

/// The code-quality gate (aislop), scored out of 100. aislop's version is pinned in Tools.pkl, because Dependabot does not read an inline npx
/// version.
function aislop(): Config.Workflow = new {
  header = ${raw(commentLines(upText("workflows/aislop.yml"), true), 4)}
  name = "aislop"
  on = mainTriggers()
  permissions = readOnly
  concurrency = concurrencyPerRef
  jobs {
    ["quality-gate"] {
      name = "aislop quality gate"
      \`timeout-minutes\` = 10
      steps {
        checkout
        new {
          uses = Pins.setupNode
          \`with\` = new Mapping { ["node-version"] = 22 }
        }
        new { name = "aislop ci"; run = "npx --yes aislop@\\(Tools.aislop) ci ." }
      }
    }
  }
}

local yamlConfig = new YamlRenderer { omitNullProperties = true }

/// Comment lines for the top of a rendered file, the same as the header of a workflow.
local function commentBlock(text: String): String =
  text.split("\\n").map((l) -> if (l == "") "#" else "# \\(l)").join("\\n") + "\\n"

local dependabotGroup = (name: String) -> new Dynamic {
  patterns = new Listing { "*" }
  \`update-types\` = new Listing { "minor"; "patch" }
}

/// Dependabot's configuration. The git dependencies in \`options.gitDependencyIgnore\` are ignored, because Dependabot cannot resolve a git
/// dependency pinned by tag and that error fails the whole cargo job. The github-actions ecosystem is not in the consumer file.
function dependabot(opts: Config.Options): Config.GenFile =
  let (names = opts.gitDependencyIgnore.toList().join(" and "))
  new Config.GenFile {
    content =
      commentBlock(
        if (opts.gitDependencyIgnore.isEmpty) ${raw(dbKit, 10)}
        else ${raw(dbAdr, 10)}
      ) + yamlConfig.renderValue(new Dynamic {
        version = 2
        updates = new Listing {
          new Dynamic {
            \`package-ecosystem\` = "cargo"
            directory = "/"
            schedule = new Dynamic { interval = "weekly"; day = "monday" }
            \`open-pull-requests-limit\` = 5
            cooldown = new Dynamic { \`default-days\` = 7 }
            when (!opts.gitDependencyIgnore.isEmpty) {
              ignore = opts.gitDependencyIgnore.toList().map((n) -> new Dynamic { \`dependency-name\` = n }).toListing()
            }
            labels = new Listing { "dependencies"; "rust" }
            \`commit-message\` = new Dynamic { prefix = "chore"; \`prefix-development\` = "chore"; include = "scope" }
            groups = new Dynamic { \`cargo-minor-patch\` = dependabotGroup.apply("cargo") }
          }
          new Dynamic {
            \`package-ecosystem\` = "github-actions"
            directory = "/"
            schedule = new Dynamic { interval = "weekly"; day = "monday" }
            \`open-pull-requests-limit\` = 5
            cooldown = new Dynamic { \`default-days\` = 7 }
            labels = new Listing { "dependencies"; "github-actions" }
            \`commit-message\` = new Dynamic { prefix = "ci"; include = "scope" }
            groups = new Dynamic { \`actions-minor-patch\` = dependabotGroup.apply("actions") }
          }
        }
      })
  }

/// The config files that go with the wave B builtins: .agnix.toml (the first line depends on whether the repository has a skill), dependabot.yml,
/// and, for a repository with a binary, .github/zizmor.yml (the variant with the extra entry for action.yml when there is a composite action).
function analysisFiles(opts: Config.Options): Mapping<String, Config.GenFile> = new {
  [".agnix.toml"] = new Config.GenFile {
    content =
      if (opts.skills) read("assets/agnix.toml").text
      else read("assets/agnix.toml").text.replaceFirst("# agnix lints the bundled skill against Claude Code's documented formats:", "# agnix lints agent skills and config files against Claude Code's documented formats:")
  }
  [".github/dependabot.yml"] = dependabot(opts)
  when (opts.binary) {
    [".github/zizmor.yml"] = new Config.GenFile { content = read(if (opts.compositeAction) "assets/zizmor-composite.yml" else "assets/zizmor.yml").text }
  }
}

${END}
`;

let b = readFileSync("pkl/Builtins.pkl", "utf8");
const i = b.indexOf(BEGIN);
const j = b.indexOf(END);
if ((i === -1) !== (j === -1)) throw new Error("only one marker found in pkl/Builtins.pkl");
b = i === -1 ? b.trimEnd() + "\n\n" + block : b.slice(0, i) + block + b.slice(j + END.length + 1);
writeFileSync("pkl/Builtins.pkl", b);
console.log("wrote the wave B builtins into pkl/Builtins.pkl");
