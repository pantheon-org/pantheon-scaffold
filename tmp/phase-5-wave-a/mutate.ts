// Mutation check for Phase 5 wave A: each entry breaks one property of verbatimFiles and phase5-verbatim.test.ts must then fail.
// Run from the repository root: bun tmp/phase-5-wave-a/mutate.ts [text]
import { $ } from "bun";

const B = "pkl/Builtins.pkl";
const M: [string, string, string, string][] = [
  [B, `  when (opts.binary) {\n    [".github/actionlint.yaml"]`, `  when (true) {\n    [".github/actionlint.yaml"]`, "the binary files are rendered only with a binary"],
  [B, `[".github/workflows/release.yml"] = new Config.GenFile { \`class\` = "external" }`, `[".github/workflows/release.yml"] = new Config.GenFile { content = "x\\n" }`, "release.yml is external, never written"],
  [B, `read("assets/dist-workspace.toml").text`, `read("assets/actionlint.yaml").text`, "dist-workspace.toml is the pinned file"],
  [B, `[".github/actionlint.yaml"] = new Config.GenFile { content = read("assets/actionlint.yaml").text }`, `[".github/actionlint.yaml"] = new Config.GenFile { content = read("assets/actionlint.yaml").text; executable = true }`, "actionlint.yaml is not executable"],
  ["pkl/assets/gitignore", `target/\n`, `/target\n`, ".gitignore is the decided target/ pick"],
  ["pkl/assets/gitignore", `.pkl-gen/cache/\n`, ``, ".gitignore ignores the runner's package cache (decision 87)"],
  [B, `  [".gitignore"] = new Config.GenFile { content = read("assets/gitignore").text }\n  when`, `  when`, ".gitignore is rendered for every repository"],
  ["pkl/assets/actionlint.yaml", `\n`, `\n\n`, "actionlint.yaml is the pinned file"],
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
  const r = await $`cd runner && bun test test/phase5-verbatim.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
