// Mutation check for Phase 4 wave C: each entry breaks one property and the named test files must then fail.
// Run from the repository root: bun tmp/phase-4-wave-c/mutate.ts [text]  (with text, only the entries whose description contains it)
import { $ } from "bun";

const B = "pkl/Builtins.pkl";
const UW = "test/workflows-c.test.ts";
const UP = "test/update-pins-workflow.test.ts";
const US = "test/update-pins-script.test.ts";
const W = ".github/workflows/update-pins.yml";
const S = "runner/scripts/update-pins.ts";
const M: [string, string, string, string, string][] = [
  [B, `  removeOnDetach = true\n  name = "Upgrade pantheon-scaffold"`, `  name = "Upgrade pantheon-scaffold"`, "the consumer upgrade workflow is removed by detach", UW],
  ["pkl/Config.pkl", `removeOnDetach = w.removeOnDetach }`, `removeOnDetach = false }`, "a workflow's removeOnDetach reaches its file", UW],
  [B, `            ["permission-workflows"] = "write"\n`, ``, "the upgrade token can push workflow files", UW],
  [B, `    ["schedule"] = new Listing { new Mapping { ["cron"] = "17 5 * * 1" } }\n    ["workflow_dispatch"] = new Mapping {}\n  }\n  permissions = new Mapping {}`, `    ["schedule"] = new Listing { new Mapping { ["cron"] = "17 5 * * 1" } }\n    ["workflow_dispatch"] = new Mapping {}\n    ["pull_request"] = new Mapping {}\n  }\n  permissions = new Mapping {}`, "the upgrade workflow never runs on a pull request", UW],
  [B, `permissions = new Mapping { ["contents"] = "read" }\n      steps {\n        new {\n          name = "Mint pantheon-ai-bot token"\n          id = "app-token"\n          uses = Pins.createGithubAppToken\n          \`with\` = new Mapping {\n            ["client-id"] = opts.appClientId\n            ["private-key"] = "\${{ secrets.GH_APP_PRIVATE_KEY }}"\n            ["permission-contents"] = "write"\n            ["permission-pull-requests"] = "write"\n            ["permission-workflows"]`, `permissions = new Mapping { ["contents"] = "write" }\n      steps {\n        new {\n          name = "Mint pantheon-ai-bot token"\n          id = "app-token"\n          uses = Pins.createGithubAppToken\n          \`with\` = new Mapping {\n            ["client-id"] = opts.appClientId\n            ["private-key"] = "\${{ secrets.GH_APP_PRIVATE_KEY }}"\n            ["permission-contents"] = "write"\n            ["permission-pull-requests"] = "write"\n            ["permission-workflows"]`, "the upgrade job reads contents only", UW],
  [B, `          \`with\` = new Mapping { ["persist-credentials"] = false }\n        }\n        new {\n          name = "Install the pinned tools"`, `          \`with\` = new Mapping { ["persist-credentials"] = true }\n        }\n        new {\n          name = "Install the pinned tools"`, "the upgrade checkout keeps no credentials", UW],
  [B, `run = "mise x -- pantheon-scaffold freshness\\n"`, `run = "true\\n"`, "the upgrade workflow checks freshness", UW],
  [B, `          name = "Upgrade"\n          \`if\` = "steps.latest.outputs.needed == 'true'"`, `          name = "Upgrade"`, "the upgrade step runs only when a newer release exists", UW],
  [B, `          name = "Open the pull request"\n          \`if\` = "steps.latest.outputs.needed == 'true'"`, `          name = "Open the pull request"`, "the pull request opens only when a newer release exists", UW],
  [B, `commit --message "chore: upgrade pantheon-scaffold to \${LATEST}"`, `commit --message "upgrade pantheon-scaffold to \${LATEST}"`, "the upgrade commit is a conventional commit", UW],
  [B, `            ["GH_TOKEN"] = "\${{ steps.app-token.outputs.token }}"\n            ["LATEST"] = "\${{ steps.latest.outputs.latest }}"\n            ["REPO"]`, `            ["GH_TOKEN"] = "\${{ github.token }}"\n            ["LATEST"] = "\${{ steps.latest.outputs.latest }}"\n            ["REPO"]`, "the upgrade pull request uses the App token", UW],
  [W, `          permission-pull-requests: write\n`, ``, "the pin workflow's token can open a pull request", UP],
  [W, `          if git diff --quiet pkl/Pins.pkl; then echo "No pin changed."; exit 0; fi\n`, ``, "no pin pull request when nothing changed", UP],
  [W, `HEAD:refs/heads/freshness`, `HEAD:refs/heads/main`, "the freshness record goes to the freshness branch", UP],
  [W, `    paths:\n      - .github/workflows/update-pins.yml\n      - runner/scripts/update-pins.ts\n      - runner/src/pinupdate.ts\n`, ``, "the pin workflow's push trigger is limited to its own files", UP],
  [W, `--freshness-out "$RUNNER_TEMP/freshness.json" --summary "$RUNNER_TEMP/summary.md"`, `--summary "$RUNNER_TEMP/summary.md"`, "the pin workflow asks for the freshness record", UP],
  [S, `if (opts.noWrite) {`, `if (false) {`, "a no-write run writes nothing", US],
  [S, `if (problems.length) {`, `if (false) {`, "a problem stops the run before anything is written", US],
  [S, `if (opts.freshnessOut) writeFileSync`, `if (opts.freshnessOut && changes.length) writeFileSync`, "the freshness record is written even when no pin moved", US],
  [S, `if (opts.summary && changes.length) {`, `if (opts.summary) {`, "no summary when nothing changed", US],
];

const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
const only = Bun.argv[2];
for (const [file, from, to, what, test] of M) {
  if (only && !what.includes(only)) continue;
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, () => to));
  const r = await $`cd runner && bun test ${test}`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
