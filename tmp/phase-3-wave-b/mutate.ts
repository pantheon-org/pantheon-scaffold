// Mutation check for Phase 3 wave B: each entry breaks one property of the mise files and the mise and package URI tests must then fail.
// Run from the repository root: bun tmp/phase-3-wave-b/mutate.ts
import { $ } from "bun";

const B = "pkl/Builtins.pkl";
const T = "pkl/assets/mise.toml.tmpl";
const M: [string, string, string, string][] = [
  ["pkl/Tools.pkl", `const actionlint: String = "1.7.12"`, `const actionlint: String = "1.7.13"`, "tool pins come from Tools.pkl and equal today's"],
  ["pkl/Tools.pkl", `List("linux-x64", "linux-arm64", "macos-arm64")`, `List("linux-x64", "linux-arm64")`, "lockfile_platforms is the supported platforms"],
  [T, `lockfile = true\n`, ``, "the lock is switched on"],
  [T, `locked_verify_provenance = true\n`, ``, "provenance is enforced"],
  [T, `hk = "latest"`, `hk = "1.0.0"`, "latest stays latest (decision q14)"],
  [T, `# Workflow security linter. Pinned in the scaffold package (Tools.pkl).`, `# Workflow security linter.`, "the changed comments are the named ones"],
  [T, `"github:pantheon-org/pantheon-scaffold" = "@@RUNNER@@"`, `"github:pantheon-org/pantheon-scaffold" = "latest"`, "the runner is pinned at one version"],
  [B, `["mise.lock"] = new Config.GenFile { \`class\` = "external" }`, `["mise.lock"] = new Config.GenFile { \`class\` = "seed"; content = "" }`, "mise.lock is external"],
  [B, `\`class\` = "seed"; content = read("assets/mise-repo.toml").text`, `\`class\` = "owned"; content = read("assets/mise-repo.toml").text`, "the per-repo config is seeded, not owned"],
  [B, `opts.runnerVersion ?? packageVersion ?? throw(`, `opts.runnerVersion ?? throw(`, "the runner pin follows the package version"],
  [B, `(?:::sha256:[0-9a-f]{64})?#"#).findMatchesIn(reflect`, `#"#).findMatchesIn(reflect`, "the version is read from a checksum-pinned URI"],
  ["pkl/assets/mise-repo.toml", `[tools]`, `[tools]\njq = "1.6"`, "the seeded per-repo config holds no tools"],
];

const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
for (const [file, from, to, what] of M) {
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, () => to));
  const r = await $`cd runner && bun test test/mise.test.ts test/package-uri.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
