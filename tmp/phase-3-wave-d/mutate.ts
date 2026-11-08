// Mutation check for Phase 3 wave D: the evidence script must FAIL when the render is wrong. Each entry breaks the render in one way and
// runner/scripts/real-repo-tooling.ts must then exit non-zero. Run from the repository root: bun tmp/phase-3-wave-d/mutate.ts
// Needs the same tools and network as the script. It takes about a minute per entry.
import { $ } from "bun";

const M: [string, string, string, string][] = [
  ["pkl/assets/auto-rebase.sh", "#!", "\n#!", "a changed verbatim script is an unexpected change"],
  ["pkl/assets/hk.shared.pkl.tmpl", `["rustfmt"] {`, `["rustfmt-renamed"] {`, "a renamed hook step changes the hk outcome"],
  ["pkl/assets/mise.toml.tmpl", `lockfile = true`, `lockfile = true\n# extra`, "the render is checked only for the listed changes"],
  ["pkl/Builtins.pkl", `[".mise/conf.d/repo.toml"] = new Config.GenFile { \`class\` = "seed"`, `[".mise/conf.d/other.toml"] = new Config.GenFile { \`class\` = "seed"`, "the listed set of changed paths is exact"],
];

const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
for (const [file, from, to, what] of M) {
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, () => to));
  const r = await $`bun runner/scripts/real-repo-tooling.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
