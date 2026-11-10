// Mutation check for Phase 5 wave E: each entry breaks one property of the consumer smoke step in verify-release and
// release-workflow.test.ts must then fail. Run from the repository root: bun tmp/phase-5-wave-e/mutate.ts [text]
import { $ } from "bun";

const W = ".github/workflows/release.yml";
const M: [string, string, string, string][] = [
  [W, `          grep -q "written 0 file(s)" again.txt\n`, ``, "the second render must write nothing"],
  [W, `          mise x -- pantheon-scaffold check standard.pkl --root "$RUNNER_TEMP/standard" --attempts 8\n`, ``, "the standard render is checked"],
  [W, `\\nimport "%s#/Builtins.pkl"`, ``, "Builtins is imported from the same package URI"],
  [W, `...Builtins.standardFiles(options)`, `...Builtins.verbatimFiles(options)`, "the standard files are rendered"],
  [W, `...Builtins.standardWorkflows(options)`, `...Builtins.plumber()`, "the standard workflows are rendered"],
  [W, `::sha256:$sum`, ``, "the package is loaded with its published checksum"],
];
const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
const only = Bun.argv[2];
for (const [file, from, to, what] of M) {
  if (only && !what.includes(only)) continue;
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  const marker = original.indexOf("Render the standard workflows and files through the package");
  const at = original.indexOf(from, marker);
  if (at < 0) { console.log(`SKIP   ${what}: pattern not in the smoke step`); missed++; continue; }
  await Bun.write(file, original.slice(0, at) + to + original.slice(at + from.length));
  const r = await $`cd runner && bun test test/release-workflow.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
