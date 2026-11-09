// Mutation check for decision 86: each entry breaks one property of the dependabot builtin's release.yml ignores.
// Run from the repository root: bun tmp/phase-5-dependabot/mutate.ts [text]
import { $ } from "bun";

const B = "pkl/Builtins.pkl";
const M: [string, string, string, string][] = [
  [B, `            when (opts.binary) {\n              ignore = new Listing {\n                new Dynamic { \`dependency-name\` = "actions/upload-artifact" }`, `            when (true) {\n              ignore = new Listing {\n                new Dynamic { \`dependency-name\` = "actions/upload-artifact" }`, "only a repository with a binary ignores the artifact actions"],
  [B, `                new Dynamic { \`dependency-name\` = "actions/upload-artifact" }\n`, ``, "upload-artifact is ignored"],
  [B, `                new Dynamic { \`dependency-name\` = "actions/download-artifact" }\n`, ``, "download-artifact is ignored"],
  [B, `(if (opts.binary) distNote else "")`, `""`, "the header explains the ignores"],
  [B, `            \`package-ecosystem\` = "github-actions"\n            directory = "/"\n            schedule = new Dynamic { interval = "weekly"; day = "monday" }\n            \`open-pull-requests-limit\` = 5\n            cooldown = new Dynamic { \`default-days\` = 7 }\n            when (opts.binary) {`, `            \`package-ecosystem\` = "github-actions"\n            directory = "/"\n            schedule = new Dynamic { interval = "weekly"; day = "monday" }\n            \`open-pull-requests-limit\` = 5\n            cooldown = new Dynamic { \`default-days\` = 3 }\n            when (opts.binary) {`, "the github-actions cooldown is the repositories' seven days"],
];
const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
const only = Bun.argv[2];
for (const [file, from, to, what] of M) {
  if (only && !what.includes(only)) continue;
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, () => to));
  const r = await $`cd runner && bun test test/phase5-dependabot.test.ts test/workflows-b.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
