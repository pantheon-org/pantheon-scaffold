// Mutation check for wave C: each entry breaks one behaviour of the runner, rebuilds the darwin-arm64 binary and runs the smoke
// suite, which must then fail. Run from the repository root on an Apple silicon Mac: bun tmp/phase-2b-wave-c/mutate.ts
import { $ } from "bun";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const M = [
  ["runner/src/cli.ts", `const schema = contractSchema as Schema;`, `const schema = (await Bun.file(join(import.meta.dir, "../../contract.schema.json")).json()) as Schema;`, "the schema is inside the binary"],
  ["runner/src/version.ts", `typeof BUILD_VERSION === "string" ? BUILD_VERSION : "0.0.0"`, `"0.0.0"`, "the build sets the version"],
  ["runner/src/apply.ts", `if (renamed) {`, `if (false) {`, "a case-only rename is refused"],
  ["runner/src/guard.ts", `else if (status.out.trim() !== "")`, `else if (false)`, "a dirty tree refuses deletion"],
  ["runner/src/apply.ts", `chmodSync(tmp, f.mode === "0755" ? 0o755 : 0o644);`, `chmodSync(tmp, 0o644);`, "the executable bit is set"],
] as const;

const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
for (const [file, from, to, what] of M) {
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  const out = mkdtempSync(join(tmpdir(), "scaffold-mutate-"));
  try {
    await Bun.write(file, original.replace(from, to));
    const built = await $`bun runner/scripts/build.ts --version 0.0.1 --out ${out} --targets darwin-arm64`.quiet().nothrow();
    await Bun.write(file, original);
    if (built.exitCode !== 0) { console.log(`SKIP   ${what}: the mutated build failed`); missed++; continue; }
    const smoke = await $`bun runner/scripts/smoke.ts ${join(out, "pantheon-scaffold-darwin-arm64")} --version 0.0.1`.quiet().nothrow();
    const caught = smoke.exitCode !== 0;
    if (!caught) missed++;
    console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
  } finally {
    await Bun.write(file, original);
    if (out.startsWith(tmpdir()) && out.includes("scaffold-mutate-")) rmSync(out, { recursive: true, force: true });
  }
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a source file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
