// Mutation check for Phase 4 wave A: each entry breaks one property of the wave A workflows and the workflow and equality tests must
// then fail. Run from the repository root: bun tmp/phase-4-wave-a/mutate.ts
import { $ } from "bun";

const B = "pkl/Builtins.pkl";
const C = "pkl/Config.pkl";
const M: [string, string, string, string][] = [
  [B, `critical" -gt 0 ]`, `critical" -gt 1 ]`, "a run script is byte for byte the upstream one"],
  [B, `  onComment = "zizmor: ignore[dangerous-triggers]`, `  hidden__ = "zizmor: ignore[dangerous-triggers]`, "the zizmor ignore comment reaches the file"],
  [B, `name = "Conventional Commit Title"\n      \`timeout-minutes\``, `name = "Conventional Commit Titles"\n      \`timeout-minutes\``, "the required check's job name is stable"],
  [B, `["cron"] = "37 8 * * 1"`, `["cron"] = "37 9 * * 1"`, "the schedule is the upstream one"],
  [B, `["client-id"] = opts.appClientId`, `["client-id"] = "Iv23ligzn16vkDCWfHgc"`, "the App client id is an option"],
  [B, `["security-events"] = "write"`, `["security-events"] = "read"`, "job permissions are the upstream ones"],
  [B, `  permissions = new Mapping {}\n`, ``, "an explicit empty permissions block is kept"],
  [B, `\`runs-on\` = "ubuntu-24.04"`, `\`runs-on\` = "ubuntu-latest"`, "the plumber runner image is ubuntu-24.04"],
  ["pkl/Pins.pkl", `sha = "e9ad2dc6d6faa2dfd84395aac18e461284062640"`, `sha = "e9ad2dc6d6faa2dfd84395aac18e461284062641"`, "the plumber pin is the release tag's commit"],
  [C, `  when (s.\`continue-on-error\` != null) { \`continue-on-error\` = s.\`continue-on-error\` }\n`, ``, "continue-on-error is rendered"],
  ["pkl/assets/labels.txt", `\n`, `\nextra\n`, "the labels file is byte for byte the upstream one"],
  ["runner/src/equality.ts", `const DIRECTIVE = /#\\s*zizmor:`, `const DIRECTIVE = /#\\s*zizmorX:`, "the directive check sees zizmor comments"],
];

const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
for (const [file, from, to, what] of M) {
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, () => to));
  const r = await $`cd runner && bun test test/workflows-a.test.ts test/equality.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
