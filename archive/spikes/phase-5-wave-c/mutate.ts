// Mutation check for Phase 5 wave C: each entry breaks one property of the toggles and phase5-toggles.test.ts must then fail.
// Run from the repository root: bun tmp/phase-5-wave-c/mutate.ts [text]
import { $ } from "bun";

const B = "pkl/Builtins.pkl";
const M: [string, string, string, string][] = [
  [B, `  when (opts.skills) { ["audit-skill"] = auditSkill(opts) }`, `  ["audit-skill"] = auditSkill(opts)`, "audit-skill.yml belongs to skills"],
  [B, `  ...verbatimFiles(opts)\n  ...parameterisedFiles(opts)\n}`, `  ...parameterisedFiles(opts)\n}`, "the verbatim files are part of the standard set"],
  [B, `  ...analysisFiles(opts)\n  ...verbatimFiles(opts)\n  ...parameterisedFiles(opts)\n}`, `  ...analysisFiles(opts)\n  ...verbatimFiles(opts)\n}`, "the parameterised files are part of the standard set"],
  [B, `  when (opts.binary) {\n    [".github/actionlint.yaml"]`, `  when (true) {\n    [".github/actionlint.yaml"]`, "the binary files belong to binary"],
  [B, `  when (opts.binary) {\n    [".github/scripts/tag-release.sh"]`, `  when (true) {\n    [".github/scripts/tag-release.sh"]`, "tag-release.sh belongs to binary"],
  [B, `opts.skillName ?? throw("options.skillName is required`, `opts.skillName ?? "x" ?? throw("options.skillName is required`, "a missing skillName is an error"],
  [B, `opts.crateName ?? throw("options.crateName is required when options.binary is true`, `opts.crateName ?? "x" ?? throw("options.crateName is required when options.binary is true`, "a missing crateName is an error"],
  [B, `Set crateName, or set binary = false.`, `Set crateName.`, "the crateName error names the way out"],
  [B, `Set skillName, or set skills = false.`, `Set skillName.`, "the skillName error names the way out"],
  [B, `[".github/workflows/release.yml"] = new Config.GenFile { \`class\` = "external" }`, ``, "release.yml is listed with binary"],
  [B, `  ["rust-ci"] = rustCi(opts)\n  ["cargo-deny"]`, `  ["cargo-deny"]`, "rust-ci is part of the standard workflows"],
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
  const r = await $`cd runner && bun test test/phase5-toggles.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
