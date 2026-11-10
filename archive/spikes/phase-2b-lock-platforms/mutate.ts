// Mutation check for the lock platform guard: each entry breaks one behaviour and the lock, upgrade and release workflow tests must then
// fail. Run from the repository root: bun tmp/phase-2b-lock-platforms/mutate.ts
import { $ } from "bun";

const LC = "runner/src/lockcheck.ts";
const UP = "runner/src/upgrade.ts";
const WF = ".github/workflows/release.yml";
const M: [string, string, string, string][] = [
  [LC, `else if (expected === undefined) problems.push`, `else if (false) problems.push`, "a platform the runner is not built for is a problem"],
  [LC, `else if (asset !== expected) problems.push`, `else if (false) problems.push`, "the wrong asset for a supported platform is a problem"],
  [LC, `if (asset === undefined) problems.push`, `if (false) problems.push`, "an entry with no url is a problem"],
  [UP, `if (problems.length) {\n      restore(saved);`, `if (problems.length) {`, "a wrong lock is rolled back"],
  [UP, `if (problems.length) {`, `if (false) {`, "a wrong lock fails the upgrade"],
  [WF, `          lockfile_platforms = ["linux-x64", "linux-arm64", "macos-arm64"]\n`, ``, "the verified consumer limits the lock platforms"],
  [WF, `          if echo "$urls" | grep -vE '/pantheon-scaffold-(linux-x64|linux-arm64|darwin-arm64)"$'; then`, `          if false; then`, "the verified lock must name only runner binaries"],
];

const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
for (const [file, from, to, what] of M) {
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, () => to));
  const r = await $`cd runner && bun test test/lockcheck.test.ts test/upgrade.test.ts test/release-workflow.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
